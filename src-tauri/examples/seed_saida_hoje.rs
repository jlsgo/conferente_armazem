//! Ferramenta de uso unico: gera N lancamentos de teste em "Saida de Armazem"
//! na data de hoje, no banco real usado por `npm run dev`, so pra conferir
//! como fica a impressao do fechamento com muitas linhas. Nao roda como parte
//! do app nem dos testes.
//!
//!   cd src-tauri
//!   cargo run --example seed_saida_hoje -- [quantidade] [armazem_codigo] [usuario_login]
//!
//! Padrao: 40 lancamentos, armazem A4, usuario "jhon". Passe um caminho de
//! diretorio de dados alternativo com a variavel de ambiente ECOVIVA_DB_DIR.

use std::path::PathBuf;

use app_lib::db;
use app_lib::domain::movimentos::{self, MovimentoItemInput, NovoMovimento};

fn diretorio_dados() -> PathBuf {
    if let Ok(caminho) = std::env::var("ECOVIVA_DB_DIR") {
        return PathBuf::from(caminho);
    }
    let home = std::env::var("HOME").expect("defina HOME ou ECOVIVA_DB_DIR");
    PathBuf::from(home).join(".local/share/com.ecoviva.controlearmazem")
}

fn data_de_hoje() -> String {
    let saida = std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .expect("falha ao rodar date");
    String::from_utf8(saida.stdout).unwrap().trim().to_string()
}

fn item(
    categoria: &str,
    descricao: Option<&str>,
    montagem: Option<&str>,
    qtd: i64,
) -> MovimentoItemInput {
    MovimentoItemInput {
        categoria: categoria.into(),
        descricao: descricao.map(String::from),
        montagem: montagem.map(String::from),
        condicao: None,
        quantidade: qtd,
        observacao: None,
        quantidade_enviada: None,
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let quantidade: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(40);
    let armazem_codigo = args.next().unwrap_or_else(|| "A4".to_string());
    let usuario_login = args.next().unwrap_or_else(|| "jhon".to_string());

    let dir = diretorio_dados();
    println!("Usando diretorio de dados: {}", dir.display());

    let mut conn = db::abrir(&dir).expect("nao foi possivel abrir o banco");

    let armazem_id: i64 = conn
        .query_row(
            "SELECT id FROM armazens WHERE codigo = ?1",
            [&armazem_codigo],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| panic!("armazem {armazem_codigo} nao encontrado"));

    let usuario_id: i64 = conn
        .query_row(
            "SELECT id FROM usuarios WHERE login = ?1",
            [&usuario_login],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| panic!("usuario {usuario_login} nao encontrado"));

    let data = data_de_hoje();

    let clientes = [
        "DISK&TENHA LOGISTICA",
        "Correios",
        "HEP EMPREENDIMENTOS LTDA",
        "Transportadora Rapidex",
        "Cliente Final - Marcos Vieira",
        "Cliente Final - Ana Souza",
        "Jadlog",
        "Total Express",
    ];
    let retiradores = [
        "MARCELO",
        "JOAO PEDRO",
        "TIAGO",
        "ANDRE",
        "LUCAS",
        "FERNANDA",
        "PAULO",
        "RENATA",
    ];
    let modelos_scooter = ["HE-15 GREEN", "HE-15 CARBON", "HE-15 BLACK"];
    let modelos_patinete = ["SE-85 BLACK", "SE-85 RED"];
    let pecas = [
        "Carregador 48V",
        "Bateria 48V",
        "Guidao HE-15",
        "Retrovisor esquerdo",
    ];

    let mut criados = 0;
    for i in 0..quantidade {
        let numero_pedido = format!("{}", 4000 + i);
        let hora = format!("{:02}:{:02}", 7 + (i / 6) % 11, (i * 7) % 60);
        let cliente = clientes[i % clientes.len()];
        let quem_retirou = retiradores[i % retiradores.len()];

        let itens = match i % 4 {
            0 => vec![item(
                "scooter",
                Some(modelos_scooter[i % modelos_scooter.len()]),
                Some(if i % 2 == 0 { "montado" } else { "caixa" }),
                1 + (i as i64 % 3),
            )],
            1 => vec![item(
                "patinete",
                Some(modelos_patinete[i % modelos_patinete.len()]),
                Some("caixa"),
                2 + (i as i64 % 4),
            )],
            2 => vec![
                item(
                    "scooter",
                    Some(modelos_scooter[i % modelos_scooter.len()]),
                    Some("montado"),
                    1,
                ),
                item(
                    "peca",
                    Some(pecas[i % pecas.len()]),
                    None,
                    2 + (i as i64 % 3),
                ),
            ],
            _ => vec![item(
                "triciclo",
                Some("TRICICLO ADULTO XL"),
                Some(if i % 2 == 0 { "montado" } else { "caixa" }),
                1,
            )],
        };

        // Uma a cada ~10 fica marcada como retirada parcial, pra conferir o
        // marcador "(parcial)" na impressao tambem.
        let retirada_completa = i % 10 != 3;

        let resultado = movimentos::criar_movimento(
            &mut conn,
            NovoMovimento {
                armazem_id,
                armazem_destino_id: None,
                fluxo: "saida_armazem".into(),
                tipo: "saida".into(),
                data: data.clone(),
                hora,
                turno: "diurno".into(),
                usuario_id,
                numero_pedido: Some(numero_pedido.clone()),
                codigo_rastreio: None,
                contraparte: Some(cliente.to_string()),
                quem_retirou: Some(quem_retirou.to_string()),
                motivo: None,
                valor_centavos: None,
                observacoes: if i % 7 == 0 {
                    Some("Cliente pediu embalagem reforcada e nota fiscal na caixa".into())
                } else {
                    None
                },
                recebido_de_armazem_codigo: None,
                recebido_de_id_origem: None,
                retirada_completa,
                itens,
            },
        );

        match resultado {
            Ok(_) => criados += 1,
            Err(e) => eprintln!("aviso: pulei pedido {numero_pedido}: {e}"),
        }
    }

    println!("Criados {criados}/{quantidade} lancamentos de Saida de Armazem em {data} ({armazem_codigo}).");
}
