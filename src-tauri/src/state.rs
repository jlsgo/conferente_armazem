use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

use crate::domain::errors::{AppError, AppResult};

/// Estado compartilhado da aplicacao: uma unica conexao SQLite protegida por
/// mutex. E uma app desktop de uso local (poucas escritas concorrentes), entao
/// um pool de conexoes seria complexidade desnecessaria aqui.
pub struct AppState {
    db: Mutex<Connection>,
}

impl AppState {
    pub fn new(conn: Connection) -> Self {
        Self {
            db: Mutex::new(conn),
        }
    }

    pub fn conn(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.db
            .lock()
            .map_err(|_| AppError::Interno("Falha ao acessar o banco de dados.".into()))
    }
}
