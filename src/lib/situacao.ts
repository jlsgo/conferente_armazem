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

/** Texto do motivo de um atendimento SAC (entrada: garantia/venda/outro; saida: entregue/descarte/garantia/venda/outro). */
export function motivoSacTexto(m: Pick<Movimento, 'motivo' | 'valor_centavos' | 'observacoes'>): string {
  switch (m.motivo) {
    case 'venda':
      return `Venda (R$ ${((m.valor_centavos ?? 0) / 100).toFixed(2)})`;
    case 'garantia':
      return 'Garantia';
    case 'entregue':
      return 'Entregue ao cliente';
    case 'descarte':
      return 'Descarte';
    case 'outro':
      return `Outro${m.observacoes ? ' - ' + m.observacoes : ''}`;
    default:
      return '-';
  }
}

export interface ResumoMovimentos {
  totalLancamentos: number;
  totalUnidades: number;
  porSituacao: Record<string, number>;
  porCategoria: Record<string, number>;
}

/**
 * Contadores por situacao (ENTRADA/BAIXA/ESTORNO) e por categoria de item -
 * usado tanto no resumo da folha impressa (`FechamentoImpressao.tsx`) quanto
 * na faixa de insights ao vivo nas telas de lancamento (`ResumoDoDia.tsx`),
 * pra nao duplicar a mesma conta em dois lugares.
 *
 * `porSituacao` conta linhas (um estorno e uma linha a mais no dia, mesma
 * logica do "X pedidos" que ja aparece embaixo da tabela em cada tela).
 * `porCategoria`/`totalUnidades` sao quantidade liquida: um estorno copia os
 * itens do original sem inverter a quantidade (`domain::movimentos::estornar_movimento`,
 * so o `estornado_de` marca a linha como reversao), entao aqui a quantidade
 * entra negativa quando `estornado_de` esta preenchido - mesma logica ja
 * usada em `totalGeral` (`FechamentoImpressao.tsx`), sem isso um dia com um
 * estorno mostraria a quantidade em dobro em vez de zerada.
 */
export function resumoMovimentos(lancamentos: Movimento[]): ResumoMovimentos {
  const porSituacao: Record<string, number> = {};
  const porCategoria: Record<string, number> = {};
  let totalUnidades = 0;

  for (const m of lancamentos) {
    const s = situacaoInfo(m).texto;
    porSituacao[s] = (porSituacao[s] ?? 0) + 1;
    const sinal = m.estornado_de ? -1 : 1;
    for (const it of m.itens) {
      porCategoria[it.categoria] = (porCategoria[it.categoria] ?? 0) + sinal * it.quantidade;
      totalUnidades += sinal * it.quantidade;
    }
  }

  return { totalLancamentos: lancamentos.length, totalUnidades, porSituacao, porCategoria };
}
