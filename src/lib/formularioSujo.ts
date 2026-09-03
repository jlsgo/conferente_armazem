/**
 * Heuristica "tem algo digitado que nao e o estado inicial" - usada pelas 4
 * telas de lancamento (Lancamentos/Montagem/Sac/ReparoExterno) pra avisar
 * antes de trocar de aba e descartar o formulario em andamento
 * (`Dashboard.tsx`, prop `onSujoChange`). Nao precisa cobrir 100% dos campos,
 * so o suficiente pra nao apitar em falso num formulario realmente vazio:
 * mais de uma linha de item, ou qualquer item com descricao/observacao
 * preenchida ou quantidade diferente do padrao (1).
 */
export function itensPreenchidos(
  itens: { descricao?: string; observacao?: string | null; quantidade: number }[]
): boolean {
  if (itens.length > 1) return true;
  return itens.some(
    (it) => (it.descricao ?? '').trim() !== '' || (it.observacao ?? '').trim() !== '' || it.quantidade !== 1
  );
}
