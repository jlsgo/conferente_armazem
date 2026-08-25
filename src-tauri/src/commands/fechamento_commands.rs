use serde::Deserialize;
use tauri::State;

use crate::domain::errors::AppResult;
use crate::domain::fechamentos::{self, Fechamento};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct FecharDiaPayload {
    pub armazem_id: i64,
    pub fluxo: String,
    pub data: String,
}

#[tauri::command(rename_all = "snake_case")]
pub fn fechar_dia(state: State<AppState>, payload: FecharDiaPayload) -> AppResult<Fechamento> {
    let usuario_id = state.usuario_logado()?;
    let mut conn = state.conn()?;
    fechamentos::fechar_dia(
        &mut conn,
        payload.armazem_id,
        &payload.fluxo,
        &payload.data,
        usuario_id,
    )
}

#[tauri::command(rename_all = "snake_case")]
pub fn buscar_fechamento_do_dia(
    state: State<AppState>,
    armazem_id: i64,
    fluxo: String,
    data: String,
) -> AppResult<Option<Fechamento>> {
    let conn = state.conn()?;
    fechamentos::buscar_fechamento(&conn, armazem_id, &fluxo, &data)
}
