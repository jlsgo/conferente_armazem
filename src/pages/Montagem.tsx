import { FormEvent, useEffect, useState } from 'react';
import type {
  Armazem,
  Categoria,
  Condicao,
  Fechamento,
  Montagem as MontagemVeiculo,
  Movimento,
  MovimentoItemInput,
  TipoMovimento,
  Usuario,
} from '../types';
import {
  criarMovimento,
  buscarFechamentoDoDia,
  estornarMovimento,
  fecharDia,
  listarMovimentosDoDia,
  sugestoesDescricao,
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
  // e opcional, ao contrario de condicao que exige a palavra "outro" de
  // verdade - ver `itemPrecisaObservacao`).
  montagem: MontagemVeiculo | '' | 'outro';
  condicao: Condicao | '';
  quantidade: number;
  observacao: string;
}

type Destino = 'armazem' | 'externo';

const CATEGORIAS: { valor: Categoria; rotulo: string }[] = [
  { valor: 'peca', rotulo: 'Peca' },
  { valor: 'scooter', rotulo: 'Scooter' },
  { valor: 'triciclo', rotulo: 'Triciclo' },
  { valor: 'patinete', rotulo: 'Patinete' },
  { valor: 'outro', rotulo: 'Outro' },
];

const CATEGORIAS_VEICULO: Categoria[] = ['scooter', 'triciclo', 'patinete'];

function itemPrecisaObservacao(item: Pick<ItemForm, 'categoria' | 'montagem' | 'condicao'>): boolean {
  return algumCampoEhOutro(item.categoria, item.montagem, item.condicao);
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
  return { categoria: 'peca', descricao: '', montagem: '', condicao: '', quantidade: 1, observacao: '' };
}

export default function Montagem({ usuario, armazem, armazens, onTransferenciaConfirmada }: Props) {
  const armazemId = usuario.armazem_id as number;
  const data = dataDeHoje();
  const outroArmazem = armazens.find((a) => a.id !== armazem?.id);

  const [lancamentos, setLancamentos] = useState<Movimento[]>([]);
  const [fechamento, setFechamento] = useState<Fechamento | null>(null);
  const [carregandoLista, setCarregandoLista] = useState(true);
  const [erroCarregamento, setErroCarregamento] = useState('');
  const [sugestoesPorCategoria, setSugestoesPorCategoria] = useState<Partial<Record<Categoria, string[]>>>({});
  const { notificar } = useToast();

  const [hora, setHora] = useState(horaAtual());
  const [tipo, setTipo] = useState<TipoMovimento>('saida');
  const [destino, setDestino] = useState<Destino>('armazem');
  const [enviadoPara, setEnviadoPara] = useState('');
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
        listarMovimentosDoDia({ armazem_id: armazemId, fluxo: 'peca_montagem', data }),
        buscarFechamentoDoDia({ armazem_id: armazemId, fluxo: 'peca_montagem', data }),
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
    garantirSugestoes('peca');
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
    setItens([novoItemVazio()]);
    setEnviadoPara('');
  }

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setErro('');

    if (itens.some((it) => !it.condicao)) {
      setErro('Informe a condicao (boa, defeito, sucata ou outro) de cada item.');
      return;
    }
    if (tipo === 'saida' && destino === 'externo' && !enviadoPara.trim()) {
      setErro('Informe pra quem foi enviado (ex: nome do tecnico).');
      return;
    }
    if (itens.some((it) => it.quantidade > 0 && itemPrecisaObservacao(it) && !it.observacao.trim())) {
      setErro('Descreva o item na observacao quando escolher "Outro" na categoria, montagem ou condicao.');
      return;
    }

    const itensValidos: MovimentoItemInput[] = itens
      .filter((it) => it.quantidade > 0)
      .map((it) => ({
        categoria: it.categoria,
        descricao: it.descricao.trim() || null,
        montagem: it.montagem === 'outro' ? null : it.montagem || null,
        condicao: it.condicao || null,
        quantidade: it.quantidade,
        observacao: it.observacao.trim() || null,
      }));

    if (itensValidos.length === 0) {
      setErro('Informe ao menos um item com quantidade valida.');
      return;
    }

    const paraOutroArmazem = tipo === 'saida' && destino === 'armazem';
    const paraDestinoExterno = tipo === 'saida' && destino === 'externo';

    setEnviando(true);
    const resultado = await criarMovimento({
      armazem_id: armazemId,
      armazem_destino_id: paraOutroArmazem ? (outroArmazem?.id ?? null) : null,
      fluxo: 'peca_montagem',
      tipo,
      data,
      hora,
      turno: 'diurno',
      contraparte: paraDestinoExterno ? enviadoPara.trim() : null,
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
    const resultado = await fecharDia({ armazem_id: armazemId, fluxo: 'peca_montagem', data });
    setFechando(false);

    if (!resultado.ok) {
      setErro(resultado.error ?? 'Nao foi possivel fechar o dia.');
      return;
    }

    await carregarTudo();
  }

  async function handleEstornar(movimento: Movimento) {
    const justificativa = window.prompt(
      `Justificativa para estornar o lancamento nº ${movimento.numero}:`
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

  function nomeArmazemPorId(id: number | null): string {
    if (id == null) return '-';
    return armazens.find((a) => a.id === id)?.codigo ?? '-';
  }

  function direcaoTexto(m: Movimento): string {
    if (m.tipo === 'entrada') {
      return m.recebido_de_armazem_codigo ? `Recebido de ${m.recebido_de_armazem_codigo}` : 'Entrada';
    }
    if (m.armazem_destino_id) return `Enviado para ${nomeArmazemPorId(m.armazem_destino_id)}`;
    if (m.contraparte) return `Enviado para ${m.contraparte}`;
    return 'Saida';
  }

  const idsJaEstornados = new Set(
    lancamentos.filter((m) => m.estornado_de != null).map((m) => m.estornado_de)
  );

  const totalGeralDoDia = lancamentos.reduce(
    (soma, m) => soma + (m.estornado_de ? -1 : 1) * m.itens.reduce((s, it) => s + it.quantidade, 0),
    0
  );

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
                        Nº {m.numero} - {m.itens.reduce((s, it) => s + it.quantidade, 0)} un.
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
        <FechamentoImpressao
          armazem={armazem}
          data={data}
          fechamento={fechamento}
          lancamentos={lancamentos}
          variante="montagem"
        />
      </div>
    );
  }

  return (
    <div>
      <ResumoDoDia lancamentos={lancamentos} />
      <TransferenciasChegando
        fluxo="peca_montagem"
        outroArmazem={outroArmazem}
        onConfirmado={async () => {
          await carregarTudo();
          onTransferenciaConfirmada?.();
        }}
      />

      <section className="cartao">
        <h2>Registrar {tipo === 'saida' ? 'saida' : 'entrada'} do galpao</h2>
        <p className="subtitulo">
          {formatarData(data)} - responsavel: {usuario.nome}. Pecas soltas ou scooters montados
          {tipo === 'saida' ? ' saindo daqui.' : ' chegando aqui (ex: compra de fornecedor).'}
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
              onClick={() => setTipo('entrada')}
            >
              Entrada
            </button>
          </div>

          {tipo === 'saida' && (
            <div className="abas" style={{ marginBottom: 20 }}>
              <button type="button" className={destino === 'armazem' ? 'ativo' : ''} onClick={() => setDestino('armazem')}>
                {outroArmazem ? `Para ${outroArmazem.codigo}` : 'Para o outro armazem'}
              </button>
              <button type="button" className={destino === 'externo' ? 'ativo' : ''} onClick={() => setDestino('externo')}>
                Outro destino (ex: tecnico externo)
              </button>
            </div>
          )}

          <div className="grade-formulario">
            <label>
              Horario
              <input type="time" value={hora} onChange={(e) => setHora(e.target.value)} required />
            </label>
            {tipo === 'saida' && destino === 'externo' && (
              <label>
                Enviado para
                <input
                  value={enviadoPara}
                  onChange={(e) => setEnviadoPara(e.target.value)}
                  placeholder="Ex: Tecnico Joao - Eletronica Silva"
                  required
                />
              </label>
            )}
          </div>
          {tipo === 'saida' && destino === 'externo' && (
            <p className="subtitulo">
              Anote o codigo/serie de cada peca no campo "Observacao" dela abaixo.
            </p>
          )}

          <h3>Itens deste lancamento</h3>
          {itens.map((item, indice) => (
            <div className="linha-item linha-item-peca" key={indice}>
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
                placeholder="Descricao (ex: Retrovisor)"
                list={`sugestoes-${item.categoria}`}
              />
              <datalist id={`sugestoes-${item.categoria}`}>
                {(sugestoesPorCategoria[item.categoria] ?? []).map((s) => (
                  <option key={s} value={s} />
                ))}
              </datalist>

              {CATEGORIAS_VEICULO.includes(item.categoria) ? (
                <select
                  value={item.montagem}
                  onChange={(e) =>
                    atualizarItem(indice, { montagem: e.target.value as MontagemVeiculo | '' | 'outro' })
                  }
                >
                  <option value="">Montagem</option>
                  <option value="montado">Montado</option>
                  <option value="caixa">Em caixa</option>
                  <option value="outro">Outro</option>
                </select>
              ) : (
                // Placeholder vazio pra manter o numero de colunas do grid
                // constante (peca nao tem campo de montagem) - sem isso, as
                // colunas seguintes ficam desalinhadas/espremidas.
                <span />
              )}

              <select
                value={item.condicao}
                onChange={(e) => atualizarItem(indice, { condicao: e.target.value as Condicao | '' })}
                required
              >
                <option value="">Condicao</option>
                <option value="boa">Boa</option>
                <option value="defeito">Defeito</option>
                <option value="sucata">Sucata</option>
                <option value="outro">Outro</option>
              </select>

              <input
                value={item.observacao}
                onChange={(e) => atualizarItem(indice, { observacao: e.target.value })}
                placeholder={
                  itemPrecisaObservacao(item)
                    ? 'Descreva o item (obrigatorio)'
                    : tipo === 'saida' && destino === 'externo'
                      ? 'Codigo/serie da peca'
                      : 'Observacao (opcional)'
                }
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
              <th>Direcao</th>
              <th>Itens</th>
              <th>Qtd.</th>
              <th>Condicao</th>
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
                <td>{direcaoTexto(m)}</td>
                <td>
                  {m.itens
                    .map((it) => {
                      const base = `${it.quantidade}x ${it.categoria}${it.descricao ? ' (' + it.descricao + ')' : ''}`;
                      const divergente = it.quantidade_enviada != null && it.quantidade_enviada !== it.quantidade;
                      return divergente ? `${base} [enviado: ${it.quantidade_enviada}]` : base;
                    })
                    .join(' + ')}
                </td>
                <td>{m.itens.reduce((s, it) => s + it.quantidade, 0)}</td>
                <td>{m.itens.map((it) => it.condicao).filter(Boolean).join(', ') || '-'}</td>
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
                <td colSpan={9} className="rodape-tabela">
                  Nenhum lancamento registrado ainda hoje.
                </td>
              </tr>
            )}
          </tbody>
        </table>
        </div>
        <p className="rodape-tabela">
          <strong>{totalGeralDoDia}</strong> unidades no total ({lancamentos.length} lancamentos)
        </p>

        <button className="aviso" onClick={handleFecharDia} disabled={fechando || lancamentos.length === 0}>
          {fechando ? 'Fechando...' : 'Fechar o dia'}
        </button>
      </section>
    </div>
  );
}
