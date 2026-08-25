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

#[cfg(test)]
mod tests {
    use super::*;

    /// `AppError::Database` nunca deve deixar o texto interno do erro do
    /// SQLite (nomes de coluna/tabela, caminho do arquivo etc.) vazar para o
    /// frontend - a mensagem exibida precisa ser sempre a generica fixa,
    /// nao importa o que o driver reportou.
    #[test]
    fn erro_de_banco_nunca_vaza_detalhe_interno_do_sql() {
        let erro_interno = rusqlite::Error::InvalidColumnName(
            "senha_hash contem segredo interno da tabela usuarios".into(),
        );
        let erro_app = AppError::Database(erro_interno);
        let mensagem = erro_app.to_string();

        assert_eq!(mensagem, "Erro interno de banco de dados.");
        assert!(!mensagem.contains("senha_hash"));
        assert!(!mensagem.contains("usuarios"));
    }

    #[test]
    fn validation_preserva_a_mensagem_informada() {
        let erro = AppError::Validation("Informe o nome completo.".into());
        assert_eq!(erro.to_string(), "Informe o nome completo.");
    }
}
