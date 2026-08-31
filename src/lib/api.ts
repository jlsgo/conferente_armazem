import { invoke } from '@tauri-apps/api/core';
import type {
  AppStatus,
  Categoria,
  Fechamento,
  Fluxo,
  Movimento,
  NovoMovimento,
  NovoUsuarioInput,
  ReparoConcluido,
  ReparoPendente,
  ResultadoHistorico,
  StatusSincronizacao,
  TransferenciaPendente,
  Usuario,
} from '../types';

export interface OkResult {
  ok: boolean;
  error?: string;
}

export interface LoginResult extends OkResult {
  usuario?: Usuario;
}

export interface CriarMovimentoResult extends OkResult {
  movimento?: Movimento;
}

export interface FecharDiaResult extends OkResult {
  fechamento?: Fechamento;
}

function erroParaTexto(err: unknown): string {
  if (typeof err === 'string') return err;
  if (err instanceof Error) return err.message;
  return 'Erro inesperado. Tente novamente.';
}

export function getStatus(): Promise<AppStatus> {
  return invoke<AppStatus>('get_status');
}

export async function setupPrimeiroUsuario(payload: {
  nome: string;
  login: string;
  senha: string;
  armazem_id: number | null;
}): Promise<OkResult> {
  try {
    await invoke('setup_primeiro_usuario', { payload });
    return { ok: true };
  } catch (err) {
    return { ok: false, error: erroParaTexto(err) };
  }
}

export async function login(payload: { login: string; senha: string }): Promise<LoginResult> {
  try {
    const usuario = await invoke<Usuario>('login', { payload });
    return { ok: true, usuario };
  } catch (err) {
    return { ok: false, error: erroParaTexto(err) };
  }
}

export async function logout(): Promise<void> {
  await invoke('logout');
}

export async function criarMovimento(payload: NovoMovimento): Promise<CriarMovimentoResult> {
  try {
    const movimento = await invoke<Movimento>('criar_movimento', { payload });
    return { ok: true, movimento };
  } catch (err) {
    return { ok: false, error: erroParaTexto(err) };
  }
}

export async function estornarMovimento(
  movimentoId: number,
  justificativa: string
): Promise<CriarMovimentoResult> {
  try {
    const movimento = await invoke<Movimento>('estornar_movimento', {
      movimento_id: movimentoId,
      justificativa,
    });
    return { ok: true, movimento };
  } catch (err) {
    return { ok: false, error: erroParaTexto(err) };
  }
}

export function listarMovimentosDoDia(params: {
  armazem_id: number;
  fluxo: Fluxo;
  data: string;
}): Promise<Movimento[]> {
  return invoke<Movimento[]>('listar_movimentos_do_dia', params);
}

export function sugestoesDescricao(categoria: Categoria): Promise<string[]> {
  return invoke<string[]>('sugestoes_descricao', { categoria });
}

export async function verificarRetiradaPendente(params: {
  armazem_id: number;
  fluxo: Fluxo;
  numero_pedido: string;
}): Promise<Movimento | null> {
  try {
    return await invoke<Movimento | null>('verificar_retirada_pendente', params);
  } catch {
    return null;
  }
}

export function buscarReparosEmAberto(armazemId: number): Promise<ReparoPendente[]> {
  return invoke<ReparoPendente[]>('buscar_reparos_em_aberto', { armazem_id: armazemId });
}

export function buscarReparosConcluidos(params: {
  armazem_id: number;
  data_inicio: string;
  data_fim: string;
}): Promise<ReparoConcluido[]> {
  return invoke<ReparoConcluido[]>('buscar_reparos_concluidos', params);
}

export function buscarHistorico(params: {
  armazem_id: number;
  fluxo: Fluxo;
  data_inicio?: string | null;
  data_fim?: string | null;
  cliente?: string | null;
  numero_pedido?: string | null;
  offset: number;
}): Promise<ResultadoHistorico> {
  return invoke<ResultadoHistorico>('buscar_historico', params);
}

export function buscarFechamentoDoDia(params: {
  armazem_id: number;
  fluxo: Fluxo;
  data: string;
}): Promise<Fechamento | null> {
  return invoke<Fechamento | null>('buscar_fechamento_do_dia', params);
}

export async function fecharDia(params: {
  armazem_id: number;
  fluxo: Fluxo;
  data: string;
}): Promise<FecharDiaResult> {
  try {
    const fechamento = await invoke<Fechamento>('fechar_dia', { payload: params });
    return { ok: true, fechamento };
  } catch (err) {
    return { ok: false, error: erroParaTexto(err) };
  }
}

export function listarUsuarios(armazemId?: number | null): Promise<Usuario[]> {
  return invoke<Usuario[]>('listar_usuarios', { armazem_id: armazemId ?? null });
}

export async function criarUsuario(payload: NovoUsuarioInput): Promise<OkResult> {
  try {
    await invoke('criar_usuario', { payload });
    return { ok: true };
  } catch (err) {
    return { ok: false, error: erroParaTexto(err) };
  }
}

export interface SincronizarResult extends OkResult {
  mensagem?: string;
}

export async function sincronizarAgora(): Promise<SincronizarResult> {
  try {
    const mensagem = await invoke<string>('sincronizar_agora');
    return { ok: true, mensagem };
  } catch (err) {
    return { ok: false, error: erroParaTexto(err) };
  }
}

export async function statusSincronizacao(): Promise<StatusSincronizacao | null> {
  try {
    return await invoke<StatusSincronizacao>('status_sincronizacao');
  } catch {
    return null;
  }
}

export function buscarTransferenciasPendentes(): Promise<TransferenciaPendente[]> {
  // "Sem sync configurado" ja volta Ok([]) do lado do Rust (nao rejeita) - so
  // chega aqui como rejeicao uma falha de verdade (rede/IPC), que o chamador
  // deve tratar (ver carregarPendentes em Montagem.tsx).
  return invoke<TransferenciaPendente[]>('buscar_transferencias_pendentes');
}

export interface ConfirmarRecebimentoResult extends OkResult {
  movimento?: Movimento;
}

export async function confirmarRecebimento(
  origemArmazemCodigo: string,
  origemId: number,
  hora: string,
  quantidadesRecebidas: number[]
): Promise<ConfirmarRecebimentoResult> {
  try {
    const movimento = await invoke<Movimento>('confirmar_recebimento', {
      origem_armazem_codigo: origemArmazemCodigo,
      origem_id: origemId,
      hora,
      quantidades_recebidas: quantidadesRecebidas,
    });
    return { ok: true, movimento };
  } catch (err) {
    return { ok: false, error: erroParaTexto(err) };
  }
}
