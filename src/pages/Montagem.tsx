import { FormEvent, useEffect, useState } from 'react';
import type { Armazem, Condicao, Fechamento, Movimento, MovimentoItemInput, TipoMovimento, Usuario } from '../types';
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
  descricao: string;
  condicao: Condicao | '';
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
  return { descricao: '', condicao: '', quantidade: 1 };
}

export default function Montagem({ usuario, armazem }: Props) {
  const armazemId = usuario.armazem_id as number;
  const data = dataDeHoje();

  const [lancamentos, setLancamentos] = useState<Movimento[]>([]);
  const [fechamento, setFechamento] = useState<Fechamento | null>(null);
  const [carregandoLista, setCarregandoLista] = useState(true);
  const [sugestoes, setSugestoes] = useState<string[]>([]);

  const [tipo, setTipo] = useState<TipoMovimento>('saida');
  const [hora, setHora] = useState(horaAtual());
  const [itens, setItens] = useState<ItemForm[]>([novoItemVazio()]);
  const [erro, setErro] = useState('');
  const [enviando, setEnviando] = useState(false);
  const [fechando, setFechando] = useState(false);
  const [estornando, setEstornando] = useState<number | null>(null);
  const ehGestor = usuario.papel === 'gestor';

  async function carregarTudo() {
    setCarregandoLista(true);
    const [lista, fechamentoDoDia] = await Promise.all([
      listarMovimentosDoDia({ armazem_id: armazemId, fluxo: 'peca_montagem', data }),
      buscarFechamentoDoDia({ armazem_id: armazemId, fluxo: 'peca_montagem', data }),
    ]);
    setLancamentos(lista);
    setFechamento(fechamentoDoDia);
    setCarregandoLista(false);
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
    setItens([novoItemVazio()]);
  }

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setErro('');

    if (itens.some((it) => !it.condicao)) {
      setErro('Informe a condicao (boa, defeito ou sucata) de cada peca.');
      return;
    }

    const itensValidos: MovimentoItemInput[] = itens
      .filter((it) => it.quantidade > 0)
      .map((it) => ({
        categoria: 'peca',
        descricao: it.descricao.trim() || null,
        condicao: it.condicao || null,
        quantidade: it.quantidade,
      }));

    if (itensValidos.length === 0) {
      setErro('Informe ao menos uma peca com quantidade valida.');
      return;
    }

    setEnviando(true);
    const resultado = await criarMovimento({
      armazem_id: armazemId,
      fluxo: 'peca_montagem',
      tipo,
      data,
      hora,
      turno: 'diurno',
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
                        Nº {m.numero} - {m.itens.reduce((s, it) => s + it.quantidade, 0)} un.
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
      <section className="cartao">
        <h2>Peca para montagem {tipo === 'saida' ? '- saida' : '- entrada'} do galpao</h2>
        <p className="subtitulo">
          {data} - responsavel: {usuario.nome}. Use para pecas soltas indo ou voltando entre B2 e a
          montagem.
        </p>

        <form onSubmit={handleSubmit}>
          <div className="abas" style={{ marginBottom: 20 }}>
            <button type="button" className={tipo === 'saida' ? 'ativo' : ''} onClick={() => setTipo('saida')}>
              Saida do galpao
            </button>
            <button type="button" className={tipo === 'entrada' ? 'ativo' : ''} onClick={() => setTipo('entrada')}>
              Entrada no galpao
            </button>
          </div>

          <div className="grade-formulario">
            <label>
              Horario
              <input type="time" value={hora} onChange={(e) => setHora(e.target.value)} required />
            </label>
          </div>

          <h3>Pecas deste lancamento</h3>
          {itens.map((item, indice) => (
            <div className="linha-item linha-item-peca" key={indice}>
              <input
                value={item.descricao}
                onChange={(e) => atualizarItem(indice, { descricao: e.target.value })}
                placeholder="Descricao da peca (ex: Retrovisor)"
                list="sugestoes-peca"
              />
              <datalist id="sugestoes-peca">
                {sugestoes.map((s) => (
                  <option key={s} value={s} />
                ))}
              </datalist>

              <select
                value={item.condicao}
                onChange={(e) => atualizarItem(indice, { condicao: e.target.value as Condicao | '' })}
                required
              >
                <option value="">Condicao</option>
                <option value="boa">Boa</option>
                <option value="defeito">Defeito</option>
                <option value="sucata">Sucata</option>
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
              <th>Direcao</th>
              <th>Itens</th>
              <th>Qtd.</th>
              <th>Condicao</th>
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
                <td>{m.tipo === 'saida' ? 'Saida B2' : 'Entrada B2'}</td>
                <td>
                  {m.itens
                    .map((it) => `${it.quantidade}x ${it.categoria}${it.descricao ? ' (' + it.descricao + ')' : ''}`)
                    .join(' + ')}
                </td>
                <td>{m.itens.reduce((s, it) => s + it.quantidade, 0)}</td>
                <td>{m.itens.map((it) => it.condicao).filter(Boolean).join(', ') || '-'}</td>
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
                <td colSpan={ehGestor ? 9 : 8} className="rodape-tabela">
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

        {ehGestor && (
          <button onClick={handleFecharDia} disabled={fechando || lancamentos.length === 0}>
            {fechando ? 'Fechando...' : 'Fechar o dia'}
          </button>
        )}
      </section>
    </div>
  );
}
