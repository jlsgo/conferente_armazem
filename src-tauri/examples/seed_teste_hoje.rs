//! Limpa os lancamentos/fechamentos de HOJE (data hardcoded abaixo - ajuste
//! se rodar em outro dia) e repopula os 3 fluxos com cenarios de teste
//! variados, usando o usuario/armazem reais ja cadastrados no banco de
//! desenvolvimento (nao cria usuario novo). Ferramenta manual, nao roda como
//! parte do app nem dos testes:
//!
//!   cd src-tauri
//!   cargo run --example seed_teste_hoje
//!
//! Sem argumento usa o diretorio de dados padrao do Linux
//! (~/.local/share/com.ecoviva.controlearmazem, o mesmo que `npm run dev` usa).

use std::path::PathBuf;

use rusqlite::{params, Connection};

use app_lib::db;
use app_lib::domain::movimentos::{self, MovimentoItemInput, NovoMovimento};

const HOJE: &str = "2026-08-27";

fn diretorio_dados() -> PathBuf {
    if let Some(caminho) = std::env::args().nth(1) {
        return PathBuf::from(caminho);
    }
    let home = std::env::var("HOME").expect("defina HOME ou passe o diretorio como argumento");
    PathBuf::from(home).join(".local/share/com.ecoviva.controlearmazem")
}

fn limpar_hoje(conn: &Connection) {
    // Estornos primeiro (referenciam o original via estornado_de - com
    // foreign_keys=ON, apagar o original antes quebraria a FK).
    conn.execute(
        "DELETE FROM movimentos WHERE data = ?1 AND estornado_de IS NOT NULL",
        params![HOJE],
    )
    .unwrap();
    conn.execute("DELETE FROM movimentos WHERE data = ?1", params![HOJE])
        .unwrap();
    conn.execute("DELETE FROM fechamentos WHERE data = ?1", params![HOJE])
        .unwrap();
    println!("Limpos lancamentos/fechamentos de {HOJE}.");
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
        codigo_componente: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn registrar(
    conn: &mut Connection,
    armazem_id: i64,
    armazem_destino_id: Option<i64>,
    fluxo: &str,
    tipo: &str,
    hora: &str,
    usuario_id: i64,
    numero_pedido: Option<&str>,
    codigo_rastreio: Option<&str>,
    contraparte: Option<&str>,
    quem_retirou: Option<&str>,
    motivo: Option<&str>,
    valor_centavos: Option<i64>,
    observacoes: Option<&str>,
    retirada_completa: bool,
    itens: Vec<MovimentoItemInput>,
) -> Option<i64> {
    match movimentos::criar_movimento(
        conn,
        NovoMovimento {
            armazem_id,
            armazem_destino_id,
            fluxo: fluxo.into(),
            tipo: tipo.into(),
            data: HOJE.into(),
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
            retirada_completa,
            itens,
        },
    ) {
        Ok(movimento) => Some(movimento.id),
        Err(e) => {
            eprintln!("aviso: pulei {numero_pedido:?} em {fluxo}/{tipo} ({hora}): {e}");
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
        .expect("armazem A4 deveria existir (seed do proprio app)");
    let armazem_b2: i64 = conn
        .query_row("SELECT id FROM armazens WHERE codigo = 'B2'", [], |r| {
            r.get(0)
        })
        .expect("armazem B2 deveria existir (seed do proprio app)");
    let jhon: i64 = conn
        .query_row("SELECT id FROM usuarios WHERE login = 'jhon'", [], |r| {
            r.get(0)
        })
        .expect("usuario 'jhon' deveria existir - rode isso num banco de dev ja em uso");

    limpar_hoje(&conn);

    // ---- Saida de Armazem (A4) ----
    let id_5001 = registrar(
        &mut conn,
        armazem_a4,
        None,
        "saida_armazem",
        "saida",
        "08:30",
        jhon,
        Some("5001"),
        Some("BR100200300BR"),
        Some("Transportadora Rapidex"),
        Some("Marcos"),
        None,
        None,
        None,
        true,
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
                1,
                None,
            ),
        ],
    );
    registrar(
        &mut conn,
        armazem_a4,
        None,
        "saida_armazem",
        "saida",
        "09:15",
        jhon,
        Some("5002"),
        None,
        Some("Correios"),
        Some("Ana"),
        None,
        None,
        None,
        false, // retirada parcial
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
        Some(armazem_b2), // transferencia entre armazens
        "saida_armazem",
        "saida",
        "10:00",
        jhon,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        true,
        vec![item(
            "scooter",
            Some("HE-15 CARBON"),
            Some("montado"),
            None,
            3,
            None,
        )],
    );
    registrar(
        &mut conn,
        armazem_a4,
        None,
        "saida_armazem",
        "entrada",
        "11:20",
        jhon,
        Some("5004"),
        None,
        Some("Fornecedor XYZ Distribuidora"),
        None,
        None,
        None,
        Some("Devolucao - cliente desistiu da compra"),
        true,
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
        None,
        "saida_armazem",
        "saida",
        "13:45",
        jhon,
        Some("5005"),
        Some("BR999888777BR"),
        Some("DISK&TENHA LOGISTICA"),
        Some("Marcelo"),
        None,
        None,
        Some("Pedido combinado por telefone com a gerencia"),
        true,
        vec![
            item("peca", Some("Carregador 48V"), None, None, 4, None),
            item(
                "scooter",
                Some("HE-15 GREEN"),
                Some("montado"),
                None,
                1,
                None,
            ),
        ],
    );
    registrar(
        &mut conn,
        armazem_a4,
        None,
        "saida_armazem",
        "saida",
        "14:30",
        jhon,
        Some("5006"),
        None,
        Some("Cliente Final - Roberto Lima"),
        Some("Roberto"),
        None,
        None,
        None,
        true,
        vec![item(
            "triciclo",
            Some("TRICICLO ADULTO XL"),
            Some("montado"),
            None,
            1,
            None,
        )],
    );
    if let Some(id) = id_5001 {
        let _ = movimentos::estornar_movimento(
            &mut conn,
            id,
            jhon,
            "Pedido cancelado pelo cliente apos a saida",
        );
    }

    // ---- Peca para Montagem (A4) ----
    registrar(
        &mut conn,
        armazem_a4,
        None,
        "peca_montagem",
        "entrada",
        "08:00",
        jhon,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        true,
        vec![item(
            "peca",
            Some("Farol Dianteiro LED"),
            None,
            Some("boa"),
            3,
            None,
        )],
    );
    let id_bateria_defeito = registrar(
        &mut conn,
        armazem_a4,
        None,
        "peca_montagem",
        "entrada",
        "08:30",
        jhon,
        None,
        None,
        None,
        None,
        None,
        None,
        Some("Chegou com barulho estranho, aguardando avaliacao"),
        true,
        vec![item(
            "peca",
            Some("Bateria 48V com ruido"),
            None,
            Some("defeito"),
            1,
            None,
        )],
    );
    registrar(
        &mut conn,
        armazem_a4,
        Some(armazem_b2),
        "peca_montagem",
        "saida",
        "09:10",
        jhon,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        true,
        vec![item(
            "peca",
            Some("Motor Traseiro 500W"),
            None,
            Some("boa"),
            2,
            None,
        )],
    );
    registrar(
        &mut conn,
        armazem_a4,
        None,
        "peca_montagem",
        "saida",
        "09:40",
        jhon,
        None,
        None,
        Some("Tecnico Externo - Ricardo Alves"),
        None,
        None,
        None,
        None,
        true,
        vec![item(
            "peca",
            Some("Modulo controlador"),
            None,
            Some("defeito"),
            1,
            Some("Serial MT-2291"),
        )],
    );
    registrar(
        &mut conn,
        armazem_a4,
        None,
        "peca_montagem",
        "saida",
        "10:15",
        jhon,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        true,
        vec![item(
            "peca",
            Some("Pneu 8 polegadas rasgado"),
            None,
            Some("sucata"),
            4,
            None,
        )],
    );
    registrar(
        &mut conn,
        armazem_a4,
        None,
        "peca_montagem",
        "entrada",
        "11:00",
        jhon,
        None,
        None,
        None,
        None,
        None,
        None,
        Some("Compra direta fornecedor XYZ"),
        true,
        vec![item(
            "peca",
            Some("Guidao HE-15"),
            None,
            Some("boa"),
            5,
            None,
        )],
    );
    if let Some(id) = id_bateria_defeito {
        let _ =
            movimentos::estornar_movimento(&mut conn, id, jhon, "Lancamento duplicado por engano");
    }

    // ---- SAC (A4) ----
    registrar(
        &mut conn,
        armazem_a4,
        None,
        "sac",
        "entrada",
        "09:00",
        jhon,
        Some("PROT-70011"),
        None,
        Some("Correios"),
        None,
        Some("garantia"),
        None,
        Some("Peca trincada, cliente enviou foto"),
        true,
        vec![item("peca", Some("Guidao HE-15"), None, None, 1, None)],
    );
    registrar(
        &mut conn,
        armazem_a4,
        None,
        "sac",
        "entrada",
        "09:30",
        jhon,
        Some("PROT-70012"),
        None,
        Some("Cliente Final - Julia Alves"),
        None,
        Some("venda"),
        Some(4590),
        None,
        true,
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
        None,
        "sac",
        "saida",
        "10:00",
        jhon,
        Some("PROT-70011"),
        None,
        None,
        None,
        Some("entregue"),
        None,
        None,
        true,
        vec![item("peca", Some("Guidao HE-15"), None, None, 1, None)],
    );
    let id_descarte = registrar(
        &mut conn,
        armazem_a4,
        None,
        "sac",
        "saida",
        "10:30",
        jhon,
        Some("PROT-70013"),
        None,
        None,
        None,
        Some("descarte"),
        None,
        None,
        true,
        vec![item("peca", Some("Bateria 48V"), None, None, 1, None)],
    );
    // Cenarios novos: saida do SAC resolvida como garantia/venda (antes so
    // entregue/descarte eram aceitos na saida - ver ROADMAP).
    registrar(
        &mut conn,
        armazem_a4,
        None,
        "sac",
        "saida",
        "11:00",
        jhon,
        Some("PROT-70014"),
        None,
        None,
        None,
        Some("garantia"),
        None,
        None,
        true,
        vec![item(
            "peca",
            Some("Modulo controlador"),
            None,
            None,
            1,
            None,
        )],
    );
    registrar(
        &mut conn,
        armazem_a4,
        None,
        "sac",
        "saida",
        "11:30",
        jhon,
        Some("PROT-70015"),
        None,
        None,
        None,
        Some("venda"),
        Some(8900),
        None,
        true,
        vec![item(
            "peca",
            Some("Farol Dianteiro LED"),
            None,
            None,
            1,
            None,
        )],
    );
    if let Some(id) = id_descarte {
        let _ = movimentos::estornar_movimento(
            &mut conn,
            id,
            jhon,
            "Peca na verdade tinha conserto, revertido",
        );
    }

    println!("Cenarios de teste de {HOJE} inseridos com sucesso (6 lancamentos + 1 estorno em cada aba).");
    println!("Login: jhon / senha ja configurada.");
}
