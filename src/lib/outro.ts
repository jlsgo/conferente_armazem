/**
 * "outro" nao diz por si so o que o item/motivo era de fato - todo campo que
 * aceita essa opcao (categoria, montagem, condicao, motivo do SAC) exige uma
 * observacao descrevendo o caso, tanto no frontend (aqui, feedback imediato)
 * quanto no backend (`domain::movimentos::exigir_detalhe_para_outro`, nunca
 * confia so na validacao da tela). Usado por Lancamentos.tsx/Montagem.tsx pra
 * decidir quando marcar o campo de observacao do item como obrigatorio.
 */
export function algumCampoEhOutro(...campos: (string | undefined)[]): boolean {
  return campos.some((c) => c === 'outro');
}
