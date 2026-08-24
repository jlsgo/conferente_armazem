use tauri::State;

use crate::domain::errors::AppResult;
use crate::domain::movimentos::{self, Movimento, NovoMovimento};
use crate::state::AppState;

#[tauri::command]
pub fn criar_movimento(state: State<AppState>, payload: NovoMovimento) -> AppResult<Movimento> {
    let mut conn = state.conn()?;
    movimentos::criar_movimento(&mut conn, payload)
}

#[tauri::command(rename_all = "snake_case")]
pub fn listar_movimentos_do_dia(
    state: State<AppState>,
    armazem_id: i64,
    fluxo: String,
    data: String,
) -> AppResult<Vec<Movimento>> {
    let conn = state.conn()?;
    movimentos::listar_movimentos_do_dia(&conn, armazem_id, &fluxo, &data)
}

#[tauri::command]
pub fn sugestoes_descricao(state: State<AppState>, categoria: String) -> AppResult<Vec<String>> {
    let conn = state.conn()?;
    movimentos::sugestoes_descricao(&conn, &categoria)
}
