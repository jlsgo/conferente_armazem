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
    auth::login(&conn, &payload.login, &payload.senha)
}
