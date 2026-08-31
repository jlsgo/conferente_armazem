import { FormEvent, useEffect, useState } from 'react';
import type {
  Armazem,
  Categoria,
  Condicao,
  Fechamento,
  Movimento,
  MovimentoItemInput,
  ReparoPendente,
  TipoMovimento,
  Usuario,
} from '../types';
import {
  buscarFechamentoDoDia,
  buscarReparosEmAberto,
  criarMovimento,
  estornarMovimento,
  fecharDia,
  listarMovimentosDoDia,
  sugestoesDescricao,
} from '../lib/api';
import FechamentoImpressao from '../components/FechamentoImpressao';
import Carregando from '../components/Carregando';
import ReparosEmAberto from '../components/ReparosEmAberto';
import ResumoDoDia from '../components/ResumoDoDia';
import { situacaoInfo } from '../lib/situacao';
import { formatarData } from '../lib/data';
import { algumCampoEhOutro } from '../lib/outro';
import { useToast } from '../lib/toast';

interface Props {
  usuario: Usuario;
  armazem: Armazem | undefined;
  /** Avisa o Dashboard pra atualizar o contador de reparos em aberto na hora, sem esperar o polling de 60s. */
  onReparoAtualizado?: () => void;
}

interface ItemForm {
  categoria: Categoria;
  descricao: string;
  codigoComponente: string;
  // Resultado do conserto - so faz sentido (e e obrigatorio) na entrada,
  // quando a peca volta do tecnico. Reaproveita o mesmo campo/valores de
  // `condicao` ja usado em Montagem: 'boa' = consertada, 'defeito'/'sucata'
  // = nao consertada, 'outro' = caso raro com observacao obrigatoria.
  condicao: Condicao | '';
  observacao: string;
  quantidade: number;
}

const CATEGORIAS: { valor: Categoria; rotulo: string }[] = [
  { valor: 'peca', rotulo: 'Peca' },
  { valor: 'scooter', rotulo: 'Scooter' },
  { valor: 'triciclo', rotulo: 'Triciclo' },
  { valor: 'patinete', rotulo: 'Patinete' },
  { valor: 'outro', rotulo: 'Outro' },
];

function itemPrecisaObservacao(item: Pick<ItemForm, 'categoria' | 'condicao'>): boolean {
  return algumCampoEhOutro(item.categoria, item.condicao);
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
  return { categoria: 'peca', descricao: '', codigoComponente: '', condicao: '', observacao: '', quantidade: 1 };
}

export default function ReparoExterno({ usuario, armazem, onReparoAtualizado }: Props) {
  const armazemId = usuario.armazem_id as number;
  const data = dataDeHoje();

  const [lancamentos, setLancamentos] = useState<Movimento[]>([]);
  const [fechamento, setFechamento] = useState<Fechamento | null>(null);
  const [reparosAbertos, setReparosAbertos] = useState<ReparoPendente[]>([]);
  const [carregandoLista, setCarregandoLista] = useState(true);
  const [erroCarregamento, setErroCarregamento] = useState('');
  const [sugestoesPorCategoria, setSugestoesPorCategoria] = useState<Partial<Record<Categoria, string[]>>>({});
  const { notificar } = useToast();

  const [hora, setHora] = useState(horaAtual());
  const [tipo, setTipo] = useState<TipoMovimento>('saida');
  const [tecnico, setTecnico] = useState('');
  const [itens, setItens] = useState<ItemForm[]>([novoItemVazio()]);
  const [alertasCodigo, setAlertasCodigo] = useState<Record<number, ReparoPendente | 'nao-encontrado' | undefined>>(
    {}
  );
  const [erro, setErro] = useState('');
  const [enviando, setEnviando] = useState(false);
  const [fechando, setFechando] = useState(false);
  const [estornando, setEstornando] = useState<number | null>(null);

  async function carregarTudo() {
    setCarregandoLista(true);
    setErroCarregamento('');
    try {
      const [lista, fechamentoDoDia, abertos] = await Promise.all([
        listarMovimentosDoDia({ armazem_id: armazemId, fluxo: 'reparo_externo', data }),
        buscarFechamentoDoDia({ armazem_id: armazemId, fluxo: 'reparo_externo', data }),
        buscarReparosEmAberto(armazemId),
      ]);
      setLancamentos(lista);
      setFechamento(fechamentoDoDia);
      setReparosAbertos(abertos);
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
    setAlertasCodigo((atual) => {
      const { [indice]: _removido, ...resto } = atual;
      return resto;
    });
  }

  function limparFormulario() {
    setItens([novoItemVazio()]);
    setTecnico('');
    setAlertasCodigo({});
  }

  // Na entrada (retorno do conserto), confere se o codigo digitado bate com
  // alguma saida ainda em aberto - so um aviso (o backend nao exige a
  // correspondencia), ajuda a pegar erro de digitacao na hora.
  function verificarCodigo(indice: number) {
    if (tipo !== 'entrada') return;
    const codigo = itens[indice].codigoComponente.trim();
    if (!codigo) {
      setAlertasCodigo((atual) => ({ ...atual, [indice]: undefined }));
      return;
    }
    const achou = reparosAbertos.find((r) => r.codigo_componente === codigo);
    setAlertasCodigo((atual) => ({ ...atual, [indice]: achou ?? 'nao-encontrado' }));
  }

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setErro('');

    if (!tecnico.trim()) {
      setErro('Informe o tecnico/oficina externa.');
      return;
    }
    const itensComQuantidade = itens.filter((it) => it.quantidade > 0);
    if (itensComQuantidade.some((it) => !it.codigoComponente.trim())) {
      setErro('Informe o codigo/serie do componente (bateria, motor, modulo) de cada item.');
      return;
    }
    if (tipo === 'entrada' && itensComQuantidade.some((it) => !it.condicao)) {
      setErro('Informe o resultado do reparo (consertada, com defeito, sem conserto ou outro) de cada item.');
      return;
    }
    if (itensComQuantidade.some((it) => itemPrecisaObservacao(it) && !it.observacao.trim())) {
      setErro('Descreva o item na observacao quando escolher "Outro" na categoria ou no resultado.');
      return;
    }

    const itensValidos: MovimentoItemInput[] = itensComQuantidade.map((it) => ({
      categoria: it.categoria,
      descricao: it.descricao.trim() || null,
      quantidade: it.quantidade,
      observacao: it.observacao.trim() || null,
      codigo_componente: it.codigoComponente.trim(),
      condicao: tipo === 'entrada' ? it.condicao || null : null,
    }));

    if (itensValidos.length === 0) {
      setErro('Informe ao menos um item com quantidade valida.');
      return;
    }

    setEnviando(true);
    const resultado = await criarMovimento({
      armazem_id: armazemId,
      fluxo: 'reparo_externo',
      tipo,
      data,
      hora,
      turno: 'diurno',
      contraparte: tecnico.trim(),
      itens: itensValidos,
    });
    setEnviando(false);

    if (!resultado.ok) {
      setErro(resultado.error ?? 'Nao foi possivel registrar o lancamento.');
      return;
    }

    limparFormulario();
    await carregarTudo();
    onReparoAtualizado?.();
  }

  async function handleFecharDia() {
    if (lancamentos.length === 0) return;
    const confirmado = window.confirm(
      `Fechar o dia ${formatarData(data)}? Depois disso nao sera mais possivel adicionar ou corrigir lancamentos deste dia neste armazem.`
    );
    if (!confirmado) return;

    setErro('');
    setFechando(true);
    const resultado = await fecharDia({ armazem_id: armazemId, fluxo: 'reparo_externo', data });
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
    onReparoAtualizado?.();
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
          variante="reparo_externo"
        />
      </div>
    );
  }

  return (
    <div>
      <ResumoDoDia lancamentos={lancamentos} />
      <ReparosEmAberto pendentes={reparosAbertos} />

      <section className="cartao">
        <h2>Registrar {tipo === 'saida' ? 'saida' : 'entrada'} de reparo externo</h2>
        <p className="subtitulo">
          {formatarData(data)} - responsavel: {usuario.nome}. Bateria, motor ou modulo indo pro
          conserto{tipo === 'saida' ? ' com o tecnico externo.' : ' voltando consertado.'}
        </p>

        <form onSubmit={handleSubmit}>
          <div className="abas" style={{ marginBottom: 20 }}>
            <button
              type="button"
              className={`tipo-saida ${tipo === 'saida' ? 'ativo' : ''}`}
              onClick={() => setTipo('saida')}
            >
              Saida (vai pro tecnico)
            </button>
            <button
              type="button"
              className={`tipo-entrada ${tipo === 'entrada' ? 'ativo' : ''}`}
              onClick={() => {
                setTipo('entrada');
                setAlertasCodigo({});
              }}
            >
              Entrada (retorno do conserto)
            </button>
          </div>

          <div className="grade-formulario">
            <label>
              Tecnico/oficina externa
              <input
                value={tecnico}
                onChange={(e) => setTecnico(e.target.value)}
                placeholder="Ex: Tecnico Joao - Eletronica Silva"
                required
              />
            </label>

            <label>
              Horario
              <input type="time" value={hora} onChange={(e) => setHora(e.target.value)} required />
            </label>
          </div>

          <h3>Itens deste lancamento</h3>
          {itens.map((item, indice) => {
            const alerta = tipo === 'entrada' ? alertasCodigo[indice] : undefined;
            return (
              <div key={indice}>
                <div className="linha-item linha-item-reparo">
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
                    placeholder="Descricao (ex: Bateria 48V)"
                    list={`sugestoes-reparo-${item.categoria}`}
                  />
                  <datalist id={`sugestoes-reparo-${item.categoria}`}>
                    {(sugestoesPorCategoria[item.categoria] ?? []).map((s) => (
                      <option key={s} value={s} />
                    ))}
                  </datalist>

                  <input
                    value={item.codigoComponente}
                    onChange={(e) => atualizarItem(indice, { codigoComponente: e.target.value })}
                    onBlur={() => verificarCodigo(indice)}
                    placeholder="Codigo/serie"
                    required
                  />

                  {tipo === 'entrada' ? (
                    <select
                      value={item.condicao}
                      onChange={(e) => atualizarItem(indice, { condicao: e.target.value as Condicao | '' })}
                      required
                    >
                      <option value="">Resultado</option>
                      <option value="boa">Consertada</option>
                      <option value="defeito">Com defeito</option>
                      <option value="sucata">Sem conserto (sucata)</option>
                      <option value="outro">Outro</option>
                    </select>
                  ) : (
                    // Placeholder vazio pra manter o numero de colunas do
                    // grid constante (so faz sentido escolher resultado na
                    // entrada) - mesmo truque de Montagem.tsx.
                    <span />
                  )}

                  <input
                    value={item.observacao}
                    onChange={(e) => atualizarItem(indice, { observacao: e.target.value })}
                    placeholder={itemPrecisaObservacao(item) ? 'Descreva o item (obrigatorio)' : 'Observacao (opcional)'}
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
                {alerta === 'nao-encontrado' && (
                  <p className="erro" style={{ marginTop: -6, marginBottom: 10 }}>
                    Nenhuma saida em aberto com este codigo neste armazem - confira o codigo digitado.
                  </p>
                )}
                {alerta && alerta !== 'nao-encontrado' && (
                  <p className="sucesso" style={{ marginTop: -6, marginBottom: 10 }}>
                    Confere: saiu em {formatarData(alerta.data)} para {alerta.contraparte || 'tecnico externo'}.
                  </p>
                )}
              </div>
            );
          })}

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
                <th>Tecnico/Oficina</th>
                <th>Itens</th>
                <th>Qtd.</th>
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
                  <td>{m.contraparte || '-'}</td>
                  <td>
                    {m.itens
                      .map(
                        (it) =>
                          `${it.quantidade}x ${it.categoria}${it.descricao ? ' (' + it.descricao + ')' : ''}${it.codigo_componente ? ' [cod: ' + it.codigo_componente + ']' : ''}`
                      )
                      .join(' + ')}
                  </td>
                  <td>{m.itens.reduce((s, it) => s + it.quantidade, 0)}</td>
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
                  <td colSpan={8} className="rodape-tabela">
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
