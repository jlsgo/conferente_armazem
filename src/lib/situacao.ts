import type { Armazem, Montagem, Movimento } from '../types';

/** Texto e classe CSS do badge de situacao, usado em toda tabela/impressao de movimentos. */
export function situacaoInfo(
  m: Pick<Movimento, 'tipo' | 'estornado_de' | 'motivo' | 'recebido_de_armazem_codigo'>
): {
  texto: string;
  classe: string;
} {
  if (m.estornado_de) return { texto: 'ESTORNO', classe: 'badge badge-estorno' };
  // Sentinela gravado por `recusar_recebimento` (ver MOTIVO_RECUSA_RECEBIMENTO
  // no backend) - so faz sentido junto com `recebido_de_armazem_codigo`
  // preenchido, entao nunca colide com um motivo de SAC de verdade.
  if (m.motivo === 'recusado' && m.recebido_de_armazem_codigo) {
    return { texto: 'RECUSADO', classe: 'badge badge-recusado' };
  }
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

/**
 * Coluna "Coleta" de Saida de Armazem e SAC (as duas telas com o mesmo
 * significado pro campo: pra quem/onde a peca foi - Correios/cliente por
 * padrao, ou o outro armazem quando e uma transferencia). Antes duplicada
 * (quase) identica em `Lancamentos.tsx` e `Sac.tsx`; extraida pra um lugar so
 * ao estender pra `FechamentoImpressao.tsx`/`exportFechamento.ts`/
 * `Historico.tsx`, que mostravam so `contraparte` cru e perdiam a direcao da
 * transferencia no documento impresso/exportado.
 */
export function colunaColeta(
  m: Pick<Movimento, 'armazem_destino_id' | 'recebido_de_armazem_codigo' | 'contraparte'>,
  armazens: Pick<Armazem, 'id' | 'codigo'>[]
): string {
  if (m.armazem_destino_id != null) {
    return `Enviado para ${armazens.find((a) => a.id === m.armazem_destino_id)?.codigo ?? '?'}`;
  }
  if (m.recebido_de_armazem_codigo) return `Recebido de ${m.recebido_de_armazem_codigo}`;
  return m.contraparte || '-';
}

/**
 * Texto "Estorno do Nº X (pedido Y) - motivo" pra uma linha de estorno, ou `null`
 * se `m` nao for um estorno. `todos` e a lista de lancamentos onde procurar o
 * original pelo `id` (o dia inteiro, ou o resultado de uma busca no Historico) -
 * o `numero` exibido nas telas so existe calculado ali dentro, nao em `m.estornado_de`
 * (que so guarda o `id` interno do banco). Se o original nao estiver em `todos`
 * (por exemplo, paginacao do Historico separou as duas linhas), cai pra um texto
 * generico em vez de nao mostrar nada.
 */
export function detalheEstorno(
  m: Pick<Movimento, 'estornado_de' | 'observacoes'>,
  todos: Pick<Movimento, 'id' | 'numero' | 'numero_pedido'>[]
): string | null {
  if (!m.estornado_de) return null;
  const original = todos.find((o) => o.id === m.estornado_de);
  const alvo = original
    ? `Nº ${original.numero}${original.numero_pedido ? ` (pedido ${original.numero_pedido})` : ''}`
    : 'um lancamento anterior';
  return `ESTORNO do ${alvo}${m.observacoes ? ` - ${m.observacoes}` : ''}`;
}

/** "Montado"/"Em caixa" pro `montagem` de um item (Saida de Armazem e Montagem,
 * unico lugar do frontend que traduz esse valor - usado em `itensResumoTexto`
 * abaixo e no formulario de cada tela). */
export function montagemTexto(montagem: Montagem | null | undefined): string | null {
  if (montagem === 'montado') return 'Montado';
  if (montagem === 'caixa') return 'Em caixa';
  return null;
}

/**
 * Texto "2x categoria (descricao) - observacao [Montado] [cod: X] [enviado: N]"
 * por item, unido por " + ", com o detalhe do estorno (`detalheEstorno`)
 * acrescentado no final quando `m` for um estorno - usado em toda
 * tela/impressao/export que lista lancamentos (telas do dia, Historico,
 * fechamento impresso, CSV/XLSX). Antes reimplementada quase-identica (e
 * inconsistente: observacao do item sumia em algumas) em 6 lugares diferentes -
 * consolidada aqui pelo mesmo motivo de `colunaColeta`/`motivoSacTexto`/
 * `resultadoReparoTexto` acima. Nao inclui `condicao`: essas telas ja tem
 * coluna dedicada propria pra isso (`Condicao` em Montagem/FechamentoImpressao,
 * `resultadoReparoTexto`). `montagem` (montado/em caixa) nao tem coluna
 * dedicada em nenhuma tela - fica inline aqui, unico jeito de saber se um
 * veiculo saiu montado ou em caixa depois do lancamento (achado real: essa
 * informacao era so preenchida no formulario e nunca aparecia em lugar
 * nenhum depois - nem aqui, nem no painel web).
 */
export function itensResumoTexto(m: Movimento, todos: Movimento[]): string {
  const itens = m.itens
    .map((it) => {
      const base = `${it.quantidade}x ${it.categoria}${it.descricao ? ' (' + it.descricao + ')' : ''}${it.observacao ? ' - ' + it.observacao : ''}`;
      const montagem = montagemTexto(it.montagem);
      const comMontagem = montagem ? `${base} [${montagem}]` : base;
      const comCodigo = it.codigo_componente ? `${comMontagem} [cod: ${it.codigo_componente}]` : comMontagem;
      const divergente = it.quantidade_enviada != null && it.quantidade_enviada !== it.quantidade;
      return divergente ? `${comCodigo} [enviado: ${it.quantidade_enviada}]` : comCodigo;
    })
    .join(' + ');
  const estorno = detalheEstorno(m, todos);
  return estorno ? `${itens} | ${estorno}` : itens;
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
