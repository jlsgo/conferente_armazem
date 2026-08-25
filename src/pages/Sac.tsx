import { FormEvent, useEffect, useState } from 'react';
import type { Armazem, Fechamento, Movimento, MovimentoItemInput, Usuario } from '../types';
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

type Motivo = 'garantia' | 'venda';

interface ItemForm {
  descricao: string;
  quantidade: number;
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
  return { descricao: '', quantidade: 1 };
}

export default function Sac({ usuario, armazem }: Props) {
  const armazemId = usuario.armazem_id as number;
  const data = dataDeHoje();

  const [lancamentos, setLancamentos] = useState<Movimento[]>([]);
  const [fechamento, setFechamento] = useState<Fechamento | null>(null);
  const [carregandoLista, setCarregandoLista] = useState(true);
  const [erroCarregamento, setErroCarregamento] = useState('');
  const [sugestoes, setSugestoes] = useState<string[]>([]);

  const [hora, setHora] = useState(horaAtual());
  const [protocolo, setProtocolo] = useState('');
  const [coleta, setColeta] = useState('');
  const [motivo, setMotivo] = useState<Motivo | ''>('');
  const [valorReais, setValorReais] = useState('');
  const [itens, setItens] = useState<ItemForm[]>([novoItemVazio()]);
  const [erro, setErro] = useState('');
  const [enviando, setEnviando] = useState(false);
  const [fechando, setFechando] = useState(false);
  const [estornando, setEstornando] = useState<number | null>(null);
  const ehGestor = usuario.papel === 'gestor';

  async function carregarTudo() {
    setCarregandoLista(true);
    setErroCarregamento('');
    try {
      const [lista, fechamentoDoDia] = await Promise.all([
        listarMovimentosDoDia({ armazem_id: armazemId, fluxo: 'sac', data }),
        buscarFechamentoDoDia({ armazem_id: armazemId, fluxo: 'sac', data }),
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

  useEffect(() => {
    carregarTudo();
    sugestoesDescricao('peca').then(setSugestoes);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function atualizarItem(indice: number, alteracoes: Partial<ItemForm>) {
    setItens((atual) => atual.map((it, i) => (i === indice ? { ...it, ...alteracoes } : it)));
  }

  function adicionarLinhaItem() {
    setItens((atual) => [...atual, novoItemVazio()]);
  }

  function removerLinhaItem(indice: number) {
    setItens((atual) => (atual.length === 1 ? atual : atual.filter((_, i) => i !== indice)));
  }

  function limparFormulario() {
    // Protocolo/coleta nao reseta: mesma logica ja usada em Lancamentos.tsx - varias
    // pecas do mesmo atendimento costumam ser lancadas em sequencia.
    setItens([novoItemVazio()]);
  }

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setErro('');

    if (!motivo) {
      setErro('Informe se e garantia ou venda.');
      return;
    }
    let valorCentavos: number | null = null;
    if (motivo === 'venda') {
      const valor = Number(valorReais.replace(',', '.'));
      if (!valorReais || Number.isNaN(valor) || valor <= 0) {
        setErro('Informe o valor da venda.');
        return;
      }
      valorCentavos = Math.round(valor * 100);
    }

    const itensValidos: MovimentoItemInput[] = itens
      .filter((it) => it.quantidade > 0)
      .map((it) => ({
        categoria: 'peca',
        descricao: it.descricao.trim() || null,
        quantidade: it.quantidade,
      }));

    if (itensValidos.length === 0) {
      setErro('Informe ao menos uma peca com quantidade valida.');
      return;
    }

    setEnviando(true);
    const resultado = await criarMovimento({
      armazem_id: armazemId,
      fluxo: 'sac',
      tipo: 'entrada',
      data,
      hora,
      turno: 'diurno',
      numero_pedido: protocolo || null,
      contraparte: coleta || null,
      motivo,
      valor_centavos: valorCentavos,
      itens: itensValidos,
    });
    setEnviando(false);

    if (!resultado.ok) {
      setErro(resultado.error ?? 'Nao foi possivel registrar o lancamento.');
      return;
    }

    setProtocolo('');
    setColeta('');
    setMotivo('');
    setValorReais('');
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
    const resultado = await fecharDia({ armazem_id: armazemId, fluxo: 'sac', data });
    setFechando(false);

    if (!resultado.ok) {
      setErro(resultado.error ?? 'Nao foi possivel fechar o dia.');
      return;
    }

    await carregarTudo();
  }

  async function handleEstornar(movimento: Movimento) {
    const justificativa = window.prompt(
      `Justificativa para estornar o lancamento nº ${movimento.numero} (protocolo ${movimento.numero_pedido ?? '-'}):`
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
                        Nº {m.numero} - protocolo {m.numero_pedido || '-'} -{' '}
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
        )}
        <FechamentoImpressao
          armazem={armazem}
          data={data}
          fechamento={fechamento}
          lancamentos={lancamentos}
          variante="sac"
        />
      </div>
    );
  }

  return (
    <div>
      <section className="cartao">
        <h2>Registrar atendimento SAC</h2>
        <p className="subtitulo">
          {data} - responsavel: {usuario.nome}.
        </p>

        <form onSubmit={handleSubmit}>
          <div className="grade-formulario">
            <label>
              Protocolo
              <input
                value={protocolo}
                onChange={(e) => setProtocolo(e.target.value)}
                placeholder="Numero do protocolo"
                required
              />
            </label>

            <label>
              Horario
              <input type="time" value={hora} onChange={(e) => setHora(e.target.value)} required />
            </label>

            <label>
              Coleta (Correios / cliente)
              <input value={coleta} onChange={(e) => setColeta(e.target.value)} />
            </label>

            <label>
              Garantia ou venda
              <select value={motivo} onChange={(e) => setMotivo(e.target.value as Motivo | '')} required>
                <option value="">Selecione</option>
                <option value="garantia">Garantia</option>
                <option value="venda">Venda</option>
              </select>
            </label>

            {motivo === 'venda' && (
              <label>
                Valor da venda (R$)
                <input
                  type="number"
                  min={0}
                  step="0.01"
                  value={valorReais}
                  onChange={(e) => setValorReais(e.target.value)}
                  placeholder="0,00"
                  required
                />
              </label>
            )}
          </div>

          <h3>Pecas deste atendimento</h3>
          {itens.map((item, indice) => (
            <div className="linha-item" key={indice}>
              <input
                value={item.descricao}
                onChange={(e) => atualizarItem(indice, { descricao: e.target.value })}
                placeholder="Descricao da peca (ex: Retrovisor)"
                list="sugestoes-peca-sac"
              />
              <datalist id="sugestoes-peca-sac">
                {sugestoes.map((s) => (
                  <option key={s} value={s} />
                ))}
              </datalist>

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
            + adicionar peca
          </button>

          {erro && <p className="erro">{erro}</p>}

          <div style={{ marginTop: 20 }}>
            <button type="submit" disabled={enviando}>
              {enviando ? 'Registrando...' : 'Registrar'}
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
              <th>Protocolo</th>
              <th>Coleta</th>
              <th>Itens</th>
              <th>Qtd.</th>
              <th>Garantia/Venda</th>
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
                <td>
                  {m.itens
                    .map((it) => `${it.quantidade}x ${it.categoria}${it.descricao ? ' (' + it.descricao + ')' : ''}`)
                    .join(' + ')}
                </td>
                <td>{m.itens.reduce((s, it) => s + it.quantidade, 0)}</td>
                <td>
                  {m.motivo === 'venda'
                    ? `Venda (R$ ${((m.valor_centavos ?? 0) / 100).toFixed(2)})`
                    : m.motivo === 'garantia'
                      ? 'Garantia'
                      : '-'}
                </td>
                <td>{m.usuario_nome}</td>
                <td>
                  <span className={situacaoInfo(m).classe}>{situacaoInfo(m).texto}</span>
                </td>
                {ehGestor && (
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
                )}
              </tr>
            ))}
            {lancamentos.length === 0 && (
              <tr>
                <td colSpan={ehGestor ? 10 : 9} className="rodape-tabela">
                  Nenhum lancamento registrado ainda hoje.
                </td>
              </tr>
            )}
          </tbody>
        </table>
        </div>
        <p className="rodape-tabela">
          <strong>{totalGeralDoDia}</strong> pecas no total ({lancamentos.length} atendimentos)
        </p>

        {ehGestor && (
          <button className="aviso" onClick={handleFecharDia} disabled={fechando || lancamentos.length === 0}>
            {fechando ? 'Fechando...' : 'Fechar o dia'}
          </button>
        )}
      </section>
    </div>
  );
}
