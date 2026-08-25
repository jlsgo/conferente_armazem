use serde::Deserialize;
use tauri::State;

use crate::domain::auth::{self, NovoUsuario, Usuario};
use crate::domain::errors::{AppError, AppResult};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct SetupPrimeiroUsuarioPayload {
    pub nome: String,
    pub login: String,
    pub senha: String,
    pub armazem_id: Option<i64>,
}

#[tauri::command]
pub fn setup_primeiro_usuario(
    state: State<AppState>,
    payload: SetupPrimeiroUsuarioPayload,
) -> AppResult<()> {
    let conn = state.conn()?;

    if auth::contar_usuarios(&conn)? > 0 {
        return Err(AppError::Validation(
            "Ja existe usuario cadastrado neste computador.".into(),
        ));
    }

    auth::criar_usuario(
        &conn,
        NovoUsuario {
            nome: &payload.nome,
            login: &payload.login,
            senha: &payload.senha,
            armazem_id: payload.armazem_id,
            papel: "gestor",
        },
    )?;

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct LoginPayload {
    pub login: String,
    pub senha: String,
}

#[tauri::command]
pub fn login(state: State<AppState>, payload: LoginPayload) -> AppResult<Usuario> {
    let conn = state.conn()?;
    let usuario = auth::login(&conn, &payload.login, &payload.senha)?;
    state.iniciar_sessao(usuario.id);
    Ok(usuario)
}

#[tauri::command]
pub fn logout(state: State<AppState>) -> AppResult<()> {
    state.encerrar_sessao();
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn listar_usuarios(state: State<AppState>, armazem_id: Option<i64>) -> AppResult<Vec<Usuario>> {
    let solicitante_id = state.usuario_logado()?;
    let conn = state.conn()?;
    auth::listar_usuarios_como_gestor(&conn, solicitante_id, armazem_id)
}

#[derive(Debug, Deserialize)]
pub struct CriarUsuarioPayload {
    pub nome: String,
    pub login: String,
    pub senha: String,
    pub armazem_id: Option<i64>,
    pub papel: String,
}

#[tauri::command(rename_all = "snake_case")]
pub fn criar_usuario(state: State<AppState>, payload: CriarUsuarioPayload) -> AppResult<()> {
    let solicitante_id = state.usuario_logado()?;
    let conn = state.conn()?;
    auth::criar_usuario_como_gestor(
        &conn,
        solicitante_id,
        NovoUsuario {
            nome: &payload.nome,
            login: &payload.login,
            senha: &payload.senha,
            armazem_id: payload.armazem_id,
            papel: &payload.papel,
        },
    )?;
    Ok(())
}
