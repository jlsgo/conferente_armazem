/**
 * Formatacao de data pro padrao brasileiro (DD/MM/AAAA) - o banco/backend
 * sempre guarda e manda data em AAAA-MM-DD (ou "AAAA-MM-DD HH:MM:SS" pra
 * timestamps), formato ISO que nao e o que a conferente le no dia a dia.
 * Manipulacao pura de string (sem `Date`/`toLocaleDateString`) para nao
 * arriscar um desvio de fuso horario - a string ja representa a data certa,
 * so precisa trocar a ordem dos campos.
 */
export function formatarData(data: string): string {
  const [ano, mes, dia] = data.split('-');
  return `${dia}/${mes}/${ano}`;
}

/** Mesma ideia, para timestamps completos ("AAAA-MM-DD HH:MM:SS"). */
export function formatarDataHora(dataHora: string): string {
  const [data, hora] = dataHora.split(' ');
  return hora ? `${formatarData(data)} ${hora}` : formatarData(data);
}
