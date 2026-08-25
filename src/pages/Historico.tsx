import { FormEvent, useEffect, useState } from 'react';
import type { Fluxo, Movimento, Usuario } from '../types';
import { buscarHistorico, estornarMovimento } from '../lib/api';
import { situacaoInfo } from '../lib/situacao';

interface Props {
  usuario: Usuario;
}

const ABAS: { valor: Fluxo; rotulo: string }[] = [
  { valor: 'saida_armazem', rotulo: 'Saida de Armazem' },
  { valor: 'peca_montagem', rotulo: 'Montagem' },
  { valor: 'sac', rotulo: 'SAC' },
];

const LIMITE_RESULTADOS = 500;

function formatarData(data: string): string {
  const [ano, mes, dia] = data.split('-');
  return `${dia}/${mes}/${ano}`;
}

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

export default function Historico({ usuario }: Props) {
  const armazemId = usuario.armazem_id as number;
  const ehGestor = usuario.papel === 'gestor';

  const [fluxo, setFluxo] = useState<Fluxo>('saida_armazem');
  const [dataInicio, setDataInicio] = useState(dataHaDias(30));
  const [dataFim, setDataFim] = useState(dataDeHoje());
  const [cliente, setCliente] = useState('');
  const [numeroPedido, setNumeroPedido] = useState('');

  const [resultados, setResultados] = useState<Movimento[]>([]);
  const [carregando, setCarregando] = useState(true);
  const [erro, setErro] = useState('');
  const [estornando, setEstornando] = useState<number | null>(null);

  const mostraFiltrosDePedido = fluxo !== 'peca_montagem';

  async function buscar() {
    setCarregando(true);
    setErro('');
    try {
      const lista = await buscarHistorico({
        armazem_id: armazemId,
        fluxo,
        data_inicio: dataInicio || null,
        data_fim: dataFim || null,
        cliente: mostraFiltrosDePedido && cliente ? cliente : null,
        numero_pedido: mostraFiltrosDePedido && numeroPedido ? numeroPedido : null,
      });
      setResultados(lista);
    } catch (err) {
      setErro(typeof err === 'string' ? err : 'Nao foi possivel buscar o historico.');
    } finally {
      setCarregando(false);
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

  async function handleEstornar(movimento: Movimento) {
    const justificativa = window.prompt(
      `Justificativa para estornar o lancamento de ${formatarData(movimento.data)} (pedido ${movimento.numero_pedido ?? '-'}):`
    );
    if (!justificativa || !justificativa.trim()) return;

    setErro('');
    setEstornando(movimento.id);
    const resultado = await estornarMovimento(movimento.id, justificativa);
    setEstornando(null);

    if (!resultado.ok) {
      setErro(resultado.error ?? 'Nao foi possivel estornar o lancamento.');
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
            {mostraFiltrosDePedido && (
              <>
                <label>
                  Cliente / coleta
                  <input value={cliente} onChange={(e) => setCliente(e.target.value)} placeholder="Ex: Disk" />
                </label>
                <label>
                  Numero do pedido
                  <input
                    value={numeroPedido}
                    onChange={(e) => setNumeroPedido(e.target.value)}
                    placeholder="Ex: 3893"
                  />
                </label>
              </>
            )}
          </div>
          <button type="submit" disabled={carregando}>
            {carregando ? 'Buscando...' : 'Buscar'}
          </button>
        </form>
      </section>

      <section className="cartao">
        {erro && <p className="erro">{erro}</p>}
        {carregando ? (
          <p className="carregando">Buscando...</p>
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
                    <th>Itens</th>
                    <th>Qtd.</th>
                    {fluxo === 'saida_armazem' && <th>Quem retirou</th>}
                    {fluxo === 'sac' && <th>Garantia/Venda</th>}
                    <th>Registrado por</th>
                    <th>Situacao</th>
                    {ehGestor && <th className="somente-tela">Acoes</th>}
                  </tr>
                </thead>
                <tbody>
                  {resultados.map((m) => (
                    <tr key={m.id}>
                      <td>{formatarData(m.data)}</td>
                      <td>{m.hora}</td>
                      {fluxo === 'saida_armazem' && (
                        <>
                          <td>{m.numero_pedido || '-'}</td>
                          <td>{m.contraparte || '-'}</td>
                        </>
                      )}
                      {fluxo === 'peca_montagem' && <td>{m.tipo === 'saida' ? 'Saida B2' : 'Entrada B2'}</td>}
                      {fluxo === 'sac' && (
                        <>
                          <td>{m.numero_pedido || '-'}</td>
                          <td>{m.contraparte || '-'}</td>
                        </>
                      )}
                      <td>
                        {m.itens
                          .map(
                            (it) =>
                              `${it.quantidade}x ${it.categoria}${it.descricao ? ' (' + it.descricao + ')' : ''}`
                          )
                          .join(' + ')}
                      </td>
                      <td>{m.itens.reduce((s, it) => s + it.quantidade, 0)}</td>
                      {fluxo === 'saida_armazem' && <td>{m.quem_retirou || '-'}</td>}
                      {fluxo === 'sac' && (
                        <td>
                          {m.motivo === 'venda'
                            ? `Venda (R$ ${((m.valor_centavos ?? 0) / 100).toFixed(2)})`
                            : m.motivo === 'garantia'
                              ? 'Garantia'
                              : '-'}
                        </td>
                      )}
                      <td>{m.usuario_nome}</td>
                      <td>
                        <span className={situacaoInfo(m).classe}>{situacaoInfo(m).texto}</span>
                      </td>
                      {ehGestor && (
                        <td className="somente-tela">
                          {!m.estornado_de && !idsJaEstornados.has(m.id) && (
                            <button
                              type="button"
                              className="secundario"
                              onClick={() => handleEstornar(m)}
                              disabled={estornando === m.id}
                            >
                              {estornando === m.id ? 'Estornando...' : 'Estornar'}
                            </button>
                          )}
                        </td>
                      )}
                    </tr>
                  ))}
                  {resultados.length === 0 && (
                    <tr>
                      <td colSpan={ehGestor ? 10 : 9} className="rodape-tabela">
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
            {resultados.length >= LIMITE_RESULTADOS && (
              <p className="rodape-tabela">
                Mostrando os {LIMITE_RESULTADOS} resultados mais recentes - refine os filtros para ver mais.
              </p>
            )}
          </>
        )}
      </section>
    </div>
  );
}
