import { FormEvent, useEffect, useState } from 'react';
import type { Armazem, Categoria, Fechamento, Montagem, Movimento, MovimentoItemInput, TipoMovimento, Usuario } from '../types';
import { criarMovimento, buscarFechamentoDoDia, fecharDia, listarMovimentosDoDia, sugestoesDescricao } from '../lib/api';
import FechamentoImpressao from '../components/FechamentoImpressao';

interface Props {
  usuario: Usuario;
  armazem: Armazem | undefined;
}

interface ItemForm {
  categoria: Categoria;
  descricao: string;
  montagem: Montagem | '';
  quantidade: number;
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
  return { categoria: 'scooter', descricao: '', montagem: '', quantidade: 1 };
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
  const [contraparte, setContraparte] = useState('');
  const [quemRetirou, setQuemRetirou] = useState('');
  const [itens, setItens] = useState<ItemForm[]>([novoItemVazio()]);
  const [erro, setErro] = useState('');
  const [enviando, setEnviando] = useState(false);
  const [fechando, setFechando] = useState(false);

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
    setContraparte('');
    setQuemRetirou('');
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
      usuario_id: usuario.id,
      numero_pedido: numeroPedido || null,
      contraparte: contraparte || null,
      quem_retirou: quemRetirou || null,
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
    const resultado = await fecharDia({ armazem_id: armazemId, fluxo: 'saida_armazem', data, usuario_id: usuario.id });
    setFechando(false);

    if (!resultado.ok) {
      setErro(resultado.error ?? 'Nao foi possivel fechar o dia.');
      return;
    }

    await carregarTudo();
  }

  const totalGeralDoDia = lancamentos.reduce(
    (soma, m) => soma + m.itens.reduce((s, it) => s + it.quantidade, 0),
    0
  );

  if (carregandoLista) {
    return <p>Carregando...</p>;
  }

  if (fechamento) {
    return (
      <div>
        <p className="aviso-fechado">
          O dia {data} ja foi fechado por {fechamento.usuario_nome}. Os lancamentos abaixo sao somente leitura.
        </p>
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
          </div>

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
        <table>
          <thead>
            <tr>
              <th>Nº</th>
              <th>Horario</th>
              <th>Pedido</th>
              <th>Coleta</th>
              <th>Itens</th>
              <th>Qtd.</th>
              <th>Quem retirou</th>
              <th>Registrado por</th>
              <th>Situacao</th>
            </tr>
          </thead>
          <tbody>
            {lancamentos.map((m) => (
              <tr key={m.id}>
                <td>{m.numero}</td>
                <td>{m.hora}</td>
                <td>{m.numero_pedido || '-'}</td>
                <td>{m.contraparte || '-'}</td>
                <td>
                  {m.itens
                    .map((it) => `${it.quantidade}x ${it.categoria}${it.descricao ? ' (' + it.descricao + ')' : ''}`)
                    .join(' + ')}
                </td>
                <td>{m.itens.reduce((s, it) => s + it.quantidade, 0)}</td>
                <td>{m.quem_retirou || '-'}</td>
                <td>{m.usuario_nome}</td>
                <td>{m.tipo === 'saida' ? 'BAIXA' : 'ENTRADA'}</td>
              </tr>
            ))}
            {lancamentos.length === 0 && (
              <tr>
                <td colSpan={9} className="rodape-tabela">
                  Nenhum lancamento registrado ainda hoje.
                </td>
              </tr>
            )}
          </tbody>
        </table>
        <p className="rodape-tabela">
          <strong>{totalGeralDoDia}</strong> unidades no total ({lancamentos.length} pedidos)
        </p>

        <button onClick={handleFecharDia} disabled={fechando || lancamentos.length === 0}>
          {fechando ? 'Fechando...' : 'Fechar o dia'}
        </button>
      </section>
    </div>
  );
}
