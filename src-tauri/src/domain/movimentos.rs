use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::auth::buscar_usuario_ativo;
use super::errors::{AppError, AppResult};

/// "outro" e uma valvula de escape deliberada, nao um catalogo aberto: ainda
/// e uma lista curta e fixa, so que agora cobre o caso raro que nao se encaixa
/// nas outras 4 - exige observacao preenchida (ver `validar_novo_movimento`),
/// entao sempre fica registrado o que era de fato.
const CATEGORIAS_VALIDAS: [&str; 5] = ["scooter", "triciclo", "patinete", "peca", "outro"];
const FLUXOS_VALIDOS: [&str; 4] = ["saida_armazem", "peca_montagem", "sac", "reparo_externo"];
const TIPOS_VALIDOS: [&str; 2] = ["entrada", "saida"];
const TURNOS_VALIDOS: [&str; 2] = ["diurno", "noturno"];
const MONTAGENS_VALIDAS: [&str; 2] = ["montado", "caixa"];
const CONDICOES_VALIDAS: [&str; 4] = ["boa", "defeito", "sucata", "outro"];
const MOTIVOS_SAC_ENTRADA_VALIDOS: [&str; 3] = ["garantia", "venda", "outro"];
/// Saida do SAC: peca entregue de volta ao cliente (consertada, trocada),
/// descartada por nao ter conserto, ou resolvida como garantia/venda (troca
/// por peca nova sob garantia, ou vendida como reposicao) - nao existe
/// "devolvida ao fabricante" hoje, confirmado com o cliente.
const MOTIVOS_SAC_SAIDA_VALIDOS: [&str; 5] = ["entregue", "descarte", "garantia", "venda", "outro"];
const TEXTO_LIVRE_MAX: usize = 500;
const QUANTIDADE_MAX: i64 = 100_000;

#[derive(Debug, Deserialize)]
pub struct MovimentoItemInput {
    pub categoria: String,
    pub descricao: Option<String>,
    pub montagem: Option<String>,
    pub condicao: Option<String>,
    pub quantidade: i64,
    pub observacao: Option<String>,
    /// Preenchido so quando este item veio de uma confirmacao de
    /// recebimento (`commands::sync_commands::confirmar_recebimento`) -
    /// guarda quanto o remetente registrou, pra comparar com `quantidade`
    /// (quanto realmente chegou). `None` em todo lancamento normal.
    pub quantidade_enviada: Option<i64>,
    /// Codigo/serie do componente (bateria, motor, modulo) - obrigatorio
    /// quando `fluxo == "reparo_externo"`, usado para casar a saida pro
    /// tecnico externo com a entrada de retorno (ver `buscar_reparos_em_aberto`).
    /// `None` nos outros fluxos.
    pub codigo_componente: Option<String>,
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
    /// Preenchidos so quando este movimento e a confirmacao de recebimento de
    /// uma transferencia vinda do outro armazem - identifica a linha original
    /// em `movimentos_consolidados` no Turso (ver `db::sync`). `None` no caso
    /// normal (lancamento local comum).
    pub recebido_de_armazem_codigo: Option<String>,
    pub recebido_de_id_origem: Option<i64>,
    /// So faz sentido pra `saida_armazem`/`saida`: o cliente retirou tudo
    /// (`true`, padrao) ou so parte do pedido, voltando outro dia buscar o
    /// resto (`false`). Generico nos outros fluxos (sempre `true`).
    pub retirada_completa: bool,
    pub itens: Vec<MovimentoItemInput>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MovimentoItem {
    pub id: i64,
    pub categoria: String,
    pub descricao: Option<String>,
    pub montagem: Option<String>,
    pub condicao: Option<String>,
    pub quantidade: i64,
    pub observacao: Option<String>,
    pub quantidade_enviada: Option<i64>,
    pub codigo_componente: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Movimento {
    pub id: i64,
    pub numero: i64,
    pub armazem_id: i64,
    pub armazem_destino_id: Option<i64>,
    pub fluxo: String,
    pub tipo: String,
    pub data: String,
    pub hora: String,
    pub turno: String,
    pub usuario_id: i64,
    pub usuario_nome: String,
    pub numero_pedido: Option<String>,
    pub codigo_rastreio: Option<String>,
    pub contraparte: Option<String>,
    pub quem_retirou: Option<String>,
    pub motivo: Option<String>,
    pub valor_centavos: Option<i64>,
    pub observacoes: Option<String>,
    pub status: String,
    pub estornado_de: Option<i64>,
    pub recebido_de_armazem_codigo: Option<String>,
    pub recebido_de_id_origem: Option<i64>,
    pub retirada_completa: bool,
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

fn validar_texto_livre(campo: &str, valor: Option<&str>) -> AppResult<()> {
    if let Some(v) = valor {
        if v.chars().count() > TEXTO_LIVRE_MAX {
            return Err(AppError::Validation(format!(
                "{campo} nao pode passar de {TEXTO_LIVRE_MAX} caracteres."
            )));
        }
    }
    Ok(())
}

/// "outro" (categoria/condicao/motivo) so e aceito com um texto livre
/// explicando o que era de fato - senao a palavra sozinha nao diz nada pra
/// quem le o historico/impressao depois. `campo_erro` e o nome do campo que
/// deveria ter o detalhe (aparece na mensagem de erro).
fn exigir_detalhe_para_outro(detalhe: Option<&str>, campo_erro: &str) -> AppResult<()> {
    match detalhe {
        Some(d) if !d.trim().is_empty() => Ok(()),
        _ => Err(AppError::Validation(format!(
            "Descreva {campo_erro} quando escolher 'Outro'."
        ))),
    }
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
    if !TURNOS_VALIDOS.contains(&novo.turno.as_str()) {
        return Err(AppError::Validation(format!(
            "Turno invalido: {}",
            novo.turno
        )));
    }
    validar_texto_livre("Numero do pedido", novo.numero_pedido.as_deref())?;
    validar_texto_livre("Codigo de rastreio", novo.codigo_rastreio.as_deref())?;
    validar_texto_livre("Coleta/contraparte", novo.contraparte.as_deref())?;
    validar_texto_livre("Quem retirou", novo.quem_retirou.as_deref())?;
    validar_texto_livre("Motivo", novo.motivo.as_deref())?;
    validar_texto_livre("Observacoes", novo.observacoes.as_deref())?;

    if novo.fluxo == "sac" {
        // Entrada = devolucao do cliente (garantia/venda); saida = destino da
        // peca depois do atendimento (entregue de volta ao cliente ou
        // descartada) - cada tipo tem seu proprio conjunto de motivos, nao
        // faz sentido por exemplo uma "entrada" com motivo "descarte".
        let motivos_validos = if novo.tipo == "saida" {
            &MOTIVOS_SAC_SAIDA_VALIDOS[..]
        } else {
            &MOTIVOS_SAC_ENTRADA_VALIDOS[..]
        };
        match novo.motivo.as_deref() {
            Some(motivo) if motivos_validos.contains(&motivo) => {}
            _ if novo.tipo == "saida" => {
                return Err(AppError::Validation(
                    "Informe o motivo da saida do SAC: entregue ao cliente, descarte, garantia, venda ou outro.".into(),
                ));
            }
            _ => {
                return Err(AppError::Validation(
                    "Informe o motivo do SAC: garantia, venda ou outro.".into(),
                ));
            }
        }
        if novo.motivo.as_deref() == Some("venda") {
            match novo.valor_centavos {
                Some(v) if v > 0 => {}
                _ => {
                    return Err(AppError::Validation(
                        "Informe o valor da venda (maior que zero).".into(),
                    ));
                }
            }
        }
        if novo.motivo.as_deref() == Some("outro") {
            exigir_detalhe_para_outro(novo.observacoes.as_deref(), "o motivo nas observacoes")?;
        }
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
        if item.quantidade <= 0 || item.quantidade > QUANTIDADE_MAX {
            return Err(AppError::Validation(format!(
                "A quantidade de cada item precisa ser maior que zero e ate {QUANTIDADE_MAX}."
            )));
        }
        if let Some(montagem) = item.montagem.as_deref() {
            if !MONTAGENS_VALIDAS.contains(&montagem) {
                return Err(AppError::Validation(format!(
                    "Montagem invalida: {montagem}"
                )));
            }
        }
        if let Some(condicao) = item.condicao.as_deref() {
            if !CONDICOES_VALIDAS.contains(&condicao) {
                return Err(AppError::Validation(format!(
                    "Condicao invalida: {condicao}"
                )));
            }
        } else if novo.fluxo == "peca_montagem"
            || (novo.fluxo == "reparo_externo" && novo.tipo == "entrada")
        {
            return Err(AppError::Validation(
                "Informe a condicao da peca: boa, defeito, sucata ou outro.".into(),
            ));
        }
        validar_texto_livre("Descricao do item", item.descricao.as_deref())?;
        validar_texto_livre("Observacao do item", item.observacao.as_deref())?;
        validar_texto_livre("Codigo do componente", item.codigo_componente.as_deref())?;
        if item.categoria == "outro" || item.condicao.as_deref() == Some("outro") {
            exigir_detalhe_para_outro(item.observacao.as_deref(), "o item na observacao")?;
        }
        if novo.fluxo == "reparo_externo" {
            match item.codigo_componente.as_deref() {
                Some(codigo) if !codigo.trim().is_empty() => {}
                _ => {
                    return Err(AppError::Validation(
                        "Informe o codigo/serie do componente (bateria, motor, modulo).".into(),
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Usado por `commands::sync_commands::confirmar_recebimento` pra transformar
/// os itens de uma transferencia (o que o remetente registrou) nos itens do
/// movimento de entrada local, deixando o conferente que recebe informar a
/// quantidade realmente chegada. Nunca confia no frontend pra isso: rejeita
/// se a quantidade recebida de qualquer item for maior que a enviada (nao da
/// pra "receber" mais do que foi de fato mandado) ou <= 0, e rejeita se a
/// lista de quantidades nao tiver exatamente um valor por item enviado.
/// Quantidade recebida menor que a enviada e aceita normalmente (divergencia
/// legitima) - fica registrada em `quantidade` (recebida) vs
/// `quantidade_enviada` (o que foi mandado) pra auditoria/painel.
pub fn validar_quantidades_recebidas(
    enviados: &[MovimentoItem],
    recebidos: &[i64],
) -> AppResult<Vec<MovimentoItemInput>> {
    if enviados.len() != recebidos.len() {
        return Err(AppError::Validation(
            "A lista de quantidades recebidas nao bate com os itens da transferencia.".into(),
        ));
    }

    enviados
        .iter()
        .zip(recebidos.iter())
        .map(|(item, &recebido)| {
            if recebido <= 0 || recebido > item.quantidade {
                return Err(AppError::Validation(format!(
                    "Quantidade recebida invalida para {} (enviado: {}).",
                    item.descricao.as_deref().unwrap_or(&item.categoria),
                    item.quantidade
                )));
            }
            Ok(MovimentoItemInput {
                categoria: item.categoria.clone(),
                descricao: item.descricao.clone(),
                montagem: item.montagem.clone(),
                condicao: item.condicao.clone(),
                quantidade: recebido,
                observacao: item.observacao.clone(),
                quantidade_enviada: Some(item.quantidade),
                codigo_componente: item.codigo_componente.clone(),
            })
        })
        .collect()
}

fn buscar_armazem_ativo(conn: &Connection, armazem_id: i64) -> AppResult<()> {
    let ativo: Option<bool> = conn
        .query_row(
            "SELECT ativo FROM armazens WHERE id = ?1",
            params![armazem_id],
            |r| r.get(0),
        )
        .optional()?;

    match ativo {
        Some(true) => Ok(()),
        Some(false) => Err(AppError::Validation("Armazem esta inativo.".into())),
        None => Err(AppError::Validation("Armazem nao encontrado.".into())),
    }
}

/// Confere que quem esta registrando o movimento existe, esta ativo, e (se
/// tiver um armazem fixo) so pode registrar para o proprio armazem. Tambem
/// confere que o(s) armazem(ns) envolvidos existem e estao ativos. Chamada em
/// todo ponto de escrita (`criar_movimento`, `estornar_movimento`).
pub(crate) fn autorizar_movimento(
    conn: &Connection,
    usuario_id: i64,
    armazem_id: i64,
) -> AppResult<()> {
    let usuario = buscar_usuario_ativo(conn, usuario_id)?;
    if let Some(armazem_do_usuario) = usuario.armazem_id {
        if armazem_do_usuario != armazem_id {
            return Err(AppError::Validation(
                "Voce nao pode registrar movimentos para outro armazem.".into(),
            ));
        }
    }
    buscar_armazem_ativo(conn, armazem_id)?;
    Ok(())
}

/// Confere que quem esta consultando existe, esta ativo, e (se tiver um
/// armazem fixo) so pode consultar dados do proprio armazem. Usada em todo
/// comando de leitura que recebe `armazem_id` (`listar_movimentos_do_dia`,
/// `buscar_historico`, `buscar_fechamento_do_dia`) - ao contrario de
/// `autorizar_movimento`, nao confere "armazem ativo": consultar o
/// historico de um armazem desativado ainda e uma leitura legitima.
pub fn autorizar_leitura(conn: &Connection, usuario_id: i64, armazem_id: i64) -> AppResult<()> {
    let usuario = buscar_usuario_ativo(conn, usuario_id)?;
    if let Some(armazem_do_usuario) = usuario.armazem_id {
        if armazem_do_usuario != armazem_id {
            return Err(AppError::Validation(
                "Voce nao pode consultar dados de outro armazem.".into(),
            ));
        }
    }
    Ok(())
}

struct ItemHash {
    categoria: String,
    descricao: Option<String>,
    montagem: Option<String>,
    condicao: Option<String>,
    quantidade: i64,
    observacao: Option<String>,
    quantidade_enviada: Option<i64>,
    codigo_componente: Option<String>,
}

/// Tudo que entra no hash de auditoria de uma linha - precisa cobrir todo
/// campo que um `UPDATE` direto no banco poderia alterar sem deixar rastro,
/// senao a cadeia de hash (`verificar_cadeia`) nao detecta a adulteracao.
struct CamposHash {
    armazem_id: i64,
    armazem_destino_id: Option<i64>,
    fluxo: String,
    tipo: String,
    data: String,
    hora: String,
    turno: String,
    usuario_id: i64,
    numero_pedido: Option<String>,
    codigo_rastreio: Option<String>,
    contraparte: Option<String>,
    quem_retirou: Option<String>,
    motivo: Option<String>,
    valor_centavos: Option<i64>,
    observacoes: Option<String>,
    estornado_de: Option<i64>,
    recebido_de_armazem_codigo: Option<String>,
    recebido_de_id_origem: Option<i64>,
    retirada_completa: bool,
    itens: Vec<ItemHash>,
}

impl CamposHash {
    fn de_novo_movimento(novo: &NovoMovimento, estornado_de: Option<i64>) -> Self {
        Self {
            armazem_id: novo.armazem_id,
            armazem_destino_id: novo.armazem_destino_id,
            fluxo: novo.fluxo.clone(),
            tipo: novo.tipo.clone(),
            data: novo.data.clone(),
            hora: novo.hora.clone(),
            turno: novo.turno.clone(),
            usuario_id: novo.usuario_id,
            numero_pedido: novo.numero_pedido.clone(),
            codigo_rastreio: novo.codigo_rastreio.clone(),
            contraparte: novo.contraparte.clone(),
            quem_retirou: novo.quem_retirou.clone(),
            motivo: novo.motivo.clone(),
            valor_centavos: novo.valor_centavos,
            observacoes: novo.observacoes.clone(),
            estornado_de,
            recebido_de_armazem_codigo: novo.recebido_de_armazem_codigo.clone(),
            recebido_de_id_origem: novo.recebido_de_id_origem,
            retirada_completa: novo.retirada_completa,
            itens: novo
                .itens
                .iter()
                .map(|i| ItemHash {
                    categoria: i.categoria.clone(),
                    descricao: i.descricao.clone(),
                    montagem: i.montagem.clone(),
                    condicao: i.condicao.clone(),
                    quantidade: i.quantidade,
                    observacao: i.observacao.clone(),
                    quantidade_enviada: i.quantidade_enviada,
                    codigo_componente: i.codigo_componente.clone(),
                })
                .collect(),
        }
    }
}

fn calcular_hash(hash_anterior: &str, campos: &CamposHash) -> String {
    let itens_resumo: Vec<String> = campos
        .itens
        .iter()
        .map(|i| {
            format!(
                "{}:{}:{}:{}:{}:{}:{}:{}",
                i.categoria,
                i.descricao.as_deref().unwrap_or(""),
                i.montagem.as_deref().unwrap_or(""),
                i.condicao.as_deref().unwrap_or(""),
                i.quantidade,
                i.observacao.as_deref().unwrap_or(""),
                i.quantidade_enviada
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                i.codigo_componente.as_deref().unwrap_or(""),
            )
        })
        .collect();

    let partes: [String; 19] = [
        campos.armazem_id.to_string(),
        campos
            .armazem_destino_id
            .map(|v| v.to_string())
            .unwrap_or_default(),
        campos.fluxo.clone(),
        campos.tipo.clone(),
        campos.data.clone(),
        campos.hora.clone(),
        campos.turno.clone(),
        campos.usuario_id.to_string(),
        campos.numero_pedido.clone().unwrap_or_default(),
        campos.codigo_rastreio.clone().unwrap_or_default(),
        campos.contraparte.clone().unwrap_or_default(),
        campos.quem_retirou.clone().unwrap_or_default(),
        campos.motivo.clone().unwrap_or_default(),
        campos
            .valor_centavos
            .map(|v| v.to_string())
            .unwrap_or_default(),
        campos.observacoes.clone().unwrap_or_default(),
        campos
            .estornado_de
            .map(|v| v.to_string())
            .unwrap_or_default(),
        campos
            .recebido_de_armazem_codigo
            .clone()
            .unwrap_or_default(),
        campos
            .recebido_de_id_origem
            .map(|v| v.to_string())
            .unwrap_or_default(),
        campos.retirada_completa.to_string(),
    ];

    let conteudo = format!(
        "{hash_anterior}|{}|{}",
        partes.join("|"),
        itens_resumo.join(";")
    );

    let mut hasher = Sha256::new();
    hasher.update(conteudo.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn ultimo_hash(tx: &Connection) -> String {
    tx.query_row(
        "SELECT hash_integridade FROM movimentos ORDER BY id DESC LIMIT 1",
        [],
        |r| r.get(0),
    )
    .unwrap_or_else(|_| "GENESIS-ECOVIVA".to_string())
}

pub fn criar_movimento(conn: &mut Connection, novo: NovoMovimento) -> AppResult<Movimento> {
    validar_novo_movimento(&novo)?;
    autorizar_movimento(conn, novo.usuario_id, novo.armazem_id)?;
    if let Some(armazem_destino_id) = novo.armazem_destino_id {
        buscar_armazem_ativo(conn, armazem_destino_id)?;
    }

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

    let hash_anterior = ultimo_hash(&tx);
    let hash = calcular_hash(&hash_anterior, &CamposHash::de_novo_movimento(&novo, None));

    tx.execute(
        "INSERT INTO movimentos (
            armazem_id, armazem_destino_id, fluxo, tipo, data, hora, turno, usuario_id,
            numero_pedido, codigo_rastreio, contraparte, quem_retirou,
            motivo, valor_centavos, observacoes, status,
            recebido_de_armazem_codigo, recebido_de_id_origem, retirada_completa, hash_integridade,
            criado_em
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 'aberto', ?16, ?17, ?18, ?19,
            datetime('now', 'localtime'))",
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
            novo.recebido_de_armazem_codigo,
            novo.recebido_de_id_origem,
            novo.retirada_completa,
            hash,
        ],
    )?;

    let movimento_id = tx.last_insert_rowid();

    {
        let mut inserir_item = tx.prepare(
            "INSERT INTO movimento_itens (movimento_id, categoria, descricao, montagem, condicao, quantidade, observacao, quantidade_enviada, codigo_componente)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
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
                item.quantidade_enviada,
                item.codigo_componente,
            ])?;
        }
    }

    tx.commit()?;

    buscar_movimento(conn, movimento_id)
}

/// Registra a correcao de um lancamento existente sem editar o original
/// (append-only): grava uma nova linha, apontando de volta para o original
/// via `estornado_de`. Nao passa pela trava de "dia fechado" de proposito -
/// e exatamente o mecanismo para corrigir um erro depois do fechamento.
pub fn estornar_movimento(
    conn: &mut Connection,
    movimento_id: i64,
    usuario_id: i64,
    justificativa: &str,
) -> AppResult<Movimento> {
    let justificativa = justificativa.trim();
    if justificativa.is_empty() {
        return Err(AppError::Validation(
            "Informe uma justificativa para o estorno.".into(),
        ));
    }
    validar_texto_livre("Justificativa", Some(justificativa))?;

    let original = buscar_movimento(conn, movimento_id)?;

    autorizar_movimento(conn, usuario_id, original.armazem_id)?;

    if original.estornado_de.is_some() {
        return Err(AppError::Validation(
            "Nao e possivel estornar um lancamento que ja e um estorno.".into(),
        ));
    }

    let tx = conn.transaction()?;

    let ja_estornado: bool = tx
        .query_row(
            "SELECT 1 FROM movimentos WHERE estornado_de = ?1",
            params![movimento_id],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if ja_estornado {
        return Err(AppError::Validation(
            "Este lancamento ja foi estornado.".into(),
        ));
    }

    let itens_originais = carregar_itens(&tx, movimento_id)?;
    let observacoes = format!("ESTORNO do lancamento #{movimento_id}: {justificativa}");

    let campos = CamposHash {
        armazem_id: original.armazem_id,
        armazem_destino_id: None,
        fluxo: original.fluxo.clone(),
        tipo: original.tipo.clone(),
        data: original.data.clone(),
        hora: original.hora.clone(),
        turno: original.turno.clone(),
        usuario_id,
        numero_pedido: original.numero_pedido.clone(),
        codigo_rastreio: None,
        contraparte: original.contraparte.clone(),
        quem_retirou: original.quem_retirou.clone(),
        motivo: None,
        valor_centavos: None,
        observacoes: Some(observacoes.clone()),
        estornado_de: Some(movimento_id),
        recebido_de_armazem_codigo: None,
        recebido_de_id_origem: None,
        retirada_completa: original.retirada_completa,
        itens: itens_originais
            .iter()
            .map(|i| ItemHash {
                categoria: i.categoria.clone(),
                descricao: i.descricao.clone(),
                montagem: i.montagem.clone(),
                condicao: i.condicao.clone(),
                quantidade: i.quantidade,
                observacao: i.observacao.clone(),
                quantidade_enviada: i.quantidade_enviada,
                codigo_componente: i.codigo_componente.clone(),
            })
            .collect(),
    };

    let hash_anterior = ultimo_hash(&tx);
    let hash = calcular_hash(&hash_anterior, &campos);

    tx.execute(
        "INSERT INTO movimentos (
            armazem_id, armazem_destino_id, fluxo, tipo, data, hora, turno, usuario_id,
            numero_pedido, codigo_rastreio, contraparte, quem_retirou,
            motivo, valor_centavos, observacoes, status, estornado_de, retirada_completa, hash_integridade,
            criado_em
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 'estorno', ?16, ?17, ?18,
            datetime('now', 'localtime'))",
        params![
            original.armazem_id,
            None::<i64>,
            original.fluxo,
            original.tipo,
            original.data,
            original.hora,
            original.turno,
            usuario_id,
            original.numero_pedido,
            None::<String>,
            original.contraparte,
            original.quem_retirou,
            None::<String>,
            None::<i64>,
            observacoes,
            movimento_id,
            original.retirada_completa,
            hash,
        ],
    )?;

    let estorno_id = tx.last_insert_rowid();

    {
        let mut inserir_item = tx.prepare(
            "INSERT INTO movimento_itens (movimento_id, categoria, descricao, montagem, condicao, quantidade, observacao, quantidade_enviada, codigo_componente)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        for item in &itens_originais {
            inserir_item.execute(params![
                estorno_id,
                item.categoria,
                item.descricao,
                item.montagem,
                item.condicao,
                item.quantidade,
                item.observacao,
                item.quantidade_enviada,
                item.codigo_componente,
            ])?;
        }
    }

    tx.commit()?;

    buscar_movimento(conn, estorno_id)
}

#[derive(Debug, Serialize, PartialEq)]
pub struct QuebraCadeia {
    pub movimento_id: i64,
    pub numero_pedido: Option<String>,
}

/// Percorre todos os movimentos em ordem e recalcula o hash de cada um a
/// partir do hash *armazenado* da linha anterior, comparando com o hash
/// gravado naquela linha. Detecta qualquer alteracao feita diretamente no
/// banco (fora do fluxo normal de insercao) sem precisar de trigger de SQL.
/// `None` significa cadeia intacta; `Some` aponta a primeira linha divergente.
pub fn verificar_cadeia(conn: &Connection) -> AppResult<Option<QuebraCadeia>> {
    let mut stmt = conn.prepare(
        "SELECT id, armazem_id, armazem_destino_id, fluxo, tipo, data, hora, turno, usuario_id,
                numero_pedido, codigo_rastreio, contraparte, quem_retirou, motivo,
                valor_centavos, observacoes, estornado_de, recebido_de_armazem_codigo,
                recebido_de_id_origem, retirada_completa, hash_integridade
         FROM movimentos ORDER BY id ASC",
    )?;

    struct Linha {
        id: i64,
        numero_pedido: Option<String>,
        hash_integridade: String,
        campos: CamposHash,
    }

    let linhas = stmt
        .query_map([], |r| {
            let id: i64 = r.get(0)?;
            Ok(Linha {
                id,
                numero_pedido: r.get(9)?,
                hash_integridade: r.get(20)?,
                campos: CamposHash {
                    armazem_id: r.get(1)?,
                    armazem_destino_id: r.get(2)?,
                    fluxo: r.get(3)?,
                    tipo: r.get(4)?,
                    data: r.get(5)?,
                    hora: r.get(6)?,
                    turno: r.get(7)?,
                    usuario_id: r.get(8)?,
                    numero_pedido: r.get(9)?,
                    codigo_rastreio: r.get(10)?,
                    contraparte: r.get(11)?,
                    quem_retirou: r.get(12)?,
                    motivo: r.get(13)?,
                    valor_centavos: r.get(14)?,
                    observacoes: r.get(15)?,
                    estornado_de: r.get(16)?,
                    recebido_de_armazem_codigo: r.get(17)?,
                    recebido_de_id_origem: r.get(18)?,
                    retirada_completa: r.get(19)?,
                    itens: Vec::new(),
                },
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut hash_anterior = "GENESIS-ECOVIVA".to_string();
    for mut linha in linhas {
        linha.campos.itens = carregar_itens(conn, linha.id)?
            .into_iter()
            .map(|i| ItemHash {
                categoria: i.categoria,
                descricao: i.descricao,
                montagem: i.montagem,
                condicao: i.condicao,
                quantidade: i.quantidade,
                observacao: i.observacao,
                quantidade_enviada: i.quantidade_enviada,
                codigo_componente: i.codigo_componente,
            })
            .collect();

        let esperado = calcular_hash(&hash_anterior, &linha.campos);
        if esperado != linha.hash_integridade {
            return Ok(Some(QuebraCadeia {
                movimento_id: linha.id,
                numero_pedido: linha.numero_pedido,
            }));
        }
        hash_anterior = linha.hash_integridade;
    }

    Ok(None)
}

pub(crate) fn carregar_itens(
    conn: &Connection,
    movimento_id: i64,
) -> AppResult<Vec<MovimentoItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, categoria, descricao, montagem, condicao, quantidade, observacao, quantidade_enviada, codigo_componente
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
                quantidade_enviada: r.get(7)?,
                codigo_componente: r.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(itens)
}

/// Lista de colunas compartilhada por toda consulta que devolve `Movimento`
/// (`buscar_movimento`/`listar_movimentos_do_dia`/`buscar_historico`) - as
/// tres liam a mesma coisa com a lista repetida (e uma delas, `id` fora da
/// lista por vir do parametro), risco real de uma migration nova de coluna
/// atualizar so 2 das 3 sem ninguem notar. `m.id` sempre primeiro, na mesma
/// ordem que `mapear_movimento` espera.
const COLUNAS_MOVIMENTO: &str =
    "m.id, m.armazem_id, m.armazem_destino_id, m.fluxo, m.tipo, m.data, m.hora,
                m.turno, m.usuario_id, u.nome, m.numero_pedido, m.codigo_rastreio, m.contraparte,
                m.quem_retirou, m.motivo, m.valor_centavos, m.observacoes, m.status,
                m.estornado_de, m.recebido_de_armazem_codigo, m.recebido_de_id_origem,
                m.retirada_completa, m.hash_integridade";

fn mapear_movimento(r: &Row) -> rusqlite::Result<Movimento> {
    Ok(Movimento {
        id: r.get(0)?,
        numero: 0,
        armazem_id: r.get(1)?,
        armazem_destino_id: r.get(2)?,
        fluxo: r.get(3)?,
        tipo: r.get(4)?,
        data: r.get(5)?,
        hora: r.get(6)?,
        turno: r.get(7)?,
        usuario_id: r.get(8)?,
        usuario_nome: r.get(9)?,
        numero_pedido: r.get(10)?,
        codigo_rastreio: r.get(11)?,
        contraparte: r.get(12)?,
        quem_retirou: r.get(13)?,
        motivo: r.get(14)?,
        valor_centavos: r.get(15)?,
        observacoes: r.get(16)?,
        status: r.get(17)?,
        estornado_de: r.get(18)?,
        recebido_de_armazem_codigo: r.get(19)?,
        recebido_de_id_origem: r.get(20)?,
        retirada_completa: r.get(21)?,
        hash_integridade: r.get(22)?,
        itens: Vec::new(),
    })
}

pub fn buscar_movimento(conn: &Connection, id: i64) -> AppResult<Movimento> {
    let encontrado = conn
        .query_row(
            &format!(
                "SELECT {COLUNAS_MOVIMENTO}
                 FROM movimentos m JOIN usuarios u ON u.id = m.usuario_id
                 WHERE m.id = ?1"
            ),
            params![id],
            mapear_movimento,
        )
        .optional()?;

    let mut movimento =
        encontrado.ok_or_else(|| AppError::Validation("Lancamento nao encontrado.".into()))?;
    movimento.itens = carregar_itens(conn, id)?;

    Ok(movimento)
}

pub fn listar_movimentos_do_dia(
    conn: &Connection,
    armazem_id: i64,
    fluxo: &str,
    data: &str,
) -> AppResult<Vec<Movimento>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUNAS_MOVIMENTO}
         FROM movimentos m JOIN usuarios u ON u.id = m.usuario_id
         WHERE m.armazem_id = ?1 AND m.fluxo = ?2 AND m.data = ?3
         ORDER BY m.id ASC"
    ))?;

    let mut movimentos = stmt
        .query_map(params![armazem_id, fluxo, data], mapear_movimento)?
        .collect::<Result<Vec<_>, _>>()?;

    for (indice, movimento) in movimentos.iter_mut().enumerate() {
        movimento.numero = indice as i64 + 1;
        movimento.itens = carregar_itens(conn, movimento.id)?;
    }

    Ok(movimentos)
}

const LIMITE_HISTORICO: i64 = 500;

/// Escapa `\`, `%` e `_` (nessa ordem) pra um termo de busca usado num LIKE
/// com `ESCAPE '\'` - sem isso, um usuario buscando um pedido com `%`/`_` no
/// meio (ex. "50_1") casaria como curinga em vez de texto literal.
fn escapar_curinga_like(termo: &str) -> String {
    termo
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Uma pagina de `buscar_historico`: os movimentos da pagina e se ha mais
/// alem dela. `tem_mais` vem de pedir uma linha a mais que o limite ao banco
/// (`LIMITE_HISTORICO + 1`) e conferir se ela veio, em vez de um `COUNT(*)`
/// separado - uma consulta so.
#[derive(Debug, Serialize)]
pub struct ResultadoHistorico {
    pub movimentos: Vec<Movimento>,
    pub tem_mais: bool,
}

/// Busca lancamentos de qualquer dia (nao so hoje), com filtros opcionais de
/// intervalo de data, cliente/coleta (`contraparte`, busca parcial) e numero do
/// pedido (busca parcial). Usada pela aba de Historico. `armazem_id`/`fluxo`
/// continuam obrigatorios - a busca nunca cruza armazem nem fluxo. Pagina de
/// ate `LIMITE_HISTORICO` linhas por vez, mais recentes primeiro; `offset`
/// pula as paginas ja carregadas (frontend acumula localmente ao "Carregar
/// mais", nao troca a pagina anterior).
#[allow(clippy::too_many_arguments)]
pub fn buscar_historico(
    conn: &Connection,
    armazem_id: i64,
    fluxo: &str,
    data_inicio: Option<&str>,
    data_fim: Option<&str>,
    cliente: Option<&str>,
    numero_pedido: Option<&str>,
    offset: i64,
) -> AppResult<ResultadoHistorico> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUNAS_MOVIMENTO}
         FROM movimentos m JOIN usuarios u ON u.id = m.usuario_id
         WHERE m.armazem_id = ?1 AND m.fluxo = ?2
           AND (?3 IS NULL OR m.data >= ?3)
           AND (?4 IS NULL OR m.data <= ?4)
           AND (?5 IS NULL OR m.contraparte LIKE '%' || ?5 || '%' ESCAPE '\\')
           AND (?6 IS NULL OR m.numero_pedido LIKE '%' || ?6 || '%' ESCAPE '\\')
         ORDER BY m.data DESC, m.hora DESC, m.id DESC
         LIMIT ?7 OFFSET ?8"
    ))?;

    let cliente_escapado = cliente.map(escapar_curinga_like);
    let numero_pedido_escapado = numero_pedido.map(escapar_curinga_like);

    let mut movimentos = stmt
        .query_map(
            params![
                armazem_id,
                fluxo,
                data_inicio,
                data_fim,
                cliente_escapado,
                numero_pedido_escapado,
                LIMITE_HISTORICO + 1,
                offset
            ],
            mapear_movimento,
        )?
        .collect::<Result<Vec<_>, _>>()?;

    let tem_mais = movimentos.len() as i64 > LIMITE_HISTORICO;
    movimentos.truncate(LIMITE_HISTORICO as usize);

    for movimento in movimentos.iter_mut() {
        movimento.itens = carregar_itens(conn, movimento.id)?;
    }

    Ok(ResultadoHistorico {
        movimentos,
        tem_mais,
    })
}

/// Busca a retirada mais recente (por data/hora) desse `numero_pedido` nesse
/// armazem/fluxo, ignorando estornos - e devolve so se ela ainda estiver
/// marcada como parcial (`retirada_completa = false`). Usada pra avisar o
/// conferente, ao digitar o numero do pedido, que ha uma retirada anterior
/// aguardando complemento. Se a retirada mais recente ja for completa (uma
/// visita posterior resolveu a pendencia), devolve `None` - nao ha estado
/// mutavel de "resolvido", so a comparacao com a entrada mais nova.
pub fn buscar_retirada_parcial_pendente(
    conn: &Connection,
    armazem_id: i64,
    fluxo: &str,
    numero_pedido: &str,
) -> AppResult<Option<Movimento>> {
    let id: Option<i64> = conn
        .query_row(
            "SELECT m.id FROM movimentos m
             WHERE m.armazem_id = ?1 AND m.fluxo = ?2 AND m.numero_pedido = ?3
               AND m.tipo = 'saida' AND m.estornado_de IS NULL
               AND NOT EXISTS (SELECT 1 FROM movimentos x WHERE x.estornado_de = m.id)
             ORDER BY m.data DESC, m.hora DESC, m.id DESC
             LIMIT 1",
            params![armazem_id, fluxo, numero_pedido],
            |r| r.get(0),
        )
        .optional()?;

    let Some(id) = id else {
        return Ok(None);
    };

    let movimento = buscar_movimento(conn, id)?;
    if movimento.retirada_completa {
        Ok(None)
    } else {
        Ok(Some(movimento))
    }
}

/// Um item enviado para reparo externo (`fluxo = "reparo_externo"`, saida)
/// que ainda nao tem uma entrada correspondente com o mesmo
/// `codigo_componente` no mesmo armazem - ver `buscar_reparos_em_aberto`.
#[derive(Debug, Serialize)]
pub struct ReparoPendente {
    pub movimento_id: i64,
    pub item_id: i64,
    pub codigo_componente: String,
    pub categoria: String,
    pub descricao: Option<String>,
    pub quantidade: i64,
    pub contraparte: Option<String>,
    pub data: String,
    pub hora: String,
}

/// Lista os itens de reparo externo que ja sairam mas ainda nao voltaram -
/// isto e, nao ha nenhuma entrada (nao estornada) do mesmo armazem/fluxo com
/// o mesmo `codigo_componente`. Mesmo idioma `NOT EXISTS` de
/// `buscar_retirada_parcial_pendente`, mas listando todos os itens em aberto
/// em vez de um so numero de pedido.
pub fn buscar_reparos_em_aberto(
    conn: &Connection,
    armazem_id: i64,
) -> AppResult<Vec<ReparoPendente>> {
    let mut stmt = conn.prepare(
        "SELECT mi.id, mi.movimento_id, mi.categoria, mi.descricao, mi.quantidade,
                mi.codigo_componente, m.contraparte, m.data, m.hora
         FROM movimento_itens mi
         JOIN movimentos m ON m.id = mi.movimento_id
         WHERE m.armazem_id = ?1 AND m.fluxo = 'reparo_externo' AND m.tipo = 'saida'
           AND m.estornado_de IS NULL
           AND NOT EXISTS (SELECT 1 FROM movimentos x WHERE x.estornado_de = m.id)
           AND mi.codigo_componente IS NOT NULL AND mi.codigo_componente != ''
           AND NOT EXISTS (
             SELECT 1 FROM movimento_itens ei
             JOIN movimentos em ON em.id = ei.movimento_id
             WHERE em.armazem_id = m.armazem_id AND em.fluxo = 'reparo_externo'
               AND em.tipo = 'entrada' AND em.estornado_de IS NULL
               AND ei.codigo_componente = mi.codigo_componente
           )
         ORDER BY m.data ASC, m.hora ASC",
    )?;
    let pendentes = stmt
        .query_map(params![armazem_id], |r| {
            Ok(ReparoPendente {
                item_id: r.get(0)?,
                movimento_id: r.get(1)?,
                categoria: r.get(2)?,
                descricao: r.get(3)?,
                quantidade: r.get(4)?,
                codigo_componente: r.get(5)?,
                contraparte: r.get(6)?,
                data: r.get(7)?,
                hora: r.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(pendentes)
}

/// Um reparo externo que saiu e voltou consertado (`condicao = 'boa'` na
/// entrada) dentro de um intervalo de datas - usado pelo relatorio de
/// pagamento por quinzena do tecnico externo (so reparo que "deu certo"
/// conta pro pagamento; `defeito`/`sucata`/`outro` na entrada nao entram
/// aqui, mesmo que a peca tenha voltado fisicamente).
#[derive(Debug, Serialize)]
pub struct ReparoConcluido {
    pub movimento_id_saida: i64,
    pub movimento_id_entrada: i64,
    pub item_id_saida: i64,
    pub codigo_componente: String,
    pub categoria: String,
    pub descricao: Option<String>,
    pub quantidade: i64,
    pub contraparte: Option<String>,
    pub data_saida: String,
    pub hora_saida: String,
    pub data_entrada: String,
    pub hora_entrada: String,
    pub observacao_entrada: Option<String>,
}

/// Casa item de saida com item de entrada pelo mesmo `codigo_componente`
/// (inverso de `buscar_reparos_em_aberto`: aqui e um `JOIN`/par casado, la e
/// `NOT EXISTS`), filtrando pela `condicao` da entrada ('boa' = consertada)
/// e pela data da *entrada* (quando o reparo efetivamente concluiu) dentro
/// do intervalo pedido.
pub fn buscar_reparos_concluidos(
    conn: &Connection,
    armazem_id: i64,
    data_inicio: &str,
    data_fim: &str,
) -> AppResult<Vec<ReparoConcluido>> {
    let mut stmt = conn.prepare(
        "SELECT si.id, si.movimento_id, si.categoria, si.descricao, si.quantidade, si.codigo_componente,
                sm.contraparte, sm.data, sm.hora, em.id, em.data, em.hora, ei.observacao
         FROM movimento_itens si
         JOIN movimentos sm ON sm.id = si.movimento_id
         JOIN movimento_itens ei ON ei.codigo_componente = si.codigo_componente
         JOIN movimentos em ON em.id = ei.movimento_id
         WHERE sm.armazem_id = ?1 AND sm.fluxo = 'reparo_externo' AND sm.tipo = 'saida'
           AND sm.estornado_de IS NULL
           AND NOT EXISTS (SELECT 1 FROM movimentos x WHERE x.estornado_de = sm.id)
           AND si.codigo_componente IS NOT NULL AND si.codigo_componente != ''
           AND em.armazem_id = sm.armazem_id AND em.fluxo = 'reparo_externo' AND em.tipo = 'entrada'
           AND em.estornado_de IS NULL
           AND NOT EXISTS (SELECT 1 FROM movimentos x WHERE x.estornado_de = em.id)
           AND ei.condicao = 'boa'
           AND em.data BETWEEN ?2 AND ?3
         ORDER BY em.data ASC, em.hora ASC",
    )?;
    let concluidos = stmt
        .query_map(params![armazem_id, data_inicio, data_fim], |r| {
            Ok(ReparoConcluido {
                item_id_saida: r.get(0)?,
                movimento_id_saida: r.get(1)?,
                categoria: r.get(2)?,
                descricao: r.get(3)?,
                quantidade: r.get(4)?,
                codigo_componente: r.get(5)?,
                contraparte: r.get(6)?,
                data_saida: r.get(7)?,
                hora_saida: r.get(8)?,
                movimento_id_entrada: r.get(9)?,
                data_entrada: r.get(10)?,
                hora_entrada: r.get(11)?,
                observacao_entrada: r.get(12)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(concluidos)
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
            recebido_de_armazem_codigo: None,
            recebido_de_id_origem: None,
            retirada_completa: true,
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
                quantidade_enviada: None,
                codigo_componente: None,
            },
            MovimentoItemInput {
                categoria: "patinete".into(),
                descricao: None,
                montagem: Some("caixa".into()),
                condicao: None,
                quantidade: 2,
                observacao: None,
                quantidade_enviada: None,
                codigo_componente: None,
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
            quantidade_enviada: None,
            codigo_componente: None,
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
            quantidade_enviada: None,
            codigo_componente: None,
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
                quantidade_enviada: None,
                codigo_componente: None,
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
                quantidade_enviada: None,
                codigo_componente: None,
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
                quantidade_enviada: None,
                codigo_componente: None,
            },
            MovimentoItemInput {
                categoria: "peca".into(),
                descricao: Some("Retrovisor".into()),
                montagem: None,
                condicao: Some("boa".into()),
                quantidade: 1,
                observacao: None,
                quantidade_enviada: None,
                codigo_componente: None,
            },
        ];
        criar_movimento(&mut conn, movimento_base(armazem_id, usuario_id, itens)).unwrap();

        let sugestoes = sugestoes_descricao(&conn, "scooter").unwrap();
        assert_eq!(sugestoes, vec!["HE-15 GREEN".to_string()]);
    }

    fn item_simples() -> Vec<MovimentoItemInput> {
        vec![MovimentoItemInput {
            categoria: "scooter".into(),
            descricao: None,
            montagem: None,
            condicao: None,
            quantidade: 1,
            observacao: None,
            quantidade_enviada: None,
            codigo_componente: None,
        }]
    }

    fn criar_gestor(conn: &Connection, armazem_id: Option<i64>) -> i64 {
        criar_usuario(
            conn,
            NovoUsuario {
                nome: "Brenda",
                login: "brenda",
                senha: "senha123",
                armazem_id,
                papel: "gestor",
            },
        )
        .unwrap()
    }

    // --- Stage 2: autorizacao e validacao ---

    #[test]
    fn rejeita_movimento_de_usuario_de_outro_armazem() {
        let (mut conn, _armazem_id, usuario_id) = conexao_de_teste();
        let armazem_a4: i64 = conn
            .query_row("SELECT id FROM armazens WHERE codigo = 'A4'", [], |r| {
                r.get(0)
            })
            .unwrap();
        // usuario_id pertence ao armazem B2 (ver conexao_de_teste), tentando
        // registrar para A4.
        let resultado = criar_movimento(
            &mut conn,
            movimento_base(armazem_a4, usuario_id, item_simples()),
        );
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn rejeita_movimento_de_usuario_inativo() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        conn.execute(
            "UPDATE usuarios SET ativo = 0 WHERE id = ?1",
            params![usuario_id],
        )
        .unwrap();
        let resultado = criar_movimento(
            &mut conn,
            movimento_base(armazem_id, usuario_id, item_simples()),
        );
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn rejeita_movimento_de_usuario_inexistente() {
        let (mut conn, armazem_id, _usuario_id) = conexao_de_teste();
        let resultado = criar_movimento(
            &mut conn,
            movimento_base(armazem_id, 999_999, item_simples()),
        );
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn rejeita_turno_invalido() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.turno = "madrugada".into();
        assert!(matches!(
            criar_movimento(&mut conn, novo),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn rejeita_montagem_invalida() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let itens = vec![MovimentoItemInput {
            categoria: "scooter".into(),
            descricao: None,
            montagem: Some("meio-montado".into()),
            condicao: None,
            quantidade: 1,
            observacao: None,
            quantidade_enviada: None,
            codigo_componente: None,
        }];
        let resultado = criar_movimento(&mut conn, movimento_base(armazem_id, usuario_id, itens));
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn rejeita_condicao_invalida() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let itens = vec![MovimentoItemInput {
            categoria: "peca".into(),
            descricao: None,
            montagem: None,
            condicao: Some("meio-boa".into()),
            quantidade: 1,
            observacao: None,
            quantidade_enviada: None,
            codigo_componente: None,
        }];
        let resultado = criar_movimento(&mut conn, movimento_base(armazem_id, usuario_id, itens));
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn rejeita_texto_acima_do_limite() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.observacoes = Some("x".repeat(TEXTO_LIVRE_MAX + 1));
        assert!(matches!(
            criar_movimento(&mut conn, novo),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn rejeita_quantidade_acima_do_limite() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let itens = vec![MovimentoItemInput {
            categoria: "scooter".into(),
            descricao: None,
            montagem: None,
            condicao: None,
            quantidade: QUANTIDADE_MAX + 1,
            observacao: None,
            quantidade_enviada: None,
            codigo_componente: None,
        }];
        let resultado = criar_movimento(&mut conn, movimento_base(armazem_id, usuario_id, itens));
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    // --- Sprint 3: regras especificas de peca_montagem e sac ---

    #[test]
    fn rejeita_peca_montagem_sem_condicao() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.fluxo = "peca_montagem".into();
        novo.itens[0].condicao = None;
        assert!(matches!(
            criar_movimento(&mut conn, novo),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn aceita_peca_montagem_com_condicao() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.fluxo = "peca_montagem".into();
        novo.itens[0].condicao = Some("boa".into());
        assert!(criar_movimento(&mut conn, novo).is_ok());
    }

    #[test]
    fn rejeita_categoria_outro_sem_observacao() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.itens[0].categoria = "outro".into();
        assert!(matches!(
            criar_movimento(&mut conn, novo),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn aceita_categoria_outro_com_observacao() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.itens[0].categoria = "outro".into();
        novo.itens[0].observacao = Some("Kit de ferramentas avulso".into());
        assert!(criar_movimento(&mut conn, novo).is_ok());
    }

    #[test]
    fn rejeita_condicao_outro_sem_observacao() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.fluxo = "peca_montagem".into();
        novo.itens[0].condicao = Some("outro".into());
        assert!(matches!(
            criar_movimento(&mut conn, novo),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn aceita_condicao_outro_com_observacao() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.fluxo = "peca_montagem".into();
        novo.itens[0].condicao = Some("outro".into());
        novo.itens[0].observacao =
            Some("Peca meio amassada, nao se encaixa em nenhuma das 3".into());
        assert!(criar_movimento(&mut conn, novo).is_ok());
    }

    #[test]
    fn rejeita_sac_sem_motivo() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.fluxo = "sac".into();
        novo.motivo = None;
        assert!(matches!(
            criar_movimento(&mut conn, novo),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn rejeita_sac_venda_sem_valor() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.fluxo = "sac".into();
        novo.motivo = Some("venda".into());
        novo.valor_centavos = None;
        assert!(matches!(
            criar_movimento(&mut conn, novo),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn aceita_sac_venda_com_valor() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.fluxo = "sac".into();
        novo.tipo = "entrada".into();
        novo.motivo = Some("venda".into());
        novo.valor_centavos = Some(15_000);
        assert!(criar_movimento(&mut conn, novo).is_ok());
    }

    #[test]
    fn aceita_sac_garantia_sem_valor() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.fluxo = "sac".into();
        novo.tipo = "entrada".into();
        novo.motivo = Some("garantia".into());
        novo.valor_centavos = None;
        assert!(criar_movimento(&mut conn, novo).is_ok());
    }

    #[test]
    fn aceita_sac_saida_entregue_ao_cliente() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.fluxo = "sac".into();
        novo.tipo = "saida".into();
        novo.motivo = Some("entregue".into());
        assert!(criar_movimento(&mut conn, novo).is_ok());
    }

    #[test]
    fn aceita_sac_saida_descarte() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.fluxo = "sac".into();
        novo.tipo = "saida".into();
        novo.motivo = Some("descarte".into());
        assert!(criar_movimento(&mut conn, novo).is_ok());
    }

    #[test]
    fn rejeita_sac_entrada_com_motivo_de_saida() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.fluxo = "sac".into();
        novo.tipo = "entrada".into();
        novo.motivo = Some("descarte".into());
        assert!(criar_movimento(&mut conn, novo).is_err());
    }

    #[test]
    fn aceita_sac_saida_garantia() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.fluxo = "sac".into();
        novo.tipo = "saida".into();
        novo.motivo = Some("garantia".into());
        assert!(criar_movimento(&mut conn, novo).is_ok());
    }

    #[test]
    fn aceita_sac_saida_venda_com_valor() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.fluxo = "sac".into();
        novo.tipo = "saida".into();
        novo.motivo = Some("venda".into());
        novo.valor_centavos = Some(15_000);
        assert!(criar_movimento(&mut conn, novo).is_ok());
    }

    #[test]
    fn rejeita_sac_saida_venda_sem_valor() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.fluxo = "sac".into();
        novo.tipo = "saida".into();
        novo.motivo = Some("venda".into());
        novo.valor_centavos = None;
        assert!(matches!(
            criar_movimento(&mut conn, novo),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn rejeita_sac_motivo_outro_sem_observacoes() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.fluxo = "sac".into();
        novo.tipo = "entrada".into();
        novo.motivo = Some("outro".into());
        assert!(matches!(
            criar_movimento(&mut conn, novo),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn aceita_sac_motivo_outro_com_observacoes() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.fluxo = "sac".into();
        novo.tipo = "entrada".into();
        novo.motivo = Some("outro".into());
        novo.observacoes = Some("Cliente trouxe peca sem nota, caso atipico".into());
        assert!(criar_movimento(&mut conn, novo).is_ok());
    }

    #[test]
    fn aceita_sac_saida_motivo_outro_com_observacoes() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.fluxo = "sac".into();
        novo.tipo = "saida".into();
        novo.motivo = Some("outro".into());
        novo.observacoes = Some("Peca ficou retida com o tecnico, fora do fluxo normal".into());
        assert!(criar_movimento(&mut conn, novo).is_ok());
    }

    // --- Stage 3: hash de auditoria e verificacao de cadeia ---

    #[test]
    fn hash_muda_quando_campo_nao_coberto_antes_muda() {
        // Duas conexoes novas (cadeia comecando do zero) para que a unica
        // diferenca entre os dois hashes seja o campo sob teste, nao o
        // hash_anterior (que dependeria de quantos movimentos vieram antes).
        let (mut conn_a, armazem_a, usuario_a) = conexao_de_teste();
        let (mut conn_b, armazem_b, usuario_b) = conexao_de_teste();

        let mut base_a = movimento_base(armazem_a, usuario_a, item_simples());
        let mut base_b = movimento_base(armazem_b, usuario_b, item_simples());
        base_a.motivo = Some("garantia".into());
        base_b.motivo = Some("venda".into());

        let m_a = criar_movimento(&mut conn_a, base_a).unwrap();
        let m_b = criar_movimento(&mut conn_b, base_b).unwrap();
        assert_ne!(m_a.hash_integridade, m_b.hash_integridade);
    }

    #[test]
    fn verificar_cadeia_intacta_retorna_none() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        criar_movimento(
            &mut conn,
            movimento_base(armazem_id, usuario_id, item_simples()),
        )
        .unwrap();
        criar_movimento(
            &mut conn,
            movimento_base(armazem_id, usuario_id, item_simples()),
        )
        .unwrap();

        assert!(verificar_cadeia(&conn).unwrap().is_none());
    }

    #[test]
    fn verificar_cadeia_detecta_campo_alterado_direto_no_banco() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        criar_movimento(
            &mut conn,
            movimento_base(armazem_id, usuario_id, item_simples()),
        )
        .unwrap();
        let alvo = criar_movimento(
            &mut conn,
            movimento_base(armazem_id, usuario_id, item_simples()),
        )
        .unwrap();

        conn.execute(
            "UPDATE movimentos SET motivo = 'adulterado' WHERE id = ?1",
            params![alvo.id],
        )
        .unwrap();

        let quebra = verificar_cadeia(&conn)
            .unwrap()
            .expect("deveria detectar a quebra");
        assert_eq!(quebra.movimento_id, alvo.id);
    }

    // --- Stage 4: estorno ---

    #[test]
    fn gestor_pode_estornar_um_lancamento() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let gestor_id = criar_gestor(&conn, Some(armazem_id));
        let original = criar_movimento(
            &mut conn,
            movimento_base(armazem_id, usuario_id, item_simples()),
        )
        .unwrap();

        let estorno = estornar_movimento(
            &mut conn,
            original.id,
            gestor_id,
            "pedido duplicado por engano",
        )
        .unwrap();

        assert_eq!(estorno.estornado_de, Some(original.id));
        assert_eq!(estorno.status, "estorno");
        assert_eq!(estorno.itens.len(), original.itens.len());
    }

    #[test]
    fn conferente_pode_estornar_um_lancamento_do_proprio_armazem() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let original = criar_movimento(
            &mut conn,
            movimento_base(armazem_id, usuario_id, item_simples()),
        )
        .unwrap();

        // usuario_id (Alice) e conferente, nao gestor.
        let resultado = estornar_movimento(&mut conn, original.id, usuario_id, "engano");
        assert!(resultado.is_ok());
    }

    #[test]
    fn conferente_nao_pode_estornar_lancamento_de_outro_armazem() {
        let (mut conn, armazem_b2, usuario_id) = conexao_de_teste();
        let armazem_a4: i64 = conn
            .query_row("SELECT id FROM armazens WHERE codigo = 'A4'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let original = criar_movimento(
            &mut conn,
            movimento_base(armazem_b2, usuario_id, item_simples()),
        )
        .unwrap();
        let conferente_a4 = criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Marcelo",
                login: "marcelo_a4",
                senha: "senha123",
                armazem_id: Some(armazem_a4),
                papel: "conferente",
            },
        )
        .unwrap();

        let resultado = estornar_movimento(&mut conn, original.id, conferente_a4, "engano");
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn estorno_exige_justificativa() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let gestor_id = criar_gestor(&conn, Some(armazem_id));
        let original = criar_movimento(
            &mut conn,
            movimento_base(armazem_id, usuario_id, item_simples()),
        )
        .unwrap();

        let resultado = estornar_movimento(&mut conn, original.id, gestor_id, "   ");
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn estorno_funciona_mesmo_com_o_dia_fechado() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let gestor_id = criar_gestor(&conn, Some(armazem_id));
        let original = criar_movimento(
            &mut conn,
            movimento_base(armazem_id, usuario_id, item_simples()),
        )
        .unwrap();

        crate::domain::fechamentos::fechar_dia(
            &mut conn,
            armazem_id,
            "saida_armazem",
            &original.data,
            gestor_id,
        )
        .unwrap();

        // Sem o estorno bypassar a trava de dia fechado, isso falharia com o
        // mesmo erro que `criar_movimento` daria num dia fechado.
        let estorno =
            estornar_movimento(&mut conn, original.id, gestor_id, "erro descoberto depois");
        assert!(estorno.is_ok());
    }

    #[test]
    fn nao_pode_estornar_o_mesmo_lancamento_duas_vezes() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let gestor_id = criar_gestor(&conn, Some(armazem_id));
        let original = criar_movimento(
            &mut conn,
            movimento_base(armazem_id, usuario_id, item_simples()),
        )
        .unwrap();

        estornar_movimento(&mut conn, original.id, gestor_id, "primeira vez").unwrap();
        let resultado = estornar_movimento(&mut conn, original.id, gestor_id, "segunda vez");
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn nao_pode_estornar_um_estorno() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let gestor_id = criar_gestor(&conn, Some(armazem_id));
        let original = criar_movimento(
            &mut conn,
            movimento_base(armazem_id, usuario_id, item_simples()),
        )
        .unwrap();
        let estorno =
            estornar_movimento(&mut conn, original.id, gestor_id, "primeira vez").unwrap();

        let resultado = estornar_movimento(&mut conn, estorno.id, gestor_id, "estornar o estorno?");
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn estorno_rejeita_gestor_de_outro_armazem() {
        let (mut conn, armazem_b2, usuario_id) = conexao_de_teste();
        let armazem_a4: i64 = conn
            .query_row("SELECT id FROM armazens WHERE codigo = 'A4'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let original = criar_movimento(
            &mut conn,
            movimento_base(armazem_b2, usuario_id, item_simples()),
        )
        .unwrap();

        // Gestor fixo no armazem A4 nao pode estornar um lancamento do B2,
        // mesmo sendo gestor.
        let gestor_a4 = criar_gestor(&conn, Some(armazem_a4));
        let resultado = estornar_movimento(&mut conn, original.id, gestor_a4, "engano");
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn estorno_rejeita_movimento_inexistente() {
        let (mut conn, armazem_id, _usuario_id) = conexao_de_teste();
        let gestor_id = criar_gestor(&conn, Some(armazem_id));
        let resultado = estornar_movimento(&mut conn, 999_999, gestor_id, "engano");
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn estorno_rejeita_usuario_desativado_apos_virar_gestor() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let gestor_id = criar_gestor(&conn, Some(armazem_id));
        let original = criar_movimento(
            &mut conn,
            movimento_base(armazem_id, usuario_id, item_simples()),
        )
        .unwrap();

        // Gestor foi desativado (ex.: desligado da empresa) depois de ter
        // sido cadastrado - a sessao antiga nao pode continuar autorizando
        // estornos.
        conn.execute(
            "UPDATE usuarios SET ativo = 0 WHERE id = ?1",
            params![gestor_id],
        )
        .unwrap();

        let resultado = estornar_movimento(&mut conn, original.id, gestor_id, "engano");
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn buscar_movimento_retorna_erro_para_id_inexistente() {
        let (conn, _armazem_id, _usuario_id) = conexao_de_teste();
        let resultado = buscar_movimento(&conn, 999_999);
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn rejeita_quantidade_negativa() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let itens = vec![MovimentoItemInput {
            categoria: "scooter".into(),
            descricao: None,
            montagem: None,
            condicao: None,
            quantidade: -5,
            observacao: None,
            quantidade_enviada: None,
            codigo_componente: None,
        }];
        let resultado = criar_movimento(&mut conn, movimento_base(armazem_id, usuario_id, itens));
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn rejeita_movimento_com_armazem_destino_inativo() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let armazem_a4: i64 = conn
            .query_row("SELECT id FROM armazens WHERE codigo = 'A4'", [], |r| {
                r.get(0)
            })
            .unwrap();
        conn.execute(
            "UPDATE armazens SET ativo = 0 WHERE id = ?1",
            params![armazem_a4],
        )
        .unwrap();

        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.armazem_destino_id = Some(armazem_a4);
        let resultado = criar_movimento(&mut conn, novo);
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    // --- Historico ---

    fn movimento_com(
        armazem_id: i64,
        usuario_id: i64,
        data: &str,
        numero_pedido: &str,
        contraparte: &str,
    ) -> NovoMovimento {
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.data = data.into();
        novo.numero_pedido = Some(numero_pedido.into());
        novo.contraparte = Some(contraparte.into());
        novo
    }

    #[test]
    fn historico_filtra_por_intervalo_de_data() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        criar_movimento(
            &mut conn,
            movimento_com(armazem_id, usuario_id, "2026-08-10", "1", "Cliente A"),
        )
        .unwrap();
        criar_movimento(
            &mut conn,
            movimento_com(armazem_id, usuario_id, "2026-08-15", "2", "Cliente A"),
        )
        .unwrap();
        criar_movimento(
            &mut conn,
            movimento_com(armazem_id, usuario_id, "2026-08-20", "3", "Cliente A"),
        )
        .unwrap();

        let resultado = buscar_historico(
            &conn,
            armazem_id,
            "saida_armazem",
            Some("2026-08-12"),
            Some("2026-08-18"),
            None,
            None,
            0,
        )
        .unwrap();

        assert_eq!(resultado.movimentos.len(), 1);
        assert_eq!(resultado.movimentos[0].numero_pedido.as_deref(), Some("2"));
        assert!(!resultado.tem_mais);
    }

    #[test]
    fn historico_filtra_por_cliente_parcial_sem_case() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        criar_movimento(
            &mut conn,
            movimento_com(
                armazem_id,
                usuario_id,
                "2026-08-10",
                "1",
                "DISK&TENHA LOGISTICA",
            ),
        )
        .unwrap();
        criar_movimento(
            &mut conn,
            movimento_com(armazem_id, usuario_id, "2026-08-11", "2", "Correios"),
        )
        .unwrap();

        let resultado = buscar_historico(
            &conn,
            armazem_id,
            "saida_armazem",
            None,
            None,
            Some("disk"),
            None,
            0,
        )
        .unwrap();

        assert_eq!(resultado.movimentos.len(), 1);
        assert_eq!(resultado.movimentos[0].numero_pedido.as_deref(), Some("1"));
    }

    #[test]
    fn historico_filtra_por_numero_pedido_parcial() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        criar_movimento(
            &mut conn,
            movimento_com(armazem_id, usuario_id, "2026-08-10", "3893", "Cliente A"),
        )
        .unwrap();
        criar_movimento(
            &mut conn,
            movimento_com(armazem_id, usuario_id, "2026-08-11", "4001", "Cliente A"),
        )
        .unwrap();

        let resultado = buscar_historico(
            &conn,
            armazem_id,
            "saida_armazem",
            None,
            None,
            None,
            Some("389"),
            0,
        )
        .unwrap();

        assert_eq!(resultado.movimentos.len(), 1);
        assert_eq!(
            resultado.movimentos[0].numero_pedido.as_deref(),
            Some("3893")
        );
    }

    #[test]
    fn historico_trata_underscore_no_pedido_como_texto_literal_nao_curinga() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        criar_movimento(
            &mut conn,
            movimento_com(armazem_id, usuario_id, "2026-08-10", "50_1", "Cliente A"),
        )
        .unwrap();
        criar_movimento(
            &mut conn,
            movimento_com(armazem_id, usuario_id, "2026-08-11", "5011", "Cliente A"),
        )
        .unwrap();

        // Sem escape, "_" no LIKE do SQLite casa qualquer caractere - "50_1"
        // tambem bateria com "5011". O filtro deve trazer so o pedido exato.
        let resultado = buscar_historico(
            &conn,
            armazem_id,
            "saida_armazem",
            None,
            None,
            None,
            Some("50_1"),
            0,
        )
        .unwrap();

        assert_eq!(resultado.movimentos.len(), 1);
        assert_eq!(
            resultado.movimentos[0].numero_pedido.as_deref(),
            Some("50_1")
        );
    }

    #[test]
    fn historico_combina_filtros() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        criar_movimento(
            &mut conn,
            movimento_com(armazem_id, usuario_id, "2026-08-10", "3893", "DISK&TENHA"),
        )
        .unwrap();
        criar_movimento(
            &mut conn,
            movimento_com(armazem_id, usuario_id, "2026-08-11", "3893", "Correios"),
        )
        .unwrap();

        let resultado = buscar_historico(
            &conn,
            armazem_id,
            "saida_armazem",
            None,
            None,
            Some("disk"),
            Some("3893"),
            0,
        )
        .unwrap();

        assert_eq!(resultado.movimentos.len(), 1);
        assert_eq!(
            resultado.movimentos[0].contraparte.as_deref(),
            Some("DISK&TENHA")
        );
    }

    #[test]
    fn historico_nao_vaza_outro_armazem_nem_outro_fluxo() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let armazem_a4: i64 = conn
            .query_row("SELECT id FROM armazens WHERE codigo = 'A4'", [], |r| {
                r.get(0)
            })
            .unwrap();

        criar_movimento(
            &mut conn,
            movimento_com(armazem_id, usuario_id, "2026-08-10", "1", "Cliente A"),
        )
        .unwrap();

        let mut outro_fluxo = movimento_com(armazem_id, usuario_id, "2026-08-10", "2", "Cliente A");
        outro_fluxo.fluxo = "peca_montagem".into();
        outro_fluxo.itens[0].condicao = Some("boa".into());
        criar_movimento(&mut conn, outro_fluxo).unwrap();

        let resultado_fluxo_certo = buscar_historico(
            &conn,
            armazem_id,
            "saida_armazem",
            None,
            None,
            None,
            None,
            0,
        )
        .unwrap();
        assert_eq!(resultado_fluxo_certo.movimentos.len(), 1);

        let resultado_outro_armazem = buscar_historico(
            &conn,
            armazem_a4,
            "saida_armazem",
            None,
            None,
            None,
            None,
            0,
        )
        .unwrap();
        assert!(resultado_outro_armazem.movimentos.is_empty());
    }

    #[test]
    fn historico_pagina_com_offset_e_avisa_quando_tem_mais() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        for i in 1..=3 {
            criar_movimento(
                &mut conn,
                movimento_com(
                    armazem_id,
                    usuario_id,
                    &format!("2026-08-{:02}", 10 + i),
                    &i.to_string(),
                    "Cliente A",
                ),
            )
            .unwrap();
        }

        // LIMITE_HISTORICO e 500, entao simulamos uma pagina pequena filtrando
        // por um intervalo estreito nao ajudaria - testamos so que offset
        // pula os mais recentes e que 3 cabem numa pagina sem "tem_mais".
        let pagina = buscar_historico(
            &conn,
            armazem_id,
            "saida_armazem",
            None,
            None,
            None,
            None,
            0,
        )
        .unwrap();
        assert_eq!(pagina.movimentos.len(), 3);
        assert!(!pagina.tem_mais);

        let pagina_offset = buscar_historico(
            &conn,
            armazem_id,
            "saida_armazem",
            None,
            None,
            None,
            None,
            1,
        )
        .unwrap();
        assert_eq!(pagina_offset.movimentos.len(), 2);
        // Mais recente primeiro (2026-08-13, pedido "3") fica de fora com offset 1.
        assert!(pagina_offset
            .movimentos
            .iter()
            .all(|m| m.numero_pedido.as_deref() != Some("3")));
    }

    // --- Leitura protegida por sessao/armazem ---

    #[test]
    fn autorizar_leitura_rejeita_usuario_de_outro_armazem() {
        let (conn, _armazem_b2, usuario_id) = conexao_de_teste();
        let armazem_a4: i64 = conn
            .query_row("SELECT id FROM armazens WHERE codigo = 'A4'", [], |r| {
                r.get(0)
            })
            .unwrap();
        // usuario_id pertence ao B2 (ver conexao_de_teste); tentando ler A4.
        let resultado = autorizar_leitura(&conn, usuario_id, armazem_a4);
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn autorizar_leitura_rejeita_usuario_inativo() {
        let (conn, armazem_id, usuario_id) = conexao_de_teste();
        conn.execute(
            "UPDATE usuarios SET ativo = 0 WHERE id = ?1",
            params![usuario_id],
        )
        .unwrap();
        let resultado = autorizar_leitura(&conn, usuario_id, armazem_id);
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn autorizar_leitura_permite_gestor_sem_armazem_fixo_ler_qualquer_armazem() {
        let (conn, armazem_b2, _usuario_id) = conexao_de_teste();
        let armazem_a4: i64 = conn
            .query_row("SELECT id FROM armazens WHERE codigo = 'A4'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let gestor_sem_armazem = criar_gestor(&conn, None);

        assert!(autorizar_leitura(&conn, gestor_sem_armazem, armazem_b2).is_ok());
        assert!(autorizar_leitura(&conn, gestor_sem_armazem, armazem_a4).is_ok());
    }

    #[test]
    fn autorizar_leitura_permite_usuario_do_proprio_armazem() {
        let (conn, armazem_id, usuario_id) = conexao_de_teste();
        assert!(autorizar_leitura(&conn, usuario_id, armazem_id).is_ok());
    }

    fn item_enviado(quantidade: i64) -> MovimentoItem {
        MovimentoItem {
            id: 1,
            categoria: "peca".into(),
            descricao: Some("Bateria 48V".into()),
            montagem: None,
            condicao: Some("boa".into()),
            quantidade,
            observacao: None,
            quantidade_enviada: None,
            codigo_componente: None,
        }
    }

    #[test]
    fn validar_quantidades_recebidas_aceita_igual_ao_enviado() {
        let enviados = vec![item_enviado(5)];
        let resultado = validar_quantidades_recebidas(&enviados, &[5]).unwrap();
        assert_eq!(resultado[0].quantidade, 5);
        assert_eq!(resultado[0].quantidade_enviada, Some(5));
    }

    #[test]
    fn validar_quantidades_recebidas_aceita_menor_que_o_enviado() {
        let enviados = vec![item_enviado(5)];
        let resultado = validar_quantidades_recebidas(&enviados, &[3]).unwrap();
        assert_eq!(resultado[0].quantidade, 3);
        assert_eq!(resultado[0].quantidade_enviada, Some(5));
    }

    #[test]
    fn validar_quantidades_recebidas_rejeita_maior_que_o_enviado() {
        let enviados = vec![item_enviado(5)];
        let resultado = validar_quantidades_recebidas(&enviados, &[6]);
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn validar_quantidades_recebidas_rejeita_zero_ou_negativo() {
        let enviados = vec![item_enviado(5)];
        assert!(validar_quantidades_recebidas(&enviados, &[0]).is_err());
        assert!(validar_quantidades_recebidas(&enviados, &[-1]).is_err());
    }

    #[test]
    fn validar_quantidades_recebidas_rejeita_tamanho_diferente() {
        let enviados = vec![item_enviado(5), item_enviado(2)];
        let resultado = validar_quantidades_recebidas(&enviados, &[5]);
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn retirada_parcial_pendente_none_quando_pedido_nao_existe() {
        let (conn, armazem_id, _usuario_id) = conexao_de_teste();
        let resultado =
            buscar_retirada_parcial_pendente(&conn, armazem_id, "saida_armazem", "9999").unwrap();
        assert!(resultado.is_none());
    }

    #[test]
    fn retirada_parcial_pendente_none_quando_retirada_foi_completa() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(
            armazem_id,
            usuario_id,
            vec![MovimentoItemInput {
                categoria: "scooter".into(),
                descricao: None,
                montagem: None,
                condicao: None,
                quantidade: 2,
                observacao: None,
                quantidade_enviada: None,
                codigo_componente: None,
            }],
        );
        novo.numero_pedido = Some("500".into());
        novo.retirada_completa = true;
        criar_movimento(&mut conn, novo).unwrap();

        let resultado =
            buscar_retirada_parcial_pendente(&conn, armazem_id, "saida_armazem", "500").unwrap();
        assert!(resultado.is_none());
    }

    #[test]
    fn retirada_parcial_pendente_encontra_a_retirada_parcial_mais_recente() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(
            armazem_id,
            usuario_id,
            vec![MovimentoItemInput {
                categoria: "scooter".into(),
                descricao: None,
                montagem: None,
                condicao: None,
                quantidade: 5,
                observacao: None,
                quantidade_enviada: None,
                codigo_componente: None,
            }],
        );
        novo.numero_pedido = Some("501".into());
        novo.retirada_completa = false;
        let criado = criar_movimento(&mut conn, novo).unwrap();

        let resultado = buscar_retirada_parcial_pendente(&conn, armazem_id, "saida_armazem", "501")
            .unwrap()
            .expect("deveria encontrar a retirada parcial");
        assert_eq!(resultado.id, criado.id);
        assert!(!resultado.retirada_completa);
    }

    #[test]
    fn retirada_parcial_pendente_resolve_quando_ha_retirada_complementar_depois() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut primeira = movimento_base(
            armazem_id,
            usuario_id,
            vec![MovimentoItemInput {
                categoria: "scooter".into(),
                descricao: None,
                montagem: None,
                condicao: None,
                quantidade: 3,
                observacao: None,
                quantidade_enviada: None,
                codigo_componente: None,
            }],
        );
        primeira.numero_pedido = Some("502".into());
        primeira.retirada_completa = false;
        primeira.hora = "09:00".into();
        criar_movimento(&mut conn, primeira).unwrap();

        let mut complementar = movimento_base(
            armazem_id,
            usuario_id,
            vec![MovimentoItemInput {
                categoria: "scooter".into(),
                descricao: None,
                montagem: None,
                condicao: None,
                quantidade: 2,
                observacao: None,
                quantidade_enviada: None,
                codigo_componente: None,
            }],
        );
        complementar.numero_pedido = Some("502".into());
        complementar.retirada_completa = true;
        complementar.hora = "15:00".into();
        criar_movimento(&mut conn, complementar).unwrap();

        let resultado =
            buscar_retirada_parcial_pendente(&conn, armazem_id, "saida_armazem", "502").unwrap();
        assert!(resultado.is_none());
    }

    fn item_reparo(codigo_componente: Option<&str>, condicao: Option<&str>) -> MovimentoItemInput {
        MovimentoItemInput {
            categoria: "peca".into(),
            descricao: Some("Bateria 48V".into()),
            montagem: None,
            condicao: condicao.map(String::from),
            quantidade: 1,
            observacao: None,
            quantidade_enviada: None,
            codigo_componente: codigo_componente.map(String::from),
        }
    }

    /// `tipo == "entrada"` ja vem com `condicao = "boa"` por padrao (exigida
    /// pela validacao) - testes que precisam de outro resultado (defeito,
    /// sucata) sobrescrevem `novo.itens[0].condicao` depois de chamar isto.
    fn movimento_reparo(
        armazem_id: i64,
        usuario_id: i64,
        tipo: &str,
        codigo_componente: Option<&str>,
    ) -> NovoMovimento {
        let condicao = if tipo == "entrada" { Some("boa") } else { None };
        let mut novo = movimento_base(
            armazem_id,
            usuario_id,
            vec![item_reparo(codigo_componente, condicao)],
        );
        novo.fluxo = "reparo_externo".into();
        novo.tipo = tipo.into();
        novo.numero_pedido = None;
        novo.contraparte = Some("Tecnico Joao - Eletronica Silva".into());
        novo.quem_retirou = None;
        novo
    }

    #[test]
    fn rejeita_reparo_externo_sem_codigo_componente() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let novo = movimento_reparo(armazem_id, usuario_id, "saida", None);
        let resultado = criar_movimento(&mut conn, novo);
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn reparo_externo_ciclo_completo_fecha_a_pendencia() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();

        let saida = movimento_reparo(armazem_id, usuario_id, "saida", Some("BAT-001"));
        criar_movimento(&mut conn, saida).unwrap();

        let abertos = buscar_reparos_em_aberto(&conn, armazem_id).unwrap();
        assert_eq!(abertos.len(), 1);
        assert_eq!(abertos[0].codigo_componente, "BAT-001");

        let mut entrada = movimento_reparo(armazem_id, usuario_id, "entrada", Some("BAT-001"));
        entrada.hora = "15:00".into();
        criar_movimento(&mut conn, entrada).unwrap();

        let abertos = buscar_reparos_em_aberto(&conn, armazem_id).unwrap();
        assert!(abertos.is_empty());
    }

    #[test]
    fn reparo_externo_estorno_da_saida_tira_da_lista_de_abertos() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();

        let saida = movimento_reparo(armazem_id, usuario_id, "saida", Some("BAT-002"));
        let criado = criar_movimento(&mut conn, saida).unwrap();
        assert_eq!(
            buscar_reparos_em_aberto(&conn, armazem_id).unwrap().len(),
            1
        );

        estornar_movimento(&mut conn, criado.id, usuario_id, "saiu por engano").unwrap();
        assert!(buscar_reparos_em_aberto(&conn, armazem_id)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn reparo_externo_entrada_com_codigo_diferente_nao_fecha_a_pendencia_original() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();

        let saida = movimento_reparo(armazem_id, usuario_id, "saida", Some("BAT-003"));
        criar_movimento(&mut conn, saida).unwrap();

        let mut entrada = movimento_reparo(armazem_id, usuario_id, "entrada", Some("BAT-999"));
        entrada.hora = "15:00".into();
        criar_movimento(&mut conn, entrada).unwrap();

        let abertos = buscar_reparos_em_aberto(&conn, armazem_id).unwrap();
        assert_eq!(abertos.len(), 1);
        assert_eq!(abertos[0].codigo_componente, "BAT-003");
    }

    #[test]
    fn rejeita_reparo_externo_entrada_sem_condicao() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let saida = movimento_reparo(armazem_id, usuario_id, "saida", Some("BAT-010"));
        criar_movimento(&mut conn, saida).unwrap();

        let mut entrada = movimento_reparo(armazem_id, usuario_id, "entrada", Some("BAT-010"));
        entrada.hora = "15:00".into();
        entrada.itens[0].condicao = None;
        let resultado = criar_movimento(&mut conn, entrada);
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn reparos_concluidos_lista_so_entrada_com_condicao_boa_dentro_do_periodo() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();

        // Consertado dentro do periodo - deve aparecer.
        let saida1 = movimento_reparo(armazem_id, usuario_id, "saida", Some("BAT-020"));
        criar_movimento(&mut conn, saida1).unwrap();
        let mut entrada1 = movimento_reparo(armazem_id, usuario_id, "entrada", Some("BAT-020"));
        entrada1.data = "2026-08-20".into();
        entrada1.hora = "15:00".into();
        criar_movimento(&mut conn, entrada1).unwrap();

        // Sem conserto (condicao != boa) - nao deve aparecer, mesmo dentro do periodo.
        let saida2 = movimento_reparo(armazem_id, usuario_id, "saida", Some("BAT-021"));
        criar_movimento(&mut conn, saida2).unwrap();
        let mut entrada2 = movimento_reparo(armazem_id, usuario_id, "entrada", Some("BAT-021"));
        entrada2.data = "2026-08-21".into();
        entrada2.hora = "15:00".into();
        entrada2.itens[0].condicao = Some("defeito".into());
        criar_movimento(&mut conn, entrada2).unwrap();

        // Consertado fora do periodo pedido - nao deve aparecer.
        let saida3 = movimento_reparo(armazem_id, usuario_id, "saida", Some("BAT-022"));
        criar_movimento(&mut conn, saida3).unwrap();
        let mut entrada3 = movimento_reparo(armazem_id, usuario_id, "entrada", Some("BAT-022"));
        entrada3.data = "2026-09-01".into();
        entrada3.hora = "15:00".into();
        criar_movimento(&mut conn, entrada3).unwrap();

        let concluidos =
            buscar_reparos_concluidos(&conn, armazem_id, "2026-08-01", "2026-08-31").unwrap();
        assert_eq!(concluidos.len(), 1);
        assert_eq!(concluidos[0].codigo_componente, "BAT-020");
        assert_eq!(concluidos[0].data_entrada, "2026-08-20");
    }

    #[test]
    fn reparos_concluidos_ignora_saida_estornada() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();

        let saida = movimento_reparo(armazem_id, usuario_id, "saida", Some("BAT-030"));
        let criado = criar_movimento(&mut conn, saida).unwrap();
        estornar_movimento(&mut conn, criado.id, usuario_id, "saiu por engano").unwrap();

        let mut entrada = movimento_reparo(armazem_id, usuario_id, "entrada", Some("BAT-030"));
        entrada.hora = "15:00".into();
        criar_movimento(&mut conn, entrada).unwrap();

        let concluidos =
            buscar_reparos_concluidos(&conn, armazem_id, "2026-08-01", "2026-08-31").unwrap();
        assert!(concluidos.is_empty());
    }
}
