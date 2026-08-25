import type { Armazem, Fechamento, Movimento } from '../types';
import { situacaoInfo } from '../lib/situacao';

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

function formatarReais(centavos: number): string {
  return (centavos / 100).toLocaleString('pt-BR', { style: 'currency', currency: 'BRL' });
}

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
          <strong>Data:</strong> {data} &nbsp; <strong>Responsavel(is):</strong> {responsaveis || '-'}
        </p>
        <p className="rodape-tabela">
          Fechado por {fechamento.usuario_nome} em {fechamento.criado_em} - hash de auditoria:{' '}
          {fechamento.hash_integridade.slice(0, 16)}...
        </p>
      </div>

      <div className="tabela-scroll">
      <table>
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
            {variante === 'armazem' && <th>Rastreio</th>}
            <th>Itens</th>
            <th>Qtd.</th>
            {variante === 'armazem' && <th>Quem retirou</th>}
            {variante === 'montagem' && <th>Condicao</th>}
            {variante === 'sac' && <th>Garantia/Venda</th>}
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
                  <td>{m.numero_pedido || '-'}</td>
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
              {variante === 'armazem' && <td>{m.codigo_rastreio || '-'}</td>}
              <td>
                {m.itens
                  .map(
                    (it) =>
                      `${it.quantidade}x ${it.categoria}${it.descricao ? ' (' + it.descricao + ')' : ''}${it.observacao ? ' - ' + it.observacao : ''}`
                  )
                  .join(' + ')}
              </td>
              <td>{m.itens.reduce((s, it) => s + it.quantidade, 0)}</td>
              {variante === 'armazem' && <td>{m.quem_retirou || '-'}</td>}
              {variante === 'montagem' && (
                <td>{m.itens.map((it) => it.condicao).filter(Boolean).join(', ') || '-'}</td>
              )}
              {variante === 'sac' && (
                <td>
                  {m.motivo === 'venda'
                    ? `Venda (${formatarReais(m.valor_centavos ?? 0)})`
                    : m.motivo === 'garantia'
                      ? 'Garantia'
                      : '-'}
                </td>
              )}
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
