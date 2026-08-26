import { useEffect, useState } from 'react';
import type { Armazem, Fluxo, TransferenciaPendente } from '../types';
import { buscarTransferenciasPendentes, confirmarRecebimento } from '../lib/api';
import { useToast } from '../lib/toast';

interface Props {
  fluxo: Fluxo;
  outroArmazem: Armazem | undefined;
  onConfirmado: () => void;
}

function chaveTransferencia(t: TransferenciaPendente): string {
  return `${t.armazem_origem_codigo}:${t.id_origem}`;
}

function horaAtual(): string {
  const agora = new Date();
  return `${String(agora.getHours()).padStart(2, '0')}:${String(agora.getMinutes()).padStart(2, '0')}`;
}

/**
 * Secao "transferencias aguardando confirmacao" compartilhada entre
 * Lancamentos (fluxo saida_armazem) e Montagem (fluxo peca_montagem) - a
 * lista que vem de `buscarTransferenciasPendentes` mistura os dois fluxos
 * (mesmo armazem de destino), entao cada tela filtra so a sua parte.
 */
export default function TransferenciasChegando({ fluxo, outroArmazem, onConfirmado }: Props) {
  const [pendentes, setPendentes] = useState<TransferenciaPendente[]>([]);
  const [carregando, setCarregando] = useState(true);
  const [erro, setErro] = useState('');
  const [confirmando, setConfirmando] = useState<string | null>(null);
  const [quantidadesRecebidas, setQuantidadesRecebidas] = useState<Record<string, number[]>>({});
  const { notificar } = useToast();

  async function carregar() {
    setCarregando(true);
    setErro('');
    try {
      const todas = await buscarTransferenciasPendentes();
      setPendentes(todas.filter((t) => t.fluxo === fluxo));
    } catch (err) {
      const mensagem =
        typeof err === 'string' ? err : 'Nao foi possivel verificar transferencias aguardando confirmacao.';
      setErro(mensagem);
      notificar(mensagem, 'erro');
    } finally {
      setCarregando(false);
    }
  }

  useEffect(() => {
    carregar();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fluxo]);

  useEffect(() => {
    setQuantidadesRecebidas((atual) => {
      const novo = { ...atual };
      for (const t of pendentes) {
        const chave = chaveTransferencia(t);
        if (!novo[chave]) {
          novo[chave] = t.itens.map((it) => it.quantidade);
        }
      }
      return novo;
    });
  }, [pendentes]);

  function atualizarQuantidadeRecebida(chave: string, indice: number, valor: number) {
    setQuantidadesRecebidas((atual) => ({
      ...atual,
      [chave]: (atual[chave] ?? []).map((q, i) => (i === indice ? valor : q)),
    }));
  }

  async function handleConfirmar(t: TransferenciaPendente) {
    const chave = chaveTransferencia(t);
    setConfirmando(chave);
    const quantidades = quantidadesRecebidas[chave] ?? t.itens.map((it) => it.quantidade);
    const resultado = await confirmarRecebimento(
      t.armazem_origem_codigo,
      t.id_origem,
      horaAtual(),
      quantidades
    );
    setConfirmando(null);

    if (!resultado.ok) {
      notificar(resultado.error ?? 'Nao foi possivel confirmar o recebimento.', 'erro');
      return;
    }

    await carregar();
    onConfirmado();
  }

  if (erro) {
    return (
      <section className="cartao somente-tela">
        <p className="erro">{erro}</p>
        <button type="button" onClick={carregar}>
          Tentar novamente
        </button>
      </section>
    );
  }

  if (carregando || pendentes.length === 0) return null;

  return (
    <section className="cartao somente-tela">
      <h2>Transferencias chegando{outroArmazem ? ` de ${outroArmazem.codigo}` : ''}</h2>
      <p className="subtitulo">
        Confira fisicamente o que chegou antes de confirmar. A quantidade ja vem preenchida com o
        que foi enviado - so mude se chegou diferente.
      </p>
      <div className="tabela-scroll">
        <table>
          <thead>
            <tr>
              <th>Data do envio</th>
              <th>Itens (enviado / recebido)</th>
              <th className="somente-tela">Acoes</th>
            </tr>
          </thead>
          <tbody>
            {pendentes.map((t) => {
              const chave = chaveTransferencia(t);
              return (
                <tr key={chave}>
                  <td>
                    {t.data} {t.hora}
                  </td>
                  <td>
                    {t.itens.map((it, indice) => (
                      <div
                        key={indice}
                        style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 4 }}
                      >
                        <span>
                          {it.categoria}
                          {it.descricao ? ` (${it.descricao})` : ''} - enviado {it.quantidade}:
                        </span>
                        <input
                          type="number"
                          min={1}
                          max={it.quantidade}
                          value={quantidadesRecebidas[chave]?.[indice] ?? it.quantidade}
                          onChange={(e) =>
                            atualizarQuantidadeRecebida(chave, indice, Number(e.target.value))
                          }
                          style={{ width: 70 }}
                        />
                      </div>
                    ))}
                  </td>
                  <td className="somente-tela">
                    <button
                      type="button"
                      onClick={() => handleConfirmar(t)}
                      disabled={confirmando === chave}
                    >
                      {confirmando === chave ? 'Confirmando...' : 'Confirmar recebimento'}
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </section>
  );
}
