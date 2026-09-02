import { useEffect, useState } from 'react';
import type { Armazem, Fluxo, TransferenciaRecusada } from '../types';
import { buscarTransferenciasRecusadas } from '../lib/api';
import { formatarData } from '../lib/data';
import { useToast } from '../lib/toast';

interface Props {
  fluxo: Fluxo;
  outroArmazem: Armazem | undefined;
}

const INTERVALO_POLL_MS = 60 * 1000;

function chaveTransferencia(t: TransferenciaRecusada): string {
  return `${t.armazem_que_recusou_codigo}:${t.meu_movimento_id}`;
}

/**
 * Faixa "transferencias recusadas" - o espelho de `TransferenciasChegando`,
 * mostrado nas telas de quem ENVIA (nao de quem recebe). So leitura: a acao
 * de corrigir e usar o botao Estornar que ja existe na propria lista de
 * lancamentos do dia, no lancamento com o numero indicado aqui (ver o
 * comentario em `db::sync::TransferenciaRecusada` sobre por que nao ha um
 * botao dedicado - assim que estornar, este aviso some sozinho).
 */
export default function TransferenciasRecusadas({ fluxo, outroArmazem }: Props) {
  const [recusadas, setRecusadas] = useState<TransferenciaRecusada[]>([]);
  const [carregando, setCarregando] = useState(true);
  const [erro, setErro] = useState('');
  const { notificar } = useToast();

  async function carregar() {
    setCarregando(true);
    setErro('');
    try {
      const todas = await buscarTransferenciasRecusadas();
      setRecusadas(todas.filter((t) => t.fluxo === fluxo));
    } catch (err) {
      const mensagem = typeof err === 'string' ? err : 'Nao foi possivel verificar transferencias recusadas.';
      setErro(mensagem);
      notificar(mensagem, 'erro');
    } finally {
      setCarregando(false);
    }
  }

  useEffect(() => {
    carregar();
    const intervalo = setInterval(carregar, INTERVALO_POLL_MS);
    return () => clearInterval(intervalo);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fluxo]);

  if (erro || carregando || recusadas.length === 0) return null;

  return (
    <section className="cartao somente-tela">
      <h2 style={{ color: 'var(--aviso-escuro)' }}>
        Transferencias recusadas{outroArmazem ? ` por ${outroArmazem.codigo}` : ''}
      </h2>
      <p className="subtitulo">
        O armazem que ia receber recusou o recebimento abaixo. Corrija o lancamento original (numero
        indicado) usando o botao Estornar na lista de lancamentos do dia - assim que estornar, este
        aviso some.
      </p>
      <div className="tabela-scroll">
        <table>
          <thead>
            <tr>
              <th>Data da recusa</th>
              <th>Pedido</th>
              <th>Meu lancamento</th>
              <th>Motivo da recusa</th>
              <th>Itens recusados</th>
            </tr>
          </thead>
          <tbody>
            {recusadas.map((t) => (
              <tr key={chaveTransferencia(t)}>
                <td>
                  {formatarData(t.data)} {t.hora}
                </td>
                <td>{t.numero_pedido ?? '-'}</td>
                <td>#{t.meu_movimento_id}</td>
                <td>{t.justificativa ?? '-'}</td>
                <td>
                  {t.itens
                    .map((it) => `${it.categoria}${it.descricao ? ` (${it.descricao})` : ''} x${it.quantidade}`)
                    .join(', ')}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
