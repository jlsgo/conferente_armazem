import type { Armazem, Fechamento, Movimento } from '../types';

interface Props {
  armazem: Armazem | undefined;
  data: string;
  fechamento: Fechamento;
  lancamentos: Movimento[];
}

export default function FechamentoImpressao({ armazem, data, fechamento, lancamentos }: Props) {
  const responsaveis = Array.from(new Set(lancamentos.map((m) => m.usuario_nome))).join(', ');
  const totalGeral = lancamentos.reduce((soma, m) => soma + m.itens.reduce((s, it) => s + it.quantidade, 0), 0);

  return (
    <section className="cartao area-impressao">
      <div className="cabecalho-impressao">
        <h2>Controle de Saidas de Armazem {armazem ? `- ${armazem.codigo}` : ''}</h2>
        <p>
          <strong>Data:</strong> {data} &nbsp; <strong>Responsavel(is):</strong> {responsaveis || '-'}
        </p>
        <p className="rodape-tabela">
          Fechado por {fechamento.usuario_nome} em {fechamento.criado_em} - hash de auditoria:{' '}
          {fechamento.hash_integridade.slice(0, 16)}...
        </p>
      </div>

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
        </tbody>
      </table>

      <p className="total-fechamento">
        <strong>{totalGeral}</strong> unidades no total ({lancamentos.length} pedidos)
      </p>

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
