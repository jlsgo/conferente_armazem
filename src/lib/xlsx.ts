// `xlsx` (SheetJS) 0.18.5: `npm audit` acusa uma pilha de vulnerabilidades de
// alta severidade (poluicao de prototipo / ReDoS), mas o proprio aviso do
// fabricante diz que fluxos que so EXPORTAM dados (nunca leem um arquivo
// arbitrario) nao sao afetados - e exatamente o nosso caso: so chamamos
// `aoa_to_sheet`/`write` sobre dados que a propria aplicacao gerou, nunca
// `read`/`readFile` sobre um arquivo de fora. Import dinamico pra nao inflar
// o bundle inicial de toda tela com uma lib de ~1MB usada so sob demanda.
export async function baixarXlsx(
  nomeArquivo: string,
  abas: { nome: string; cabecalhos: string[]; linhas: (string | number)[][] }[]
): Promise<void> {
  const XLSX = await import('xlsx');
  const wb = XLSX.utils.book_new();
  for (const aba of abas) {
    const ws = XLSX.utils.aoa_to_sheet([aba.cabecalhos, ...aba.linhas]);
    XLSX.utils.book_append_sheet(wb, ws, aba.nome.slice(0, 31));
  }
  const buffer = XLSX.write(wb, { type: 'array', bookType: 'xlsx' });
  const blob = new Blob([buffer], {
    type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = nomeArquivo;
  link.click();
  URL.revokeObjectURL(url);
}
