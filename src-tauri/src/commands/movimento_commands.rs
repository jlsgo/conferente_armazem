use serde::Deserialize;
use tauri::State;

use crate::domain::errors::AppResult;
use crate::domain::movimentos::{self, Movimento, MovimentoItemInput, NovoMovimento};
use crate::state::AppState;

/// Espelha `domain::movimentos::NovoMovimento`, mas sem `usuario_id`: quem
/// esta registrando o movimento vem sempre da sessao do backend
/// (`state.usuario_logado()`), nunca do payload que o frontend manda.
#[derive(Debug, Deserialize)]
pub struct NovoMovimentoPayload {
    pub armazem_id: i64,
    pub armazem_destino_id: Option<i64>,
    pub fluxo: String,
    pub tipo: String,
    pub data: String,
    pub hora: String,
    pub turno: String,
    pub numero_pedido: Option<String>,
    pub codigo_rastreio: Option<String>,
    pub contraparte: Option<String>,
    pub quem_retirou: Option<String>,
    pub motivo: Option<String>,
    pub valor_centavos: Option<i64>,
    pub observacoes: Option<String>,
    /// So relevante pra `saida_armazem`/`saida`; nao enviado pelas telas de
    /// Montagem/SAC, que sempre valem `true` (retirada/registro completo).
    #[serde(default = "retirada_completa_padrao")]
    pub retirada_completa: bool,
    pub itens: Vec<MovimentoItemInput>,
}

fn retirada_completa_padrao() -> bool {
    true
}

#[tauri::command]
pub fn criar_movimento(
    state: State<AppState>,
    payload: NovoMovimentoPayload,
) -> AppResult<Movimento> {
    let usuario_id = state.usuario_logado()?;
    let mut conn = state.conn()?;
    movimentos::criar_movimento(
        &mut conn,
        NovoMovimento {
            armazem_id: payload.armazem_id,
            armazem_destino_id: payload.armazem_destino_id,
            fluxo: payload.fluxo,
            tipo: payload.tipo,
            data: payload.data,
            hora: payload.hora,
            turno: payload.turno,
            usuario_id,
            numero_pedido: payload.numero_pedido,
            codigo_rastreio: payload.codigo_rastreio,
            contraparte: payload.contraparte,
            quem_retirou: payload.quem_retirou,
            motivo: payload.motivo,
            valor_centavos: payload.valor_centavos,
            observacoes: payload.observacoes,
            // So a confirmacao de recebimento (comando dedicado
            // `sync_commands::confirmar_recebimento`) preenche isso - nunca vem
            // do payload comum de lancamento.
            recebido_de_armazem_codigo: None,
            recebido_de_id_origem: None,
            retirada_completa: payload.retirada_completa,
            itens: payload.itens,
        },
    )
}

#[tauri::command(rename_all = "snake_case")]
pub fn estornar_movimento(
    state: State<AppState>,
    movimento_id: i64,
    justificativa: String,
) -> AppResult<Movimento> {
    let usuario_id = state.usuario_logado()?;
    let mut conn = state.conn()?;
    movimentos::estornar_movimento(&mut conn, movimento_id, usuario_id, &justificativa)
}

#[tauri::command(rename_all = "snake_case")]
pub fn listar_movimentos_do_dia(
    state: State<AppState>,
    armazem_id: i64,
    fluxo: String,
    data: String,
) -> AppResult<Vec<Movimento>> {
    let usuario_id = state.usuario_logado()?;
    let conn = state.conn()?;
    movimentos::autorizar_leitura(&conn, usuario_id, armazem_id)?;
    movimentos::listar_movimentos_do_dia(&conn, armazem_id, &fluxo, &data)
}

#[tauri::command]
pub fn sugestoes_descricao(state: State<AppState>, categoria: String) -> AppResult<Vec<String>> {
    state.usuario_logado()?;
    let conn = state.conn()?;
    movimentos::sugestoes_descricao(&conn, &categoria)
}

/// Usado pela tela de Saida de Armazem ao digitar o numero do pedido: avisa
/// se a retirada mais recente desse pedido ficou marcada como parcial, pra
/// alertar que pode ser a retirada complementar. `None` (sem alerta) tanto
/// se nunca houve pedido com esse numero quanto se a retirada mais recente
/// ja foi completa.
#[tauri::command(rename_all = "snake_case")]
pub fn verificar_retirada_pendente(
    state: State<AppState>,
    armazem_id: i64,
    fluxo: String,
    numero_pedido: String,
) -> AppResult<Option<Movimento>> {
    let usuario_id = state.usuario_logado()?;
    let conn = state.conn()?;
    movimentos::autorizar_leitura(&conn, usuario_id, armazem_id)?;
    movimentos::buscar_retirada_parcial_pendente(&conn, armazem_id, &fluxo, &numero_pedido)
}

#[tauri::command(rename_all = "snake_case")]
#[allow(clippy::too_many_arguments)]
pub fn buscar_historico(
    state: State<AppState>,
    armazem_id: i64,
    fluxo: String,
    data_inicio: Option<String>,
    data_fim: Option<String>,
    cliente: Option<String>,
    numero_pedido: Option<String>,
) -> AppResult<Vec<Movimento>> {
    let usuario_id = state.usuario_logado()?;
    let conn = state.conn()?;
    movimentos::autorizar_leitura(&conn, usuario_id, armazem_id)?;
    movimentos::buscar_historico(
        &conn,
        armazem_id,
        &fluxo,
        data_inicio.as_deref(),
        data_fim.as_deref(),
        cliente.as_deref(),
        numero_pedido.as_deref(),
    )
}
