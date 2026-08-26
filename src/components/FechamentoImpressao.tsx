import type { Armazem, Fechamento, Movimento } from '../types';
import { motivoSacTexto, situacaoInfo } from '../lib/situacao';
import { formatarData, formatarDataHora } from '../lib/data';

type Variante = 'armazem' | 'montagem' | 'sac';

interface Props {
  armazem: Armazem | undefined;
  data: string;
  fechamento: Fechamento;
  lancamentos: Movimento[];
  variante?: Variante;
}

const TITULOS: Record<Variante, string> = {
  armazem: 'Controle de Saidas de Armazem',
  montagem: 'Controle de Pecas para Montagem',
  sac: 'Controle de Saidas do SAC',
};

// Largura (%) de cada coluna na impressao, na mesma ordem dos <th> abaixo -
// sem isso o layout automatico espreme Coleta/Itens pra caber Observacoes,
// forcando quebra de linha em quase toda linha e estourando pra 2-3 paginas.
const LARGURAS_COLUNAS: Record<Variante, number[]> = {
  armazem: [3, 6, 8, 13, 24, 4, 8, 18, 8, 8],
  montagem: [4, 7, 10, 38, 5, 10, 14, 12],
  sac: [3, 6, 9, 15, 26, 4, 13, 12, 12],
};

export default function FechamentoImpressao({
  armazem,
  data,
  fechamento,
  lancamentos,
  variante = 'armazem',
}: Props) {
  const responsaveis = Array.from(new Set(lancamentos.map((m) => m.usuario_nome))).join(', ');
  const totalGeral = lancamentos.reduce(
    (soma, m) => soma + (m.estornado_de ? -1 : 1) * m.itens.reduce((s, it) => s + it.quantidade, 0),
    0
  );

  return (
    <section className="cartao area-impressao">
      <div className="cabecalho-impressao">
        <h2>
          {TITULOS[variante]} {armazem ? `- ${armazem.codigo}` : ''}
        </h2>
        <p>
          <strong>Data:</strong> {formatarData(data)} &nbsp; <strong>Responsavel(is):</strong> {responsaveis || '-'}
          &nbsp; <strong>Fechado em:</strong> {formatarDataHora(fechamento.criado_em)} (por {fechamento.usuario_nome})
        </p>
        <p className="rodape-tabela">
          hash de auditoria: {fechamento.hash_integridade.slice(0, 16)}...
        </p>
      </div>

      <div className="tabela-scroll">
      <table>
        <colgroup>
          {LARGURAS_COLUNAS[variante].map((largura, i) => (
            <col key={i} style={{ width: `${largura}%` }} />
          ))}
        </colgroup>
        <thead>
          <tr>
            <th>Nº</th>
            <th>Horario</th>
            {variante === 'armazem' && (
              <>
                <th>Pedido</th>
                <th>Coleta</th>
              </>
            )}
            {variante === 'montagem' && <th>Direcao</th>}
            {variante === 'sac' && (
              <>
                <th>Protocolo</th>
                <th>Coleta</th>
              </>
            )}
            <th>Itens</th>
            <th>Qtd.</th>
            {variante === 'armazem' && <th>Quem retirou</th>}
            {variante === 'montagem' && <th>Condicao</th>}
            {variante === 'sac' && <th>Motivo</th>}
            {variante === 'armazem' && <th>Observacoes</th>}
            <th>Registrado por</th>
            <th>Situacao</th>
          </tr>
        </thead>
        <tbody>
          {lancamentos.map((m) => (
            <tr key={m.id}>
              <td>{m.numero}</td>
              <td>{m.hora}</td>
              {variante === 'armazem' && (
                <>
                  <td>
                    {m.numero_pedido || '-'}
                    {!m.retirada_completa && ' (parcial)'}
                  </td>
                  <td>{m.contraparte || '-'}</td>
                </>
              )}
              {variante === 'montagem' && <td>{m.tipo === 'saida' ? 'Saida B2' : 'Entrada B2'}</td>}
              {variante === 'sac' && (
                <>
                  <td>{m.numero_pedido || '-'}</td>
                  <td>{m.contraparte || '-'}</td>
                </>
              )}
              <td>
                {m.itens
                  .map((it) => {
                    const base = `${it.quantidade}x ${it.categoria}${it.descricao ? ' (' + it.descricao + ')' : ''}${it.observacao ? ' - ' + it.observacao : ''}`;
                    const divergente = it.quantidade_enviada != null && it.quantidade_enviada !== it.quantidade;
                    return divergente ? `${base} [enviado: ${it.quantidade_enviada}]` : base;
                  })
                  .join(' + ')}
              </td>
              <td>{m.itens.reduce((s, it) => s + it.quantidade, 0)}</td>
              {variante === 'armazem' && <td>{m.quem_retirou || '-'}</td>}
              {variante === 'montagem' && (
                <td>{m.itens.map((it) => it.condicao).filter(Boolean).join(', ') || '-'}</td>
              )}
              {variante === 'sac' && <td>{motivoSacTexto(m)}</td>}
              {variante === 'armazem' && <td>{m.observacoes || '-'}</td>}
              <td>{m.usuario_nome}</td>
              <td>
                <span className={situacaoInfo(m).classe}>{situacaoInfo(m).texto}</span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      </div>

      <p className="total-fechamento">
        <strong>{totalGeral}</strong> unidades no total ({lancamentos.length} pedidos)
      </p>
      {fechamento.total_estornado > 0 && (
        <p className="rodape-tabela">
          Ajuste por estorno: -{fechamento.total_estornado} unidades. Total liquido do dia:{' '}
          <strong>{fechamento.total_liquido}</strong>.
        </p>
      )}

      <div className="assinatura">
        <div className="linha-assinatura" />
        <p>Assinatura da conferente responsavel</p>
      </div>

      <button className="somente-tela" onClick={() => window.print()}>
        Imprimir / Salvar como PDF
      </button>
    </section>
  );
}
