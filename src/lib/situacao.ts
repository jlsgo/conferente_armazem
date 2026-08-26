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

/** Texto do motivo de um atendimento SAC (entrada: garantia/venda; saida: entregue/descarte). */
export function motivoSacTexto(m: Pick<Movimento, 'motivo' | 'valor_centavos'>): string {
  switch (m.motivo) {
    case 'venda':
      return `Venda (R$ ${((m.valor_centavos ?? 0) / 100).toFixed(2)})`;
    case 'garantia':
      return 'Garantia';
    case 'entregue':
      return 'Entregue ao cliente';
    case 'descarte':
      return 'Descarte';
    default:
      return '-';
  }
}
