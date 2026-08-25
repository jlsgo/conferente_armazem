const BOM_UTF8 = '﻿';

// `;` como separador (nao `,`) porque o Excel em locale PT-BR usa `,` como
// separador decimal - abrir por duplo-clique quebraria colunas com numero.
export function paraCsv(cabecalhos: string[], linhas: string[][]): string {
  const escapar = (valor: string) => {
    if (/[;"\n]/.test(valor)) {
      return `"${valor.replace(/"/g, '""')}"`;
    }
    return valor;
  };
  const todasLinhas = [cabecalhos, ...linhas].map((l) => l.map(escapar).join(';'));
  return BOM_UTF8 + todasLinhas.join('\r\n');
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
