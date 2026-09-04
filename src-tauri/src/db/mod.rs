use std::path::Path;

use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};

use crate::domain::errors::{AppError, AppResult};

pub mod backup;
pub mod backup_nuvem;
pub mod sync;

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(include_str!("../../migrations/0001_init.sql")),
        M::up(include_str!("../../migrations/0002_fechamentos.sql")),
        M::up(include_str!("../../migrations/0003_sync.sql")),
        M::up(include_str!("../../migrations/0004_transferencias.sql")),
        M::up(include_str!("../../migrations/0005_sync_retry.sql")),
        M::up(include_str!(
            "../../migrations/0006_divergencia_recebimento.sql"
        )),
        M::up(include_str!("../../migrations/0007_retirada_parcial.sql")),
        M::up(include_str!("../../migrations/0008_lockout_login.sql")),
        M::up(include_str!("../../migrations/0009_reparo_externo.sql")),
    ])
}

fn aplicar_pragmas_e_migrations(conn: &mut Connection) -> AppResult<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrations()
        .to_latest(conn)
        .map_err(|e| AppError::Migration(e.to_string()))?;
    garantir_armazens_padrao(conn)?;
    Ok(())
}

/// Garante que os dois armazens conhecidos existam. Nao ha catalogo de
/// produtos para semear (removido de proposito - ver docs/ARQUITETURA.md),
/// so a lista fixa e pequena de armazens.
fn garantir_armazens_padrao(conn: &Connection) -> AppResult<()> {
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM armazens", [], |r| r.get(0))?;
    if total == 0 {
        conn.execute(
            "INSERT INTO armazens (codigo, nome) VALUES ('A4', 'Armazem A4')",
            [],
        )?;
        conn.execute(
            "INSERT INTO armazens (codigo, nome) VALUES ('B2', 'Armazem B2')",
            [],
        )?;
    }
    Ok(())
}

/// Abre (ou cria) o banco SQLite no diretorio de dados do usuario e aplica
/// as migrations pendentes. Um arquivo por computador/armazem (offline-first).
pub fn abrir(diretorio_dados: &Path) -> AppResult<Connection> {
    std::fs::create_dir_all(diretorio_dados).map_err(|e| {
        AppError::Migration(format!("Nao foi possivel criar o diretorio de dados: {e}"))
    })?;

    let caminho_banco = diretorio_dados.join("ecoviva-armazem.db");
    let mut conn = Connection::open(caminho_banco)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    aplicar_pragmas_e_migrations(&mut conn)?;
    Ok(conn)
}

/// Banco em memoria, usado nos testes automatizados (unitarios e de integracao).
pub fn abrir_em_memoria() -> AppResult<Connection> {
    let mut conn = Connection::open_in_memory()?;
    aplicar_pragmas_e_migrations(&mut conn)?;
    Ok(conn)
}
