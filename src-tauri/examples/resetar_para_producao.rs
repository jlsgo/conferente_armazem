//! Apaga TODOS os lancamentos/itens/fechamentos (de qualquer data, nao so
//! hoje) do banco local real desta maquina, deixando usuarios e armazens
//! intactos - o "zerar tudo antes de comecar a usar em producao de verdade"
//! pre-v1.0. NAO mexe no Turso (ver `limpar_turso`, rodar separado). So roda
//! manualmente, com o app fechado (senao dois processos disputam o mesmo
//! arquivo):
//!
//!   cd src-tauri
//!   cargo run --example resetar_para_producao

use std::path::PathBuf;

use rusqlite::Connection;

fn diretorio_dados() -> PathBuf {
    if let Some(caminho) = std::env::args().nth(1) {
        return PathBuf::from(caminho);
    }
    let home = std::env::var("HOME").expect("defina HOME ou passe o diretorio como argumento");
    PathBuf::from(home).join(".local/share/com.ecoviva.controlearmazem")
}

fn contar(conn: &Connection, tabela: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {tabela}"), [], |r| r.get(0))
        .unwrap()
}

fn main() {
    let dir = diretorio_dados();
    let mut conn = app_lib::db::abrir(&dir).unwrap();

    println!(
        "Antes: {} movimentos, {} itens, {} fechamentos",
        contar(&conn, "movimentos"),
        contar(&conn, "movimento_itens"),
        contar(&conn, "fechamentos")
    );

    let tx = conn.transaction().unwrap();
    // Estornos primeiro (referenciam o original via estornado_de - com
    // foreign_keys=ON, apagar o original antes quebraria a FK).
    tx.execute("DELETE FROM movimentos WHERE estornado_de IS NOT NULL", [])
        .unwrap();
    tx.execute("DELETE FROM movimentos", []).unwrap(); // movimento_itens cai junto via ON DELETE CASCADE
    tx.execute("DELETE FROM fechamentos", []).unwrap();
    tx.commit().unwrap();

    println!(
        "Depois: {} movimentos, {} itens, {} fechamentos",
        contar(&conn, "movimentos"),
        contar(&conn, "movimento_itens"),
        contar(&conn, "fechamentos")
    );
    println!("usuarios e armazens preservados (nao tocados).");
}
