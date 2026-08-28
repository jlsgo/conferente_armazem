use std::fs;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, ToSql};
use serde::Serialize;

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
/// bancos diferentes) e o codigo do armazem destino, se houver.
pub struct LinhaPendente {
    pub movimento: Movimento,
    pub armazem_codigo: String,
    pub armazem_destino_codigo: Option<String>,
    pub itens: Vec<MovimentoItem>,
}

/// Backoff progressivo por numero de tentativas ja feitas (antes desta):
/// 1min, 5min, 15min, 30min, e a partir da 5a tentativa fica fixo em 60min.
/// Funcao pura, sem estado - so decide quanto esperar antes de tentar de novo.
pub fn calcular_backoff_minutos(tentativas: i64) -> i64 {
    match tentativas {
        ..=1 => 1,
        2 => 5,
        3 => 15,
        4 => 30,
        _ => 60,
    }
}

/// Busca os movimentos que ainda nao foram enviados pro Turso
/// (`sincronizado_em IS NULL`) e que nao estao "esperando o backoff" de uma
/// falha recente (`sync_proxima_tentativa` no futuro), mais antigos primeiro.
/// Pura leitura local, sem depender de rede - testada normalmente com SQLite
/// em memoria.
pub fn movimentos_pendentes(conn: &Connection) -> AppResult<Vec<LinhaPendente>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.armazem_id, a.codigo, m.armazem_destino_id, ad.codigo, m.fluxo,
                m.tipo, m.data, m.hora, m.turno, m.usuario_id, u.nome, m.numero_pedido,
                m.codigo_rastreio, m.contraparte, m.quem_retirou, m.motivo, m.valor_centavos,
                m.observacoes, m.status, m.estornado_de, m.recebido_de_armazem_codigo,
                m.recebido_de_id_origem, m.retirada_completa, m.hash_integridade
         FROM movimentos m
         JOIN armazens a ON a.id = m.armazem_id
         LEFT JOIN armazens ad ON ad.id = m.armazem_destino_id
         JOIN usuarios u ON u.id = m.usuario_id
         WHERE m.sincronizado_em IS NULL
           AND (m.sync_proxima_tentativa IS NULL OR m.sync_proxima_tentativa <= datetime('now'))
         ORDER BY m.id ASC",
    )?;

    let linhas = stmt
        .query_map([], |r| {
            let id: i64 = r.get(0)?;
            let armazem_codigo: String = r.get(2)?;
            let armazem_destino_codigo: Option<String> = r.get(4)?;
            Ok((
                Movimento {
                    id,
                    numero: 0,
                    armazem_id: r.get(1)?,
                    armazem_destino_id: r.get(3)?,
                    fluxo: r.get(5)?,
                    tipo: r.get(6)?,
                    data: r.get(7)?,
                    hora: r.get(8)?,
                    turno: r.get(9)?,
                    usuario_id: r.get(10)?,
                    usuario_nome: r.get(11)?,
                    numero_pedido: r.get(12)?,
                    codigo_rastreio: r.get(13)?,
                    contraparte: r.get(14)?,
                    quem_retirou: r.get(15)?,
                    motivo: r.get(16)?,
                    valor_centavos: r.get(17)?,
                    observacoes: r.get(18)?,
                    status: r.get(19)?,
                    estornado_de: r.get(20)?,
                    recebido_de_armazem_codigo: r.get(21)?,
                    recebido_de_id_origem: r.get(22)?,
                    retirada_completa: r.get(23)?,
                    hash_integridade: r.get(24)?,
                    itens: Vec::new(),
                },
                armazem_codigo,
                armazem_destino_codigo,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut resultado = Vec::with_capacity(linhas.len());
    for (movimento, armazem_codigo, armazem_destino_codigo) in linhas {
        let itens = carregar_itens(conn, movimento.id)?;
        resultado.push(LinhaPendente {
            movimento,
            armazem_codigo,
            armazem_destino_codigo,
            itens,
        });
    }

    Ok(resultado)
}

/// Horario local (fuso do PC que esta enviando, ex.: Brasilia) formatado
/// como o SQLite formata (`AAAA-MM-DD HH:MM:SS`). Usado por quem chama
/// `enviar_para_turso` pra fornecer `enviado_em` - o Turso roda na nuvem
/// (fuso do servidor, nao o do armazem), entao carimbar isso no lado
/// remoto daria hora errada; melhor calcular aqui, no PC de origem, com a
/// mesma conexao que ja busca os pendentes, e mandar pronto.
pub fn agora_local(conn: &Connection) -> AppResult<String> {
    Ok(conn.query_row("SELECT datetime('now', 'localtime')", [], |r| r.get(0))?)
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

/// Registra uma tentativa de envio que falhou: incrementa `sync_tentativas`,
/// grava o motivo em `sync_erro` e agenda `sync_proxima_tentativa` com o
/// backoff correspondente - a linha so volta a aparecer em
/// `movimentos_pendentes` depois desse horario.
pub fn marcar_falha_sincronizacao(conn: &Connection, falhas: &[(i64, String)]) -> AppResult<()> {
    for (id, erro) in falhas {
        let erro_curto: String = erro.chars().take(300).collect();
        conn.execute(
            "UPDATE movimentos
             SET sync_tentativas = sync_tentativas + 1,
                 sync_erro = ?1,
                 sync_proxima_tentativa = datetime('now', '+' || ?2 || ' minutes')
             WHERE id = ?3",
            rusqlite::params![
                erro_curto,
                calcular_backoff_minutos(
                    conn.query_row(
                        "SELECT sync_tentativas FROM movimentos WHERE id = ?1",
                        rusqlite::params![id],
                        |r| r.get::<_, i64>(0)
                    )? + 1
                ),
                id
            ],
        )?;
    }
    Ok(())
}

/// Retrato local do estado da fila de sincronizacao, pra mostrar ao gestor
/// sem precisar de rede.
#[derive(Debug, Serialize)]
pub struct StatusSincronizacao {
    pub pendentes: i64,
    pub com_erro: i64,
    pub ultimo_erro: Option<String>,
}

pub fn status_sincronizacao(conn: &Connection) -> AppResult<StatusSincronizacao> {
    let pendentes = conn.query_row(
        "SELECT COUNT(*) FROM movimentos WHERE sincronizado_em IS NULL",
        [],
        |r| r.get(0),
    )?;
    let com_erro = conn.query_row(
        "SELECT COUNT(*) FROM movimentos WHERE sincronizado_em IS NULL AND sync_tentativas > 0",
        [],
        |r| r.get(0),
    )?;
    let ultimo_erro = conn
        .query_row(
            "SELECT sync_erro FROM movimentos
             WHERE sincronizado_em IS NULL AND sync_erro IS NOT NULL
             ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    Ok(StatusSincronizacao {
        pendentes,
        com_erro,
        ultimo_erro,
    })
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

/// Adicionadas depois da v1 (confirmacao de recebimento entre armazens).
/// `ALTER TABLE ADD COLUMN` nao tem `IF NOT EXISTS` no SQLite/libSQL - os
/// erros sao ignorados de proposito (`let _ =`) porque a unica forma de
/// falhar aqui e a coluna ja existir (banco criado por uma versao anterior
/// do app), o que e inofensivo.
const SQL_ALTER_TABELA_REMOTA: [&str; 3] = [
    "ALTER TABLE movimentos_consolidados ADD COLUMN armazem_destino_codigo TEXT",
    "ALTER TABLE movimentos_consolidados ADD COLUMN recebido_de_armazem_codigo TEXT",
    "ALTER TABLE movimentos_consolidados ADD COLUMN recebido_de_id_origem INTEGER",
];

const SQL_UPSERT: &str = "
    INSERT OR REPLACE INTO movimentos_consolidados
        (armazem_codigo, id_origem, fluxo, tipo, data, hora, turno, usuario_nome,
         numero_pedido, codigo_rastreio, contraparte, quem_retirou, motivo,
         valor_centavos, observacoes, status, estornado_de, hash_integridade,
         itens_json, armazem_destino_codigo, recebido_de_armazem_codigo,
         recebido_de_id_origem, enviado_em)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
            ?19, ?20, ?21, ?22, ?23)
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
#[derive(Debug, Default)]
pub struct ResultadoSincronizacao {
    pub enviados: Vec<i64>,
    pub falhas: Vec<(i64, String)>,
}

pub async fn enviar_para_turso(
    url: &str,
    token: &str,
    pendentes: &[LinhaPendente],
    enviado_em_local: &str,
) -> AppResult<ResultadoSincronizacao> {
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

    for alter in SQL_ALTER_TABELA_REMOTA {
        let _ = remoto.execute(alter, ()).await;
    }

    let mut resultado = ResultadoSincronizacao::default();

    for linha in pendentes {
        let itens_json = serde_json::to_string(&linha.itens)
            .map_err(|e| AppError::Interno(format!("Nao foi possivel serializar os itens: {e}")))?;

        let execucao = remoto
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
                    linha.armazem_destino_codigo.clone(),
                    linha.movimento.recebido_de_armazem_codigo.clone(),
                    linha.movimento.recebido_de_id_origem,
                    enviado_em_local,
                ],
            )
            .await;

        match execucao {
            Ok(_) => resultado.enviados.push(linha.movimento.id),
            Err(e) => resultado.falhas.push((linha.movimento.id, e.to_string())),
        }
    }

    Ok(resultado)
}

/// Um envio (`saida` de `peca_montagem` OU `saida_armazem` com
/// `armazem_destino_codigo` preenchido) visto do lado de quem vai receber -
/// vem direto do Turso, nunca do banco local (o envio original vive no PC do
/// outro armazem). O fluxo viaja junto pra `confirmar_recebimento` gravar a
/// entrada de confirmacao no mesmo fluxo do envio original (uma transferencia
/// de veiculo confirmada em Saida de Armazem, uma de peca solta em Montagem)
/// e pra cada tela filtrar so as pendencias do seu proprio fluxo.
#[derive(Debug, Serialize)]
pub struct TransferenciaPendente {
    pub armazem_origem_codigo: String,
    pub id_origem: i64,
    pub fluxo: String,
    pub data: String,
    pub hora: String,
    /// Sempre `Some` na pratica (so existe transferencia com destino
    /// definido) - usado por `confirmar_recebimento` pra conferir que a
    /// transferencia buscada por chave realmente era endereçada a quem esta
    /// confirmando, nao so a quem sabia o `(armazem_codigo, id_origem)`.
    pub armazem_destino_codigo: Option<String>,
    pub itens: Vec<MovimentoItem>,
}

#[allow(clippy::too_many_arguments)]
fn linha_para_transferencia(
    armazem_origem_codigo: String,
    id_origem: i64,
    fluxo: String,
    data: String,
    hora: String,
    armazem_destino_codigo: Option<String>,
    itens_json: String,
) -> AppResult<TransferenciaPendente> {
    let itens: Vec<MovimentoItem> = serde_json::from_str(&itens_json).map_err(|e| {
        AppError::Interno(format!(
            "Nao foi possivel ler os itens da transferencia: {e}"
        ))
    })?;
    Ok(TransferenciaPendente {
        armazem_origem_codigo,
        id_origem,
        fluxo,
        data,
        hora,
        armazem_destino_codigo,
        itens,
    })
}

const SQL_PENDENTES_RECEBIMENTO: &str = "
    SELECT armazem_codigo, id_origem, fluxo, data, hora, armazem_destino_codigo, itens_json
    FROM movimentos_consolidados m
    WHERE armazem_destino_codigo = ?1
      -- Fluxos que suportam transferencia fisica entre A4 e B2: veiculos
      -- (saida_armazem) e peca solta (peca_montagem). SAC fica de fora: nao
      -- faz sentido de negocio transferir uma devolucao de garantia entre
      -- armazens.
      AND fluxo IN ('peca_montagem', 'saida_armazem')
      AND tipo = 'saida'
      AND estornado_de IS NULL
      AND NOT EXISTS (
        SELECT 1 FROM movimentos_consolidados x
        WHERE x.armazem_codigo = m.armazem_codigo AND x.estornado_de = m.id_origem
      )
      AND NOT EXISTS (
        SELECT 1 FROM movimentos_consolidados c
        WHERE c.recebido_de_armazem_codigo = m.armazem_codigo
          AND c.recebido_de_id_origem = m.id_origem
      )
    ORDER BY data ASC, hora ASC
";

/// Busca no Turso o que foi enviado pro `meu_armazem_codigo` e ainda nao foi
/// confirmado (nem estornado do lado de quem enviou). So pode ser testado de
/// ponta a ponta com uma conta Turso real - ver docs/ARQUITETURA.md.
pub async fn buscar_pendentes_recebimento(
    url: &str,
    token: &str,
    meu_armazem_codigo: &str,
) -> AppResult<Vec<TransferenciaPendente>> {
    let banco = libsql::Builder::new_remote(url.to_string(), token.to_string())
        .build()
        .await
        .map_err(|e| AppError::Interno(format!("Nao foi possivel conectar ao Turso: {e}")))?;
    let remoto = banco
        .connect()
        .map_err(|e| AppError::Interno(format!("Nao foi possivel abrir a conexao remota: {e}")))?;

    let mut rows = remoto
        .query(
            SQL_PENDENTES_RECEBIMENTO,
            libsql::params![meu_armazem_codigo],
        )
        .await
        .map_err(|e| {
            AppError::Interno(format!(
                "Nao foi possivel buscar transferencias pendentes: {e}"
            ))
        })?;

    let mut resultado = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| AppError::Interno(format!("Erro lendo transferencias pendentes: {e}")))?
    {
        let armazem_origem_codigo: String = row
            .get(0)
            .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
        let id_origem: i64 = row
            .get(1)
            .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
        let fluxo: String = row
            .get(2)
            .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
        let data: String = row
            .get(3)
            .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
        let hora: String = row
            .get(4)
            .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
        let armazem_destino_codigo: Option<String> = row
            .get(5)
            .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
        let itens_json: String = row
            .get(6)
            .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
        resultado.push(linha_para_transferencia(
            armazem_origem_codigo,
            id_origem,
            fluxo,
            data,
            hora,
            armazem_destino_codigo,
            itens_json,
        )?);
    }

    Ok(resultado)
}

/// Busca de novo, direto no Turso, uma transferencia especifica pela sua
/// chave `(armazem_codigo, id_origem)` - usada por
/// `commands::sync_commands::confirmar_recebimento` pra nunca confiar nos
/// itens que o frontend mandar de volta, so no que esta realmente gravado.
pub async fn buscar_transferencia(
    url: &str,
    token: &str,
    armazem_origem_codigo: &str,
    id_origem: i64,
) -> AppResult<Option<TransferenciaPendente>> {
    let banco = libsql::Builder::new_remote(url.to_string(), token.to_string())
        .build()
        .await
        .map_err(|e| AppError::Interno(format!("Nao foi possivel conectar ao Turso: {e}")))?;
    let remoto = banco
        .connect()
        .map_err(|e| AppError::Interno(format!("Nao foi possivel abrir a conexao remota: {e}")))?;

    let mut rows = remoto
        .query(
            "SELECT armazem_codigo, id_origem, fluxo, data, hora, armazem_destino_codigo, itens_json
             FROM movimentos_consolidados WHERE armazem_codigo = ?1 AND id_origem = ?2",
            libsql::params![armazem_origem_codigo, id_origem],
        )
        .await
        .map_err(|e| AppError::Interno(format!("Nao foi possivel buscar a transferencia: {e}")))?;

    let Some(row) = rows
        .next()
        .await
        .map_err(|e| AppError::Interno(format!("Erro lendo a transferencia: {e}")))?
    else {
        return Ok(None);
    };

    let armazem_origem_codigo: String = row
        .get(0)
        .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
    let id_origem: i64 = row
        .get(1)
        .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
    let fluxo: String = row
        .get(2)
        .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
    let data: String = row
        .get(3)
        .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
    let hora: String = row
        .get(4)
        .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
    let armazem_destino_codigo: Option<String> = row
        .get(5)
        .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
    let itens_json: String = row
        .get(6)
        .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;

    Ok(Some(linha_para_transferencia(
        armazem_origem_codigo,
        id_origem,
        fluxo,
        data,
        hora,
        armazem_destino_codigo,
        itens_json,
    )?))
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
                recebido_de_armazem_codigo: None,
                recebido_de_id_origem: None,
                retirada_completa: true,
                itens: vec![MovimentoItemInput {
                    categoria: "scooter".into(),
                    descricao: None,
                    montagem: None,
                    condicao: None,
                    quantidade: 1,
                    observacao: None,
                    quantidade_enviada: None,
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

    #[test]
    fn calcular_backoff_minutos_cresce_e_depois_estabiliza() {
        assert_eq!(calcular_backoff_minutos(0), 1);
        assert_eq!(calcular_backoff_minutos(1), 1);
        assert_eq!(calcular_backoff_minutos(2), 5);
        assert_eq!(calcular_backoff_minutos(3), 15);
        assert_eq!(calcular_backoff_minutos(4), 30);
        assert_eq!(calcular_backoff_minutos(5), 60);
        assert_eq!(calcular_backoff_minutos(50), 60);
    }

    #[test]
    fn marcar_falha_sincronizacao_tira_da_lista_de_pendentes_ate_o_backoff_passar() {
        let (conn, movimento_id) = conexao_com_movimento();
        marcar_falha_sincronizacao(&conn, &[(movimento_id, "sem internet".into())]).unwrap();

        // sync_proxima_tentativa fica no futuro (1 min) - some da lista.
        assert!(movimentos_pendentes(&conn).unwrap().is_empty());

        let status = status_sincronizacao(&conn).unwrap();
        assert_eq!(status.pendentes, 1);
        assert_eq!(status.com_erro, 1);
        assert_eq!(status.ultimo_erro.as_deref(), Some("sem internet"));

        // Depois que o horario do backoff passa, volta a aparecer.
        conn.execute(
            "UPDATE movimentos SET sync_proxima_tentativa = datetime('now', '-1 minute') WHERE id = ?1",
            rusqlite::params![movimento_id],
        )
        .unwrap();
        assert_eq!(movimentos_pendentes(&conn).unwrap().len(), 1);
    }

    #[test]
    fn status_sincronizacao_sem_pendencias_fica_zerado() {
        let (conn, movimento_id) = conexao_com_movimento();
        marcar_sincronizado(&conn, &[movimento_id]).unwrap();
        let status = status_sincronizacao(&conn).unwrap();
        assert_eq!(status.pendentes, 0);
        assert_eq!(status.com_erro, 0);
        assert_eq!(status.ultimo_erro, None);
    }

    #[test]
    fn movimentos_pendentes_resolve_codigo_do_armazem_destino() {
        let mut conn = db::abrir_em_memoria().unwrap();
        let armazem_b2: i64 = conn
            .query_row("SELECT id FROM armazens WHERE codigo = 'B2'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let armazem_a4: i64 = conn
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
                armazem_id: Some(armazem_b2),
                papel: "conferente",
            },
        )
        .unwrap();
        criar_movimento(
            &mut conn,
            NovoMovimento {
                armazem_id: armazem_b2,
                armazem_destino_id: Some(armazem_a4),
                fluxo: "peca_montagem".into(),
                tipo: "saida".into(),
                data: "2026-08-25".into(),
                hora: "09:00".into(),
                turno: "diurno".into(),
                usuario_id,
                numero_pedido: None,
                codigo_rastreio: None,
                contraparte: None,
                quem_retirou: None,
                motivo: None,
                valor_centavos: None,
                observacoes: None,
                recebido_de_armazem_codigo: None,
                recebido_de_id_origem: None,
                retirada_completa: true,
                itens: vec![MovimentoItemInput {
                    categoria: "peca".into(),
                    descricao: None,
                    montagem: None,
                    condicao: Some("boa".into()),
                    quantidade: 2,
                    observacao: None,
                    quantidade_enviada: None,
                }],
            },
        )
        .unwrap();

        let pendentes = movimentos_pendentes(&conn).unwrap();
        assert_eq!(pendentes.len(), 1);
        assert_eq!(pendentes[0].armazem_codigo, "B2");
        assert_eq!(pendentes[0].armazem_destino_codigo.as_deref(), Some("A4"));
    }

    #[test]
    fn linha_para_transferencia_parseia_itens_json() {
        let itens_json =
            r#"[{"id":0,"categoria":"peca","descricao":"Bateria","montagem":null,"condicao":"boa","quantidade":3,"observacao":"SN-123"}]"#
                .to_string();
        let transferencia = linha_para_transferencia(
            "B2".into(),
            42,
            "peca_montagem".into(),
            "2026-08-25".into(),
            "09:00".into(),
            Some("A4".into()),
            itens_json,
        )
        .unwrap();

        assert_eq!(transferencia.armazem_origem_codigo, "B2");
        assert_eq!(transferencia.id_origem, 42);
        assert_eq!(transferencia.fluxo, "peca_montagem");
        assert_eq!(transferencia.armazem_destino_codigo.as_deref(), Some("A4"));
        assert_eq!(transferencia.itens.len(), 1);
        assert_eq!(transferencia.itens[0].quantidade, 3);
        assert_eq!(transferencia.itens[0].observacao.as_deref(), Some("SN-123"));
    }

    #[test]
    fn linha_para_transferencia_preserva_fluxo_saida_armazem() {
        let transferencia = linha_para_transferencia(
            "A4".into(),
            7,
            "saida_armazem".into(),
            "2026-08-25".into(),
            "10:00".into(),
            Some("B2".into()),
            "[]".into(),
        )
        .unwrap();

        assert_eq!(transferencia.fluxo, "saida_armazem");
    }

    #[test]
    fn linha_para_transferencia_rejeita_json_invalido() {
        let resultado = linha_para_transferencia(
            "B2".into(),
            1,
            "peca_montagem".into(),
            "2026-08-25".into(),
            "09:00".into(),
            Some("A4".into()),
            "nao e json".into(),
        );
        assert!(resultado.is_err());
    }
}
