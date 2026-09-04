//! Teste de ponta a ponta contra um bucket S3 de verdade - o unico jeito de
//! validar a autenticacao/upload de verdade (`aws_sdk_s3::Client`,
//! `PutObject`), que os testes locais de `db::backup_nuvem` (parsing de
//! `backup_nuvem.txt`, arquivo ausente) deliberadamente nao cobrem. `#[ignore]`
//! por padrao, entao `cargo test` normal (o que a CI roda) nunca toca
//! rede/AWS - so roda quando pedido explicitamente:
//!
//!   AWS_TESTE_ACCESS_KEY_ID=... AWS_TESTE_SECRET_ACCESS_KEY=... \
//!   AWS_TESTE_BUCKET=... AWS_TESTE_REGIAO=... \
//!     cargo test --test backup_nuvem_real_test -- --ignored
//!
//! Use um bucket S3 descartavel de teste, NUNCA o bucket de producao apontado
//! em `backup_nuvem.txt` - o teste faz um `PutObject` de verdade (sob o
//! prefixo `teste-ecoviva/`, pra nao colidir com backups reais de A4/B2). O
//! teste e seguro de rodar varias vezes (so confere que o objeto existe
//! depois do upload, sem limpar - o app em si tambem nunca apaga nada do
//! bucket, de proposito, ver docs/ARQUITETURA.md).

use app_lib::db::backup_nuvem::{enviar_arquivo, ConfigNuvem};

fn credenciais_de_teste() -> Option<ConfigNuvem> {
    Some(ConfigNuvem {
        access_key_id: std::env::var("AWS_TESTE_ACCESS_KEY_ID").ok()?,
        secret_access_key: std::env::var("AWS_TESTE_SECRET_ACCESS_KEY").ok()?,
        bucket: std::env::var("AWS_TESTE_BUCKET").ok()?,
        regiao: std::env::var("AWS_TESTE_REGIAO").ok()?,
        prefixo: "teste-ecoviva".into(),
    })
}

fn cliente_de_teste(config: &ConfigNuvem) -> aws_sdk_s3::Client {
    let credenciais = aws_sdk_s3::config::Credentials::new(
        &config.access_key_id,
        &config.secret_access_key,
        None,
        None,
        "teste-ecoviva",
    );
    let conf = aws_sdk_s3::Config::builder()
        .region(aws_sdk_s3::config::Region::new(config.regiao.clone()))
        .credentials_provider(credenciais)
        .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
        .build();
    aws_sdk_s3::Client::from_conf(conf)
}

fn diretorio_de_teste() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ecoviva-teste-s3-real-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
#[ignore = "precisa de AWS_TESTE_ACCESS_KEY_ID/AWS_TESTE_SECRET_ACCESS_KEY/AWS_TESTE_BUCKET/AWS_TESTE_REGIAO - ver o modulo doc"]
async fn envia_arquivo_e_confirma_no_bucket_de_teste_contra_s3_real() {
    let config = credenciais_de_teste().expect("faltam as env vars de teste - ver o modulo doc");

    let dir = diretorio_de_teste();
    let caminho = dir.join("ecoviva-teste-backup-nuvem.txt");
    std::fs::write(
        &caminho,
        b"teste de upload real do backup offsite - seguro de rodar varias vezes",
    )
    .unwrap();

    let client = cliente_de_teste(&config);
    enviar_arquivo(&client, &config, &caminho).await.unwrap();

    let chave = format!("{}/ecoviva-teste-backup-nuvem.txt", config.prefixo);
    client
        .head_object()
        .bucket(&config.bucket)
        .key(&chave)
        .send()
        .await
        .expect("objeto deveria existir no bucket depois do upload");

    std::fs::remove_dir_all(&dir).ok();
}
