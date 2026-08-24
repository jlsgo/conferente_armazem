use serde::Serialize;
use tauri::State;

use crate::domain::{auth, errors::AppResult};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct Armazem {
    pub id: i64,
    pub codigo: String,
    pub nome: String,
}

#[derive(Debug, Serialize)]
pub struct AppStatus {
    pub precisa_configurar_primeiro_usuario: bool,
    pub armazens: Vec<Armazem>,
}

#[tauri::command]
pub fn get_status(state: State<AppState>) -> AppResult<AppStatus> {
    let conn = state.conn()?;
    let total_usuarios = auth::contar_usuarios(&conn)?;

    let mut stmt =
        conn.prepare("SELECT id, codigo, nome FROM armazens WHERE ativo = 1 ORDER BY codigo")?;
    let armazens = stmt
        .query_map([], |r| {
            Ok(Armazem {
                id: r.get(0)?,
                codigo: r.get(1)?,
                nome: r.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(AppStatus {
        precisa_configurar_primeiro_usuario: total_usuarios == 0,
        armazens,
    })
}
