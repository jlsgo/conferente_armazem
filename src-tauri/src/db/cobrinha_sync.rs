use serde::Serialize;

use crate::domain::errors::{AppError, AppResult};

/// Uma linha do placar da cobrinha (easter egg, ver
/// `src/components/CobrinhaSecreta.tsx`) - existe so no Turso, sem tabela
/// local/migration: ao contrario de `movimentos`, nao ha necessidade de
/// funcionar offline pra isso, e o unico jeito do placar ser "A4 contra B2"
/// de verdade e os dois armazens lerem/escreverem no mesmo lugar direto.
#[derive(Debug, Serialize)]
pub struct RecordeCobrinha {
    pub armazem_codigo: String,
    pub nome: String,
    pub pontos: i64,
}

const SQL_CRIAR_TABELA_REMOTA: &str = "
    CREATE TABLE IF NOT EXISTS cobrinha_recordes (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        armazem_codigo TEXT NOT NULL,
        nome TEXT NOT NULL,
        pontos INTEGER NOT NULL,
        criado_em TEXT NOT NULL DEFAULT (datetime('now'))
    )
";

const LIMITE_RECORDES: i64 = 10;

// Placar mensal (v4.0.0): filtra pelo mes corrente em vez de all-time, pra
// manter a competicao renovada em vez de um top-10 historico cada vez mais
// dificil de bater. NUNCA deleta nada - todo recorde de todo mes fica gravado
// pra sempre (principio de integridade), so a listagem exibida e que passa a
// ser so do mes atual. Nomeada (em vez de inline em `listar_com_conexao`)
// pra poder testar a mesma string usada em producao direto contra um
// `rusqlite::Connection` local - mesmo motivo/padrao de
// `SQL_PENDENTES_RECEBIMENTO` em `db/sync.rs` (texto SQLite generico, sem
// nada especifico de libsql).
const SQL_LISTAR_TOP: &str = "
    SELECT armazem_codigo, nome, pontos FROM cobrinha_recordes
    WHERE strftime('%Y-%m', criado_em) = strftime('%Y-%m', 'now')
    ORDER BY pontos DESC, criado_em ASC LIMIT ?1
";

async fn conectar(url: &str, token: &str) -> AppResult<libsql::Connection> {
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

/// Grava um recorde e devolve o top `LIMITE_RECORDES` atualizado (evita uma
/// segunda ida e volta so pra recarregar a lista depois de salvar).
pub async fn registrar(
    url: &str,
    token: &str,
    armazem_codigo: &str,
    nome: &str,
    pontos: i64,
) -> AppResult<Vec<RecordeCobrinha>> {
    let remoto = conectar(url, token).await?;
    remoto
        .execute(
            "INSERT INTO cobrinha_recordes (armazem_codigo, nome, pontos) VALUES (?1, ?2, ?3)",
            libsql::params![armazem_codigo, nome, pontos],
        )
        .await
        .map_err(|e| AppError::Interno(format!("Nao foi possivel salvar o recorde: {e}")))?;
    listar_com_conexao(&remoto).await
}

pub async fn listar_top(url: &str, token: &str) -> AppResult<Vec<RecordeCobrinha>> {
    let remoto = conectar(url, token).await?;
    listar_com_conexao(&remoto).await
}

async fn listar_com_conexao(remoto: &libsql::Connection) -> AppResult<Vec<RecordeCobrinha>> {
    let mut rows = remoto
        .query(SQL_LISTAR_TOP, libsql::params![LIMITE_RECORDES])
        .await
        .map_err(|e| AppError::Interno(format!("Nao foi possivel buscar os recordes: {e}")))?;

    let mut resultado = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| AppError::Interno(format!("Erro lendo os recordes: {e}")))?
    {
        let armazem_codigo: String = row
            .get(0)
            .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
        let nome: String = row
            .get(1)
            .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
        let pontos: i64 = row
            .get(2)
            .map_err(|e| AppError::Interno(format!("Coluna invalida: {e}")))?;
        resultado.push(RecordeCobrinha {
            armazem_codigo,
            nome,
            pontos,
        });
    }
    Ok(resultado)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    // `SQL_LISTAR_TOP`/`SQL_CRIAR_TABELA_REMOTA` sao SQLite generico (sem
    // nada especifico de libsql), entao testam-se direto contra um
    // `rusqlite::Connection` em memoria - deliberadamente NAO
    // `libsql::Builder::new_local`, que panica de forma intermitente nesta
    // suite por disputar a configuracao global de threading do SQLite com o
    // SQLite bundled do rusqlite no mesmo processo (mesmo motivo documentado
    // em `db::sync::tests::conexao_remota_de_teste`).
    fn conexao_local_de_teste() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(SQL_CRIAR_TABELA_REMOTA, []).unwrap();
        conn
    }

    fn listar_top_via_sql(conn: &Connection) -> Vec<(String, String, i64)> {
        let mut stmt = conn.prepare(SQL_LISTAR_TOP).unwrap();
        stmt.query_map(rusqlite::params![LIMITE_RECORDES], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
    }

    #[test]
    fn placar_mensal_ignora_recorde_de_mes_anterior() {
        let conn = conexao_local_de_teste();
        conn.execute(
            "INSERT INTO cobrinha_recordes (armazem_codigo, nome, pontos, criado_em) \
             VALUES ('A4', 'ANTG', 9999, datetime('now', '-2 months'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO cobrinha_recordes (armazem_codigo, nome, pontos, criado_em) \
             VALUES ('B2', 'ATUA', 50, datetime('now'))",
            [],
        )
        .unwrap();

        let top = listar_top_via_sql(&conn);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].1, "ATUA");
    }

    #[test]
    fn placar_mensal_ordena_por_pontos_dentro_do_mes_atual() {
        let conn = conexao_local_de_teste();
        conn.execute(
            "INSERT INTO cobrinha_recordes (armazem_codigo, nome, pontos, criado_em) \
             VALUES ('A4', 'BAIX', 10, datetime('now'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO cobrinha_recordes (armazem_codigo, nome, pontos, criado_em) \
             VALUES ('B2', 'ALTO', 90, datetime('now'))",
            [],
        )
        .unwrap();

        let top = listar_top_via_sql(&conn);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].1, "ALTO");
        assert_eq!(top[1].1, "BAIX");
    }
}
