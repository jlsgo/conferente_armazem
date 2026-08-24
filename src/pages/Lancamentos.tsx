import { FormEvent, useEffect, useState } from 'react';
import type { Categoria, Montagem, Movimento, MovimentoItemInput, TipoMovimento, Usuario } from '../types';
import { criarMovimento, listarMovimentosDoDia, sugestoesDescricao } from '../lib/api';

interface Props {
  usuario: Usuario;
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

export default function Lancamentos({ usuario }: Props) {
  const armazemId = usuario.armazem_id as number;
  const data = dataDeHoje();

  const [lancamentos, setLancamentos] = useState<Movimento[]>([]);
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

  async function carregarLista() {
    setCarregandoLista(true);
    const lista = await listarMovimentosDoDia({ armazem_id: armazemId, fluxo: 'saida_armazem', data });
    setLancamentos(lista);
    setCarregandoLista(false);
  }

  async function garantirSugestoes(categoria: Categoria) {
    if (sugestoesPorCategoria[categoria]) return;
    const lista = await sugestoesDescricao(categoria);
    setSugestoesPorCategoria((atual) => ({ ...atual, [categoria]: lista }));
  }

  useEffect(() => {
    carregarLista();
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
    setHora(horaAtual());
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
    await carregarLista();
  }

  const totalGeralDoDia = lancamentos.reduce(
    (soma, m) => soma + m.itens.reduce((s, it) => s + it.quantidade, 0),
    0
  );

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
        {carregandoLista ? (
          <p>Carregando...</p>
        ) : (
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
        )}
        <p className="rodape-tabela">
          <strong>{totalGeralDoDia}</strong> unidades no total ({lancamentos.length} pedidos)
        </p>
      </section>
    </div>
  );
}
