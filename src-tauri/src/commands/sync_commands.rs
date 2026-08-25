use tauri::{Manager, State};

use crate::db::sync;
use crate::domain::auth::buscar_usuario_ativo;
use crate::domain::errors::{AppError, AppResult};
use crate::state::AppState;

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

    let enviados = sync::enviar_para_turso(&url, &token, &pendentes).await?;

    {
        let conn = state.conn()?;
        sync::marcar_sincronizado(&conn, &enviados)?;
    }

    Ok(match enviados.len() {
        0 => "Nenhum lancamento novo para enviar.".to_string(),
        1 => "1 lancamento enviado.".to_string(),
        n => format!("{n} lancamentos enviados."),
    })
}
