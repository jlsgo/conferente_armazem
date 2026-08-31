import { FormEvent, useEffect, useState } from 'react';
import type { Armazem, Fluxo, Movimento, Usuario } from '../types';
import { buscarFechamentoDoDia, buscarHistorico, estornarMovimento } from '../lib/api';
import { motivoSacTexto, situacaoInfo } from '../lib/situacao';
import { baixarCsv, paraCsv } from '../lib/csv';
import { baixarXlsx } from '../lib/xlsx';
import { agoraLocalTexto, formatarData, formatarDataArquivo, formatarDataHora } from '../lib/data';
import { resultadoReparoTexto } from '../lib/exportFechamento';
import Carregando from '../components/Carregando';
import RelatorioPagamentoReparo from '../components/RelatorioPagamentoReparo';

interface Props {
  usuario: Usuario;
  armazem: Armazem | undefined;
}

const ABAS: { valor: Fluxo; rotulo: string }[] = [
  { valor: 'saida_armazem', rotulo: 'Saida de Armazem' },
  { valor: 'peca_montagem', rotulo: 'Montagem' },
  { valor: 'sac', rotulo: 'SAC' },
  { valor: 'reparo_externo', rotulo: 'Reparo Externo' },
];

function dataDeHoje(): string {
  const agora = new Date();
  const ano = agora.getFullYear();
  const mes = String(agora.getMonth() + 1).padStart(2, '0');
  const dia = String(agora.getDate()).padStart(2, '0');
  return `${ano}-${mes}-${dia}`;
}

function dataHaDias(dias: number): string {
  const agora = new Date();
  agora.setDate(agora.getDate() - dias);
  const ano = agora.getFullYear();
  const mes = String(agora.getMonth() + 1).padStart(2, '0');
  const dia = String(agora.getDate()).padStart(2, '0');
  return `${ano}-${mes}-${dia}`;
}

function itensResumo(m: Movimento): string {
  return m.itens
    .map((it) => {
      const base = `${it.quantidade}x ${it.categoria}${it.descricao ? ' (' + it.descricao + ')' : ''}`;
      const comCodigo = it.codigo_componente ? `${base} [cod: ${it.codigo_componente}]` : base;
      const divergente = it.quantidade_enviada != null && it.quantidade_enviada !== it.quantidade;
      return divergente ? `${comCodigo} [enviado: ${it.quantidade_enviada}]` : comCodigo;
    })
    .join(' + ');
}

function pedidoTexto(m: Movimento): string {
  const base = m.numero_pedido || '-';
  return m.fluxo === 'saida_armazem' && !m.retirada_completa ? `${base} (parcial)` : base;
}

function qtdTotal(m: Movimento): number {
  return m.itens.reduce((s, it) => s + it.quantidade, 0);
}

function direcaoTexto(m: Movimento): string {
  return m.tipo === 'saida' ? 'Saida B2' : 'Entrada B2';
}

export default function Historico({ usuario, armazem }: Props) {
  const armazemId = usuario.armazem_id as number;

  const [fluxo, setFluxo] = useState<Fluxo>('saida_armazem');
  const [dataInicio, setDataInicio] = useState(dataHaDias(30));
  const [dataFim, setDataFim] = useState(dataDeHoje());
  const [cliente, setCliente] = useState('');
  const [numeroPedido, setNumeroPedido] = useState('');

  const [resultados, setResultados] = useState<Movimento[]>([]);
  const [temMais, setTemMais] = useState(false);
  const [carregando, setCarregando] = useState(true);
  const [carregandoMais, setCarregandoMais] = useState(false);
  // Separado em dois: erro de busca (bloqueia a lista, mostra "Tentar
  // novamente" chamando buscar() de novo) vs erro de acao (exportar/estornar
  // - um "Tentar novamente" generico ali chamaria buscar() por engano em vez
  // de reexecutar a acao que de fato falhou).
  const [erroBusca, setErroBusca] = useState('');
  const [erroAcao, setErroAcao] = useState('');
  const [estornando, setEstornando] = useState<number | null>(null);
  const [exportando, setExportando] = useState(false);
  const [exportandoXlsx, setExportandoXlsx] = useState(false);

  // Montagem nao tem contraparte/pedido; reparo externo tem contraparte
  // (tecnico/oficina) mas nunca preenche numero_pedido.
  const mostraFiltroCliente = fluxo !== 'peca_montagem';
  const mostraFiltroPedido = fluxo === 'saida_armazem' || fluxo === 'sac';

  async function buscarPagina(offset: number) {
    return buscarHistorico({
      armazem_id: armazemId,
      fluxo,
      data_inicio: dataInicio || null,
      data_fim: dataFim || null,
      cliente: mostraFiltroCliente && cliente ? cliente : null,
      numero_pedido: mostraFiltroPedido && numeroPedido ? numeroPedido : null,
      offset,
    });
  }

  async function buscar() {
    setCarregando(true);
    setErroBusca('');
    try {
      const resultado = await buscarPagina(0);
      setResultados(resultado.movimentos);
      setTemMais(resultado.tem_mais);
    } catch (err) {
      setErroBusca(typeof err === 'string' ? err : 'Nao foi possivel buscar o historico.');
    } finally {
      setCarregando(false);
    }
  }

  async function carregarMais() {
    setCarregandoMais(true);
    setErroBusca('');
    try {
      const resultado = await buscarPagina(resultados.length);
      setResultados((atual) => [...atual, ...resultado.movimentos]);
      setTemMais(resultado.tem_mais);
    } catch (err) {
      setErroBusca(typeof err === 'string' ? err : 'Nao foi possivel carregar mais resultados.');
    } finally {
      setCarregandoMais(false);
    }
  }

  useEffect(() => {
    buscar();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fluxo]);

  function handleSubmit(e: FormEvent) {
    e.preventDefault();
    buscar();
  }

  async function construirColunas(): Promise<{
    cabecalhos: string[];
    linhas: string[][];
    fechamentosPorData: Map<string, string>;
  }> {
    const datasUnicas = Array.from(new Set(resultados.map((m) => m.data)));
    const fechamentosPorData = new Map<string, string>();
    await Promise.all(
      datasUnicas.map(async (d) => {
        const fechamento = await buscarFechamentoDoDia({ armazem_id: armazemId, fluxo, data: d });
        fechamentosPorData.set(d, fechamento ? formatarDataHora(fechamento.criado_em) : '');
      })
    );
    const fechadoEm = (m: Movimento) => fechamentosPorData.get(m.data) || 'dia ainda aberto';

    let cabecalhos: string[];
    let linhas: string[][];

    if (fluxo === 'saida_armazem') {
        cabecalhos = [
          'Data',
          'Horario',
          'Pedido',
          'Coleta',
          'Itens',
          'Qtd.',
          'Quem retirou',
          'Registrado por',
          'Situacao',
          'Fechado em',
        ];
        linhas = resultados.map((m) => [
          formatarData(m.data),
          m.hora,
          pedidoTexto(m),
          m.contraparte || '-',
          itensResumo(m),
          String(qtdTotal(m)),
          m.quem_retirou || '-',
          m.usuario_nome,
          situacaoInfo(m).texto,
          fechadoEm(m),
        ]);
      } else if (fluxo === 'peca_montagem') {
        cabecalhos = ['Data', 'Horario', 'Direcao', 'Itens', 'Qtd.', 'Registrado por', 'Situacao', 'Fechado em'];
        linhas = resultados.map((m) => [
          formatarData(m.data),
          m.hora,
          direcaoTexto(m),
          itensResumo(m),
          String(qtdTotal(m)),
          m.usuario_nome,
          situacaoInfo(m).texto,
          fechadoEm(m),
        ]);
      } else if (fluxo === 'reparo_externo') {
        cabecalhos = [
          'Data',
          'Horario',
          'Tecnico/Oficina',
          'Itens',
          'Qtd.',
          'Resultado',
          'Registrado por',
          'Situacao',
          'Fechado em',
        ];
        linhas = resultados.map((m) => [
          formatarData(m.data),
          m.hora,
          m.contraparte || '-',
          itensResumo(m),
          String(qtdTotal(m)),
          resultadoReparoTexto(m),
          m.usuario_nome,
          situacaoInfo(m).texto,
          fechadoEm(m),
        ]);
      } else {
        cabecalhos = [
          'Data',
          'Horario',
          'Protocolo',
          'Coleta',
          'Itens',
          'Qtd.',
          'Motivo',
          'Registrado por',
          'Situacao',
          'Fechado em',
        ];
        linhas = resultados.map((m) => [
          formatarData(m.data),
          m.hora,
          m.numero_pedido || '-',
          m.contraparte || '-',
          itensResumo(m),
          String(qtdTotal(m)),
          motivoSacTexto(m),
          m.usuario_nome,
          situacaoInfo(m).texto,
          fechadoEm(m),
        ]);
      }

    return { cabecalhos, linhas, fechamentosPorData };
  }

  async function handleExportarCsv() {
    setExportando(true);
    setErroAcao('');
    try {
      const { cabecalhos, linhas } = await construirColunas();
      const csv = paraCsv(cabecalhos, linhas);
      baixarCsv(
        `historico_${fluxo}_${formatarDataArquivo(dataInicio)}_a_${formatarDataArquivo(dataFim)}.csv`,
        csv
      );
    } catch (err) {
      setErroAcao(typeof err === 'string' ? err : 'Nao foi possivel exportar o CSV.');
    } finally {
      setExportando(false);
    }
  }

  async function handleExportarXlsx() {
    setExportandoXlsx(true);
    setErroAcao('');
    try {
      const { cabecalhos, linhas, fechamentosPorData } = await construirColunas();
      const auditoria: string[][] = [
        ['Sistema', 'Ecoviva - Sistema de Controle de Armazens'],
        ['Periodo', `${formatarData(dataInicio)} a ${formatarData(dataFim)}`],
        ['Exportado em', formatarDataHora(agoraLocalTexto())],
        [],
        ['Data', 'Fechado em'],
        ...Array.from(fechamentosPorData.entries()).map(([d, fechado]) => [
          formatarData(d),
          fechado || 'dia ainda aberto',
        ]),
      ];
      await baixarXlsx(
        `historico_${fluxo}_${formatarDataArquivo(dataInicio)}_a_${formatarDataArquivo(dataFim)}.xlsx`,
        [
          { nome: 'Historico', cabecalhos, linhas },
          { nome: 'Auditoria', cabecalhos: ['Item', 'Valor'], linhas: auditoria },
        ]
      );
    } catch (err) {
      setErroAcao(typeof err === 'string' ? err : 'Nao foi possivel exportar o XLSX.');
    } finally {
      setExportandoXlsx(false);
    }
  }

  async function handleEstornar(movimento: Movimento) {
    const justificativa = window.prompt(
      `Justificativa para estornar o lancamento de ${formatarData(movimento.data)} (pedido ${movimento.numero_pedido ?? '-'}):`
    );
    if (!justificativa || !justificativa.trim()) return;

    setErroAcao('');
    setEstornando(movimento.id);
    const resultado = await estornarMovimento(movimento.id, justificativa);
    setEstornando(null);

    if (!resultado.ok) {
      setErroAcao(resultado.error ?? 'Nao foi possivel estornar o lancamento.');
      return;
    }

    await buscar();
  }

  const idsJaEstornados = new Set(
    resultados.filter((m) => m.estornado_de != null).map((m) => m.estornado_de)
  );

  const totalGeral = resultados.reduce(
    (soma, m) => soma + (m.estornado_de ? -1 : 1) * m.itens.reduce((s, it) => s + it.quantidade, 0),
    0
  );

  return (
    <div>
      <section className="cartao">
        <h2>Historico</h2>
        <p className="subtitulo">
          Consulte lancamentos de dias anteriores por periodo, cliente ou numero do pedido.
        </p>

        <div className="abas" style={{ marginBottom: 20 }}>
          {ABAS.map((a) => (
            <button
              key={a.valor}
              type="button"
              className={fluxo === a.valor ? 'ativo' : ''}
              onClick={() => setFluxo(a.valor)}
            >
              {a.rotulo}
            </button>
          ))}
        </div>

        <form onSubmit={handleSubmit}>
          <div className="grade-formulario">
            <label>
              De
              <input type="date" value={dataInicio} onChange={(e) => setDataInicio(e.target.value)} />
            </label>
            <label>
              Ate
              <input type="date" value={dataFim} onChange={(e) => setDataFim(e.target.value)} />
            </label>
            {mostraFiltroCliente && (
              <label>
                {fluxo === 'reparo_externo' ? 'Tecnico/oficina' : 'Cliente / coleta'}
                <input value={cliente} onChange={(e) => setCliente(e.target.value)} placeholder="Ex: Disk" />
              </label>
            )}
            {mostraFiltroPedido && (
              <label>
                Numero do pedido
                <input
                  value={numeroPedido}
                  onChange={(e) => setNumeroPedido(e.target.value)}
                  placeholder="Ex: 3893"
                />
              </label>
            )}
          </div>
          <div style={{ display: 'flex', gap: 10 }}>
            <button type="submit" disabled={carregando}>
              {carregando ? 'Buscando...' : 'Buscar'}
            </button>
            <button
              type="button"
              className="secundario"
              onClick={handleExportarCsv}
              disabled={carregando || exportando || resultados.length === 0}
            >
              {exportando ? 'Exportando...' : 'Exportar CSV'}
            </button>
            <button
              type="button"
              className="secundario"
              onClick={handleExportarXlsx}
              disabled={carregando || exportandoXlsx || resultados.length === 0}
            >
              {exportandoXlsx ? 'Exportando...' : 'Exportar XLSX'}
            </button>
          </div>
        </form>
      </section>

      <section className="cartao">
        {erroBusca && (
          <div>
            <p className="erro">{erroBusca}</p>
            <button type="button" onClick={buscar}>
              Tentar novamente
            </button>
          </div>
        )}
        {erroAcao && <p className="erro">{erroAcao}</p>}
        {carregando ? (
          <Carregando texto="Buscando..." />
        ) : (
          <>
            <div className="tabela-scroll">
              <table>
                <thead>
                  <tr>
                    <th>Data</th>
                    <th>Horario</th>
                    {fluxo === 'saida_armazem' && (
                      <>
                        <th>Pedido</th>
                        <th>Coleta</th>
                      </>
                    )}
                    {fluxo === 'peca_montagem' && <th>Direcao</th>}
                    {fluxo === 'sac' && (
                      <>
                        <th>Protocolo</th>
                        <th>Coleta</th>
                      </>
                    )}
                    {fluxo === 'reparo_externo' && <th>Tecnico/Oficina</th>}
                    <th>Itens</th>
                    <th>Qtd.</th>
                    {fluxo === 'saida_armazem' && <th>Quem retirou</th>}
                    {fluxo === 'sac' && <th>Motivo</th>}
                    {fluxo === 'reparo_externo' && <th>Resultado</th>}
                    <th>Registrado por</th>
                    <th>Situacao</th>
                    <th className="somente-tela">Acoes</th>
                  </tr>
                </thead>
                <tbody>
                  {resultados.map((m) => (
                    <tr key={m.id}>
                      <td>{formatarData(m.data)}</td>
                      <td>{m.hora}</td>
                      {fluxo === 'saida_armazem' && (
                        <>
                          <td>{pedidoTexto(m)}</td>
                          <td>{m.contraparte || '-'}</td>
                        </>
                      )}
                      {fluxo === 'peca_montagem' && <td>{direcaoTexto(m)}</td>}
                      {fluxo === 'sac' && (
                        <>
                          <td>{m.numero_pedido || '-'}</td>
                          <td>{m.contraparte || '-'}</td>
                        </>
                      )}
                      {fluxo === 'reparo_externo' && <td>{m.contraparte || '-'}</td>}
                      <td>{itensResumo(m)}</td>
                      <td>{qtdTotal(m)}</td>
                      {fluxo === 'saida_armazem' && <td>{m.quem_retirou || '-'}</td>}
                      {fluxo === 'sac' && <td>{motivoSacTexto(m)}</td>}
                      {fluxo === 'reparo_externo' && <td>{resultadoReparoTexto(m)}</td>}
                      <td>{m.usuario_nome}</td>
                      <td>
                        <span className={situacaoInfo(m).classe}>{situacaoInfo(m).texto}</span>
                      </td>
                      <td className="somente-tela">
                        {!m.estornado_de && !idsJaEstornados.has(m.id) && (
                          <button
                            type="button"
                            className="perigo"
                            onClick={() => handleEstornar(m)}
                            disabled={estornando === m.id}
                          >
                            {estornando === m.id ? 'Estornando...' : 'Estornar'}
                          </button>
                        )}
                      </td>
                    </tr>
                  ))}
                  {resultados.length === 0 && (
                    <tr>
                      <td colSpan={10} className="rodape-tabela">
                        Nenhum lancamento encontrado para esses filtros.
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
            <p className="rodape-tabela">
              <strong>{totalGeral}</strong> unidades no total ({resultados.length} lancamentos)
            </p>
            {temMais && (
              <button type="button" className="secundario" onClick={carregarMais} disabled={carregandoMais}>
                {carregandoMais ? 'Carregando...' : 'Carregar mais'}
              </button>
            )}
          </>
        )}
      </section>

      {fluxo === 'reparo_externo' && <RelatorioPagamentoReparo armazemId={armazemId} armazem={armazem} />}
    </div>
  );
}
