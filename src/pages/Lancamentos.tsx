import { FormEvent, useEffect, useState } from 'react';
import type { Armazem, Categoria, Fechamento, Montagem, Movimento, MovimentoItemInput, TipoMovimento, Usuario } from '../types';
import {
  criarMovimento,
  buscarFechamentoDoDia,
  estornarMovimento,
  fecharDia,
  listarMovimentosDoDia,
  sugestoesDescricao,
  verificarRetiradaPendente,
} from '../lib/api';
import FechamentoImpressao from '../components/FechamentoImpressao';
import Carregando from '../components/Carregando';
import TransferenciasChegando from '../components/TransferenciasChegando';
import ResumoDoDia from '../components/ResumoDoDia';
import { situacaoInfo } from '../lib/situacao';
import { formatarData } from '../lib/data';
import { algumCampoEhOutro } from '../lib/outro';
import { useToast } from '../lib/toast';

interface Props {
  usuario: Usuario;
  armazem: Armazem | undefined;
  armazens: Armazem[];
  /** Avisa o Dashboard pra atualizar o contador de pendentes nas abas na hora, sem esperar o polling de 60s. */
  onTransferenciaConfirmada?: () => void;
}

interface ItemForm {
  categoria: Categoria;
  descricao: string;
  // 'outro' aqui e so um estado de UI - na hora de enviar vira null (o campo
  // e opcional, "outro" so serve pra sinalizar que precisa descrever na
  // observacao, ver `itemPrecisaObservacao`).
  montagem: Montagem | '' | 'outro';
  quantidade: number;
  observacao: string;
}

type Destino = 'cliente' | 'armazem';

const CATEGORIAS: { valor: Categoria; rotulo: string }[] = [
  { valor: 'scooter', rotulo: 'Scooter' },
  { valor: 'triciclo', rotulo: 'Triciclo' },
  { valor: 'patinete', rotulo: 'Patinete' },
  { valor: 'peca', rotulo: 'Peca' },
  { valor: 'outro', rotulo: 'Outro' },
];

function itemPrecisaObservacao(item: Pick<ItemForm, 'categoria' | 'montagem'>): boolean {
  return algumCampoEhOutro(item.categoria, item.montagem);
}

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

export default function Lancamentos({ usuario, armazem, armazens, onTransferenciaConfirmada }: Props) {
  const armazemId = usuario.armazem_id as number;
  const data = dataDeHoje();
  const outroArmazem = armazens.find((a) => a.id !== armazem?.id);

  const [lancamentos, setLancamentos] = useState<Movimento[]>([]);
  const [fechamento, setFechamento] = useState<Fechamento | null>(null);
  const [carregandoLista, setCarregandoLista] = useState(true);
  const [erroCarregamento, setErroCarregamento] = useState('');
  const [sugestoesPorCategoria, setSugestoesPorCategoria] = useState<Partial<Record<Categoria, string[]>>>({});
  const { notificar } = useToast();

  const [tipo, setTipo] = useState<TipoMovimento>('saida');
  const [destino, setDestino] = useState<Destino>('cliente');
  const [hora, setHora] = useState(horaAtual());
  const [numeroPedido, setNumeroPedido] = useState('');
  const [contraparte, setContraparte] = useState('');
  const [quemRetirou, setQuemRetirou] = useState('');
  const [observacoes, setObservacoes] = useState('');
  const [retiradaParcial, setRetiradaParcial] = useState(false);
  const [alertaRetiradaPendente, setAlertaRetiradaPendente] = useState<Movimento | null>(null);
  const [itens, setItens] = useState<ItemForm[]>([novoItemVazio()]);
  const [erro, setErro] = useState('');
  const [enviando, setEnviando] = useState(false);
  const [fechando, setFechando] = useState(false);
  const [estornando, setEstornando] = useState<number | null>(null);

  async function carregarTudo() {
    setCarregandoLista(true);
    setErroCarregamento('');
    try {
      const [lista, fechamentoDoDia] = await Promise.all([
        listarMovimentosDoDia({ armazem_id: armazemId, fluxo: 'saida_armazem', data }),
        buscarFechamentoDoDia({ armazem_id: armazemId, fluxo: 'saida_armazem', data }),
      ]);
      setLancamentos(lista);
      setFechamento(fechamentoDoDia);
    } catch (err) {
      setErroCarregamento(
        typeof err === 'string' ? err : 'Nao foi possivel carregar os lancamentos de hoje.'
      );
    } finally {
      setCarregandoLista(false);
    }
  }

  async function garantirSugestoes(categoria: Categoria) {
    if (sugestoesPorCategoria[categoria]) return;
    try {
      const lista = await sugestoesDescricao(categoria);
      setSugestoesPorCategoria((atual) => ({ ...atual, [categoria]: lista }));
    } catch {
      notificar('Nao foi possivel carregar as sugestoes de descricao. Pode digitar normalmente.', 'erro');
    }
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
    setObservacoes('');
    setRetiradaParcial(false);
    setDestino('cliente');
    setAlertaRetiradaPendente(null);
    setItens([novoItemVazio()]);
  }

  async function verificarPedidoPendente() {
    if (!numeroPedido.trim()) {
      setAlertaRetiradaPendente(null);
      return;
    }
    const pendente = await verificarRetiradaPendente({
      armazem_id: armazemId,
      fluxo: 'saida_armazem',
      numero_pedido: numeroPedido.trim(),
    });
    setAlertaRetiradaPendente(pendente);
  }

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setErro('');

    const paraOutroArmazem = tipo === 'saida' && destino === 'armazem';

    const itensComQuantidade = itens.filter((it) => it.quantidade > 0);
    const itemSemDescricao = itensComQuantidade.find(
      (it) => itemPrecisaObservacao(it) && !it.observacao.trim()
    );
    if (itemSemDescricao) {
      setErro('Descreva o item na observacao quando escolher "Outro" na categoria ou montagem.');
      return;
    }

    const itensValidos: MovimentoItemInput[] = itensComQuantidade.map((it) => ({
      categoria: it.categoria,
      descricao: it.descricao.trim() || null,
      montagem: it.montagem === 'outro' ? null : it.montagem || null,
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
      armazem_destino_id: paraOutroArmazem ? (outroArmazem?.id ?? null) : null,
      fluxo: 'saida_armazem',
      tipo,
      data,
      hora,
      turno: 'diurno',
      numero_pedido: numeroPedido || null,
      codigo_rastreio: null,
      contraparte: paraOutroArmazem ? null : contraparte || null,
      quem_retirou: paraOutroArmazem ? null : quemRetirou || null,
      observacoes: observacoes || null,
      retirada_completa: paraOutroArmazem ? true : tipo === 'saida' ? !retiradaParcial : true,
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
      `Fechar o dia ${formatarData(data)}? Depois disso nao sera mais possivel adicionar ou corrigir lancamentos deste dia neste armazem.`
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

  const paraOutroArmazemNoForm = tipo === 'saida' && destino === 'armazem';

  function nomeArmazemPorId(id: number | null): string {
    if (id == null) return '-';
    return armazens.find((a) => a.id === id)?.codigo ?? '-';
  }

  function colunaColeta(m: Movimento): string {
    if (m.armazem_destino_id) return `Enviado para ${nomeArmazemPorId(m.armazem_destino_id)}`;
    if (m.recebido_de_armazem_codigo) return `Recebido de ${m.recebido_de_armazem_codigo}`;
    return m.contraparte || '-';
  }

  if (carregandoLista) {
    return <Carregando />;
  }

  if (erroCarregamento) {
    return (
      <div className="cartao">
        <p className="erro">{erroCarregamento}</p>
        <button type="button" onClick={carregarTudo}>
          Tentar novamente
        </button>
      </div>
    );
  }

  if (fechamento) {
    return (
      <div>
        <p className="aviso-fechado">
          O dia {formatarData(data)} ja foi fechado por {fechamento.usuario_nome}. Os lancamentos abaixo sao somente leitura.
        </p>
        {erro && <p className="erro">{erro}</p>}
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
                      Nº {m.numero}
                      {m.numero_pedido ? ` - pedido ${m.numero_pedido}` : ''} -{' '}
                      {m.itens.reduce((s, it) => s + it.quantidade, 0)} un.
                    </td>
                    <td>
                      <button
                        type="button"
                        className="perigo"
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
        <FechamentoImpressao armazem={armazem} data={data} fechamento={fechamento} lancamentos={lancamentos} />
      </div>
    );
  }

  return (
    <div>
      <ResumoDoDia lancamentos={lancamentos} />
      <TransferenciasChegando
        fluxo="saida_armazem"
        outroArmazem={outroArmazem}
        onConfirmado={async () => {
          await carregarTudo();
          onTransferenciaConfirmada?.();
        }}
      />

      <section className="cartao">
        <h2>Registrar {tipo === 'saida' ? 'saida' : 'entrada'} do armazem</h2>
        <p className="subtitulo">
          {formatarData(data)} - responsavel: {usuario.nome}. O detalhe completo do pedido fica na outra
          ferramenta - aqui basta o numero do pedido e a quantidade.
        </p>

        <form onSubmit={handleSubmit}>
          <div className="abas" style={{ marginBottom: 20 }}>
            <button
              type="button"
              className={`tipo-saida ${tipo === 'saida' ? 'ativo' : ''}`}
              onClick={() => setTipo('saida')}
            >
              Saida
            </button>
            <button
              type="button"
              className={`tipo-entrada ${tipo === 'entrada' ? 'ativo' : ''}`}
              onClick={() => {
                setTipo('entrada');
                setDestino('cliente');
              }}
            >
              Entrada
            </button>
          </div>

          {tipo === 'saida' && outroArmazem && (
            <div className="abas" style={{ marginBottom: 20 }}>
              <button type="button" className={destino === 'cliente' ? 'ativo' : ''} onClick={() => setDestino('cliente')}>
                Cliente / coleta
              </button>
              <button type="button" className={destino === 'armazem' ? 'ativo' : ''} onClick={() => setDestino('armazem')}>
                Transferir para {outroArmazem.codigo}
              </button>
            </div>
          )}

          <div className="grade-formulario">
            <label>
              Numero do pedido {paraOutroArmazemNoForm && '(opcional)'}
              <input
                value={numeroPedido}
                onChange={(e) => setNumeroPedido(e.target.value)}
                onBlur={verificarPedidoPendente}
                placeholder="Ex: 3932"
                required={!paraOutroArmazemNoForm}
              />
            </label>

            <label>
              Horario
              <input type="time" value={hora} onChange={(e) => setHora(e.target.value)} required />
            </label>

            {!paraOutroArmazemNoForm && (
              <>
                <label>
                  {tipo === 'saida' ? 'Coleta (transportadora / cliente)' : 'Fornecedor / origem'}
                  <input value={contraparte} onChange={(e) => setContraparte(e.target.value)} />
                </label>

                {tipo === 'saida' && (
                  <label>
                    Quem retirou
                    <input value={quemRetirou} onChange={(e) => setQuemRetirou(e.target.value)} />
                  </label>
                )}
              </>
            )}
          </div>

          {alertaRetiradaPendente && (
            <p className="erro" style={{ background: 'var(--aviso-claro)', color: 'var(--aviso-escuro)', borderColor: '#f0c36d' }}>
              Atencao: o pedido {numeroPedido} teve uma retirada parcial em{' '}
              {formatarData(alertaRetiradaPendente.data)} ({alertaRetiradaPendente.itens.reduce((s, it) => s + it.quantidade, 0)} un.).
              Confirme se esta e a retirada complementar.
            </p>
          )}

          <label>
            Observacoes do pedido
            <textarea
              value={observacoes}
              onChange={(e) => setObservacoes(e.target.value)}
              rows={2}
              placeholder="Opcional - qualquer detalhe que nao caiba nos campos acima"
            />
          </label>

          {tipo === 'saida' && !paraOutroArmazemNoForm && (
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontWeight: 400 }}>
              <input
                type="checkbox"
                style={{ width: 'auto', margin: 0 }}
                checked={retiradaParcial}
                onChange={(e) => setRetiradaParcial(e.target.checked)}
              />
              Retirada parcial - cliente ainda vai voltar buscar o restante do pedido
            </label>
          )}

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
                onChange={(e) => atualizarItem(indice, { montagem: e.target.value as Montagem | '' | 'outro' })}
              >
                <option value="">Montagem</option>
                <option value="montado">Montado</option>
                <option value="caixa">Em caixa</option>
                <option value="outro">Outro</option>
              </select>

              <input
                value={item.observacao}
                onChange={(e) => atualizarItem(indice, { observacao: e.target.value })}
                placeholder={itemPrecisaObservacao(item) ? "Descreva o item (obrigatorio)" : 'Observacao (opcional)'}
                required={itemPrecisaObservacao(item)}
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
        <h2>Lancamentos de hoje ({formatarData(data)})</h2>
        <div className="tabela-scroll">
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
              <th>Observacoes</th>
              <th>Registrado por</th>
              <th>Situacao</th>
              <th className="somente-tela">Acoes</th>
            </tr>
          </thead>
          <tbody>
            {lancamentos.map((m) => (
              <tr key={m.id}>
                <td>{m.numero}</td>
                <td>{m.hora}</td>
                <td>
                  {m.numero_pedido || '-'}
                  {!m.retirada_completa && <span className="badge badge-parcial"> parcial</span>}
                </td>
                <td>{colunaColeta(m)}</td>
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
            {lancamentos.length === 0 && (
              <tr>
                <td colSpan={11} className="rodape-tabela">
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

        <button className="aviso" onClick={handleFecharDia} disabled={fechando || lancamentos.length === 0}>
          {fechando ? 'Fechando...' : 'Fechar o dia'}
        </button>
      </section>
    </div>
  );
}
