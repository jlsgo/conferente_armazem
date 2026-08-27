//! Apaga TODAS as linhas da tabela remota `movimentos_consolidados` no Turso
//! configurado nesta maquina (turso.txt na pasta de dados real). So dados de
//! teste devem estar la - nunca rode isso contra um Turso com dados reais de
//! producao. Roda so manualmente:
//!
//!   cd src-tauri
//!   cargo run --example limpar_turso

use std::path::PathBuf;

use app_lib::db::sync;

#[tokio::main]
async fn main() {
    let diretorio_dados_real = {
        let home = std::env::var("HOME").expect("defina HOME");
        PathBuf::from(home).join(".local/share/com.ecoviva.controlearmazem")
    };
    let (url, token) = sync::ler_config_turso(&diretorio_dados_real)
        .expect("turso.txt nao encontrado/incompleto na pasta de dados real");

    let banco = libsql::Builder::new_remote(url, token)
        .build()
        .await
        .expect("nao foi possivel conectar ao Turso");
    let remoto = banco
        .connect()
        .expect("nao foi possivel abrir a conexao remota");

    let antes: i64 = remoto
        .query("SELECT COUNT(*) FROM movimentos_consolidados", ())
        .await
        .expect("erro ao contar linhas (tabela existe?)")
        .next()
        .await
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap();
    println!("Linhas antes: {antes}");

    remoto
        .execute("DELETE FROM movimentos_consolidados", ())
        .await
        .expect("erro ao apagar linhas");

    let depois: i64 = remoto
        .query("SELECT COUNT(*) FROM movimentos_consolidados", ())
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap();
    println!("Linhas depois: {depois}");
    assert_eq!(depois, 0);
    println!("Turso (movimentos_consolidados) limpo.");
}
