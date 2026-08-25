use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::auth::buscar_usuario_ativo;
use super::errors::{AppError, AppResult};
use super::movimentos::{listar_movimentos_do_dia, Movimento};

#[derive(Debug, Serialize)]
pub struct Fechamento {
    pub id: i64,
    pub armazem_id: i64,
    pub fluxo: String,
    pub data: String,
    pub usuario_id: i64,
    pub usuario_nome: String,
    pub total_itens: i64,
    pub hash_integridade: String,
    pub criado_em: String,
    /// Soma dos itens de estornos lancados para este dia (calculada na hora
    /// da consulta, nunca gravada em `fechamentos` - o registro do
    /// fechamento em si nunca e editado).
    pub total_estornado: i64,
    pub total_liquido: i64,
}

/// Soma os itens de todo movimento que estorna algo deste
/// armazem/fluxo/data - usado para mostrar o total ja considerando
/// correcoes lancadas depois do fechamento, sem tocar no registro original.
fn calcular_total_estornado(
    conn: &Connection,
    armazem_id: i64,
    fluxo: &str,
    data: &str,
) -> AppResult<i64> {
    let total: Option<i64> = conn.query_row(
        "SELECT SUM(mi.quantidade)
         FROM movimentos m JOIN movimento_itens mi ON mi.movimento_id = m.id
         WHERE m.armazem_id = ?1 AND m.fluxo = ?2 AND m.data = ?3 AND m.estornado_de IS NOT NULL",
        params![armazem_id, fluxo, data],
        |r| r.get(0),
    )?;
    Ok(total.unwrap_or(0))
}

fn calcular_hash_fechamento(
    armazem_id: i64,
    fluxo: &str,
    data: &str,
    movimentos: &[Movimento],
) -> String {
    let cadeia_movimentos: String = movimentos
        .iter()
        .map(|m| m.hash_integridade.as_str())
        .collect::<Vec<_>>()
        .join(",");

    let conteudo = format!("{armazem_id}|{fluxo}|{data}|{cadeia_movimentos}");
    let mut hasher = Sha256::new();
    hasher.update(conteudo.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Busca o fechamento do dia, se ja existir. `None` significa que o dia
/// ainda esta aberto (lancamentos podem ser adicionados normalmente).
pub fn buscar_fechamento(
    conn: &Connection,
    armazem_id: i64,
    fluxo: &str,
    data: &str,
) -> AppResult<Option<Fechamento>> {
    let resultado: Option<Fechamento> = conn
        .query_row(
            "SELECT f.id, f.armazem_id, f.fluxo, f.data, f.usuario_id, u.nome, f.total_itens,
                    f.hash_integridade, f.criado_em
             FROM fechamentos f JOIN usuarios u ON u.id = f.usuario_id
             WHERE f.armazem_id = ?1 AND f.fluxo = ?2 AND f.data = ?3",
            params![armazem_id, fluxo, data],
            |r| {
                Ok(Fechamento {
                    id: r.get(0)?,
                    armazem_id: r.get(1)?,
                    fluxo: r.get(2)?,
                    data: r.get(3)?,
                    usuario_id: r.get(4)?,
                    usuario_nome: r.get(5)?,
                    total_itens: r.get(6)?,
                    hash_integridade: r.get(7)?,
                    criado_em: r.get(8)?,
                    total_estornado: 0,
                    total_liquido: 0,
                })
            },
        )
        .optional()?;

    let Some(mut fechamento) = resultado else {
        return Ok(None);
    };

    fechamento.total_estornado = calcular_total_estornado(conn, armazem_id, fluxo, data)?;
    fechamento.total_liquido = fechamento.total_itens - fechamento.total_estornado;

    Ok(Some(fechamento))
}

/// Fecha o dia: trava (`status = 'fechado'`) todos os lancamentos abertos do
/// armazem/fluxo/data e grava um resumo auditavel. Depois disso,
/// `movimentos::criar_movimento` passa a rejeitar novos lancamentos para o
/// mesmo armazem/fluxo/data.
pub fn fechar_dia(
    conn: &mut Connection,
    armazem_id: i64,
    fluxo: &str,
    data: &str,
    usuario_id: i64,
) -> AppResult<Fechamento> {
    let usuario = buscar_usuario_ativo(conn, usuario_id)?;
    if usuario.papel != "gestor" {
        return Err(AppError::Validation(
            "Somente um gestor pode fechar o dia.".into(),
        ));
    }
    if let Some(armazem_do_usuario) = usuario.armazem_id {
        if armazem_do_usuario != armazem_id {
            return Err(AppError::Validation(
                "Voce nao pode fechar o dia de outro armazem.".into(),
            ));
        }
    }

    if buscar_fechamento(conn, armazem_id, fluxo, data)?.is_some() {
        return Err(AppError::Validation("Este dia ja foi fechado.".into()));
    }

    let movimentos = listar_movimentos_do_dia(conn, armazem_id, fluxo, data)?;
    if movimentos.is_empty() {
        return Err(AppError::Validation(
            "Nao ha lancamentos neste dia para fechar.".into(),
        ));
    }

    // Estornos nao entram no total do dia (nem lancados antes nem depois do
    // fechamento) - eles aparecem separados em `total_estornado`, calculado
    // ao vivo em `buscar_fechamento`. Assim `total_liquido = total_itens -
    // total_estornado` fica correto nos dois casos.
    let total_itens: i64 = movimentos
        .iter()
        .filter(|m| m.estornado_de.is_none())
        .flat_map(|m| &m.itens)
        .map(|i| i.quantidade)
        .sum();
    let hash = calcular_hash_fechamento(armazem_id, fluxo, data, &movimentos);

    let tx = conn.transaction()?;

    tx.execute(
        "INSERT INTO fechamentos (armazem_id, fluxo, data, usuario_id, total_itens, hash_integridade)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![armazem_id, fluxo, data, usuario_id, total_itens, hash],
    )?;

    tx.execute(
        "UPDATE movimentos SET status = 'fechado'
         WHERE armazem_id = ?1 AND fluxo = ?2 AND data = ?3 AND status = 'aberto'",
        params![armazem_id, fluxo, data],
    )?;

    tx.commit()?;

    buscar_fechamento(conn, armazem_id, fluxo, data)?
        .ok_or_else(|| AppError::Interno("Fechamento nao encontrado logo apos ser criado.".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::domain::auth::{criar_usuario, NovoUsuario};
    use crate::domain::movimentos::{
        criar_movimento, estornar_movimento, MovimentoItemInput, NovoMovimento,
    };

    fn conexao_de_teste() -> (Connection, i64, i64) {
        let conn = db::abrir_em_memoria().unwrap();
        let armazem_id: i64 = conn
            .query_row("SELECT id FROM armazens WHERE codigo = 'A4'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let usuario_id = criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Brenda",
                login: "brenda",
                senha: "senha123",
                armazem_id: Some(armazem_id),
                papel: "gestor",
            },
        )
        .unwrap();
        (conn, armazem_id, usuario_id)
    }

    fn registrar_um_pedido(conn: &mut Connection, armazem_id: i64, usuario_id: i64, data: &str) {
        criar_movimento(
            conn,
            NovoMovimento {
                armazem_id,
                armazem_destino_id: None,
                fluxo: "saida_armazem".into(),
                tipo: "saida".into(),
                data: data.into(),
                hora: "09:00".into(),
                turno: "diurno".into(),
                usuario_id,
                numero_pedido: Some("100".into()),
                codigo_rastreio: None,
                contraparte: None,
                quem_retirou: None,
                motivo: None,
                valor_centavos: None,
                observacoes: None,
                itens: vec![MovimentoItemInput {
                    categoria: "scooter".into(),
                    descricao: None,
                    montagem: None,
                    condicao: None,
                    quantidade: 2,
                    observacao: None,
                }],
            },
        )
        .unwrap();
    }

    #[test]
    fn fecha_o_dia_e_trava_novos_lancamentos() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        registrar_um_pedido(&mut conn, armazem_id, usuario_id, "2026-08-25");

        let fechamento = fechar_dia(
            &mut conn,
            armazem_id,
            "saida_armazem",
            "2026-08-25",
            usuario_id,
        )
        .expect("deveria fechar o dia");
        assert_eq!(fechamento.total_itens, 2);

        let resultado = criar_movimento(
            &mut conn,
            NovoMovimento {
                armazem_id,
                armazem_destino_id: None,
                fluxo: "saida_armazem".into(),
                tipo: "saida".into(),
                data: "2026-08-25".into(),
                hora: "10:00".into(),
                turno: "diurno".into(),
                usuario_id,
                numero_pedido: Some("101".into()),
                codigo_rastreio: None,
                contraparte: None,
                quem_retirou: None,
                motivo: None,
                valor_centavos: None,
                observacoes: None,
                itens: vec![MovimentoItemInput {
                    categoria: "scooter".into(),
                    descricao: None,
                    montagem: None,
                    condicao: None,
                    quantidade: 1,
                    observacao: None,
                }],
            },
        );
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn rejeita_fechar_dia_duas_vezes() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        registrar_um_pedido(&mut conn, armazem_id, usuario_id, "2026-08-25");
        fechar_dia(
            &mut conn,
            armazem_id,
            "saida_armazem",
            "2026-08-25",
            usuario_id,
        )
        .unwrap();

        let resultado = fechar_dia(
            &mut conn,
            armazem_id,
            "saida_armazem",
            "2026-08-25",
            usuario_id,
        );
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn rejeita_fechar_dia_sem_lancamentos() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let resultado = fechar_dia(
            &mut conn,
            armazem_id,
            "saida_armazem",
            "2026-08-25",
            usuario_id,
        );
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn nao_afeta_outro_dia_ou_outro_fluxo() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        registrar_um_pedido(&mut conn, armazem_id, usuario_id, "2026-08-25");
        registrar_um_pedido(&mut conn, armazem_id, usuario_id, "2026-08-26");
        fechar_dia(
            &mut conn,
            armazem_id,
            "saida_armazem",
            "2026-08-25",
            usuario_id,
        )
        .unwrap();

        // dia seguinte continua aberto
        registrar_um_pedido(&mut conn, armazem_id, usuario_id, "2026-08-26");
        let lista =
            listar_movimentos_do_dia(&conn, armazem_id, "saida_armazem", "2026-08-26").unwrap();
        assert_eq!(lista.len(), 2);

        assert!(
            buscar_fechamento(&conn, armazem_id, "saida_armazem", "2026-08-26")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn conferente_nao_pode_fechar_o_dia() {
        let (mut conn, armazem_id, _gestor_id) = conexao_de_teste();
        let conferente_id = criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Karol",
                login: "karol",
                senha: "senha123",
                armazem_id: Some(armazem_id),
                papel: "conferente",
            },
        )
        .unwrap();
        registrar_um_pedido(&mut conn, armazem_id, conferente_id, "2026-08-25");

        let resultado = fechar_dia(
            &mut conn,
            armazem_id,
            "saida_armazem",
            "2026-08-25",
            conferente_id,
        );
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn gestor_de_outro_armazem_nao_pode_fechar_o_dia() {
        let (mut conn, armazem_a4, usuario_id) = conexao_de_teste();
        registrar_um_pedido(&mut conn, armazem_a4, usuario_id, "2026-08-25");

        let armazem_b2: i64 = conn
            .query_row("SELECT id FROM armazens WHERE codigo = 'B2'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let gestor_b2 = criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Geson",
                login: "geson",
                senha: "senha123",
                armazem_id: Some(armazem_b2),
                papel: "gestor",
            },
        )
        .unwrap();

        let resultado = fechar_dia(
            &mut conn,
            armazem_a4,
            "saida_armazem",
            "2026-08-25",
            gestor_b2,
        );
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn total_liquido_desconta_estorno_lancado_apos_o_fechamento() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        registrar_um_pedido(&mut conn, armazem_id, usuario_id, "2026-08-25");
        let lista =
            listar_movimentos_do_dia(&conn, armazem_id, "saida_armazem", "2026-08-25").unwrap();
        let original_id = lista[0].id;

        fechar_dia(
            &mut conn,
            armazem_id,
            "saida_armazem",
            "2026-08-25",
            usuario_id,
        )
        .unwrap();

        estornar_movimento(&mut conn, original_id, usuario_id, "pedido duplicado").unwrap();

        let fechamento = buscar_fechamento(&conn, armazem_id, "saida_armazem", "2026-08-25")
            .unwrap()
            .unwrap();
        assert_eq!(fechamento.total_itens, 2);
        assert_eq!(fechamento.total_estornado, 2);
        assert_eq!(fechamento.total_liquido, 0);
    }
}
