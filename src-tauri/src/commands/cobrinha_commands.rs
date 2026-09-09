use tauri::{Manager, State};

use crate::commands::sync_commands::armazem_codigo_do_usuario;
use crate::db::cobrinha_sync::{self, RecordeCobrinha};
use crate::db::sync;
use crate::domain::auth::buscar_usuario_ativo;
use crate::domain::errors::{AppError, AppResult};
use crate::state::AppState;

const PONTOS_MAXIMO_PLAUSIVEL: i64 = 256; // TAMANHO_GRADE^2 do CobrinhaSecreta.tsx - cobra nao cabe mais peca que isso.

/// Salva um recorde da cobrinha no placar unificado (Turso, compartilhado
/// entre A4 e B2 - ver `db::cobrinha_sync`) e devolve o top atualizado.
/// Exige login (pra saber de qual armazem e o placar) - o easter egg
/// acessivel da tela de login (`AuthCard.tsx`) continua 100% local nesse
/// caso, nunca chama este comando. Falha de forma amigavel (nunca expõe erro
/// tecnico) se a sincronizacao nao estiver configurada nesta maquina - e so
/// um jogo, nao deveria nunca travar por causa disso.
#[tauri::command(rename_all = "snake_case")]
pub async fn cobrinha_registrar_recorde(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    pontos: i64,
    nome: String,
) -> AppResult<Vec<RecordeCobrinha>> {
    let usuario_id = state.usuario_logado()?;

    if !(1..=PONTOS_MAXIMO_PLAUSIVEL).contains(&pontos) {
        return Err(AppError::Validation("Pontuacao invalida.".into()));
    }
    let nome = nome.trim().to_uppercase();
    let nome = if nome.is_empty() {
        "ANON".to_string()
    } else {
        nome.chars().take(4).collect::<String>()
    };

    let armazem_codigo = {
        let conn = state.conn()?;
        let usuario = buscar_usuario_ativo(&conn, usuario_id)?;
        armazem_codigo_do_usuario(&conn, usuario.armazem_id)?
    }
    .unwrap_or_else(|| "GESTAO".to_string());

    let diretorio_dados = app.path().app_data_dir().map_err(|e| {
        AppError::Interno(format!("Nao foi possivel localizar a pasta de dados: {e}"))
    })?;
    let Some((url, token)) = sync::ler_config_turso(&diretorio_dados) else {
        return Err(AppError::Validation(
            "Sincronizacao nao configurada nesta maquina.".into(),
        ));
    };

    cobrinha_sync::registrar(&url, &token, &armazem_codigo, &nome, pontos).await
}

/// Busca o placar unificado atual. Nunca falha por sincronizacao nao
/// configurada - devolve lista vazia (a tela cai pro placar so-desta-maquina
/// nesse caso), mesmo padrao de `buscar_transferencias_pendentes`.
#[tauri::command(rename_all = "snake_case")]
pub async fn cobrinha_listar_recordes(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Vec<RecordeCobrinha>> {
    state.usuario_logado()?;

    let diretorio_dados = app.path().app_data_dir().map_err(|e| {
        AppError::Interno(format!("Nao foi possivel localizar a pasta de dados: {e}"))
    })?;
    let Some((url, token)) = sync::ler_config_turso(&diretorio_dados) else {
        return Ok(Vec::new());
    };

    match cobrinha_sync::listar_top(&url, &token).await {
        Ok(lista) => Ok(lista),
        Err(_) => Ok(Vec::new()),
    }
}
