use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::errors::{AppError, AppResult};

const CATEGORIAS_VALIDAS: [&str; 4] = ["scooter", "triciclo", "patinete", "peca"];
const FLUXOS_VALIDOS: [&str; 3] = ["saida_armazem", "peca_montagem", "sac"];
const TIPOS_VALIDOS: [&str; 2] = ["entrada", "saida"];

#[derive(Debug, Deserialize)]
pub struct MovimentoItemInput {
    pub categoria: String,
    pub descricao: Option<String>,
    pub montagem: Option<String>,
    pub condicao: Option<String>,
    pub quantidade: i64,
    pub observacao: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NovoMovimento {
    pub armazem_id: i64,
    pub armazem_destino_id: Option<i64>,
    pub fluxo: String,
    pub tipo: String,
    pub data: String,
    pub hora: String,
    pub turno: String,
    pub usuario_id: i64,
    pub numero_pedido: Option<String>,
    pub codigo_rastreio: Option<String>,
    pub contraparte: Option<String>,
    pub quem_retirou: Option<String>,
    pub motivo: Option<String>,
    pub valor_centavos: Option<i64>,
    pub observacoes: Option<String>,
    pub itens: Vec<MovimentoItemInput>,
}

#[derive(Debug, Serialize)]
pub struct MovimentoItem {
    pub id: i64,
    pub categoria: String,
    pub descricao: Option<String>,
    pub montagem: Option<String>,
    pub condicao: Option<String>,
    pub quantidade: i64,
    pub observacao: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Movimento {
    pub id: i64,
    pub numero: i64,
    pub armazem_id: i64,
    pub fluxo: String,
    pub tipo: String,
    pub data: String,
    pub hora: String,
    pub turno: String,
    pub usuario_id: i64,
    pub usuario_nome: String,
    pub numero_pedido: Option<String>,
    pub contraparte: Option<String>,
    pub quem_retirou: Option<String>,
    pub status: String,
    pub hash_integridade: String,
    pub itens: Vec<MovimentoItem>,
}

fn validar_data(data: &str) -> bool {
    let bytes = data.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && data[0..4].chars().all(|c| c.is_ascii_digit())
        && data[5..7].chars().all(|c| c.is_ascii_digit())
        && data[8..10].chars().all(|c| c.is_ascii_digit())
}

fn validar_hora(hora: &str) -> bool {
    let bytes = hora.as_bytes();
    bytes.len() == 5
        && bytes[2] == b':'
        && hora[0..2].chars().all(|c| c.is_ascii_digit())
        && hora[3..5].chars().all(|c| c.is_ascii_digit())
}

fn validar_novo_movimento(novo: &NovoMovimento) -> AppResult<()> {
    if !FLUXOS_VALIDOS.contains(&novo.fluxo.as_str()) {
        return Err(AppError::Validation(format!(
            "Fluxo invalido: {}",
            novo.fluxo
        )));
    }
    if !TIPOS_VALIDOS.contains(&novo.tipo.as_str()) {
        return Err(AppError::Validation(format!(
            "Tipo invalido: {}",
            novo.tipo
        )));
    }
    if !validar_data(&novo.data) {
        return Err(AppError::Validation(
            "Data invalida (use AAAA-MM-DD).".into(),
        ));
    }
    if !validar_hora(&novo.hora) {
        return Err(AppError::Validation("Horario invalido (use HH:MM).".into()));
    }
    if novo.itens.is_empty() {
        return Err(AppError::Validation(
            "Inclua ao menos um item no lancamento.".into(),
        ));
    }
    for item in &novo.itens {
        if !CATEGORIAS_VALIDAS.contains(&item.categoria.as_str()) {
            return Err(AppError::Validation(format!(
                "Categoria invalida: {}",
                item.categoria
            )));
        }
        if item.quantidade <= 0 {
            return Err(AppError::Validation(
                "A quantidade de cada item precisa ser maior que zero.".into(),
            ));
        }
    }
    Ok(())
}

fn calcular_hash(hash_anterior: &str, novo: &NovoMovimento) -> String {
    let itens_resumo: Vec<String> = novo
        .itens
        .iter()
        .map(|i| {
            format!(
                "{}:{}:{}",
                i.categoria,
                i.descricao.as_deref().unwrap_or(""),
                i.quantidade
            )
        })
        .collect();

    let conteudo = format!(
        "{hash_anterior}|{}|{}|{}|{}|{}|{}|{}|{}",
        novo.armazem_id,
        novo.fluxo,
        novo.tipo,
        novo.data,
        novo.hora,
        novo.usuario_id,
        novo.numero_pedido.as_deref().unwrap_or(""),
        itens_resumo.join(";")
    );

    let mut hasher = Sha256::new();
    hasher.update(conteudo.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn criar_movimento(conn: &mut Connection, novo: NovoMovimento) -> AppResult<Movimento> {
    validar_novo_movimento(&novo)?;

    let tx = conn.transaction()?;

    let dia_fechado: bool = tx
        .query_row(
            "SELECT 1 FROM fechamentos WHERE armazem_id = ?1 AND fluxo = ?2 AND data = ?3",
            params![novo.armazem_id, novo.fluxo, novo.data],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);

    if dia_fechado {
        return Err(AppError::Validation(
            "Este dia ja foi fechado. Nao e possivel adicionar novos lancamentos.".into(),
        ));
    }

    let hash_anterior: String = tx
        .query_row(
            "SELECT hash_integridade FROM movimentos ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "GENESIS-ECOVIVA".to_string());

    let hash = calcular_hash(&hash_anterior, &novo);

    tx.execute(
        "INSERT INTO movimentos (
            armazem_id, armazem_destino_id, fluxo, tipo, data, hora, turno, usuario_id,
            numero_pedido, codigo_rastreio, contraparte, quem_retirou,
            motivo, valor_centavos, observacoes, status, hash_integridade
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 'aberto', ?16)",
        params![
            novo.armazem_id,
            novo.armazem_destino_id,
            novo.fluxo,
            novo.tipo,
            novo.data,
            novo.hora,
            novo.turno,
            novo.usuario_id,
            novo.numero_pedido,
            novo.codigo_rastreio,
            novo.contraparte,
            novo.quem_retirou,
            novo.motivo,
            novo.valor_centavos,
            novo.observacoes,
            hash,
        ],
    )?;

    let movimento_id = tx.last_insert_rowid();

    {
        let mut inserir_item = tx.prepare(
            "INSERT INTO movimento_itens (movimento_id, categoria, descricao, montagem, condicao, quantidade, observacao)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for item in &novo.itens {
            inserir_item.execute(params![
                movimento_id,
                item.categoria,
                item.descricao,
                item.montagem,
                item.condicao,
                item.quantidade,
                item.observacao,
            ])?;
        }
    }

    tx.commit()?;

    buscar_movimento(conn, movimento_id)
}

fn carregar_itens(conn: &Connection, movimento_id: i64) -> AppResult<Vec<MovimentoItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, categoria, descricao, montagem, condicao, quantidade, observacao
         FROM movimento_itens WHERE movimento_id = ?1 ORDER BY id ASC",
    )?;
    let itens = stmt
        .query_map(params![movimento_id], |r| {
            Ok(MovimentoItem {
                id: r.get(0)?,
                categoria: r.get(1)?,
                descricao: r.get(2)?,
                montagem: r.get(3)?,
                condicao: r.get(4)?,
                quantidade: r.get(5)?,
                observacao: r.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(itens)
}

pub fn buscar_movimento(conn: &Connection, id: i64) -> AppResult<Movimento> {
    let (
        armazem_id,
        fluxo,
        tipo,
        data,
        hora,
        turno,
        usuario_id,
        usuario_nome,
        numero_pedido,
        contraparte,
        quem_retirou,
        status,
        hash_integridade,
    ) = conn.query_row(
        "SELECT m.armazem_id, m.fluxo, m.tipo, m.data, m.hora, m.turno, m.usuario_id, u.nome,
                m.numero_pedido, m.contraparte, m.quem_retirou, m.status, m.hash_integridade
         FROM movimentos m JOIN usuarios u ON u.id = m.usuario_id
         WHERE m.id = ?1",
        params![id],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, Option<String>>(10)?,
                r.get::<_, String>(11)?,
                r.get::<_, String>(12)?,
            ))
        },
    )?;

    let itens = carregar_itens(conn, id)?;

    Ok(Movimento {
        id,
        numero: 0,
        armazem_id,
        fluxo,
        tipo,
        data,
        hora,
        turno,
        usuario_id,
        usuario_nome,
        numero_pedido,
        contraparte,
        quem_retirou,
        status,
        hash_integridade,
        itens,
    })
}

pub fn listar_movimentos_do_dia(
    conn: &Connection,
    armazem_id: i64,
    fluxo: &str,
    data: &str,
) -> AppResult<Vec<Movimento>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.armazem_id, m.fluxo, m.tipo, m.data, m.hora, m.turno, m.usuario_id, u.nome,
                m.numero_pedido, m.contraparte, m.quem_retirou, m.status, m.hash_integridade
         FROM movimentos m JOIN usuarios u ON u.id = m.usuario_id
         WHERE m.armazem_id = ?1 AND m.fluxo = ?2 AND m.data = ?3
         ORDER BY m.id ASC",
    )?;

    let mut movimentos = stmt
        .query_map(params![armazem_id, fluxo, data], |r| {
            Ok(Movimento {
                id: r.get(0)?,
                numero: 0,
                armazem_id: r.get(1)?,
                fluxo: r.get(2)?,
                tipo: r.get(3)?,
                data: r.get(4)?,
                hora: r.get(5)?,
                turno: r.get(6)?,
                usuario_id: r.get(7)?,
                usuario_nome: r.get(8)?,
                numero_pedido: r.get(9)?,
                contraparte: r.get(10)?,
                quem_retirou: r.get(11)?,
                status: r.get(12)?,
                hash_integridade: r.get(13)?,
                itens: Vec::new(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    for (indice, movimento) in movimentos.iter_mut().enumerate() {
        movimento.numero = indice as i64 + 1;
        movimento.itens = carregar_itens(conn, movimento.id)?;
    }

    Ok(movimentos)
}

/// Sugestoes de descricao ja usadas para a categoria informada, para
/// autocompletar o formulario sem precisar de um catalogo mantido a parte.
pub fn sugestoes_descricao(conn: &Connection, categoria: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT descricao FROM movimento_itens
         WHERE categoria = ?1 AND descricao IS NOT NULL AND descricao != ''
         ORDER BY descricao ASC LIMIT 100",
    )?;
    let sugestoes = stmt
        .query_map(params![categoria], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(sugestoes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::domain::auth::{criar_usuario, NovoUsuario};

    fn conexao_de_teste() -> (Connection, i64, i64) {
        let conn = db::abrir_em_memoria().unwrap();
        let armazem_id: i64 = conn
            .query_row("SELECT id FROM armazens WHERE codigo = 'B2'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let usuario_id = criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Alice",
                login: "alice",
                senha: "senha123",
                armazem_id: Some(armazem_id),
                papel: "conferente",
            },
        )
        .unwrap();
        (conn, armazem_id, usuario_id)
    }

    fn movimento_base(
        armazem_id: i64,
        usuario_id: i64,
        itens: Vec<MovimentoItemInput>,
    ) -> NovoMovimento {
        NovoMovimento {
            armazem_id,
            armazem_destino_id: None,
            fluxo: "saida_armazem".into(),
            tipo: "saida".into(),
            data: "2026-08-24".into(),
            hora: "09:00".into(),
            turno: "diurno".into(),
            usuario_id,
            numero_pedido: Some("3893".into()),
            codigo_rastreio: None,
            contraparte: Some("DISK&TENHA".into()),
            quem_retirou: Some("KAROL".into()),
            motivo: None,
            valor_centavos: None,
            observacoes: None,
            itens,
        }
    }

    #[test]
    fn cria_movimento_com_multiplos_itens_e_soma_certo() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();

        let itens = vec![
            MovimentoItemInput {
                categoria: "scooter".into(),
                descricao: Some("HE-15 CARBON".into()),
                montagem: Some("montado".into()),
                condicao: None,
                quantidade: 1,
                observacao: None,
            },
            MovimentoItemInput {
                categoria: "patinete".into(),
                descricao: None,
                montagem: Some("caixa".into()),
                condicao: None,
                quantidade: 2,
                observacao: None,
            },
        ];

        let movimento =
            criar_movimento(&mut conn, movimento_base(armazem_id, usuario_id, itens)).unwrap();
        assert_eq!(movimento.itens.len(), 2);
        let total: i64 = movimento.itens.iter().map(|i| i.quantidade).sum();
        assert_eq!(total, 3);
        assert!(!movimento.itens[0].id.to_string().is_empty());
    }

    #[test]
    fn rejeita_movimento_sem_itens() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let resultado = criar_movimento(&mut conn, movimento_base(armazem_id, usuario_id, vec![]));
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn rejeita_item_com_categoria_invalida() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let itens = vec![MovimentoItemInput {
            categoria: "carro".into(),
            descricao: None,
            montagem: None,
            condicao: None,
            quantidade: 1,
            observacao: None,
        }];
        let resultado = criar_movimento(&mut conn, movimento_base(armazem_id, usuario_id, itens));
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn rejeita_item_com_quantidade_zero() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let itens = vec![MovimentoItemInput {
            categoria: "peca".into(),
            descricao: Some("Retrovisor".into()),
            montagem: None,
            condicao: Some("boa".into()),
            quantidade: 0,
            observacao: None,
        }];
        let resultado = criar_movimento(&mut conn, movimento_base(armazem_id, usuario_id, itens));
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn numera_sequencialmente_e_soma_o_total_do_dia() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();

        for qtd in [1, 2, 3] {
            let itens = vec![MovimentoItemInput {
                categoria: "scooter".into(),
                descricao: None,
                montagem: None,
                condicao: None,
                quantidade: qtd,
                observacao: None,
            }];
            criar_movimento(&mut conn, movimento_base(armazem_id, usuario_id, itens)).unwrap();
        }

        let lista =
            listar_movimentos_do_dia(&conn, armazem_id, "saida_armazem", "2026-08-24").unwrap();
        assert_eq!(lista.len(), 3);
        assert_eq!(
            lista.iter().map(|m| m.numero).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        let total: i64 = lista
            .iter()
            .flat_map(|m| &m.itens)
            .map(|i| i.quantidade)
            .sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn hash_integridade_muda_conforme_movimentos_anteriores() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let item = || {
            vec![MovimentoItemInput {
                categoria: "scooter".into(),
                descricao: None,
                montagem: None,
                condicao: None,
                quantidade: 1,
                observacao: None,
            }]
        };

        criar_movimento(&mut conn, movimento_base(armazem_id, usuario_id, item())).unwrap();
        criar_movimento(&mut conn, movimento_base(armazem_id, usuario_id, item())).unwrap();

        let hash1: String = conn
            .query_row(
                "SELECT hash_integridade FROM movimentos WHERE id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let hash2: String = conn
            .query_row(
                "SELECT hash_integridade FROM movimentos WHERE id = 2",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn sugestoes_descricao_retorna_valores_distintos_da_categoria() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let itens = vec![
            MovimentoItemInput {
                categoria: "scooter".into(),
                descricao: Some("HE-15 GREEN".into()),
                montagem: None,
                condicao: None,
                quantidade: 1,
                observacao: None,
            },
            MovimentoItemInput {
                categoria: "peca".into(),
                descricao: Some("Retrovisor".into()),
                montagem: None,
                condicao: Some("boa".into()),
                quantidade: 1,
                observacao: None,
            },
        ];
        criar_movimento(&mut conn, movimento_base(armazem_id, usuario_id, itens)).unwrap();

        let sugestoes = sugestoes_descricao(&conn, "scooter").unwrap();
        assert_eq!(sugestoes, vec!["HE-15 GREEN".to_string()]);
    }
}
