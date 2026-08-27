export const BOM_UTF8 = '﻿';

// `;` como separador (nao `,`) porque o Excel em locale PT-BR usa `,` como
// separador decimal - abrir por duplo-clique quebraria colunas com numero.
// Sem BOM - use `paraCsv` pro arquivo final; esta versao existe pra montar
// varios blocos (um export consolidado com secoes) sem repetir o BOM no meio.
export function linhasParaCsv(cabecalhos: string[], linhas: string[][]): string {
  const escapar = (valor: string) => {
    if (/[;"\n]/.test(valor)) {
      return `"${valor.replace(/"/g, '""')}"`;
    }
    return valor;
  };
  const todasLinhas = [cabecalhos, ...linhas].map((l) => l.map(escapar).join(';'));
  return todasLinhas.join('\r\n');
}

export function paraCsv(cabecalhos: string[], linhas: string[][]): string {
  return BOM_UTF8 + linhasParaCsv(cabecalhos, linhas);
}

export function baixarCsv(nomeArquivo: string, conteudo: string): void {
  const blob = new Blob([conteudo], { type: 'text/csv;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = nomeArquivo;
  link.click();
  URL.revokeObjectURL(url);
}
