import { useState } from 'react';
import type { Armazem, ReparoConcluido } from '../types';
import { buscarReparosConcluidos } from '../lib/api';
import {
  agoraLocalTexto,
  formatarData,
  formatarDataArquivo,
  formatarDataHora,
  intervaloQuinzena,
} from '../lib/data';
import { paraCsv, baixarCsv } from '../lib/csv';
import { baixarXlsx } from '../lib/xlsx';

interface Props {
  armazemId: number;
  armazem: Armazem | undefined;
}

interface ResumoTecnico {
  tecnico: string;
  quantidade: number;
}

const CABECALHOS_DETALHE = [
  'Data retorno',
  'Horario',
  'Codigo',
  'Item',
  'Tecnico/Oficina',
  'Data saida',
  'Observacao',
];

function mesAtual(): string {
  const agora = new Date();
  return `${agora.getFullYear()}-${String(agora.getMonth() + 1).padStart(2, '0')}`;
}

function linhaDetalhe(r: ReparoConcluido): string[] {
  return [
    formatarData(r.data_entrada),
    r.hora_entrada,
    r.codigo_componente,
    `${r.quantidade}x ${r.categoria}${r.descricao ? ' (' + r.descricao + ')' : ''}`,
    r.contraparte || '-',
    formatarData(r.data_saida),
    r.observacao_entrada || '-',
  ];
}

/**
 * Relatorio de pagamento por quinzena do tecnico externo: reparos que
 * voltaram consertados (`ReparoConcluido`, backend ja filtra
 * `condicao = 'boa'`) num intervalo dia 1-15 ou 16-fim do mes. So aparece na
 * aba de Historico quando `fluxo === 'reparo_externo'` - complementa a
 * tabela de movimentacoes do dia a dia, nao a substitui.
 */
export default function RelatorioPagamentoReparo({ armazemId, armazem }: Props) {
  const [mesAno, setMesAno] = useState(mesAtual());
  const [metade, setMetade] = useState<1 | 2>(1);
  const [resultados, setResultados] = useState<ReparoConcluido[] | null>(null);
  const [carregando, setCarregando] = useState(false);
  const [erro, setErro] = useState('');
  const [exportandoXlsx, setExportandoXlsx] = useState(false);

  const { inicio, fim } = intervaloQuinzena(mesAno, metade);

  async function gerar() {
    setCarregando(true);
    setErro('');
    try {
      const dados = await buscarReparosConcluidos({
        armazem_id: armazemId,
        data_inicio: inicio,
        data_fim: fim,
      });
      setResultados(dados);
    } catch (err) {
      setErro(typeof err === 'string' ? err : 'Nao foi possivel gerar o relatorio.');
    } finally {
      setCarregando(false);
    }
  }

  const resumoPorTecnico: ResumoTecnico[] = [];
  if (resultados) {
    const contagem = new Map<string, number>();
    for (const r of resultados) {
      const tecnico = r.contraparte || 'Sem tecnico informado';
      contagem.set(tecnico, (contagem.get(tecnico) ?? 0) + 1);
    }
    for (const [tecnico, quantidade] of contagem.entries()) {
      resumoPorTecnico.push({ tecnico, quantidade });
    }
    resumoPorTecnico.sort((a, b) => a.tecnico.localeCompare(b.tecnico));
  }

  function nomeBase(extensao: string): string {
    return `pagamento_reparo_externo_${armazem?.codigo ?? 'armazem'}_${formatarDataArquivo(inicio)}_a_${formatarDataArquivo(fim)}.${extensao}`;
  }

  function handleExportarCsv() {
    const linhasResumo: string[][] = [
      [],
      ['Resumo por tecnico', ''],
      ...resumoPorTecnico.map((r) => [r.tecnico, String(r.quantidade)]),
    ];
    const linhas = [...(resultados ?? []).map(linhaDetalhe), ...linhasResumo];
    baixarCsv(nomeBase('csv'), paraCsv(CABECALHOS_DETALHE, linhas));
  }

  async function handleExportarXlsx() {
    setExportandoXlsx(true);
    try {
      await baixarXlsx(nomeBase('xlsx'), [
        {
          nome: 'Resumo',
          cabecalhos: ['Tecnico/Oficina', 'Reparos concluidos'],
          linhas: resumoPorTecnico.map((r) => [r.tecnico, r.quantidade]),
        },
        {
          nome: 'Detalhe',
          cabecalhos: CABECALHOS_DETALHE,
          linhas: (resultados ?? []).map(linhaDetalhe),
        },
      ]);
    } finally {
      setExportandoXlsx(false);
    }
  }

  return (
    <>
      <section className="cartao somente-tela">
        <h2>Relatorio de pagamento - Reparo Externo</h2>
        <p className="subtitulo">
          Lista os reparos que voltaram consertados numa quinzena, pra pagar o tecnico externo
          corretamente.
        </p>
        <div className="grade-formulario">
          <label>
            Mes
            <input type="month" value={mesAno} onChange={(e) => setMesAno(e.target.value)} />
          </label>
          <label>
            Quinzena
            <select value={metade} onChange={(e) => setMetade(Number(e.target.value) as 1 | 2)}>
              <option value={1}>1ª quinzena (dia 1 a 15)</option>
              <option value={2}>2ª quinzena (dia 16 ao fim do mes)</option>
            </select>
          </label>
        </div>
        <button type="button" onClick={gerar} disabled={carregando}>
          {carregando ? 'Gerando...' : 'Gerar relatorio'}
        </button>
        {erro && <p className="erro">{erro}</p>}
      </section>

      {resultados && (
        <section className="cartao area-impressao">
          <h2>
            Relatorio de pagamento - Reparo Externo
            {armazem ? ` (${armazem.codigo})` : ''}
          </h2>
          <p className="subtitulo">
            Quinzena de {formatarData(inicio)} a {formatarData(fim)}
          </p>

          <h3>Resumo por tecnico</h3>
          <div className="tabela-scroll">
            <table>
              <thead>
                <tr>
                  <th>Tecnico/Oficina</th>
                  <th>Reparos concluidos</th>
                </tr>
              </thead>
              <tbody>
                {resumoPorTecnico.map((r) => (
                  <tr key={r.tecnico}>
                    <td>{r.tecnico}</td>
                    <td>{r.quantidade}</td>
                  </tr>
                ))}
                {resumoPorTecnico.length === 0 && (
                  <tr>
                    <td colSpan={2} className="rodape-tabela">
                      Nenhum reparo concluido nesta quinzena.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>

          <h3>Detalhe</h3>
          <div className="tabela-scroll">
            <table>
              <thead>
                <tr>
                  {CABECALHOS_DETALHE.map((c) => (
                    <th key={c}>{c}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {resultados.map((r) => (
                  <tr key={r.item_id_saida}>
                    {linhaDetalhe(r).map((valor, i) => (
                      <td key={i}>{valor}</td>
                    ))}
                  </tr>
                ))}
                {resultados.length === 0 && (
                  <tr>
                    <td colSpan={CABECALHOS_DETALHE.length} className="rodape-tabela">
                      Nenhum reparo concluido nesta quinzena.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>

          <p className="rodape-tabela">Gerado em {formatarDataHora(agoraLocalTexto())}</p>

          <div className="somente-tela" style={{ display: 'flex', gap: 10, flexWrap: 'wrap', marginTop: 20 }}>
            <button type="button" onClick={() => window.print()}>
              Imprimir / Salvar como PDF
            </button>
            <button type="button" className="secundario" onClick={handleExportarCsv}>
              Exportar CSV
            </button>
            <button type="button" className="secundario" onClick={handleExportarXlsx} disabled={exportandoXlsx}>
              {exportandoXlsx ? 'Exportando...' : 'Exportar XLSX'}
            </button>
          </div>
        </section>
      )}
    </>
  );
}
