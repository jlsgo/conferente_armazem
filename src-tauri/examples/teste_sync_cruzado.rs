//! Verifica de ponta a ponta, contra o Turso real configurado nesta maquina
//! (turso.txt na pasta de dados), o fluxo completo de transferencia entre A4
//! e B2: envio de um armazem, aparece pendente no outro, confirmacao de
//! recebimento fecha o ciclo (some da lista de pendentes), e um envio
//! estornado do lado de quem mandou tambem some da lista. Simula os dois PCs
//! com dois bancos SQLite locais temporarios (mesmo padrao ja documentado em
//! docs/ARQUITETURA.md como o jeito usado pra validar isso manualmente) -
//! nao mexe no banco real de nenhum PC. So roda manualmente:
//!
//!   cd src-tauri
//!   cargo run --example teste_sync_cruzado
//!
//! Escreve linhas de teste na tabela remota `movimentos_consolidados`
//! (armazem_codigo A4/B2, ids baixos) - rode so contra um Turso que nao
//! tenha dados reais de producao com os mesmos ids.

use std::path::PathBuf;

use app_lib::db;
use app_lib::db::sync;
use app_lib::domain::auth::{criar_usuario, NovoUsuario};
use app_lib::domain::movimentos::{
    self, criar_movimento, validar_quantidades_recebidas, MovimentoItemInput, NovoMovimento,
};

fn diretorio_temp(nome: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ecoviva-teste-sync-{nome}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    dir
}

fn armazem_id_por_codigo(conn: &rusqlite::Connection, codigo: &str) -> i64 {
    conn.query_row("SELECT id FROM armazens WHERE codigo = ?1", [codigo], |r| {
        r.get(0)
    })
    .unwrap()
}

#[tokio::main]
async fn main() {
    let diretorio_dados_real = {
        let home = std::env::var("HOME").expect("defina HOME");
        PathBuf::from(home).join(".local/share/com.ecoviva.controlearmazem")
    };
    let (url, token) = sync::ler_config_turso(&diretorio_dados_real)
        .expect("turso.txt nao encontrado/incompleto na pasta de dados real - configure antes de rodar este teste");

    println!(
        "Usando Turso configurado em {}",
        diretorio_dados_real.display()
    );

    let dir_b2 = diretorio_temp("b2");
    let dir_a4 = diretorio_temp("a4");
    let mut conn_b2 = db::abrir(&dir_b2).unwrap();
    let mut conn_a4 = db::abrir(&dir_a4).unwrap();

    let armazem_b2_local = armazem_id_por_codigo(&conn_b2, "B2");
    let armazem_a4_no_banco_b2 = armazem_id_por_codigo(&conn_b2, "A4");
    let armazem_a4_local = armazem_id_por_codigo(&conn_a4, "A4");

    let usuario_b2 = criar_usuario(
        &conn_b2,
        NovoUsuario {
            nome: "Teste Sync B2",
            login: "teste_sync_b2",
            senha: "senha123",
            armazem_id: Some(armazem_b2_local),
            papel: "gestor",
        },
    )
    .unwrap();
    let usuario_a4 = criar_usuario(
        &conn_a4,
        NovoUsuario {
            nome: "Teste Sync A4",
            login: "teste_sync_a4",
            senha: "senha123",
            armazem_id: Some(armazem_a4_local),
            papel: "gestor",
        },
    )
    .unwrap();

    // 1) B2 envia uma peca pra A4.
    let envio1 = criar_movimento(
        &mut conn_b2,
        NovoMovimento {
            armazem_id: armazem_b2_local,
            armazem_destino_id: Some(armazem_a4_no_banco_b2),
            fluxo: "peca_montagem".into(),
            tipo: "saida".into(),
            data: "2026-08-27".into(),
            hora: "10:00".into(),
            turno: "diurno".into(),
            usuario_id: usuario_b2,
            numero_pedido: None,
            codigo_rastreio: None,
            contraparte: None,
            quem_retirou: None,
            motivo: None,
            valor_centavos: None,
            observacoes: Some("teste_sync_cruzado envio1".into()),
            recebido_de_armazem_codigo: None,
            recebido_de_id_origem: None,
            retirada_completa: true,
            itens: vec![MovimentoItemInput {
                categoria: "peca".into(),
                descricao: Some("Motor 350W".into()),
                montagem: None,
                condicao: Some("boa".into()),
                quantidade: 2,
                observacao: None,
                quantidade_enviada: None,
            }],
        },
    )
    .unwrap();
    println!("[B2] criou envio1 (id local {})", envio1.id);

    // 2) B2 envia uma segunda peca pra A4, que sera estornada em seguida.
    let envio2 = criar_movimento(
        &mut conn_b2,
        NovoMovimento {
            armazem_id: armazem_b2_local,
            armazem_destino_id: Some(armazem_a4_no_banco_b2),
            fluxo: "peca_montagem".into(),
            tipo: "saida".into(),
            data: "2026-08-27".into(),
            hora: "10:05".into(),
            turno: "diurno".into(),
            usuario_id: usuario_b2,
            numero_pedido: None,
            codigo_rastreio: None,
            contraparte: None,
            quem_retirou: None,
            motivo: None,
            valor_centavos: None,
            observacoes: Some("teste_sync_cruzado envio2 (sera estornado)".into()),
            recebido_de_armazem_codigo: None,
            recebido_de_id_origem: None,
            retirada_completa: true,
            itens: vec![MovimentoItemInput {
                categoria: "peca".into(),
                descricao: Some("Bateria 48V".into()),
                montagem: None,
                condicao: Some("boa".into()),
                quantidade: 1,
                observacao: None,
                quantidade_enviada: None,
            }],
        },
    )
    .unwrap();
    println!("[B2] criou envio2 (id local {})", envio2.id);

    movimentos::estornar_movimento(
        &mut conn_b2,
        envio2.id,
        usuario_b2,
        "teste_sync_cruzado: estornando de proposito",
    )
    .unwrap();
    println!("[B2] estornou envio2");

    // 3) B2 sincroniza os dois (envio1 + envio2 + o estorno do envio2).
    let pendentes_b2 = sync::movimentos_pendentes(&conn_b2).unwrap();
    assert_eq!(
        pendentes_b2.len(),
        3,
        "esperava 3 linhas pendentes em B2 (envio1, envio2, estorno)"
    );
    let agora_local_b2 = sync::agora_local(&conn_b2).unwrap();
    let resultado_envio = sync::enviar_para_turso(&url, &token, &pendentes_b2, &agora_local_b2)
        .await
        .unwrap();
    assert!(
        resultado_envio.falhas.is_empty(),
        "falhas ao enviar B2->Turso: {:?}",
        resultado_envio.falhas
    );
    sync::marcar_sincronizado(&conn_b2, &resultado_envio.enviados).unwrap();
    println!(
        "[B2] sincronizou {} linha(s) com o Turso",
        resultado_envio.enviados.len()
    );

    // 4) A4 consulta o que esta pendente pra ele - espera achar so o envio1.
    let pendentes_a4 = sync::buscar_pendentes_recebimento(&url, &token, "A4")
        .await
        .unwrap();
    let achou_envio1 = pendentes_a4
        .iter()
        .find(|t| t.armazem_origem_codigo == "B2" && t.id_origem == envio1.id);
    let achou_envio2 = pendentes_a4
        .iter()
        .any(|t| t.armazem_origem_codigo == "B2" && t.id_origem == envio2.id);
    assert!(
        achou_envio1.is_some(),
        "envio1 deveria aparecer como pendente em A4, nao apareceu. Pendentes: {pendentes_a4:?}"
    );
    assert!(
        !achou_envio2,
        "envio2 foi estornado em B2, NAO deveria aparecer como pendente em A4"
    );
    let transferencia1 = achou_envio1.unwrap();
    assert_eq!(transferencia1.itens.len(), 1);
    assert_eq!(transferencia1.itens[0].quantidade, 2);
    println!(
        "[A4] confirmou: envio1 pendente (2x Motor 350W), envio2 (estornado) corretamente ausente"
    );

    // 5) A4 confirma o recebimento do envio1 (recebe a quantidade completa).
    let itens_confirmados = validar_quantidades_recebidas(&transferencia1.itens, &[2]).unwrap();
    let confirmacao = criar_movimento(
        &mut conn_a4,
        NovoMovimento {
            armazem_id: armazem_a4_local,
            armazem_destino_id: None,
            fluxo: transferencia1.fluxo.clone(),
            tipo: "entrada".into(),
            data: "2026-08-27".into(),
            hora: "11:00".into(),
            turno: "diurno".into(),
            usuario_id: usuario_a4,
            numero_pedido: None,
            codigo_rastreio: None,
            contraparte: None,
            quem_retirou: None,
            motivo: None,
            valor_centavos: None,
            observacoes: Some(format!("Recebido de B2 (envio #{})", envio1.id)),
            recebido_de_armazem_codigo: Some("B2".into()),
            recebido_de_id_origem: Some(envio1.id),
            retirada_completa: true,
            itens: itens_confirmados,
        },
    )
    .unwrap();
    println!(
        "[A4] confirmou recebimento (novo movimento local id {})",
        confirmacao.id
    );

    let pendentes_confirmacao = sync::movimentos_pendentes(&conn_a4).unwrap();
    let agora_local_a4 = sync::agora_local(&conn_a4).unwrap();
    let resultado_confirmacao =
        sync::enviar_para_turso(&url, &token, &pendentes_confirmacao, &agora_local_a4)
            .await
            .unwrap();
    assert!(
        resultado_confirmacao.falhas.is_empty(),
        "falhas ao enviar confirmacao A4->Turso: {:?}",
        resultado_confirmacao.falhas
    );
    sync::marcar_sincronizado(&conn_a4, &resultado_confirmacao.enviados).unwrap();
    println!("[A4] sincronizou a confirmacao com o Turso");

    // 6) A4 consulta de novo - envio1 nao deve mais aparecer como pendente.
    let pendentes_a4_depois = sync::buscar_pendentes_recebimento(&url, &token, "A4")
        .await
        .unwrap();
    let ainda_pendente = pendentes_a4_depois
        .iter()
        .any(|t| t.armazem_origem_codigo == "B2" && t.id_origem == envio1.id);
    assert!(
        !ainda_pendente,
        "envio1 ja foi confirmado, nao deveria mais aparecer como pendente"
    );
    println!("[A4] confirmou: envio1 nao aparece mais como pendente (ciclo fechado)");

    println!();
    println!("TUDO OK: sincronizacao A4<->B2 (envio, pendente, confirmacao, estorno-nao-aparece) verificada contra o Turso real.");

    let _ = std::fs::remove_dir_all(&dir_b2);
    let _ = std::fs::remove_dir_all(&dir_a4);
}
