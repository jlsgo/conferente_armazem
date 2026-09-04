use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::errors::{AppError, AppResult};

const NOME_ARQUIVO_CONFIG_NUVEM: &str = "backup_nuvem.txt";

/// Credenciais e destino do backup offsite (AWS S3), lidas de
/// `backup_nuvem.txt` na pasta de dados - mesmo padrao de arquivo-por-linha
/// de `turso.txt`/`backup_externo.txt` (ver `db::sync::ler_config_turso`).
/// `prefixo` existe pra nao colidir os uploads dos dois PCs (A4/B2) no mesmo
/// bucket - cada maquina configura um prefixo proprio (ex.: "A4", "B2").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigNuvem {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub bucket: String,
    pub regiao: String,
    pub prefixo: String,
}

/// Le `backup_nuvem.txt` (5 linhas: access key id, secret access key,
/// bucket, regiao, prefixo). `None` se o arquivo nao existir ou estiver
/// incompleto - o upload offsite e sempre melhor-esforco, o app funciona
/// 100% offline sem isso configurado (ver docs/ARQUITETURA.md pro passo a
/// passo de criar o bucket/IAM user na AWS).
pub fn ler_config_nuvem(diretorio_dados: &Path) -> Option<ConfigNuvem> {
    let conteudo = fs::read_to_string(diretorio_dados.join(NOME_ARQUIVO_CONFIG_NUVEM)).ok()?;
    let mut linhas = conteudo.lines().map(str::trim).filter(|l| !l.is_empty());
    Some(ConfigNuvem {
        access_key_id: linhas.next()?.to_string(),
        secret_access_key: linhas.next()?.to_string(),
        bucket: linhas.next()?.to_string(),
        regiao: linhas.next()?.to_string(),
        prefixo: linhas.next()?.to_string(),
    })
}

fn cliente_s3(config: &ConfigNuvem) -> aws_sdk_s3::Client {
    let credenciais = aws_sdk_s3::config::Credentials::new(
        &config.access_key_id,
        &config.secret_access_key,
        None,
        None,
        "ecoviva-backup-nuvem",
    );
    let conf = aws_sdk_s3::Config::builder()
        .region(aws_sdk_s3::config::Region::new(config.regiao.clone()))
        .credentials_provider(credenciais)
        .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
        .build();
    aws_sdk_s3::Client::from_conf(conf)
}

/// Envia um unico arquivo pro bucket configurado, sob
/// `{prefixo}/{nome do arquivo}`. Deliberadamente so faz `PutObject` - o app
/// nunca chama `DeleteObject` no S3, de proposito: a credencial gravada em
/// `backup_nuvem.txt` deve ter uma IAM policy que so permite `s3:PutObject`
/// nesse bucket (ver docs/ARQUITETURA.md), pra que nem um bug local nem uma
/// maquina comprometida consigam apagar a copia offsite.
pub async fn enviar_arquivo(
    client: &aws_sdk_s3::Client,
    config: &ConfigNuvem,
    caminho: &Path,
) -> AppResult<()> {
    let nome_arquivo = caminho
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| AppError::Interno("Caminho de backup sem nome de arquivo valido".into()))?;

    let corpo = aws_sdk_s3::primitives::ByteStream::from_path(caminho)
        .await
        .map_err(|e| AppError::Interno(format!("Nao foi possivel ler '{nome_arquivo}': {e}")))?;

    client
        .put_object()
        .bucket(&config.bucket)
        .key(format!("{}/{}", config.prefixo, nome_arquivo))
        .body(corpo)
        .send()
        .await
        .map_err(|e| {
            AppError::Interno(format!(
                "Falha no upload de '{nome_arquivo}' para o S3: {e}"
            ))
        })?;

    Ok(())
}

/// Envia o conjunto de arquivos do backup do dia pro S3, melhor-esforco por
/// arquivo: uma falha (ex.: um arquivo que nao existe porque o Turso nao
/// esta configurado nesta maquina) so e logada e nao impede os demais.
/// Chamado uma vez por abertura real do app (mesma cadencia do backup
/// local/externo), nunca num loop de retry - granularidade diaria ja e
/// suficiente pra um backup.
pub async fn enviar_backups_do_dia(config: &ConfigNuvem, arquivos: &[PathBuf]) {
    let client = cliente_s3(config);
    for caminho in arquivos {
        if !caminho.exists() {
            continue;
        }
        match enviar_arquivo(&client, config, caminho).await {
            Ok(()) => log::info!("Backup offsite: '{}' enviado ao S3.", caminho.display()),
            Err(e) => log::warn!(
                "Backup offsite: falha ao enviar '{}': {e}",
                caminho.display()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn ler_config_nuvem_retorna_none_quando_arquivo_nao_existe() {
        let dir = diretorio_de_teste("sem-config-nuvem");
        assert!(ler_config_nuvem(&dir).is_none());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ler_config_nuvem_retorna_none_quando_falta_alguma_linha() {
        let dir = diretorio_de_teste("config-nuvem-incompleto");
        fs::write(
            dir.join(NOME_ARQUIVO_CONFIG_NUVEM),
            "AKIA123\nsegredo\nmeu-bucket\n",
        )
        .unwrap();
        assert!(ler_config_nuvem(&dir).is_none());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ler_config_nuvem_le_as_5_linhas_configuradas() {
        let dir = diretorio_de_teste("config-nuvem-valido");
        fs::write(
            dir.join(NOME_ARQUIVO_CONFIG_NUVEM),
            "AKIA123\nsegredo\nmeu-bucket\nsa-east-1\nA4\n",
        )
        .unwrap();
        assert_eq!(
            ler_config_nuvem(&dir),
            Some(ConfigNuvem {
                access_key_id: "AKIA123".into(),
                secret_access_key: "segredo".into(),
                bucket: "meu-bucket".into(),
                regiao: "sa-east-1".into(),
                prefixo: "A4".into(),
            })
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn enviar_backups_do_dia_ignora_arquivos_que_nao_existem() {
        // Sem credenciais reais, o upload em si falharia - mas um arquivo
        // ausente deve ser pulado antes disso, entao esta chamada nao deve
        // sequer tentar conectar no S3 nem entrar em panico.
        let config = ConfigNuvem {
            access_key_id: "x".into(),
            secret_access_key: "x".into(),
            bucket: "x".into(),
            regiao: "us-east-1".into(),
            prefixo: "x".into(),
        };
        enviar_backups_do_dia(&config, &[PathBuf::from("/caminho/que/nao/existe.db")]).await;
    }
}
