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
        .query(
            "SELECT armazem_codigo, nome, pontos FROM cobrinha_recordes \
             ORDER BY pontos DESC, criado_em ASC LIMIT ?1",
            libsql::params![LIMITE_RECORDES],
        )
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
