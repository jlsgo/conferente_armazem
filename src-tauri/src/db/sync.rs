use std::fs;
use std::path::Path;

use rusqlite::{Connection, ToSql};

use crate::domain::errors::{AppError, AppResult};
use crate::domain::movimentos::{carregar_itens, Movimento, MovimentoItem};

const NOME_ARQUIVO_CONFIG_TURSO: &str = "turso.txt";

/// Le `turso.txt` (duas linhas: URL `libsql://...` e token) na pasta de
/// dados. `None` se o arquivo nao existir ou estiver incompleto - a
/// sincronizacao e sempre melhor-esforco, o app funciona 100% offline sem
/// isso configurado. As credenciais sao geradas pelo usuario com `turso db
/// create` + `turso db tokens create` (ver docs/ARQUITETURA.md).
pub fn ler_config_turso(diretorio_dados: &Path) -> Option<(String, String)> {
    let conteudo = fs::read_to_string(diretorio_dados.join(NOME_ARQUIVO_CONFIG_TURSO)).ok()?;
    let mut linhas = conteudo.lines().map(str::trim).filter(|l| !l.is_empty());
    let url = linhas.next()?.to_string();
    let token = linhas.next()?.to_string();
    Some((url, token))
}

/// Um movimento local ainda nao confirmado na nuvem, com o codigo do armazem
/// (o Turso e compartilhado entre A4 e B2, entao a chave remota usa o
/// codigo - 'A4'/'B2' - em vez do id local, que nao tem significado entre
/// bancos diferentes).
pub struct LinhaPendente {
    pub movimento: Movimento,
    pub armazem_codigo: String,
    pub itens: Vec<MovimentoItem>,
}

/// Busca os movimentos que ainda nao foram enviados pro Turso
/// (`sincronizado_em IS NULL`), mais antigos primeiro. Pura leitura local,
/// sem depender de rede - testada normalmente com SQLite em memoria.
pub fn movimentos_pendentes(conn: &Connection) -> AppResult<Vec<LinhaPendente>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.armazem_id, a.codigo, m.fluxo, m.tipo, m.data, m.hora, m.turno,
                m.usuario_id, u.nome, m.numero_pedido, m.codigo_rastreio, m.contraparte,
                m.quem_retirou, m.motivo, m.valor_centavos, m.observacoes, m.status,
                m.estornado_de, m.hash_integridade
         FROM movimentos m
         JOIN armazens a ON a.id = m.armazem_id
         JOIN usuarios u ON u.id = m.usuario_id
         WHERE m.sincronizado_em IS NULL
         ORDER BY m.id ASC",
    )?;

    let linhas = stmt
        .query_map([], |r| {
            let id: i64 = r.get(0)?;
            let armazem_codigo: String = r.get(2)?;
            Ok((
                Movimento {
                    id,
                    numero: 0,
                    armazem_id: r.get(1)?,
                    fluxo: r.get(3)?,
                    tipo: r.get(4)?,
                    data: r.get(5)?,
                    hora: r.get(6)?,
                    turno: r.get(7)?,
                    usuario_id: r.get(8)?,
                    usuario_nome: r.get(9)?,
                    numero_pedido: r.get(10)?,
                    codigo_rastreio: r.get(11)?,
                    contraparte: r.get(12)?,
                    quem_retirou: r.get(13)?,
                    motivo: r.get(14)?,
                    valor_centavos: r.get(15)?,
                    observacoes: r.get(16)?,
                    status: r.get(17)?,
                    estornado_de: r.get(18)?,
                    hash_integridade: r.get(19)?,
                    itens: Vec::new(),
                },
                armazem_codigo,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut resultado = Vec::with_capacity(linhas.len());
    for (movimento, armazem_codigo) in linhas {
        let itens = carregar_itens(conn, movimento.id)?;
        resultado.push(LinhaPendente {
            movimento,
            armazem_codigo,
            itens,
        });
    }

    Ok(resultado)
}

/// Marca os movimentos como enviados com sucesso (`sincronizado_em =
/// datetime('now')`). Chamada so com os ids que o Turso realmente confirmou.
pub fn marcar_sincronizado(conn: &Connection, ids: &[i64]) -> AppResult<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let marcadores = vec!["?"; ids.len()].join(",");
    let sql = format!(
        "UPDATE movimentos SET sincronizado_em = datetime('now') WHERE id IN ({marcadores})"
    );
    let parametros: Vec<&dyn ToSql> = ids.iter().map(|id| id as &dyn ToSql).collect();
    conn.execute(&sql, parametros.as_slice())?;
    Ok(())
}

const SQL_CRIAR_TABELA_REMOTA: &str = "
    CREATE TABLE IF NOT EXISTS movimentos_consolidados (
        armazem_codigo TEXT NOT NULL,
        id_origem INTEGER NOT NULL,
        fluxo TEXT NOT NULL,
        tipo TEXT NOT NULL,
        data TEXT NOT NULL,
        hora TEXT NOT NULL,
        turno TEXT NOT NULL,
        usuario_nome TEXT NOT NULL,
        numero_pedido TEXT,
        codigo_rastreio TEXT,
        contraparte TEXT,
        quem_retirou TEXT,
        motivo TEXT,
        valor_centavos INTEGER,
        observacoes TEXT,
        status TEXT NOT NULL,
        estornado_de INTEGER,
        hash_integridade TEXT NOT NULL,
        itens_json TEXT NOT NULL,
        enviado_em TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY (armazem_codigo, id_origem)
    )
";

const SQL_UPSERT: &str = "
    INSERT OR REPLACE INTO movimentos_consolidados
        (armazem_codigo, id_origem, fluxo, tipo, data, hora, turno, usuario_nome,
         numero_pedido, codigo_rastreio, contraparte, quem_retirou, motivo,
         valor_centavos, observacoes, status, estornado_de, hash_integridade,
         itens_json, enviado_em)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, datetime('now'))
";

/// Conecta no banco Turso configurado, garante que a tabela consolidada
/// exista, e envia os `pendentes` (upsert idempotente por
/// `(armazem_codigo, id_origem)`, seguro em caso de reenvio apos falha no
/// meio do caminho). Retorna os ids que o Turso confirmou.
///
/// Deliberadamente nao recebe a `Connection` local: `rusqlite::Connection`
/// nao e `Sync`, entao segurar uma referencia (ou o `MutexGuard` de
/// `AppState`) durante um `.await` de rede tornaria a future nao-`Send` (alem
/// de travar toda escrita/leitura no app pela duracao da chamada de rede,
/// ja que o Mutex e global). Quem chama busca os pendentes, solta a conexao,
/// so entao chama isto, e reabre a conexao so no fim pra marcar como
/// enviado - ver `commands::sync_commands::sincronizar_agora`.
///
/// So o passo local (quais linhas estao pendentes, marcar como enviado) e
/// coberto por teste automatizado - o passo de rede em si so pode ser
/// validado com uma conta/banco Turso real (ver docs/ARQUITETURA.md).
pub async fn enviar_para_turso(
    url: &str,
    token: &str,
    pendentes: &[LinhaPendente],
) -> AppResult<Vec<i64>> {
    let banco = libsql::Builder::new_remote(url.to_string(), token.to_string())
        .build()
        .await
        .map_err(|e| AppError::Interno(format!("Nao foi possivel conectar ao Turso: {e}")))?;

    let remoto = banco
        .connect()
        .map_err(|e| AppError::Interno(format!("Nao foi possivel abrir a conexao remota: {e}")))?;

    remoto
        .execute(SQL_CRIAR_TABELA_REMOTA, ())
        .await
        .map_err(|e| {
            AppError::Interno(format!("Nao foi possivel preparar a tabela remota: {e}"))
        })?;

    let mut enviados = Vec::new();

    for linha in pendentes {
        let itens_json = serde_json::to_string(&linha.itens)
            .map_err(|e| AppError::Interno(format!("Nao foi possivel serializar os itens: {e}")))?;

        let resultado = remoto
            .execute(
                SQL_UPSERT,
                libsql::params![
                    linha.armazem_codigo.clone(),
                    linha.movimento.id,
                    linha.movimento.fluxo.clone(),
                    linha.movimento.tipo.clone(),
                    linha.movimento.data.clone(),
                    linha.movimento.hora.clone(),
                    linha.movimento.turno.clone(),
                    linha.movimento.usuario_nome.clone(),
                    linha.movimento.numero_pedido.clone(),
                    linha.movimento.codigo_rastreio.clone(),
                    linha.movimento.contraparte.clone(),
                    linha.movimento.quem_retirou.clone(),
                    linha.movimento.motivo.clone(),
                    linha.movimento.valor_centavos,
                    linha.movimento.observacoes.clone(),
                    linha.movimento.status.clone(),
                    linha.movimento.estornado_de,
                    linha.movimento.hash_integridade.clone(),
                    itens_json,
                ],
            )
            .await;

        if resultado.is_ok() {
            enviados.push(linha.movimento.id);
        }
    }

    Ok(enviados)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::domain::auth::{criar_usuario, NovoUsuario};
    use crate::domain::movimentos::{criar_movimento, MovimentoItemInput, NovoMovimento};
    use std::path::PathBuf;

    fn diretorio_de_teste(nome: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ecoviva-teste-sync-{nome}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn ler_config_turso_retorna_none_quando_arquivo_nao_existe() {
        let dir = diretorio_de_teste("sem-config");
        assert!(ler_config_turso(&dir).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ler_config_turso_retorna_none_quando_falta_o_token() {
        let dir = diretorio_de_teste("config-incompleta");
        std::fs::write(
            dir.join(NOME_ARQUIVO_CONFIG_TURSO),
            "libsql://exemplo.turso.io\n",
        )
        .unwrap();
        assert!(ler_config_turso(&dir).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ler_config_turso_le_url_e_token() {
        let dir = diretorio_de_teste("config-valida");
        std::fs::write(
            dir.join(NOME_ARQUIVO_CONFIG_TURSO),
            "libsql://exemplo.turso.io\nmeu-token-secreto\n",
        )
        .unwrap();
        assert_eq!(
            ler_config_turso(&dir),
            Some((
                "libsql://exemplo.turso.io".to_string(),
                "meu-token-secreto".to_string()
            ))
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    fn conexao_com_movimento() -> (Connection, i64) {
        let mut conn = db::abrir_em_memoria().unwrap();
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
        let movimento = criar_movimento(
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
                numero_pedido: Some("1".into()),
                codigo_rastreio: None,
                contraparte: None,
                quem_retirou: None,
                motivo: None,
                valor_centavos: None,
                observacoes: None,
                itens: vec![MovimentoItemInput {
                    categoria: "scooter".into(),
                    descricao: None,
                    montagem: None,
                    condicao: None,
                    quantidade: 1,
                    observacao: None,
                }],
            },
        )
        .unwrap();
        (conn, movimento.id)
    }

    #[test]
    fn movimento_novo_aparece_como_pendente() {
        let (conn, movimento_id) = conexao_com_movimento();
        let pendentes = movimentos_pendentes(&conn).unwrap();
        assert_eq!(pendentes.len(), 1);
        assert_eq!(pendentes[0].movimento.id, movimento_id);
        assert_eq!(pendentes[0].armazem_codigo, "A4");
        assert_eq!(pendentes[0].itens.len(), 1);
    }

    #[test]
    fn marcar_sincronizado_tira_da_lista_de_pendentes() {
        let (conn, movimento_id) = conexao_com_movimento();
        marcar_sincronizado(&conn, &[movimento_id]).unwrap();
        assert!(movimentos_pendentes(&conn).unwrap().is_empty());
    }

    #[test]
    fn marcar_sincronizado_com_lista_vazia_nao_falha() {
        let (conn, _movimento_id) = conexao_com_movimento();
        assert!(marcar_sincronizado(&conn, &[]).is_ok());
        assert_eq!(movimentos_pendentes(&conn).unwrap().len(), 1);
    }
}
