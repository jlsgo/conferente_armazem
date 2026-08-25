//! Popula o banco usado por `npm run dev` com dados de exemplo (varios dias,
//! clientes, categorias, os tres fluxos, um fechamento e um estorno) so pra
//! inspecionar a UI com dados realistas. Nao roda como parte do app nem dos
//! testes - e uma ferramenta de desenvolvimento, chamada manualmente:
//!
//!   cd src-tauri
//!   cargo run --example seed_dev_data
//!
//! Sem argumento usa o diretorio de dados padrao do Linux
//! (~/.local/share/com.ecoviva.controlearmazem, o mesmo que `npm run dev` usa).
//! Passe um caminho como argumento pra usar outro diretorio (Windows/macOS).
//! E seguro rodar mais de uma vez: usuarios repetidos sao ignorados.

use std::path::PathBuf;

use rusqlite::Connection;

use app_lib::db;
use app_lib::domain::auth::{self, NovoUsuario};
use app_lib::domain::fechamentos;
use app_lib::domain::movimentos::{self, MovimentoItemInput, NovoMovimento};

fn diretorio_dados() -> PathBuf {
    if let Some(caminho) = std::env::args().nth(1) {
        return PathBuf::from(caminho);
    }
    let home = std::env::var("HOME").expect("defina HOME ou passe o diretorio como argumento");
    PathBuf::from(home).join(".local/share/com.ecoviva.controlearmazem")
}

fn garantir_usuario(
    conn: &Connection,
    nome: &str,
    login: &str,
    armazem_id: Option<i64>,
    papel: &str,
) -> i64 {
    match auth::criar_usuario(
        conn,
        NovoUsuario {
            nome,
            login,
            senha: "senha123",
            armazem_id,
            papel,
        },
    ) {
        Ok(id) => id,
        Err(_) => conn
            .query_row("SELECT id FROM usuarios WHERE login = ?1", [login], |r| {
                r.get(0)
            })
            .expect("usuario deveria existir se a criacao falhou"),
    }
}

fn item(
    categoria: &str,
    descricao: Option<&str>,
    montagem: Option<&str>,
    condicao: Option<&str>,
    quantidade: i64,
    observacao: Option<&str>,
) -> MovimentoItemInput {
    MovimentoItemInput {
        categoria: categoria.into(),
        descricao: descricao.map(String::from),
        montagem: montagem.map(String::from),
        condicao: condicao.map(String::from),
        quantidade,
        observacao: observacao.map(String::from),
        quantidade_enviada: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn registrar(
    conn: &mut Connection,
    armazem_id: i64,
    fluxo: &str,
    tipo: &str,
    data: &str,
    hora: &str,
    usuario_id: i64,
    numero_pedido: Option<&str>,
    codigo_rastreio: Option<&str>,
    contraparte: Option<&str>,
    quem_retirou: Option<&str>,
    motivo: Option<&str>,
    valor_centavos: Option<i64>,
    observacoes: Option<&str>,
    itens: Vec<MovimentoItemInput>,
) -> Option<i64> {
    match movimentos::criar_movimento(
        conn,
        NovoMovimento {
            armazem_id,
            armazem_destino_id: None,
            fluxo: fluxo.into(),
            tipo: tipo.into(),
            data: data.into(),
            hora: hora.into(),
            turno: "diurno".into(),
            usuario_id,
            numero_pedido: numero_pedido.map(String::from),
            codigo_rastreio: codigo_rastreio.map(String::from),
            contraparte: contraparte.map(String::from),
            quem_retirou: quem_retirou.map(String::from),
            motivo: motivo.map(String::from),
            valor_centavos,
            observacoes: observacoes.map(String::from),
            recebido_de_armazem_codigo: None,
            recebido_de_id_origem: None,
            itens,
        },
    ) {
        Ok(movimento) => Some(movimento.id),
        Err(e) => {
            eprintln!("aviso: pulei {numero_pedido:?} em {data} ({fluxo}): {e}");
            None
        }
    }
}

fn main() {
    let dir = diretorio_dados();
    println!("Usando diretorio de dados: {}", dir.display());

    let mut conn = db::abrir(&dir).expect("nao foi possivel abrir o banco");

    let armazem_a4: i64 = conn
        .query_row("SELECT id FROM armazens WHERE codigo = 'A4'", [], |r| {
            r.get(0)
        })
        .unwrap();
    let armazem_b2: i64 = conn
        .query_row("SELECT id FROM armazens WHERE codigo = 'B2'", [], |r| {
            r.get(0)
        })
        .unwrap();

    // Gestor "central" (sem armazem fixo) para poder fechar/estornar nos dois.
    let gestor = garantir_usuario(&conn, "Brenda Bolina", "brenda", None, "gestor");
    let karol = garantir_usuario(
        &conn,
        "Karol Pereira",
        "karol",
        Some(armazem_a4),
        "conferente",
    );
    let geson = garantir_usuario(
        &conn,
        "Geson Silva",
        "geson",
        Some(armazem_b2),
        "conferente",
    );

    // ---- Saida de Armazem (A4) - varios dias, clientes e situacoes ----
    // (2026-08-24 fica de fora de proposito - ja pode estar fechado por dados
    // reais de uso manual do app; `registrar` so avisa e pula se colidir.)
    let dias = [
        "2026-08-17",
        "2026-08-18",
        "2026-08-19",
        "2026-08-21",
        "2026-08-22",
    ];

    registrar(
        &mut conn,
        armazem_a4,
        "saida_armazem",
        "saida",
        dias[0],
        "08:15",
        karol,
        Some("3893"),
        Some("BR123456789BR"),
        Some("DISK&TENHA LOGISTICA"),
        Some("MARCELO"),
        None,
        None,
        None,
        vec![
            item(
                "scooter",
                Some("HE-15 GREEN"),
                Some("montado"),
                None,
                2,
                None,
            ),
            item(
                "patinete",
                Some("SE-85 BLACK"),
                Some("caixa"),
                None,
                3,
                Some("cliente pediu embalagem reforcada"),
            ),
        ],
    );
    registrar(
        &mut conn,
        armazem_a4,
        "saida_armazem",
        "saida",
        dias[0],
        "10:40",
        karol,
        Some("3894"),
        None,
        Some("Correios"),
        Some("JOAO PEDRO"),
        None,
        None,
        None,
        vec![item(
            "triciclo",
            Some("TRICICLO ADULTO XL"),
            Some("caixa"),
            None,
            1,
            None,
        )],
    );

    registrar(
        &mut conn,
        armazem_a4,
        "saida_armazem",
        "saida",
        dias[1],
        "09:05",
        karol,
        Some("3901"),
        Some("BR555000111BR"),
        Some("HEP EMPREENDIMENTOS LTDA"),
        Some("TIAGO"),
        None,
        None,
        Some("Pedido combinado por telefone com a gerencia"),
        vec![
            item(
                "scooter",
                Some("HE-15 CARBON"),
                Some("montado"),
                None,
                1,
                None,
            ),
            item("peca", Some("Carregador 48V"), None, None, 4, None),
        ],
    );
    registrar(
        &mut conn,
        armazem_a4,
        "saida_armazem",
        "entrada",
        dias[1],
        "14:20",
        karol,
        Some("3902"),
        None,
        Some("Cliente Final - Marcos Vieira"),
        None,
        None,
        None,
        Some("Devolucao - cliente desistiu da compra"),
        vec![item(
            "patinete",
            Some("SE-85 BLACK"),
            Some("caixa"),
            None,
            1,
            None,
        )],
    );

    registrar(
        &mut conn,
        armazem_a4,
        "saida_armazem",
        "saida",
        dias[2],
        "08:50",
        karol,
        Some("3910"),
        None,
        Some("Transportadora Rapidex"),
        Some("ANDRE"),
        None,
        None,
        None,
        vec![item(
            "scooter",
            Some("HE-15 GREEN"),
            Some("caixa"),
            None,
            5,
            None,
        )],
    );

    registrar(
        &mut conn,
        armazem_a4,
        "saida_armazem",
        "saida",
        dias[3],
        "09:30",
        karol,
        Some("3925"),
        Some("BR777888999BR"),
        Some("DISK&TENHA LOGISTICA"),
        Some("MARCELO"),
        None,
        None,
        None,
        vec![
            item(
                "scooter",
                Some("HE-15 CARBON"),
                Some("montado"),
                None,
                1,
                None,
            ),
            item(
                "triciclo",
                Some("TRICICLO ADULTO XL"),
                Some("montado"),
                None,
                1,
                None,
            ),
            item("patinete", None, Some("caixa"), None, 6, None),
        ],
    );

    registrar(
        &mut conn,
        armazem_a4,
        "saida_armazem",
        "saida",
        dias[4],
        "10:05",
        karol,
        Some("3931"),
        None,
        Some("HEP EMPREENDIMENTOS LTDA"),
        Some("TIAGO"),
        None,
        None,
        None,
        vec![item(
            "scooter",
            Some("HE-15 GREEN"),
            Some("montado"),
            None,
            2,
            None,
        )],
    );
    registrar(
        &mut conn,
        armazem_a4,
        "saida_armazem",
        "saida",
        dias[4],
        "11:15",
        karol,
        Some("3932"),
        None,
        Some("Correios"),
        Some("JOAO PEDRO"),
        None,
        None,
        None,
        vec![item(
            "patinete",
            Some("SE-85 BLACK"),
            Some("caixa"),
            None,
            2,
            None,
        )],
    );

    // Fecha o primeiro dia e estorna um dos lancamentos dele, pra mostrar os
    // dois recursos (impressao de fechamento e badge de estorno) com dado real.
    if let Ok(fechamento) =
        fechamentos::fechar_dia(&mut conn, armazem_a4, "saida_armazem", dias[0], gestor)
    {
        println!(
            "Fechado {} (saida_armazem, A4): {} un.",
            dias[0], fechamento.total_itens
        );
    }
    let lista_dia_fechado =
        movimentos::listar_movimentos_do_dia(&conn, armazem_a4, "saida_armazem", dias[0]).unwrap();
    if let Some(primeiro) = lista_dia_fechado.first() {
        let _ = movimentos::estornar_movimento(
            &mut conn,
            primeiro.id,
            gestor,
            "Pedido cancelado pelo cliente depois do fechamento do dia",
        );
    }

    // ---- Peca para Montagem (B2) ----
    for (data, tipo, descricao, condicao, qtd) in [
        (dias[1], "saida", "Retrovisor esquerdo", "boa", 4_i64),
        (dias[1], "saida", "Guidao HE-15", "boa", 2),
        (dias[2], "entrada", "Bateria 48V com defeito", "defeito", 1),
        (dias[3], "saida", "Farol dianteiro", "boa", 6),
        (dias[3], "saida", "Pneu 8 polegadas", "sucata", 2),
        (dias[4], "saida", "Freio a disco", "boa", 3),
    ] {
        registrar(
            &mut conn,
            armazem_b2,
            "peca_montagem",
            tipo,
            data,
            "09:00",
            geson,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            vec![item(
                "peca",
                Some(descricao),
                None,
                Some(condicao),
                qtd,
                None,
            )],
        );
    }

    // ---- SAC (A4) ----
    registrar(
        &mut conn,
        armazem_a4,
        "sac",
        "entrada",
        dias[2],
        "13:00",
        karol,
        Some("PROT-58211"),
        None,
        Some("Correios"),
        None,
        Some("garantia"),
        None,
        Some("Peca trincada, cliente enviou foto"),
        vec![item("peca", Some("Guidao HE-15"), None, None, 1, None)],
    );
    registrar(
        &mut conn,
        armazem_a4,
        "sac",
        "entrada",
        dias[3],
        "15:40",
        karol,
        Some("PROT-58244"),
        None,
        Some("Cliente Final - Ana Souza"),
        None,
        Some("venda"),
        Some(4590),
        None,
        vec![item(
            "peca",
            Some("Retrovisor esquerdo"),
            None,
            None,
            1,
            None,
        )],
    );
    registrar(
        &mut conn,
        armazem_a4,
        "sac",
        "entrada",
        dias[4],
        "16:10",
        karol,
        Some("PROT-58260"),
        None,
        Some("Correios"),
        None,
        Some("venda"),
        Some(12900),
        None,
        vec![item("peca", Some("Bateria 48V"), None, None, 1, None)],
    );

    println!("Dados de teste inseridos com sucesso.");
    println!(
        "Login: brenda / gestor (sem armazem) | karol / conferente A4 | geson / conferente B2"
    );
    println!("Senha de todos: senha123");
}
