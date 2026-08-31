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

/**
 * Mesma data no padrao brasileiro, mas com `-` no lugar de `/` - pra usar em
 * nome de arquivo exportado (CSV/XLSX). `/` quebraria o nome (viraria
 * separador de pasta), entao nao da pra usar `formatarData` direto aqui.
 */
export function formatarDataArquivo(data: string): string {
  return formatarData(data).replace(/\//g, '-');
}

/**
 * "Agora" no mesmo formato "AAAA-MM-DD HH:MM:SS" que o backend grava (SQLite
 * `datetime('now', 'localtime')`) - usa os getters locais do `Date` (nao
 * `toISOString`, que e UTC) pra bater com o horario que a conferente ve no
 * relogio dela, nao o horario de Greenwich.
 */
export function agoraLocalTexto(): string {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

/**
 * Intervalo (AAAA-MM-DD, inclusive dos dois lados) de uma quinzena de
 * pagamento: dia 1-15, ou dia 16 ate o ultimo dia do mes. `mesAno` vem no
 * formato de `<input type="month">` ("AAAA-MM"). O ultimo dia do mes usa o
 * truque `new Date(ano, mes, 0)` (dia 0 do mes seguinte = ultimo dia deste).
 */
export function intervaloQuinzena(
  mesAno: string,
  metade: 1 | 2
): { inicio: string; fim: string } {
  const [anoTexto, mesTexto] = mesAno.split('-');
  const ano = Number(anoTexto);
  const mes = Number(mesTexto);
  const pad = (n: number) => String(n).padStart(2, '0');

  if (metade === 1) {
    return { inicio: `${anoTexto}-${mesTexto}-01`, fim: `${anoTexto}-${mesTexto}-15` };
  }
  const ultimoDia = new Date(ano, mes, 0).getDate();
  return {
    inicio: `${anoTexto}-${mesTexto}-16`,
    fim: `${anoTexto}-${mesTexto}-${pad(ultimoDia)}`,
  };
}
