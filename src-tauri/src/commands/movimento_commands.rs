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
    pub itens: Vec<MovimentoItemInput>,
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
    let conn = state.conn()?;
    movimentos::listar_movimentos_do_dia(&conn, armazem_id, &fluxo, &data)
}

#[tauri::command]
pub fn sugestoes_descricao(state: State<AppState>, categoria: String) -> AppResult<Vec<String>> {
    let conn = state.conn()?;
    movimentos::sugestoes_descricao(&conn, &categoria)
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
    let conn = state.conn()?;
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
