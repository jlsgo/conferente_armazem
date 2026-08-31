import type { ReparoPendente } from '../types';
import { formatarData } from '../lib/data';

interface Props {
  pendentes: ReparoPendente[];
}

/**
 * Secao "reparos em aberto" da tela de Reparo Externo - pecas que ja sairam
 * pra tecnico externo e ainda nao tem entrada de retorno com o mesmo
 * `codigo_componente`. So leitura: ao contrario de `TransferenciasChegando`,
 * nao ha acao de "confirmar" aqui - a propria entrada lancada no formulario
 * abaixo (mesmo codigo) e o que fecha a pendencia.
 */
export default function ReparosEmAberto({ pendentes }: Props) {
  if (pendentes.length === 0) return null;

  return (
    <section className="cartao somente-tela">
      <h2>Reparos em aberto</h2>
      <p className="subtitulo">
        Pecas que ja sairam para conserto com tecnico externo e ainda nao tem retorno registrado.
      </p>
      <div className="tabela-scroll">
        <table>
          <thead>
            <tr>
              <th>Codigo</th>
              <th>Item</th>
              <th>Tecnico/oficina</th>
              <th>Saiu em</th>
            </tr>
          </thead>
          <tbody>
            {pendentes.map((p) => (
              <tr key={p.item_id}>
                <td>{p.codigo_componente}</td>
                <td>
                  {p.quantidade}x {p.categoria}
                  {p.descricao ? ` (${p.descricao})` : ''}
                </td>
                <td>{p.contraparte || '-'}</td>
                <td>
                  {formatarData(p.data)} {p.hora}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
