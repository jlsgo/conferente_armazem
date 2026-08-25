use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::auth::buscar_usuario_ativo;
use super::errors::{AppError, AppResult};

const CATEGORIAS_VALIDAS: [&str; 4] = ["scooter", "triciclo", "patinete", "peca"];
const FLUXOS_VALIDOS: [&str; 3] = ["saida_armazem", "peca_montagem", "sac"];
const TIPOS_VALIDOS: [&str; 2] = ["entrada", "saida"];
const TURNOS_VALIDOS: [&str; 2] = ["diurno", "noturno"];
const MONTAGENS_VALIDAS: [&str; 2] = ["montado", "caixa"];
const CONDICOES_VALIDAS: [&str; 3] = ["boa", "defeito", "sucata"];
const MOTIVOS_SAC_VALIDOS: [&str; 2] = ["garantia", "venda"];
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
    pub motivo: Option<String>,
    pub valor_centavos: Option<i64>,
    pub status: String,
    pub estornado_de: Option<i64>,
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
        match novo.motivo.as_deref() {
            Some(motivo) if MOTIVOS_SAC_VALIDOS.contains(&motivo) => {}
            _ => {
                return Err(AppError::Validation(
                    "Informe o motivo do SAC: garantia ou venda.".into(),
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
        } else if novo.fluxo == "peca_montagem" {
            return Err(AppError::Validation(
                "Informe a condicao da peca: boa, defeito ou sucata.".into(),
            ));
        }
        validar_texto_livre("Descricao do item", item.descricao.as_deref())?;
        validar_texto_livre("Observacao do item", item.observacao.as_deref())?;
    }
    Ok(())
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
fn autorizar_movimento(conn: &Connection, usuario_id: i64, armazem_id: i64) -> AppResult<()> {
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

struct ItemHash {
    categoria: String,
    descricao: Option<String>,
    montagem: Option<String>,
    condicao: Option<String>,
    quantidade: i64,
    observacao: Option<String>,
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
                "{}:{}:{}:{}:{}:{}",
                i.categoria,
                i.descricao.as_deref().unwrap_or(""),
                i.montagem.as_deref().unwrap_or(""),
                i.condicao.as_deref().unwrap_or(""),
                i.quantidade,
                i.observacao.as_deref().unwrap_or(""),
            )
        })
        .collect();

    let partes: [String; 16] = [
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

    let usuario = buscar_usuario_ativo(conn, usuario_id)?;
    if usuario.papel != "gestor" {
        return Err(AppError::Validation(
            "Somente um gestor pode estornar um lancamento.".into(),
        ));
    }
    if let Some(armazem_do_usuario) = usuario.armazem_id {
        if armazem_do_usuario != original.armazem_id {
            return Err(AppError::Validation(
                "Voce nao pode estornar um lancamento de outro armazem.".into(),
            ));
        }
    }

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
        itens: itens_originais
            .iter()
            .map(|i| ItemHash {
                categoria: i.categoria.clone(),
                descricao: i.descricao.clone(),
                montagem: i.montagem.clone(),
                condicao: i.condicao.clone(),
                quantidade: i.quantidade,
                observacao: i.observacao.clone(),
            })
            .collect(),
    };

    let hash_anterior = ultimo_hash(&tx);
    let hash = calcular_hash(&hash_anterior, &campos);

    tx.execute(
        "INSERT INTO movimentos (
            armazem_id, armazem_destino_id, fluxo, tipo, data, hora, turno, usuario_id,
            numero_pedido, codigo_rastreio, contraparte, quem_retirou,
            motivo, valor_centavos, observacoes, status, estornado_de, hash_integridade
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 'estorno', ?16, ?17)",
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
            hash,
        ],
    )?;

    let estorno_id = tx.last_insert_rowid();

    {
        let mut inserir_item = tx.prepare(
            "INSERT INTO movimento_itens (movimento_id, categoria, descricao, montagem, condicao, quantidade, observacao)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
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
                valor_centavos, observacoes, estornado_de, hash_integridade
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
                hash_integridade: r.get(17)?,
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
    let encontrado = conn
        .query_row(
            "SELECT m.armazem_id, m.fluxo, m.tipo, m.data, m.hora, m.turno, m.usuario_id, u.nome,
                    m.numero_pedido, m.contraparte, m.quem_retirou, m.motivo, m.valor_centavos,
                    m.status, m.estornado_de, m.hash_integridade
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
                    r.get::<_, Option<String>>(11)?,
                    r.get::<_, Option<i64>>(12)?,
                    r.get::<_, String>(13)?,
                    r.get::<_, Option<i64>>(14)?,
                    r.get::<_, String>(15)?,
                ))
            },
        )
        .optional()?;

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
        motivo,
        valor_centavos,
        status,
        estornado_de,
        hash_integridade,
    ) = encontrado.ok_or_else(|| AppError::Validation("Lancamento nao encontrado.".into()))?;

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
        motivo,
        valor_centavos,
        status,
        estornado_de,
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
                m.numero_pedido, m.contraparte, m.quem_retirou, m.motivo, m.valor_centavos,
                m.status, m.estornado_de, m.hash_integridade
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
                motivo: r.get(12)?,
                valor_centavos: r.get(13)?,
                status: r.get(14)?,
                estornado_de: r.get(15)?,
                hash_integridade: r.get(16)?,
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

    fn item_simples() -> Vec<MovimentoItemInput> {
        vec![MovimentoItemInput {
            categoria: "scooter".into(),
            descricao: None,
            montagem: None,
            condicao: None,
            quantidade: 1,
            observacao: None,
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
        novo.motivo = Some("venda".into());
        novo.valor_centavos = Some(15_000);
        assert!(criar_movimento(&mut conn, novo).is_ok());
    }

    #[test]
    fn aceita_sac_garantia_sem_valor() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let mut novo = movimento_base(armazem_id, usuario_id, item_simples());
        novo.fluxo = "sac".into();
        novo.motivo = Some("garantia".into());
        novo.valor_centavos = None;
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
    fn conferente_nao_pode_estornar() {
        let (mut conn, armazem_id, usuario_id) = conexao_de_teste();
        let original = criar_movimento(
            &mut conn,
            movimento_base(armazem_id, usuario_id, item_simples()),
        )
        .unwrap();

        // usuario_id (Alice) e conferente, nao gestor.
        let resultado = estornar_movimento(&mut conn, original.id, usuario_id, "engano");
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
}
