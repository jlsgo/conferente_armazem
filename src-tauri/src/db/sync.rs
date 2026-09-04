use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, ToSql};
use serde::Serialize;

use crate::domain::errors::{AppError, AppResult};
use crate::domain::movimentos::{carregar_itens, Movimento, MovimentoItem};

pub(crate) const NOME_ARQUIVO_CONFIG_TURSO: &str = "turso.txt";

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
/// O passo local (quais linhas estao pendentes, marcar como enviado) e
/// coberto por teste automatizado, e o SQL_UPSERT/SELECT em si tambem -
/// contra um `rusqlite` em memoria (ver `tests::conexao_remota_de_teste`),
/// ja que essas strings sao SQLite generico, nao especifico de libsql. So a
/// conexao de rede propriamente dita (`Builder::new_remote`, autenticacao)
/// so pode ser validada com uma conta/banco Turso real (ver
/// docs/ARQUITETURA.md).
#[derive(Debug, Default)]
pub struct ResultadoSincronizacao {
    pub enviados: Vec<i64>,
    pub falhas: Vec<(i64, String)>,
}

/// Conecta no Turso e garante que a tabela consolidada exista. Isolado de
/// `enviar_para_turso` pra que uma falha aqui (sem internet, Turso fora do
/// ar, token expirado - exatamente o cenario de uma queda de conexao
/// completa, nao uma falha por linha) vire `falhas` pra todo o lote em vez
/// de abortar a funcao inteira com `Err` - sem isso, uma queda total nunca
/// era registrada via `marcar_falha_sincronizacao`, e `status_sincronizacao`
/// continuava mostrando "0 com erro" mesmo depois de dias sem sincronizar.
async fn conectar_turso(url: &str, token: &str) -> AppResult<libsql::Connection> {
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

    Ok(remoto)
}

pub async fn enviar_para_turso(
    url: &str,
    token: &str,
    pendentes: &[LinhaPendente],
    enviado_em_local: &str,
) -> AppResult<ResultadoSincronizacao> {
    let remoto = match conectar_turso(url, token).await {
        Ok(remoto) => remoto,
        Err(e) => {
            let falhas = pendentes
                .iter()
                .map(|linha| (linha.movimento.id, e.to_string()))
                .collect();
            return Ok(ResultadoSincronizacao {
                enviados: Vec::new(),
                falhas,
            });
        }
    };

    for alter in SQL_ALTER_TABELA_REMOTA {
        let _ = remoto.execute(alter, ()).await;
    }

    let mut resultado = ResultadoSincronizacao::default();

    for linha in pendentes {
        let itens_json = match serde_json::to_string(&linha.itens) {
            Ok(json) => json,
            Err(e) => {
                resultado.falhas.push((
                    linha.movimento.id,
                    format!("Nao foi possivel serializar os itens: {e}"),
                ));
                continue;
            }
        };

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

const PREFIXO_EXPORT_CONSOLIDADO: &str = "movimentos_consolidados-";
const SUFIXO_EXPORT_CONSOLIDADO: &str = ".json";

/// Converte um valor generico do Turso pra JSON. `SELECT *` nao tem como
/// mapear pra structs Rust tipados (o export deliberadamente nao usa uma
/// lista curada de colunas - ver `exportar_consolidado`), entao cada valor
/// vira um `serde_json::Value` dinamico, igual o `sqlite3` faria num
/// `.dump`/export generico.
fn valor_para_json(valor: libsql::Value) -> serde_json::Value {
    match valor {
        libsql::Value::Null => serde_json::Value::Null,
        libsql::Value::Integer(n) => serde_json::Value::from(n),
        libsql::Value::Real(f) => serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        libsql::Value::Text(s) => serde_json::Value::String(s),
        libsql::Value::Blob(bytes) => {
            serde_json::Value::String(bytes.iter().map(|b| format!("{b:02x}")).collect())
        }
    }
}

/// Exporta um snapshot de `movimentos_consolidados` (a tabela do Turso) pra
/// um arquivo JSON local, com a mesma politica de retencao de 14 dias dos
/// backups do banco (`db::backup::limpar_backups_antigos`). Essa tabela hoje
/// so existe no Turso (free tier) e no painel publico - se a conta Turso for
/// perdida, esse dump e a unica copia offline do historico consolidado dos
/// dois armazens. Nao substitui o banco local de cada armazem (que continua
/// sendo a fonte de verdade dos proprios movimentos), e deliberadamente usa
/// `SELECT *` em vez de uma lista curada de colunas - um export generico nao
/// deve ficar desatualizado se a tabela ganhar uma coluna nova no futuro,
/// diferente das queries estruturadas acima que alimentam structs Rust
/// especificos.
pub async fn exportar_consolidado(
    url: &str,
    token: &str,
    destino: &Path,
    data: &str,
) -> AppResult<PathBuf> {
    let remoto = conectar_turso(url, token).await?;

    let mut rows = remoto
        .query(
            "SELECT * FROM movimentos_consolidados ORDER BY armazem_codigo, id_origem",
            (),
        )
        .await
        .map_err(|e| {
            AppError::Interno(format!(
                "Nao foi possivel exportar a tabela consolidada: {e}"
            ))
        })?;

    let colunas: Vec<String> = (0..rows.column_count())
        .map(|i| rows.column_name(i).unwrap_or_default().to_string())
        .collect();

    let mut linhas = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| AppError::Interno(format!("Erro lendo a tabela consolidada: {e}")))?
    {
        let mut objeto = serde_json::Map::new();
        for (indice, coluna) in colunas.iter().enumerate() {
            let valor = row
                .get_value(indice as i32)
                .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
            objeto.insert(coluna.clone(), valor_para_json(valor));
        }
        linhas.push(serde_json::Value::Object(objeto));
    }

    fs::create_dir_all(destino).map_err(|e| {
        AppError::Interno(format!("Nao foi possivel criar a pasta de backups: {e}"))
    })?;
    let caminho = destino.join(format!(
        "{PREFIXO_EXPORT_CONSOLIDADO}{data}{SUFIXO_EXPORT_CONSOLIDADO}"
    ));
    let json = serde_json::to_string_pretty(&linhas)
        .map_err(|e| AppError::Interno(format!("Nao foi possivel serializar o export: {e}")))?;
    fs::write(&caminho, json)
        .map_err(|e| AppError::Interno(format!("Nao foi possivel gravar o export: {e}")))?;

    crate::db::backup::limpar_backups_antigos(
        destino,
        PREFIXO_EXPORT_CONSOLIDADO,
        SUFIXO_EXPORT_CONSOLIDADO,
        crate::db::backup::RETENCAO_DIAS,
    )?;

    Ok(caminho)
}

/// Tenta sincronizar a fila local com o Turso uma vez: busca os pendentes,
/// envia, e grava o resultado (sucesso ou falha, incluindo uma queda de
/// conexao total - ver `conectar_turso`) de volta no banco local. Nunca
/// entra em panico nem propaga erro - so registra no log - porque quem chama
/// isto e um loop de segundo plano (`lib.rs`) que precisa continuar rodando
/// independente do resultado. Nao exige sessao/login: e infraestrutura, roda
/// pela vida inteira do processo, nao uma acao de um usuario especifico (ver
/// docs/ARQUITETURA.md).
pub async fn tentar_sincronizar_uma_vez(app_handle: &tauri::AppHandle, url: &str, token: &str) {
    use tauri::Manager;

    let state = app_handle.state::<crate::state::AppState>();

    let (pendentes, agora_local_valor) = {
        let Ok(conn) = state.conn() else {
            return;
        };
        let pendentes = match movimentos_pendentes(&conn) {
            Ok(p) => p,
            Err(e) => {
                log::warn!("Falha ao preparar sincronizacao com o Turso: {e}");
                return;
            }
        };
        let Ok(agora) = agora_local(&conn) else {
            return;
        };
        (pendentes, agora)
    };

    match enviar_para_turso(url, token, &pendentes, &agora_local_valor).await {
        Ok(resultado) => {
            if let Ok(conn) = state.conn() {
                if let Err(e) = marcar_sincronizado(&conn, &resultado.enviados) {
                    log::warn!("Falha ao marcar lancamentos como sincronizados: {e}");
                }
                if let Err(e) = marcar_falha_sincronizacao(&conn, &resultado.falhas) {
                    log::warn!("Falha ao registrar erro de sincronizacao: {e}");
                }
            }
            log::info!(
                "Sincronizacao com o Turso: {} enviados, {} com erro.",
                resultado.enviados.len(),
                resultado.falhas.len()
            );
        }
        Err(e) => log::warn!("Falha na sincronizacao com o Turso: {e}"),
    }
}

/// Um envio (`saida` de `peca_montagem`, `saida_armazem` ou `sac` com
/// `armazem_destino_codigo` preenchido) visto do lado de quem vai receber -
/// vem direto do Turso, nunca do banco local (o envio original vive no PC do
/// outro armazem). O fluxo viaja junto pra `confirmar_recebimento` gravar a
/// entrada de confirmacao no mesmo fluxo do envio original (uma transferencia
/// de veiculo confirmada em Saida de Armazem, uma de peca solta em Montagem,
/// uma peca de SAC no SAC) e pra cada tela filtrar so as pendencias do seu
/// proprio fluxo.
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
    pub numero_pedido: Option<String>,
    /// Observacao do MOVIMENTO (nao do item - ver `MovimentoItem::observacao`
    /// dentro de `itens`). Ficou faltando aqui na v2.1.1, mesma classe de bug
    /// do `numero_pedido`: um campo que existe no envio original e chega
    /// certinho no Turso, mas que a query de quem recebe esquecia de
    /// selecionar - o conferente que confirma nunca via instrucoes/detalhes
    /// importantes que o remetente escreveu ali.
    pub observacoes: Option<String>,
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
    numero_pedido: Option<String>,
    observacoes: Option<String>,
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
        numero_pedido,
        observacoes,
        itens,
    })
}

const SQL_PENDENTES_RECEBIMENTO: &str = "
    SELECT armazem_codigo, id_origem, fluxo, data, hora, armazem_destino_codigo, numero_pedido,
           observacoes, itens_json
    FROM movimentos_consolidados m
    WHERE armazem_destino_codigo = ?1
      -- Fluxos que suportam transferencia fisica entre A4 e B2: veiculos
      -- (saida_armazem), peca solta (peca_montagem) e SAC (sac) - por
      -- exemplo uma peca de garantia que precisa ser embutida numa caixa de
      -- scooter que ja vai sair por transportadora, aproveitando o mesmo
      -- frete em vez de pagar dois.
      AND fluxo IN ('peca_montagem', 'saida_armazem', 'sac')
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

/// Mesmas colunas de `SQL_PENDENTES_RECEBIMENTO` (`buscar_transferencia`
/// depende da mesma ordem posicional que `linha_para_transferencia` espera -
/// ver o teste `sql_pendentes_recebimento_e_sql_buscar_transferencia_
/// selecionam_as_mesmas_colunas`), mas buscando por chave exata em vez do
/// filtro de pendencia - usada por `confirmar_recebimento`, que ja sabe qual
/// transferencia confirmar e so precisa reler os dados de verdade (nunca
/// confiar no que o frontend mandar de volta).
const SQL_BUSCAR_TRANSFERENCIA_POR_CHAVE: &str = "
    SELECT armazem_codigo, id_origem, fluxo, data, hora, armazem_destino_codigo, numero_pedido,
           observacoes, itens_json
    FROM movimentos_consolidados WHERE armazem_codigo = ?1 AND id_origem = ?2
";

/// Busca no Turso o que foi enviado pro `meu_armazem_codigo` e ainda nao foi
/// confirmado (nem estornado do lado de quem enviou). O SQL em si (colunas,
/// filtro de pendencia) e testado diretamente contra rusqlite - ver
/// `tests::sql_pendentes_recebimento_traz_numero_pedido_do_envio` - so a
/// conexao de rede de verdade exige uma conta Turso real (ver
/// docs/ARQUITETURA.md).
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
        let numero_pedido: Option<String> = row
            .get(6)
            .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
        let observacoes: Option<String> = row
            .get(7)
            .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
        let itens_json: String = row
            .get(8)
            .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
        resultado.push(linha_para_transferencia(
            armazem_origem_codigo,
            id_origem,
            fluxo,
            data,
            hora,
            armazem_destino_codigo,
            numero_pedido,
            observacoes,
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
            SQL_BUSCAR_TRANSFERENCIA_POR_CHAVE,
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
    let numero_pedido: Option<String> = row
        .get(6)
        .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
    let observacoes: Option<String> = row
        .get(7)
        .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
    let itens_json: String = row
        .get(8)
        .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;

    Ok(Some(linha_para_transferencia(
        armazem_origem_codigo,
        id_origem,
        fluxo,
        data,
        hora,
        armazem_destino_codigo,
        numero_pedido,
        observacoes,
        itens_json,
    )?))
}

/// Uma transferencia que EU enviei e que o outro armazem recusou
/// (`domain::movimentos::recusar_recebimento`), vista do meu lado - o
/// espelho de `TransferenciaPendente`, mas pra quem enviou em vez de quem
/// recebe. `meu_movimento_id` e o id do MEU lancamento original (na minha
/// tabela `movimentos` local, nao no Turso) - a acao natural ao ver isso e
/// abrir esse lancamento na minha propria lista e usar o botao Estornar que
/// ja existe (com justificativa, ja auditado); assim que eu estornar, o
/// aviso some sozinho (ver `SQL_MINHAS_TRANSFERENCIAS_RECUSADAS`).
#[derive(Debug, Serialize)]
pub struct TransferenciaRecusada {
    pub armazem_que_recusou_codigo: String,
    pub meu_movimento_id: i64,
    pub fluxo: String,
    pub data: String,
    pub hora: String,
    pub numero_pedido: Option<String>,
    pub justificativa: Option<String>,
    pub itens: Vec<MovimentoItem>,
}

#[allow(clippy::too_many_arguments)]
fn linha_para_transferencia_recusada(
    armazem_que_recusou_codigo: String,
    meu_movimento_id: i64,
    fluxo: String,
    data: String,
    hora: String,
    numero_pedido: Option<String>,
    justificativa: Option<String>,
    itens_json: String,
) -> AppResult<TransferenciaRecusada> {
    let itens: Vec<MovimentoItem> = serde_json::from_str(&itens_json).map_err(|e| {
        AppError::Interno(format!(
            "Nao foi possivel ler os itens da transferencia recusada: {e}"
        ))
    })?;
    Ok(TransferenciaRecusada {
        armazem_que_recusou_codigo,
        meu_movimento_id,
        fluxo,
        data,
        hora,
        numero_pedido,
        justificativa,
        itens,
    })
}

/// Mesma ideia de `SQL_PENDENTES_RECEBIMENTO`, mas espelhada: em vez de "o
/// que foi enviado pra mim e ainda nao foi confirmado", aqui e "o que EU
/// enviei e que o outro lado recusou, e eu ainda nao corrigi (estornei) o
/// lancamento original". `c.recebido_de_id_origem` e o id do MEU lancamento
/// original (a chave que uso pra achar e estornar ele). O `NOT EXISTS` final
/// e o mesmo padrao ja usado em `SQL_PENDENTES_RECEBIMENTO` pra sumir um
/// envio ja estornado - sem ele, um aviso de recusa nunca sumiria mesmo
/// depois de corrigido.
const SQL_MINHAS_TRANSFERENCIAS_RECUSADAS: &str = "
    SELECT c.armazem_codigo, c.recebido_de_id_origem, c.fluxo, c.data, c.hora,
           c.numero_pedido, c.observacoes, c.itens_json
    FROM movimentos_consolidados c
    WHERE c.recebido_de_armazem_codigo = ?1
      AND c.motivo = 'recusado'
      AND NOT EXISTS (
        SELECT 1 FROM movimentos_consolidados x
        WHERE x.armazem_codigo = ?1 AND x.estornado_de = c.recebido_de_id_origem
      )
    ORDER BY c.data DESC, c.hora DESC
";

/// Busca no Turso as transferencias que EU enviei e que foram recusadas do
/// outro lado - ver `TransferenciaRecusada`. Mesmo padrao de
/// `buscar_pendentes_recebimento`: nunca falha por sincronizacao nao
/// configurada, so devolve vazio (quem chama trata isso).
pub async fn buscar_minhas_transferencias_recusadas(
    url: &str,
    token: &str,
    meu_armazem_codigo: &str,
) -> AppResult<Vec<TransferenciaRecusada>> {
    let banco = libsql::Builder::new_remote(url.to_string(), token.to_string())
        .build()
        .await
        .map_err(|e| AppError::Interno(format!("Nao foi possivel conectar ao Turso: {e}")))?;
    let remoto = banco
        .connect()
        .map_err(|e| AppError::Interno(format!("Nao foi possivel abrir a conexao remota: {e}")))?;

    let mut rows = remoto
        .query(
            SQL_MINHAS_TRANSFERENCIAS_RECUSADAS,
            libsql::params![meu_armazem_codigo],
        )
        .await
        .map_err(|e| {
            AppError::Interno(format!(
                "Nao foi possivel buscar transferencias recusadas: {e}"
            ))
        })?;

    let mut resultado = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| AppError::Interno(format!("Erro lendo transferencias recusadas: {e}")))?
    {
        let armazem_que_recusou_codigo: String = row
            .get(0)
            .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
        let meu_movimento_id: i64 = row
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
        let numero_pedido: Option<String> = row
            .get(5)
            .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
        let justificativa: Option<String> = row
            .get(6)
            .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
        let itens_json: String = row
            .get(7)
            .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
        resultado.push(linha_para_transferencia_recusada(
            armazem_que_recusou_codigo,
            meu_movimento_id,
            fluxo,
            data,
            hora,
            numero_pedido,
            justificativa,
            itens_json,
        )?);
    }

    Ok(resultado)
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
                    codigo_componente: None,
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
                    codigo_componente: None,
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
            Some("1603".into()),
            Some("Frete ja pago, so descarregar".into()),
            itens_json,
        )
        .unwrap();

        assert_eq!(transferencia.armazem_origem_codigo, "B2");
        assert_eq!(transferencia.id_origem, 42);
        assert_eq!(transferencia.fluxo, "peca_montagem");
        assert_eq!(transferencia.armazem_destino_codigo.as_deref(), Some("A4"));
        assert_eq!(transferencia.numero_pedido.as_deref(), Some("1603"));
        assert_eq!(
            transferencia.observacoes.as_deref(),
            Some("Frete ja pago, so descarregar")
        );
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
            None,
            None,
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
            None,
            None,
            "nao e json".into(),
        );
        assert!(resultado.is_err());
    }

    fn linha_pendente_de_teste(id: i64) -> LinhaPendente {
        LinhaPendente {
            movimento: Movimento {
                id,
                numero: 0,
                armazem_id: 1,
                armazem_destino_id: None,
                fluxo: "reparo_externo".into(),
                tipo: "saida".into(),
                data: "2026-08-25".into(),
                hora: "09:00".into(),
                turno: "diurno".into(),
                usuario_id: 1,
                usuario_nome: "Teste".into(),
                numero_pedido: None,
                codigo_rastreio: None,
                contraparte: None,
                quem_retirou: None,
                motivo: None,
                valor_centavos: None,
                observacoes: None,
                status: "aberto".into(),
                estornado_de: None,
                recebido_de_armazem_codigo: None,
                recebido_de_id_origem: None,
                retirada_completa: true,
                hash_integridade: "hash".into(),
                itens: Vec::new(),
            },
            armazem_codigo: "A4".into(),
            armazem_destino_codigo: None,
            itens: Vec::new(),
        }
    }

    /// Antes desta correcao, uma queda de conexao total (sem internet, Turso
    /// fora do ar, URL/token invalidos) fazia `enviar_para_turso` devolver
    /// `Err` sem registrar nada em `falhas` - `status_sincronizacao`
    /// continuava mostrando "0 com erro" mesmo com a fila parada ha dias.
    /// Agora a falha de conexao vira `falhas` pra todo o lote, igual a uma
    /// falha por linha. Timeout de guarda pra nao arriscar travar o teste
    /// esperando DNS numa rede sem internet.
    #[tokio::test]
    async fn enviar_para_turso_registra_falha_em_todos_os_pendentes_quando_a_conexao_falha() {
        let pendentes = vec![linha_pendente_de_teste(1), linha_pendente_de_teste(2)];

        let resultado = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            enviar_para_turso(
                "nao-e-uma-url-valida",
                "token-falso",
                &pendentes,
                "2026-08-25 09:00:00",
            ),
        )
        .await
        .expect("nao deveria travar esperando rede")
        .expect("uma falha de conexao deve virar Ok com falhas preenchidas, nao Err");

        assert!(resultado.enviados.is_empty());
        assert_eq!(resultado.falhas.len(), 2);
        assert_eq!(resultado.falhas[0].0, 1);
        assert_eq!(resultado.falhas[1].0, 2);
    }

    /// `SQL_CRIAR_TABELA_REMOTA`/`SQL_ALTER_TABELA_REMOTA`/`SQL_UPSERT`/
    /// `SQL_PENDENTES_RECEBIMENTO`/`SQL_BUSCAR_TRANSFERENCIA_POR_CHAVE` sao
    /// so texto SQLite generico (nada especifico de libsql - sem isso nao
    /// dariam pra reusar contra o Turso de producao nem contra este SQLite
    /// local), entao dá pra testar exatamente as mesmas strings usadas em
    /// producao contra um `rusqlite::Connection` em memoria, sem precisar de
    /// conta/rede Turso. Deliberadamente nao usa `libsql::Builder::new_local`
    /// pra isso: libsql "core" (seu proprio SQLite vendorizado) e o SQLite
    /// bundled do rusqlite (ja usado em todo o resto da suite) disputam a
    /// configuracao global de threading do SQLite no mesmo processo de
    /// teste, o que faz o libsql local panicar de forma intermitente.
    fn conexao_remota_de_teste() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(SQL_CRIAR_TABELA_REMOTA, []).unwrap();
        for alter in SQL_ALTER_TABELA_REMOTA {
            let _ = conn.execute(alter, []);
        }
        conn
    }

    fn inserir_na_tabela_remota(conn: &Connection, linha: &LinhaPendente, enviado_em: &str) {
        let itens_json = serde_json::to_string(&linha.itens).unwrap();
        conn.execute(
            SQL_UPSERT,
            rusqlite::params![
                linha.armazem_codigo,
                linha.movimento.id,
                linha.movimento.fluxo,
                linha.movimento.tipo,
                linha.movimento.data,
                linha.movimento.hora,
                linha.movimento.turno,
                linha.movimento.usuario_nome,
                linha.movimento.numero_pedido,
                linha.movimento.codigo_rastreio,
                linha.movimento.contraparte,
                linha.movimento.quem_retirou,
                linha.movimento.motivo,
                linha.movimento.valor_centavos,
                linha.movimento.observacoes,
                linha.movimento.status,
                linha.movimento.estornado_de,
                linha.movimento.hash_integridade,
                itens_json,
                linha.armazem_destino_codigo,
                linha.movimento.recebido_de_armazem_codigo,
                linha.movimento.recebido_de_id_origem,
                enviado_em,
            ],
        )
        .unwrap();
    }

    /// Roda `SQL_PENDENTES_RECEBIMENTO` de verdade e reusa
    /// `linha_para_transferencia` (a mesma funcao de mapeamento que
    /// `buscar_pendentes_recebimento` chama em producao) pra montar o
    /// resultado - so a conexao de rede em si e diferente do caminho real.
    fn buscar_pendentes_via_sql(
        conn: &Connection,
        meu_armazem_codigo: &str,
    ) -> Vec<TransferenciaPendente> {
        let mut stmt = conn.prepare(SQL_PENDENTES_RECEBIMENTO).unwrap();
        stmt.query_map(rusqlite::params![meu_armazem_codigo], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, String>(8)?,
            ))
        })
        .unwrap()
        .map(|linha| linha.unwrap())
        .map(|(a, b, c, d, e, f, g, h, i)| {
            linha_para_transferencia(a, b, c, d, e, f, g, h, i).unwrap()
        })
        .collect()
    }

    fn buscar_transferencia_via_sql(
        conn: &Connection,
        armazem_origem_codigo: &str,
        id_origem: i64,
    ) -> Option<TransferenciaPendente> {
        conn.query_row(
            SQL_BUSCAR_TRANSFERENCIA_POR_CHAVE,
            rusqlite::params![armazem_origem_codigo, id_origem],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        )
        .optional()
        .unwrap()
        .map(|(a, b, c, d, e, f, g, h, i)| {
            linha_para_transferencia(a, b, c, d, e, f, g, h, i).unwrap()
        })
    }

    fn linha_pendente_de_transferencia(
        id: i64,
        armazem_codigo: &str,
        armazem_destino_codigo: &str,
        numero_pedido: Option<&str>,
    ) -> LinhaPendente {
        let mut linha = linha_pendente_de_teste(id);
        linha.movimento.fluxo = "peca_montagem".into();
        linha.movimento.numero_pedido = numero_pedido.map(String::from);
        linha.armazem_codigo = armazem_codigo.into();
        linha.armazem_destino_codigo = Some(armazem_destino_codigo.into());
        linha.itens = vec![MovimentoItem {
            id: 0,
            categoria: "peca".into(),
            descricao: Some("CAPACETE PRETO".into()),
            montagem: None,
            condicao: Some("boa".into()),
            quantidade: 3,
            observacao: None,
            quantidade_enviada: None,
            codigo_componente: None,
        }];
        linha
    }

    /// As duas queries de leitura tem que continuar selecionando as mesmas
    /// colunas na mesma ordem - `linha_para_transferencia` espera essa ordem
    /// posicional dos dois lados. Um guarda-corpo direto contra o tipo de
    /// bug corrigido nesta versao (uma coluna que existe na tabela e no
    /// struct, mas que uma das duas queries esquece de selecionar).
    #[test]
    fn sql_pendentes_recebimento_e_sql_buscar_transferencia_selecionam_as_mesmas_colunas() {
        let conn = conexao_remota_de_teste();
        let colunas_lista: Vec<String> = conn
            .prepare(SQL_PENDENTES_RECEBIMENTO)
            .unwrap()
            .column_names()
            .into_iter()
            .map(String::from)
            .collect();
        let colunas_chave: Vec<String> = conn
            .prepare(SQL_BUSCAR_TRANSFERENCIA_POR_CHAVE)
            .unwrap()
            .column_names()
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(colunas_lista, colunas_chave);
        assert!(colunas_lista.contains(&"numero_pedido".to_string()));
    }

    /// Reproduz o bug corrigido nesta versao: `numero_pedido` era gravado no
    /// envio mas `SQL_PENDENTES_RECEBIMENTO` nao o selecionava, entao quem
    /// recebia via `buscar_transferencias_pendentes`/`confirmar_recebimento`
    /// sempre via o pedido como nulo. Testa o ciclo completo
    /// envio->consolidado->busca contra o SQL de verdade usado em producao.
    #[test]
    fn sql_pendentes_recebimento_traz_numero_pedido_do_envio() {
        let conn = conexao_remota_de_teste();
        let linha = linha_pendente_de_transferencia(1603, "B2", "A4", Some("1603"));
        inserir_na_tabela_remota(&conn, &linha, "2026-09-02 10:00:00");

        let pendentes = buscar_pendentes_via_sql(&conn, "A4");
        assert_eq!(pendentes.len(), 1);
        assert_eq!(pendentes[0].numero_pedido.as_deref(), Some("1603"));
        assert_eq!(pendentes[0].armazem_origem_codigo, "B2");
    }

    /// Mesma regressao que o teste acima, mas pelo caminho que
    /// `confirmar_recebimento` realmente usa (`buscar_transferencia`, busca
    /// por chave, nao a lista inteira).
    #[test]
    fn sql_buscar_transferencia_por_chave_traz_numero_pedido() {
        let conn = conexao_remota_de_teste();
        let linha = linha_pendente_de_transferencia(1588, "A4", "B2", Some("1588"));
        inserir_na_tabela_remota(&conn, &linha, "2026-09-02 14:28:00");

        let transferencia = buscar_transferencia_via_sql(&conn, "A4", 1588)
            .expect("a transferencia deveria ser encontrada");
        assert_eq!(transferencia.numero_pedido.as_deref(), Some("1588"));
        assert_eq!(transferencia.itens.len(), 1);
        assert_eq!(transferencia.itens[0].quantidade, 3);
    }

    /// `numero_pedido` e opcional (nem toda saida tem um pedido associado) -
    /// confere que `NULL` tambem sobrevive ao ciclo, nao so o caso feliz com
    /// valor preenchido.
    #[test]
    fn numero_pedido_ausente_continua_nulo_apos_o_ciclo() {
        let conn = conexao_remota_de_teste();
        let linha = linha_pendente_de_transferencia(2, "B2", "A4", None);
        inserir_na_tabela_remota(&conn, &linha, "2026-09-02 10:00:00");

        let pendentes = buscar_pendentes_via_sql(&conn, "A4");
        assert_eq!(pendentes.len(), 1);
        assert_eq!(pendentes[0].numero_pedido, None);
    }

    /// Mesma classe de bug do `numero_pedido` (regressao pega logo depois,
    /// na mesma sessao): `observacoes` do movimento tambem nao era
    /// selecionada por `SQL_PENDENTES_RECEBIMENTO`/`SQL_BUSCAR_TRANSFERENCIA_
    /// POR_CHAVE`, entao instrucoes/detalhes que quem enviou escreveu no
    /// campo Observacao nunca chegavam pra quem recebia decidir se confirma
    /// ou recusa.
    #[test]
    fn sql_pendentes_recebimento_traz_observacoes_do_envio() {
        let conn = conexao_remota_de_teste();
        let mut linha = linha_pendente_de_transferencia(1588, "B2", "A4", Some("1588"));
        linha.movimento.observacoes = Some("Caixa fragil, nao empilhar".into());
        inserir_na_tabela_remota(&conn, &linha, "2026-09-02 14:28:00");

        let pendentes = buscar_pendentes_via_sql(&conn, "A4");
        assert_eq!(pendentes.len(), 1);
        assert_eq!(
            pendentes[0].observacoes.as_deref(),
            Some("Caixa fragil, nao empilhar")
        );

        let transferencia = buscar_transferencia_via_sql(&conn, "B2", 1588)
            .expect("a transferencia deveria ser encontrada");
        assert_eq!(
            transferencia.observacoes.as_deref(),
            Some("Caixa fragil, nao empilhar")
        );
    }

    /// Cobertura do filtro `NOT EXISTS` de `SQL_PENDENTES_RECEBIMENTO`: uma
    /// transferencia ja confirmada (existe uma linha em
    /// `recebido_de_armazem_codigo`/`recebido_de_id_origem` apontando pra
    /// ela) nao deve aparecer de novo na lista de pendentes - sem isso, o
    /// mesmo envio reapareceria pra ser confirmado outra vez.
    #[test]
    fn transferencia_ja_confirmada_some_da_lista_de_pendentes() {
        let conn = conexao_remota_de_teste();
        let envio = linha_pendente_de_transferencia(7, "B2", "A4", Some("42"));
        inserir_na_tabela_remota(&conn, &envio, "2026-09-02 10:00:00");

        let mut confirmacao = linha_pendente_de_teste(1);
        confirmacao.armazem_codigo = "A4".into();
        confirmacao.movimento.recebido_de_armazem_codigo = Some("B2".into());
        confirmacao.movimento.recebido_de_id_origem = Some(7);
        inserir_na_tabela_remota(&conn, &confirmacao, "2026-09-02 10:05:00");

        let pendentes = buscar_pendentes_via_sql(&conn, "A4");
        assert!(pendentes.is_empty());
    }

    #[test]
    fn valor_para_json_converte_cada_variante() {
        assert_eq!(
            valor_para_json(libsql::Value::Null),
            serde_json::Value::Null
        );
        assert_eq!(
            valor_para_json(libsql::Value::Integer(42)),
            serde_json::json!(42)
        );
        assert_eq!(
            valor_para_json(libsql::Value::Real(1.5)),
            serde_json::json!(1.5)
        );
        assert_eq!(
            valor_para_json(libsql::Value::Text("ola".into())),
            serde_json::json!("ola")
        );
        assert_eq!(
            valor_para_json(libsql::Value::Blob(vec![0xde, 0xad])),
            serde_json::json!("dead")
        );
    }

    /// `exportar_consolidado` usa `SELECT *` (deliberado, ver o comentario na
    /// funcao) em vez de uma lista curada de colunas - esse teste so confere
    /// que a query roda contra o schema real e traz as colunas esperadas,
    /// sem precisar de uma conta Turso (mesmo `rusqlite` em memoria usado
    /// pelos outros testes deste arquivo).
    #[test]
    fn select_do_export_consolidado_roda_contra_o_schema_real_e_traz_as_colunas() {
        let conn = conexao_remota_de_teste();
        let linha = linha_pendente_de_teste(1);
        inserir_na_tabela_remota(&conn, &linha, "2026-09-04 08:00:00");

        let mut stmt = conn
            .prepare("SELECT * FROM movimentos_consolidados ORDER BY armazem_codigo, id_origem")
            .unwrap();
        let colunas: Vec<String> = stmt.column_names().into_iter().map(String::from).collect();
        assert!(colunas.contains(&"armazem_codigo".to_string()));
        assert!(colunas.contains(&"id_origem".to_string()));
        assert!(colunas.contains(&"hash_integridade".to_string()));

        let total = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .count();
        assert_eq!(total, 1);
    }
}
