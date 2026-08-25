use serde::Serialize;

/// Erro de dominio da aplicacao. Cada variante carrega uma mensagem em
/// portugues, segura para ser exibida diretamente na interface (nunca
/// vaza detalhes internos de SQL ou stacktraces para o frontend).
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Validation(String),

    #[error("Usuario ou senha invalidos.")]
    CredenciaisInvalidas,

    #[error("Sessao nao encontrada. Faca login novamente.")]
    NaoAutenticado,

    #[error("Erro interno de banco de dados.")]
    Database(#[from] rusqlite::Error),

    #[error("Erro interno ao processar a senha.")]
    Hashing,

    #[error("Nao foi possivel preparar o banco de dados local.")]
    Migration(String),

    #[error("Erro interno da aplicacao. Tente novamente.")]
    Interno(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
