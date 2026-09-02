//! Teste de ponta a ponta contra um Turso de verdade - o unico jeito de
//! validar a camada de rede/autenticacao do `db::sync` (`Builder::new_remote`,
//! HTTP, credenciais), que os testes locais (`db::sync::tests`, contra um
//! `rusqlite` em memoria) deliberadamente nao cobrem. `#[ignore]` por padrao,
//! entao `cargo test` normal (o que a CI roda) nunca toca rede - so roda
//! quando pedido explicitamente:
//!
//!   TURSO_TESTE_URL=libsql://... TURSO_TESTE_TOKEN=... \
//!     cargo test --test sync_turso_real_test -- --ignored
//!
//! Use um banco Turso descartavel, NUNCA o de producao (`ecoviva-armazem`) -
//! criar um novo e rapido:
//!
//!   turso db create ecoviva-armazem-teste
//!   turso db show ecoviva-armazem-teste   # pega a URL
//!   turso db tokens create ecoviva-armazem-teste
//!
//! O teste e seguro de rodar varias vezes (SQL_UPSERT e idempotente por
//! `(armazem_codigo, id_origem)`) e nao precisa de limpeza entre execucoes.

use app_lib::db::sync::{
    buscar_pendentes_recebimento, buscar_transferencia, enviar_para_turso, LinhaPendente,
};
use app_lib::domain::movimentos::{Movimento, MovimentoItem};

fn credenciais_de_teste() -> Option<(String, String)> {
    let url = std::env::var("TURSO_TESTE_URL").ok()?;
    let token = std::env::var("TURSO_TESTE_TOKEN").ok()?;
    Some((url, token))
}

fn linha_pendente_de_teste() -> LinhaPendente {
    LinhaPendente {
        movimento: Movimento {
            id: 999_001,
            numero: 0,
            armazem_id: 1,
            armazem_destino_id: None,
            fluxo: "peca_montagem".into(),
            tipo: "saida".into(),
            data: "2026-09-02".into(),
            hora: "10:00".into(),
            turno: "diurno".into(),
            usuario_id: 1,
            usuario_nome: "Teste E2E".into(),
            numero_pedido: Some("PEDIDO-E2E-1".into()),
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
            hash_integridade: "hash-de-teste".into(),
            itens: Vec::new(),
        },
        armazem_codigo: "B2".into(),
        armazem_destino_codigo: Some("A4".into()),
        itens: vec![MovimentoItem {
            id: 0,
            categoria: "peca".into(),
            descricao: Some("CAPACETE PRETO (teste e2e)".into()),
            montagem: None,
            condicao: Some("boa".into()),
            quantidade: 3,
            observacao: None,
            quantidade_enviada: None,
            codigo_componente: None,
        }],
    }
}

/// Envia uma transferencia fabricada, busca ela de volta pela lista de
/// pendentes e pela chave exata, e confere que tudo (numero_pedido incluido -
/// o campo que motivou este teste existir) sobrevive ao ciclo completo contra
/// um Turso de verdade.
#[tokio::test]
#[ignore = "precisa de TURSO_TESTE_URL/TURSO_TESTE_TOKEN - ver o modulo doc"]
async fn numero_pedido_e_itens_sobrevivem_ao_ciclo_completo_contra_turso_real() {
    let Some((url, token)) = credenciais_de_teste() else {
        panic!(
            "defina TURSO_TESTE_URL e TURSO_TESTE_TOKEN (banco Turso descartavel, \
             nunca o de producao) - ver o comentario no topo deste arquivo"
        );
    };

    let linha = linha_pendente_de_teste();
    let resultado = enviar_para_turso(&url, &token, &[linha], "2026-09-02 10:00:00")
        .await
        .expect("enviar_para_turso nao deveria falhar com credenciais validas");
    assert_eq!(resultado.enviados, vec![999_001]);
    assert!(resultado.falhas.is_empty());

    let pendentes = buscar_pendentes_recebimento(&url, &token, "A4")
        .await
        .expect("buscar_pendentes_recebimento nao deveria falhar");
    let transferencia = pendentes
        .iter()
        .find(|t| t.armazem_origem_codigo == "B2" && t.id_origem == 999_001)
        .expect("a transferencia de teste deveria aparecer na lista de pendentes");
    assert_eq!(transferencia.numero_pedido.as_deref(), Some("PEDIDO-E2E-1"));
    assert_eq!(transferencia.itens.len(), 1);
    assert_eq!(transferencia.itens[0].quantidade, 3);

    let por_chave = buscar_transferencia(&url, &token, "B2", 999_001)
        .await
        .expect("buscar_transferencia nao deveria falhar")
        .expect("a transferencia deveria ser encontrada pela chave");
    assert_eq!(por_chave.numero_pedido.as_deref(), Some("PEDIDO-E2E-1"));
}
