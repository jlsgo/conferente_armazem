import type { Movimento } from '../types';

/** Texto e classe CSS do badge de situacao, usado em toda tabela/impressao de movimentos. */
export function situacaoInfo(m: Pick<Movimento, 'tipo' | 'estornado_de'>): {
  texto: string;
  classe: string;
} {
  if (m.estornado_de) return { texto: 'ESTORNO', classe: 'badge badge-estorno' };
  if (m.tipo === 'saida') return { texto: 'BAIXA', classe: 'badge badge-saida' };
  return { texto: 'ENTRADA', classe: 'badge badge-entrada' };
}
