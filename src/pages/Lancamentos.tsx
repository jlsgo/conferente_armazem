import { FormEvent, useEffect, useState } from 'react';
import type { Armazem, Categoria, Fechamento, Montagem, Movimento, MovimentoItemInput, TipoMovimento, Usuario } from '../types';
import {
  criarMovimento,
  buscarFechamentoDoDia,
  estornarMovimento,
  fecharDia,
  listarMovimentosDoDia,
  sugestoesDescricao,
} from '../lib/api';
import FechamentoImpressao from '../components/FechamentoImpressao';
import { situacaoInfo } from '../lib/situacao';

interface Props {
  usuario: Usuario;
  armazem: Armazem | undefined;
}

interface ItemForm {
  categoria: Categoria;
  descricao: string;
  montagem: Montagem | '';
  quantidade: number;
  observacao: string;
}

const CATEGORIAS: { valor: Categoria; rotulo: string }[] = [
  { valor: 'scooter', rotulo: 'Scooter' },
  { valor: 'triciclo', rotulo: 'Triciclo' },
  { valor: 'patinete', rotulo: 'Patinete' },
  { valor: 'peca', rotulo: 'Peca' },
];

function dataDeHoje(): string {
  const agora = new Date();
  const ano = agora.getFullYear();
  const mes = String(agora.getMonth() + 1).padStart(2, '0');
  const dia = String(agora.getDate()).padStart(2, '0');
  return `${ano}-${mes}-${dia}`;
}

function horaAtual(): string {
  const agora = new Date();
  return `${String(agora.getHours()).padStart(2, '0')}:${String(agora.getMinutes()).padStart(2, '0')}`;
}

function novoItemVazio(): ItemForm {
  return { categoria: 'scooter', descricao: '', montagem: '', quantidade: 1, observacao: '' };
}

export default function Lancamentos({ usuario, armazem }: Props) {
  const armazemId = usuario.armazem_id as number;
  const data = dataDeHoje();

  const [lancamentos, setLancamentos] = useState<Movimento[]>([]);
  const [fechamento, setFechamento] = useState<Fechamento | null>(null);
  const [carregandoLista, setCarregandoLista] = useState(true);
  const [sugestoesPorCategoria, setSugestoesPorCategoria] = useState<Partial<Record<Categoria, string[]>>>({});

  const [tipo, setTipo] = useState<TipoMovimento>('saida');
  const [hora, setHora] = useState(horaAtual());
  const [turno, setTurno] = useState<'diurno' | 'noturno'>('diurno');
  const [numeroPedido, setNumeroPedido] = useState('');
  const [codigoRastreio, setCodigoRastreio] = useState('');
  const [contraparte, setContraparte] = useState('');
  const [quemRetirou, setQuemRetirou] = useState('');
  const [observacoes, setObservacoes] = useState('');
  const [itens, setItens] = useState<ItemForm[]>([novoItemVazio()]);
  const [erro, setErro] = useState('');
  const [enviando, setEnviando] = useState(false);
  const [fechando, setFechando] = useState(false);
  const [estornando, setEstornando] = useState<number | null>(null);
  const ehGestor = usuario.papel === 'gestor';

  async function carregarTudo() {
    setCarregandoLista(true);
    const [lista, fechamentoDoDia] = await Promise.all([
      listarMovimentosDoDia({ armazem_id: armazemId, fluxo: 'saida_armazem', data }),
      buscarFechamentoDoDia({ armazem_id: armazemId, fluxo: 'saida_armazem', data }),
    ]);
    setLancamentos(lista);
    setFechamento(fechamentoDoDia);
    setCarregandoLista(false);
  }

  async function garantirSugestoes(categoria: Categoria) {
    if (sugestoesPorCategoria[categoria]) return;
    const lista = await sugestoesDescricao(categoria);
    setSugestoesPorCategoria((atual) => ({ ...atual, [categoria]: lista }));
  }

  useEffect(() => {
    carregarTudo();
    garantirSugestoes('scooter');
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function atualizarItem(indice: number, alteracoes: Partial<ItemForm>) {
    setItens((atual) => atual.map((it, i) => (i === indice ? { ...it, ...alteracoes } : it)));
    if (alteracoes.categoria) garantirSugestoes(alteracoes.categoria);
  }

  function adicionarLinhaItem() {
    setItens((atual) => [...atual, novoItemVazio()]);
  }

  function removerLinhaItem(indice: number) {
    setItens((atual) => (atual.length === 1 ? atual : atual.filter((_, i) => i !== indice)));
  }

  function limparFormulario() {
    // O horario NAO reseta para "agora": nas planilhas antigas varios pedidos
    // seguidos do mesmo lote sao registrados com o mesmo horario (a conferente
    // carimba o lote todo de uma vez). Manter o ultimo valor digitado evita
    // que ela tenha que reajustar o campo a cada lancamento do mesmo lote.
    setNumeroPedido('');
    setCodigoRastreio('');
    setContraparte('');
    setQuemRetirou('');
    setObservacoes('');
    setItens([novoItemVazio()]);
  }

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setErro('');

    const itensValidos: MovimentoItemInput[] = itens
      .filter((it) => it.quantidade > 0)
      .map((it) => ({
        categoria: it.categoria,
        descricao: it.descricao.trim() || null,
        montagem: it.montagem || null,
        quantidade: it.quantidade,
        observacao: it.observacao.trim() || null,
      }));

    if (itensValidos.length === 0) {
      setErro('Informe ao menos um item com quantidade valida.');
      return;
    }

    setEnviando(true);
    const resultado = await criarMovimento({
      armazem_id: armazemId,
      fluxo: 'saida_armazem',
      tipo,
      data,
      hora,
      turno,
      numero_pedido: numeroPedido || null,
      codigo_rastreio: codigoRastreio || null,
      contraparte: contraparte || null,
      quem_retirou: quemRetirou || null,
      observacoes: observacoes || null,
      itens: itensValidos,
    });
    setEnviando(false);

    if (!resultado.ok) {
      setErro(resultado.error ?? 'Nao foi possivel registrar o lancamento.');
      return;
    }

    limparFormulario();
    await carregarTudo();
  }

  async function handleFecharDia() {
    if (lancamentos.length === 0) return;
    const confirmado = window.confirm(
      `Fechar o dia ${data}? Depois disso nao sera mais possivel adicionar ou corrigir lancamentos deste dia neste armazem.`
    );
    if (!confirmado) return;

    setErro('');
    setFechando(true);
    const resultado = await fecharDia({ armazem_id: armazemId, fluxo: 'saida_armazem', data });
    setFechando(false);

    if (!resultado.ok) {
      setErro(resultado.error ?? 'Nao foi possivel fechar o dia.');
      return;
    }

    await carregarTudo();
  }

  async function handleEstornar(movimento: Movimento) {
    const justificativa = window.prompt(
      `Justificativa para estornar o lancamento nº ${movimento.numero} (pedido ${movimento.numero_pedido ?? '-'}):`
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

    await carregarTudo();
  }

  const idsJaEstornados = new Set(
    lancamentos.filter((m) => m.estornado_de != null).map((m) => m.estornado_de)
  );

  const totalGeralDoDia = lancamentos.reduce(
    (soma, m) => soma + (m.estornado_de ? -1 : 1) * m.itens.reduce((s, it) => s + it.quantidade, 0),
    0
  );

  if (carregandoLista) {
    return <p className="carregando">Carregando...</p>;
  }

  if (fechamento) {
    return (
      <div>
        <p className="aviso-fechado">
          O dia {data} ja foi fechado por {fechamento.usuario_nome}. Os lancamentos abaixo sao somente leitura.
        </p>
        {erro && <p className="erro">{erro}</p>}
        {ehGestor && (
          <section className="cartao somente-tela">
            <h2>Corrigir um lancamento deste dia</h2>
            <p className="subtitulo">
              O dia esta fechado, mas um erro ainda pode ser corrigido por estorno (o lancamento
              original nunca e editado ou apagado).
            </p>
            <table>
              <tbody>
                {lancamentos
                  .filter((m) => !m.estornado_de && !idsJaEstornados.has(m.id))
                  .map((m) => (
                    <tr key={m.id}>
                      <td>
                        Nº {m.numero} - pedido {m.numero_pedido || '-'} -{' '}
                        {m.itens.reduce((s, it) => s + it.quantidade, 0)} un.
                      </td>
                      <td>
                        <button
                          type="button"
                          className="secundario"
                          onClick={() => handleEstornar(m)}
                          disabled={estornando === m.id}
                        >
                          {estornando === m.id ? 'Estornando...' : 'Estornar'}
                        </button>
                      </td>
                    </tr>
                  ))}
                {lancamentos.every((m) => m.estornado_de || idsJaEstornados.has(m.id)) && (
                  <tr>
                    <td className="rodape-tabela">Nenhum lancamento disponivel para estorno.</td>
                  </tr>
                )}
              </tbody>
            </table>
          </section>
        )}
        <FechamentoImpressao armazem={armazem} data={data} fechamento={fechamento} lancamentos={lancamentos} />
      </div>
    );
  }

  return (
    <div>
      <section className="cartao">
        <h2>Registrar {tipo === 'saida' ? 'saida' : 'entrada'} do armazem</h2>
        <p className="subtitulo">
          {data} - responsavel: {usuario.nome}. O detalhe completo do pedido fica na outra
          ferramenta - aqui basta o numero do pedido e a quantidade.
        </p>

        <form onSubmit={handleSubmit}>
          <div className="abas" style={{ marginBottom: 20 }}>
            <button type="button" className={tipo === 'saida' ? 'ativo' : ''} onClick={() => setTipo('saida')}>
              Saida
            </button>
            <button type="button" className={tipo === 'entrada' ? 'ativo' : ''} onClick={() => setTipo('entrada')}>
              Entrada
            </button>
          </div>

          <div className="grade-formulario">
            <label>
              Numero do pedido
              <input
                value={numeroPedido}
                onChange={(e) => setNumeroPedido(e.target.value)}
                placeholder="Ex: 3932"
                required
              />
            </label>

            <label>
              Horario
              <input type="time" value={hora} onChange={(e) => setHora(e.target.value)} required />
            </label>

            <label>
              Turno
              <select value={turno} onChange={(e) => setTurno(e.target.value as 'diurno' | 'noturno')}>
                <option value="diurno">Diurno</option>
                <option value="noturno">Noturno</option>
              </select>
            </label>

            <label>
              Coleta (transportadora / cliente)
              <input value={contraparte} onChange={(e) => setContraparte(e.target.value)} />
            </label>

            <label>
              Quem retirou
              <input value={quemRetirou} onChange={(e) => setQuemRetirou(e.target.value)} />
            </label>

            <label>
              Codigo de rastreio
              <input value={codigoRastreio} onChange={(e) => setCodigoRastreio(e.target.value)} />
            </label>
          </div>

          <label>
            Observacoes do pedido
            <textarea
              value={observacoes}
              onChange={(e) => setObservacoes(e.target.value)}
              rows={2}
              placeholder="Opcional - qualquer detalhe que nao caiba nos campos acima"
            />
          </label>

          <h3>Itens deste pedido</h3>
          {itens.map((item, indice) => (
            <div className="linha-item linha-item-veiculo" key={indice}>
              <select
                value={item.categoria}
                onChange={(e) => atualizarItem(indice, { categoria: e.target.value as Categoria })}
              >
                {CATEGORIAS.map((c) => (
                  <option key={c.valor} value={c.valor}>
                    {c.rotulo}
                  </option>
                ))}
              </select>

              <input
                value={item.descricao}
                onChange={(e) => atualizarItem(indice, { descricao: e.target.value })}
                placeholder="Detalhe opcional (ex: HE-15 GREEN)"
                list={`sugestoes-${item.categoria}`}
              />
              <datalist id={`sugestoes-${item.categoria}`}>
                {(sugestoesPorCategoria[item.categoria] ?? []).map((s) => (
                  <option key={s} value={s} />
                ))}
              </datalist>

              <select
                value={item.montagem}
                onChange={(e) => atualizarItem(indice, { montagem: e.target.value as Montagem | '' })}
              >
                <option value="">Montagem</option>
                <option value="montado">Montado</option>
                <option value="caixa">Em caixa</option>
              </select>

              <input
                value={item.observacao}
                onChange={(e) => atualizarItem(indice, { observacao: e.target.value })}
                placeholder="Observacao (opcional)"
              />

              <input
                type="number"
                min={1}
                value={item.quantidade}
                onChange={(e) => atualizarItem(indice, { quantidade: Number(e.target.value) })}
                required
              />

              <button
                type="button"
                className="secundario"
                onClick={() => removerLinhaItem(indice)}
                disabled={itens.length === 1}
              >
                Remover
              </button>
            </div>
          ))}

          <button type="button" className="secundario" onClick={adicionarLinhaItem}>
            + adicionar item
          </button>

          {erro && <p className="erro">{erro}</p>}

          <div style={{ marginTop: 20 }}>
            <button type="submit" disabled={enviando}>
              {enviando ? 'Registrando...' : `Registrar ${tipo === 'saida' ? 'saida' : 'entrada'}`}
            </button>
          </div>
        </form>
      </section>

      <section className="cartao">
        <h2>Lancamentos de hoje ({data})</h2>
        <div className="tabela-scroll">
        <table>
          <thead>
            <tr>
              <th>Nº</th>
              <th>Horario</th>
              <th>Pedido</th>
              <th>Coleta</th>
              <th>Rastreio</th>
              <th>Itens</th>
              <th>Qtd.</th>
              <th>Quem retirou</th>
              <th>Observacoes</th>
              <th>Registrado por</th>
              <th>Situacao</th>
              {ehGestor && <th className="somente-tela">Acoes</th>}
            </tr>
          </thead>
          <tbody>
            {lancamentos.map((m) => (
              <tr key={m.id}>
                <td>{m.numero}</td>
                <td>{m.hora}</td>
                <td>{m.numero_pedido || '-'}</td>
                <td>{m.contraparte || '-'}</td>
                <td>{m.codigo_rastreio || '-'}</td>
                <td>
                  {m.itens
                    .map(
                      (it) =>
                        `${it.quantidade}x ${it.categoria}${it.descricao ? ' (' + it.descricao + ')' : ''}${it.observacao ? ' - ' + it.observacao : ''}`
                    )
                    .join(' + ')}
                </td>
                <td>{m.itens.reduce((s, it) => s + it.quantidade, 0)}</td>
                <td>{m.quem_retirou || '-'}</td>
                <td>{m.observacoes || '-'}</td>
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
            {lancamentos.length === 0 && (
              <tr>
                <td colSpan={ehGestor ? 12 : 11} className="rodape-tabela">
                  Nenhum lancamento registrado ainda hoje.
                </td>
              </tr>
            )}
          </tbody>
        </table>
        </div>
        <p className="rodape-tabela">
          <strong>{totalGeralDoDia}</strong> unidades no total ({lancamentos.length} pedidos)
        </p>

        {ehGestor && (
          <button onClick={handleFecharDia} disabled={fechando || lancamentos.length === 0}>
            {fechando ? 'Fechando...' : 'Fechar o dia'}
          </button>
        )}
      </section>
    </div>
  );
}
