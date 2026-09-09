//! Teste de ponta a ponta do placar unificado da cobrinha (easter egg, ver
//! `db::cobrinha_sync`) contra um Turso de verdade - mesmo motivo/padrao de
//! `sync_turso_real_test.rs`: a camada de rede (`Builder::new_remote`, HTTP,
//! credenciais) so da pra validar contra uma conta real. `#[ignore]` por
//! padrao, `cargo test` normal nunca toca rede:
//!
//!   TURSO_TESTE_URL=libsql://... TURSO_TESTE_TOKEN=... \
//!     cargo test --test cobrinha_turso_real_test -- --ignored
//!
//! Mesmo banco Turso descartavel de `sync_turso_real_test.rs`
//! (`ecoviva-armazem-teste`), NUNCA o de producao.

use app_lib::db::cobrinha_sync::{listar_top, registrar};

fn credenciais_de_teste() -> Option<(String, String)> {
    let url = std::env::var("TURSO_TESTE_URL").ok()?;
    let token = std::env::var("TURSO_TESTE_TOKEN").ok()?;
    Some((url, token))
}

/// Registra um recorde bem alto (pra garantir que aparece no topo do
/// `LIMITE_RECORDES`, mesmo que o banco de teste ja tenha lixo de execucoes
/// anteriores) pra cada armazem, e confere que os dois aparecem na lista
/// unificada devolvida por `listar_top` - o comportamento que motiva a
/// feature existir (A4 e B2 no mesmo placar).
#[tokio::test]
#[ignore = "precisa de TURSO_TESTE_URL/TURSO_TESTE_TOKEN - ver o modulo doc"]
async fn recordes_de_a4_e_b2_aparecem_juntos_no_placar_unificado() {
    let Some((url, token)) = credenciais_de_teste() else {
        panic!(
            "defina TURSO_TESTE_URL e TURSO_TESTE_TOKEN (banco Turso descartavel, \
             nunca o de producao) - ver o comentario no topo deste arquivo"
        );
    };

    let lista = registrar(&url, &token, "A4", "JHON", 9001)
        .await
        .expect("registrar (A4) nao deveria falhar com credenciais validas");
    assert!(lista
        .iter()
        .any(|r| r.armazem_codigo == "A4" && r.nome == "JHON" && r.pontos == 9001));

    let lista = registrar(&url, &token, "B2", "ALIC", 9002)
        .await
        .expect("registrar (B2) nao deveria falhar com credenciais validas");
    assert!(lista
        .iter()
        .any(|r| r.armazem_codigo == "B2" && r.nome == "ALIC" && r.pontos == 9002));

    let top = listar_top(&url, &token)
        .await
        .expect("listar_top nao deveria falhar");
    assert!(top
        .iter()
        .any(|r| r.armazem_codigo == "A4" && r.nome == "JHON"));
    assert!(top
        .iter()
        .any(|r| r.armazem_codigo == "B2" && r.nome == "ALIC"));
    // 9002 > 9001, entao B2 deveria vir antes de A4 na ordenacao por pontos.
    let pos_a4 = top.iter().position(|r| r.nome == "JHON").unwrap();
    let pos_b2 = top.iter().position(|r| r.nome == "ALIC").unwrap();
    assert!(
        pos_b2 < pos_a4,
        "maior pontuacao deveria vir primeiro no placar"
    );
}
