use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, DatabaseName, OpenFlags, OptionalExtension};

use crate::domain::errors::{AppError, AppResult};

/// Quantos backups diarios manter antes de apagar os mais antigos.
const RETENCAO_DIAS: usize = 14;

const PREFIXO: &str = "ecoviva-armazem-";
const SUFIXO: &str = ".db";

const NOME_ARQUIVO_CONFIG_EXTERNO: &str = "backup_externo.txt";

fn data_de_hoje(conn: &Connection) -> AppResult<String> {
    Ok(conn.query_row("SELECT date('now')", [], |r| r.get(0))?)
}

/// Nucleo compartilhado por `backup_automatico` e `backup_externo`: grava o
/// backup do dia diretamente em `diretorio_backups` (sobrescreve se rodar de
/// novo no mesmo dia) e aplica a retencao configurada. Usa a Online Backup
/// API do SQLite (via `Connection::backup`), que lida corretamente com o modo
/// WAL — uma copia bruta do arquivo `.db` poderia perder escritas ainda so no
/// `-wal`.
fn fazer_backup_em(conn: &Connection, diretorio_backups: &Path) -> AppResult<PathBuf> {
    fs::create_dir_all(diretorio_backups).map_err(|e| {
        AppError::Interno(format!("Nao foi possivel criar a pasta de backups: {e}"))
    })?;

    let data = data_de_hoje(conn)?;
    let caminho = diretorio_backups.join(format!("{PREFIXO}{data}{SUFIXO}"));

    conn.backup(DatabaseName::Main, &caminho, None)?;

    limpar_backups_antigos(diretorio_backups)?;

    Ok(caminho)
}

/// Faz uma copia consistente do banco em `<diretorio_dados>/backups/`.
pub fn backup_automatico(conn: &Connection, diretorio_dados: &Path) -> AppResult<PathBuf> {
    fazer_backup_em(conn, &diretorio_dados.join("backups"))
}

/// Le `backup_externo.txt` (uma linha com o caminho de destino, ex.: um
/// pendrive/HD externo conectado no PC) na pasta de dados. `None` se o
/// arquivo nao existir ou estiver vazio — backup externo nao e obrigatorio,
/// so acontece se essa configuracao foi feita a mao na maquina.
pub fn ler_destino_externo(diretorio_dados: &Path) -> Option<PathBuf> {
    let conteudo = fs::read_to_string(diretorio_dados.join(NOME_ARQUIVO_CONFIG_EXTERNO)).ok()?;
    let caminho = conteudo.trim();
    if caminho.is_empty() {
        return None;
    }
    Some(PathBuf::from(caminho))
}

/// Copia o backup do dia tambem para `destino` (tipicamente um pendrive/HD
/// externo, configurado via `backup_externo.txt` e lido com
/// `ler_destino_externo`). Melhor-esforco por natureza: se a unidade estiver
/// desconectada no momento, esta chamada falha e quem chamou deve tratar como
/// nao-fatal (mesmo padrao de `backup_automatico` no `lib.rs`).
pub fn backup_externo(conn: &Connection, destino: &Path) -> AppResult<PathBuf> {
    fazer_backup_em(conn, destino)
}

const TABELAS_ESPERADAS: [&str; 4] = ["usuarios", "movimentos", "movimento_itens", "fechamentos"];

/// Abre um arquivo de backup como uma conexao separada, so leitura, e confere
/// que tem cara de banco valido da Ecoviva antes de alguem restaurar por cima
/// do banco real: as tabelas essenciais existem e `PRAGMA integrity_check`
/// nao acusa nada. Primeiro passo do procedimento de restauracao documentado
/// em `docs/ARQUITETURA.md`.
pub fn verificar_backup_valido(caminho: &Path) -> AppResult<()> {
    let conn = Connection::open_with_flags(caminho, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| AppError::Validation("Nao foi possivel abrir o arquivo de backup.".into()))?;

    for tabela in TABELAS_ESPERADAS {
        let existe: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params![tabela],
                |_| Ok(true),
            )
            .optional()
            .map_err(|_| AppError::Validation("Arquivo de backup nao parece valido.".into()))?
            .unwrap_or(false);
        if !existe {
            return Err(AppError::Validation(format!(
                "Arquivo de backup nao tem a tabela esperada '{tabela}' - pode estar corrompido ou incompleto."
            )));
        }
    }

    let integridade: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(|_| {
            AppError::Validation("Nao foi possivel checar a integridade do backup.".into())
        })?;
    if integridade != "ok" {
        return Err(AppError::Validation(format!(
            "Backup falhou na checagem de integridade: {integridade}"
        )));
    }

    Ok(())
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

    #[test]
    fn ler_destino_externo_retorna_none_quando_arquivo_nao_existe() {
        let dir = diretorio_de_teste("sem-config-externo");
        assert!(ler_destino_externo(&dir).is_none());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ler_destino_externo_retorna_none_quando_arquivo_vazio() {
        let dir = diretorio_de_teste("config-externo-vazio");
        fs::write(dir.join(NOME_ARQUIVO_CONFIG_EXTERNO), "   \n").unwrap();
        assert!(ler_destino_externo(&dir).is_none());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ler_destino_externo_le_caminho_configurado() {
        let dir = diretorio_de_teste("config-externo-valido");
        fs::write(
            dir.join(NOME_ARQUIVO_CONFIG_EXTERNO),
            "/media/pendrive/backups\n",
        )
        .unwrap();
        assert_eq!(
            ler_destino_externo(&dir),
            Some(PathBuf::from("/media/pendrive/backups"))
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn backup_externo_grava_no_destino_configurado() {
        let dir_dados = diretorio_de_teste("origem");
        let dir_pendrive = diretorio_de_teste("pendrive-destino");
        let conn = db::abrir_em_memoria().unwrap();

        let caminho = backup_externo(&conn, &dir_pendrive).unwrap();

        assert!(caminho.exists());
        assert!(caminho.starts_with(&dir_pendrive));

        fs::remove_dir_all(&dir_dados).ok();
        fs::remove_dir_all(&dir_pendrive).ok();
    }

    #[test]
    fn verifica_backup_valido_aceita_backup_de_verdade() {
        let dir = diretorio_de_teste("backup-valido");
        let conn = db::abrir_em_memoria().unwrap();

        let caminho = backup_automatico(&conn, &dir).unwrap();
        assert!(verificar_backup_valido(&caminho).is_ok());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn verifica_backup_valido_rejeita_arquivo_que_nao_e_banco_da_ecoviva() {
        let dir = diretorio_de_teste("backup-invalido");
        let caminho = dir.join("nao-e-um-backup.db");
        fs::write(&caminho, b"isso aqui nao e um banco SQLite").unwrap();

        assert!(matches!(
            verificar_backup_valido(&caminho),
            Err(AppError::Validation(_))
        ));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn restaura_backup_e_dados_e_cadeia_de_hash_sobrevivem() {
        use crate::domain::auth::{criar_usuario, NovoUsuario};
        use crate::domain::movimentos::{
            criar_movimento, verificar_cadeia, MovimentoItemInput, NovoMovimento,
        };

        let dir = diretorio_de_teste("restaura-round-trip");

        // Banco "de verdade" (arquivo em disco, nao em memoria), com dados reais.
        let mut conn = db::abrir(&dir).unwrap();
        let armazem_id: i64 = conn
            .query_row("SELECT id FROM armazens WHERE codigo = 'A4'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let usuario_id = criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Karol",
                login: "karol",
                senha: "senha123",
                armazem_id: Some(armazem_id),
                papel: "conferente",
            },
        )
        .unwrap();
        criar_movimento(
            &mut conn,
            NovoMovimento {
                armazem_id,
                armazem_destino_id: None,
                fluxo: "saida_armazem".into(),
                tipo: "saida".into(),
                data: "2026-08-25".into(),
                hora: "09:00".into(),
                turno: "diurno".into(),
                usuario_id,
                numero_pedido: Some("777".into()),
                codigo_rastreio: None,
                contraparte: Some("Cliente Teste Restauracao".into()),
                quem_retirou: Some("Fulano".into()),
                motivo: None,
                valor_centavos: None,
                observacoes: None,
                recebido_de_armazem_codigo: None,
                recebido_de_id_origem: None,
                retirada_completa: true,
                itens: vec![MovimentoItemInput {
                    categoria: "scooter".into(),
                    descricao: None,
                    montagem: None,
                    condicao: None,
                    quantidade: 3,
                    observacao: None,
                    quantidade_enviada: None,
                    codigo_componente: None,
                }],
            },
        )
        .unwrap();

        let caminho_backup = backup_automatico(&conn, &dir).unwrap();
        verificar_backup_valido(&caminho_backup).unwrap();

        drop(conn); // fecha a conexao original antes de mexer no arquivo em disco

        // Simula perda do banco original (disco corrompido, PC roubado etc.).
        let caminho_original = dir.join("ecoviva-armazem.db");
        fs::remove_file(&caminho_original).unwrap();
        let _ = fs::remove_file(dir.join("ecoviva-armazem.db-wal"));
        let _ = fs::remove_file(dir.join("ecoviva-armazem.db-shm"));
        assert!(!caminho_original.exists());

        // Restauracao manual documentada em docs/ARQUITETURA.md: copiar o
        // backup verificado por cima do lugar do banco real.
        fs::copy(&caminho_backup, &caminho_original).unwrap();

        let conn_restaurada = db::abrir(&dir).unwrap();
        let contraparte: String = conn_restaurada
            .query_row("SELECT contraparte FROM movimentos LIMIT 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(contraparte, "Cliente Teste Restauracao");

        // A cadeia de hash de auditoria tambem precisa sobreviver ao ciclo
        // completo de backup/restauracao, nao so os dados.
        assert!(verificar_cadeia(&conn_restaurada).unwrap().is_none());

        fs::remove_dir_all(&dir).ok();
    }
}
