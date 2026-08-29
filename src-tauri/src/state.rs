use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

use crate::domain::errors::{AppError, AppResult};

/// Estado compartilhado da aplicacao: uma unica conexao SQLite protegida por
/// mutex. E uma app desktop de uso local (poucas escritas concorrentes), entao
/// um pool de conexoes seria complexidade desnecessaria aqui.
///
/// `sessao` guarda o id de quem fez login com sucesso por ultimo. Comandos que
/// precisam saber "quem esta fazendo isso" leem daqui, nunca de um campo
/// `usuario_id` mandado pelo frontend no payload - o frontend nao e uma fonte
/// confiavel de identidade.
pub struct AppState {
    db: Mutex<Connection>,
    sessao: Mutex<Option<i64>>,
}

impl AppState {
    pub fn new(conn: Connection) -> Self {
        Self {
            db: Mutex::new(conn),
            sessao: Mutex::new(None),
        }
    }

    pub fn conn(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.db
            .lock()
            .map_err(|_| AppError::Interno("Falha ao acessar o banco de dados.".into()))
    }

    pub fn iniciar_sessao(&self, usuario_id: i64) -> AppResult<()> {
        let mut sessao = self
            .sessao
            .lock()
            .map_err(|_| AppError::Interno("Falha ao acessar a sessao.".into()))?;
        *sessao = Some(usuario_id);
        Ok(())
    }

    pub fn encerrar_sessao(&self) -> AppResult<()> {
        let mut sessao = self
            .sessao
            .lock()
            .map_err(|_| AppError::Interno("Falha ao acessar a sessao.".into()))?;
        *sessao = None;
        Ok(())
    }

    pub fn usuario_logado(&self) -> AppResult<i64> {
        self.sessao
            .lock()
            .map_err(|_| AppError::Interno("Falha ao acessar a sessao.".into()))?
            .ok_or(AppError::NaoAutenticado)
    }
}
