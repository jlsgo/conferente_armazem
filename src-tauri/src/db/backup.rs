use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, DatabaseName};

use crate::domain::errors::{AppError, AppResult};

/// Quantos backups diarios manter antes de apagar os mais antigos.
const RETENCAO_DIAS: usize = 14;

const PREFIXO: &str = "ecoviva-armazem-";
const SUFIXO: &str = ".db";

fn data_de_hoje(conn: &Connection) -> AppResult<String> {
    Ok(conn.query_row("SELECT date('now')", [], |r| r.get(0))?)
}

/// Faz uma copia consistente do banco em `<diretorio_dados>/backups/`, um arquivo
/// por dia (sobrescreve se rodar de novo no mesmo dia), e apaga backups alem da
/// retencao. Usa a Online Backup API do SQLite (via `Connection::backup`), que lida
/// corretamente com o modo WAL — uma copia bruta do arquivo `.db` poderia perder
/// escritas ainda so no `-wal`.
pub fn backup_automatico(conn: &Connection, diretorio_dados: &Path) -> AppResult<PathBuf> {
    let diretorio_backups = diretorio_dados.join("backups");
    fs::create_dir_all(&diretorio_backups).map_err(|e| {
        AppError::Interno(format!("Nao foi possivel criar a pasta de backups: {e}"))
    })?;

    let data = data_de_hoje(conn)?;
    let caminho = diretorio_backups.join(format!("{PREFIXO}{data}{SUFIXO}"));

    conn.backup(DatabaseName::Main, &caminho, None)?;

    limpar_backups_antigos(&diretorio_backups)?;

    Ok(caminho)
}

fn limpar_backups_antigos(diretorio_backups: &Path) -> AppResult<()> {
    let mut arquivos: Vec<PathBuf> = fs::read_dir(diretorio_backups)
        .map_err(|e| AppError::Interno(format!("Nao foi possivel listar backups: {e}")))?
        .filter_map(|entrada| entrada.ok())
        .map(|entrada| entrada.path())
        .filter(|caminho| {
            caminho
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(PREFIXO) && n.ends_with(SUFIXO))
        })
        .collect();

    arquivos.sort();

    if arquivos.len() > RETENCAO_DIAS {
        for antigo in &arquivos[..arquivos.len() - RETENCAO_DIAS] {
            let _ = fs::remove_file(antigo);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn diretorio_de_teste(nome: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ecoviva-teste-{nome}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn faz_backup_do_banco() {
        let dir = diretorio_de_teste("backup-simples");
        let conn = db::abrir_em_memoria().unwrap();

        let caminho = backup_automatico(&conn, &dir).unwrap();

        assert!(caminho.exists());
        assert!(caminho.starts_with(dir.join("backups")));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rodar_duas_vezes_no_mesmo_dia_nao_duplica_arquivo() {
        let dir = diretorio_de_teste("backup-idempotente");
        let conn = db::abrir_em_memoria().unwrap();

        backup_automatico(&conn, &dir).unwrap();
        backup_automatico(&conn, &dir).unwrap();

        let total = fs::read_dir(dir.join("backups")).unwrap().count();
        assert_eq!(total, 1);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn mantem_so_a_retencao_configurada() {
        let dir = diretorio_de_teste("backup-retencao");
        let diretorio_backups = dir.join("backups");
        fs::create_dir_all(&diretorio_backups).unwrap();

        for dia in 1..=(RETENCAO_DIAS + 5) {
            let nome = format!("{PREFIXO}2026-01-{dia:02}{SUFIXO}");
            fs::write(diretorio_backups.join(nome), b"fake").unwrap();
        }

        limpar_backups_antigos(&diretorio_backups).unwrap();

        let restantes = fs::read_dir(&diretorio_backups).unwrap().count();
        assert_eq!(restantes, RETENCAO_DIAS);

        // os que sobraram devem ser os mais recentes (dias mais altos)
        assert!(!diretorio_backups
            .join(format!("{PREFIXO}2026-01-01{SUFIXO}"))
            .exists());
        assert!(diretorio_backups
            .join(format!("{PREFIXO}2026-01-{:02}{SUFIXO}", RETENCAO_DIAS + 5))
            .exists());

        fs::remove_dir_all(&dir).ok();
    }
}
