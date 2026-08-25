use rusqlite::{params, Connection, OptionalExtension};
use tauri::{Manager, State};

use crate::db::sync::{self, StatusSincronizacao, TransferenciaPendente};
use crate::domain::auth::buscar_usuario_ativo;
use crate::domain::errors::{AppError, AppResult};
use crate::domain::movimentos::{self, validar_quantidades_recebidas, Movimento, NovoMovimento};
use crate::state::AppState;

/// Codigo ('A4'/'B2') do armazem do usuario logado, direto do banco local -
/// nunca aceito como parametro vindo do frontend. `None` se o usuario nao
/// tiver um armazem fixo (gestor "global", caso raro).
fn armazem_codigo_do_usuario(
    conn: &Connection,
    armazem_id: Option<i64>,
) -> AppResult<Option<String>> {
    let Some(armazem_id) = armazem_id else {
        return Ok(None);
    };
    let codigo: Option<String> = conn
        .query_row(
            "SELECT codigo FROM armazens WHERE id = ?1",
            params![armazem_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(codigo)
}

/// Dispara uma sincronizacao manual com o Turso (gestor-only, mesmo padrao
/// de `fechar_dia`). Se `turso.txt` nao estiver configurado nesta maquina,
/// devolve uma mensagem amigavel em vez de erro tecnico - sincronizacao e
/// um recurso opcional, o app funciona 100% offline sem ele.
///
/// A conexao local (`state.conn()`) e adquirida e solta duas vezes, nunca
/// segurada durante a chamada de rede - ver o comentario em
/// `db::sync::enviar_para_turso` sobre por que isso importa.
#[tauri::command]
pub async fn sincronizar_agora(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<String> {
    let usuario_id = state.usuario_logado()?;

    let pendentes = {
        let conn = state.conn()?;
        let usuario = buscar_usuario_ativo(&conn, usuario_id)?;
        if usuario.papel != "gestor" {
            return Err(AppError::Validation(
                "Somente um gestor pode sincronizar com a nuvem.".into(),
            ));
        }
        sync::movimentos_pendentes(&conn)?
    };

    let diretorio_dados = app.path().app_data_dir().map_err(|e| {
        AppError::Interno(format!("Nao foi possivel localizar a pasta de dados: {e}"))
    })?;

    let Some((url, token)) = sync::ler_config_turso(&diretorio_dados) else {
        return Err(AppError::Validation(
            "Sincronizacao nao configurada nesta maquina.".into(),
        ));
    };

    let resultado = sync::enviar_para_turso(&url, &token, &pendentes).await?;

    {
        let conn = state.conn()?;
        sync::marcar_sincronizado(&conn, &resultado.enviados)?;
        sync::marcar_falha_sincronizacao(&conn, &resultado.falhas)?;
    }

    let enviados_txt = match resultado.enviados.len() {
        0 => "Nenhum lancamento novo enviado.".to_string(),
        1 => "1 lancamento enviado.".to_string(),
        n => format!("{n} lancamentos enviados."),
    };

    Ok(if resultado.falhas.is_empty() {
        enviados_txt
    } else {
        format!(
            "{enviados_txt} {} com erro (tentando de novo automaticamente).",
            resultado.falhas.len()
        )
    })
}

/// Retrato local da fila de sincronizacao (quantos pendentes, quantos com
/// erro, ultimo erro registrado) - gestor-only, mesmo padrao de
/// `sincronizar_agora`. Nao depende de rede, so le o estado local.
#[tauri::command(rename_all = "snake_case")]
pub fn status_sincronizacao(state: State<AppState>) -> AppResult<StatusSincronizacao> {
    let usuario_id = state.usuario_logado()?;
    let conn = state.conn()?;
    let usuario = buscar_usuario_ativo(&conn, usuario_id)?;
    if usuario.papel != "gestor" {
        return Err(AppError::Validation(
            "Somente um gestor pode ver o status de sincronizacao.".into(),
        ));
    }
    sync::status_sincronizacao(&conn)
}

/// Busca no Turso o que foi enviado pro armazem do usuario logado e ainda
/// nao foi confirmado. Nunca falha por sincronizacao nao configurada -
/// devolve lista vazia (a secao correspondente na tela so nao aparece).
#[tauri::command(rename_all = "snake_case")]
pub async fn buscar_transferencias_pendentes(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Vec<TransferenciaPendente>> {
    let usuario_id = state.usuario_logado()?;

    let meu_armazem_codigo = {
        let conn = state.conn()?;
        let usuario = buscar_usuario_ativo(&conn, usuario_id)?;
        armazem_codigo_do_usuario(&conn, usuario.armazem_id)?
    };
    let Some(meu_armazem_codigo) = meu_armazem_codigo else {
        return Ok(Vec::new());
    };

    let diretorio_dados = app.path().app_data_dir().map_err(|e| {
        AppError::Interno(format!("Nao foi possivel localizar a pasta de dados: {e}"))
    })?;
    let Some((url, token)) = sync::ler_config_turso(&diretorio_dados) else {
        return Ok(Vec::new());
    };

    sync::buscar_pendentes_recebimento(&url, &token, &meu_armazem_codigo).await
}

/// Confirma o recebimento de uma transferencia vinda do outro armazem: busca
/// de novo a linha certa no Turso (nunca confia em itens vindos do frontend),
/// registra localmente uma entrada de `peca_montagem` ligada a origem
/// (`recebido_de_armazem_codigo`/`recebido_de_id_origem`), e sincroniza so
/// esse lancamento de volta pro Turso na hora - fecha o ciclo sem esperar o
/// proximo sync automatico. Qualquer usuario logado do armazem de destino
/// pode confirmar (nao e restrito a gestor - e uma tarefa fisica rotineira).
#[tauri::command(rename_all = "snake_case")]
pub async fn confirmar_recebimento(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    origem_armazem_codigo: String,
    origem_id: i64,
    hora: String,
    quantidades_recebidas: Vec<i64>,
) -> AppResult<Movimento> {
    let usuario_id = state.usuario_logado()?;

    let (armazem_id, meu_armazem_codigo) = {
        let conn = state.conn()?;
        let usuario = buscar_usuario_ativo(&conn, usuario_id)?;
        let armazem_id = usuario.armazem_id.ok_or_else(|| {
            AppError::Validation("Seu usuario nao esta associado a um armazem.".into())
        })?;
        let codigo = armazem_codigo_do_usuario(&conn, Some(armazem_id))?
            .ok_or_else(|| AppError::Validation("Armazem do usuario nao encontrado.".into()))?;
        (armazem_id, codigo)
    };

    let diretorio_dados = app.path().app_data_dir().map_err(|e| {
        AppError::Interno(format!("Nao foi possivel localizar a pasta de dados: {e}"))
    })?;
    let Some((url, token)) = sync::ler_config_turso(&diretorio_dados) else {
        return Err(AppError::Validation(
            "Sincronizacao nao configurada nesta maquina.".into(),
        ));
    };

    let transferencia = sync::buscar_transferencia(&url, &token, &origem_armazem_codigo, origem_id)
        .await?
        .ok_or_else(|| {
            AppError::Validation(
                "Essa transferencia nao foi encontrada (ja pode ter sido confirmada por outra pessoa).".into(),
            )
        })?;

    // Nunca confia que o comando foi chamado com a chave certa so porque o
    // frontend mandou - confere que a transferencia buscada de fato estava
    // endereçada ao armazem de quem esta confirmando.
    if transferencia.armazem_destino_codigo.as_deref() != Some(meu_armazem_codigo.as_str()) {
        return Err(AppError::Validation(
            "Essa transferencia nao esta endereçada ao seu armazem.".into(),
        ));
    }

    let itens = validar_quantidades_recebidas(&transferencia.itens, &quantidades_recebidas)?;

    let movimento_confirmado = {
        let mut conn = state.conn()?;
        let data: String = conn.query_row("SELECT date('now')", [], |r| r.get(0))?;
        movimentos::criar_movimento(
            &mut conn,
            NovoMovimento {
                armazem_id,
                armazem_destino_id: None,
                fluxo: "peca_montagem".into(),
                tipo: "entrada".into(),
                data,
                hora,
                turno: "diurno".into(),
                usuario_id,
                numero_pedido: None,
                codigo_rastreio: None,
                contraparte: None,
                quem_retirou: None,
                motivo: None,
                valor_centavos: None,
                observacoes: Some(format!(
                    "Recebido de {origem_armazem_codigo} (envio #{origem_id})"
                )),
                recebido_de_armazem_codigo: Some(origem_armazem_codigo),
                recebido_de_id_origem: Some(origem_id),
                itens,
            },
        )?
    };

    // Sincroniza na hora (mesma logica de `sincronizar_agora`) pra fechar o
    // ciclo sem esperar o proximo sync automatico - nao trata falha aqui
    // como erro da confirmacao em si, que ja aconteceu localmente com
    // sucesso: so avisa no log e o proximo sync tenta de novo.
    let pendentes = {
        let conn = state.conn()?;
        sync::movimentos_pendentes(&conn)?
    };
    match sync::enviar_para_turso(&url, &token, &pendentes).await {
        Ok(resultado) => {
            let conn = state.conn()?;
            sync::marcar_sincronizado(&conn, &resultado.enviados)?;
            sync::marcar_falha_sincronizacao(&conn, &resultado.falhas)?;
        }
        Err(e) => log::warn!(
            "Confirmacao de recebimento #{} salva localmente, mas falhou ao sincronizar agora: {e}",
            movimento_confirmado.id
        ),
    }

    Ok(movimento_confirmado)
}
