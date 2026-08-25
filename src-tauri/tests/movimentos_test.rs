//! Teste de integracao ponta-a-ponta: abre um banco novo (com migrations e
//! seed de armazens), cria o primeiro usuario, faz login, registra um pedido
//! com varios itens (misturando categorias) e confere o fechamento da lista
//! do dia — o mesmo fluxo que uma conferente faria na tela de Lancamentos.

use app_lib::db;
use app_lib::domain::auth::{self, NovoUsuario};
use app_lib::domain::errors::AppError;
use app_lib::domain::movimentos::{self, MovimentoItemInput, NovoMovimento};

#[test]
fn fluxo_completo_de_login_e_lancamento_de_pedido_misto() {
    let mut conn = db::abrir_em_memoria().expect("banco em memoria deveria abrir");

    let armazem_a4: i64 = conn
        .query_row("SELECT id FROM armazens WHERE codigo = 'A4'", [], |r| {
            r.get(0)
        })
        .expect("armazem A4 deveria existir apos o seed automatico");

    let usuario_id = auth::criar_usuario(
        &conn,
        NovoUsuario {
            nome: "Brenda Bolina",
            login: "brenda",
            senha: "senha-forte-123",
            armazem_id: Some(armazem_a4),
            papel: "gestor",
        },
    )
    .expect("deveria criar o primeiro usuario");

    // Senha errada deve falhar sem revelar se o usuario existe.
    let login_invalido = auth::login(&conn, "brenda", "senha-errada");
    assert!(matches!(
        login_invalido,
        Err(AppError::CredenciaisInvalidas)
    ));

    let usuario = auth::login(&conn, "brenda", "senha-forte-123").expect("login deveria funcionar");
    assert_eq!(usuario.id, usuario_id);

    // Pedido real: 1 scooter montado + 2 patinetes em caixa, so com o numero
    // do pedido (o detalhe fica na outra ferramenta, como o usuario pediu).
    let novo_pedido = NovoMovimento {
        armazem_id: armazem_a4,
        armazem_destino_id: None,
        fluxo: "saida_armazem".into(),
        tipo: "saida".into(),
        data: "2026-08-24".into(),
        hora: "10:09".into(),
        turno: "diurno".into(),
        usuario_id: usuario.id,
        numero_pedido: Some("3932".into()),
        codigo_rastreio: None,
        contraparte: Some("HEP EMPREENDIMENTOS LTDA".into()),
        quem_retirou: Some("TIAGO".into()),
        motivo: None,
        valor_centavos: None,
        observacoes: None,
        recebido_de_armazem_codigo: None,
        recebido_de_id_origem: None,
        itens: vec![
            MovimentoItemInput {
                categoria: "scooter".into(),
                descricao: None,
                montagem: Some("montado".into()),
                condicao: None,
                quantidade: 1,
                observacao: None,
            },
            MovimentoItemInput {
                categoria: "patinete".into(),
                descricao: Some("SE-85".into()),
                montagem: Some("caixa".into()),
                condicao: None,
                quantidade: 2,
                observacao: None,
            },
        ],
    };

    let movimento = movimentos::criar_movimento(&mut conn, novo_pedido)
        .expect("deveria registrar o pedido com 2 categorias diferentes");
    assert_eq!(movimento.numero_pedido.as_deref(), Some("3932"));
    assert_eq!(movimento.itens.len(), 2);

    let lista =
        movimentos::listar_movimentos_do_dia(&conn, armazem_a4, "saida_armazem", "2026-08-24")
            .expect("deveria listar os lancamentos do dia");

    assert_eq!(lista.len(), 1);
    let total_do_dia: i64 = lista
        .iter()
        .flat_map(|m| &m.itens)
        .map(|i| i.quantidade)
        .sum();
    assert_eq!(total_do_dia, 3);
}
