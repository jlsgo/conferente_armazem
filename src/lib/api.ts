import { invoke } from '@tauri-apps/api/core';
import type {
  AppStatus,
  Categoria,
  Fechamento,
  Fluxo,
  Movimento,
  NovoMovimento,
  NovoUsuarioInput,
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

export async function criarMovimento(payload: NovoMovimento): Promise<CriarMovimentoResult> {
  try {
    const movimento = await invoke<Movimento>('criar_movimento', { payload });
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
  usuario_id: number;
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

export async function criarUsuario(solicitanteId: number, payload: NovoUsuarioInput): Promise<OkResult> {
  try {
    await invoke('criar_usuario', { solicitante_id: solicitanteId, payload });
    return { ok: true };
  } catch (err) {
    return { ok: false, error: erroParaTexto(err) };
  }
}
