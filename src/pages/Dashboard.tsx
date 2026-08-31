import { useEffect, useState } from 'react';
import type { Armazem, Fluxo, StatusSincronizacao, Usuario } from '../types';
import Lancamentos from './Lancamentos';
import Montagem from './Montagem';
import Sac from './Sac';
import ReparoExterno from './ReparoExterno';
import Historico from './Historico';
import Usuarios from './Usuarios';
import logoEcoviva from '../assets/ecoviva-logo.png';
import { buscarReparosEmAberto, buscarTransferenciasPendentes, sincronizarAgora, statusSincronizacao } from '../lib/api';
import { useCliquesSecretos } from '../hooks/useCliquesSecretos';
import CobrinhaSecreta from '../components/CobrinhaSecreta';
import {
  IconAjuste,
  IconCaixa,
  IconChat,
  IconFerramenta,
  IconLogout,
  IconRelogio,
  IconSpinner,
  IconUsuarios,
} from '../components/Icon';
import { useToast } from '../lib/toast';

const INTERVALO_STATUS_SYNC_MS = 5 * 60 * 1000;
const INTERVALO_PENDENTES_MS = 60 * 1000;

interface Props {
  usuario: Usuario;
  armazem: Armazem | undefined;
  armazens: Armazem[];
  versao?: string;
  onSair: () => void;
}

type Aba = 'lancamentos' | 'montagem' | 'sac' | 'reparo_externo' | 'historico' | 'usuarios';

export default function Dashboard({ usuario, armazem, armazens, versao, onSair }: Props) {
  const [aba, setAba] = useState<Aba>('lancamentos');
  const ehGestor = usuario.papel === 'gestor';

  const [sincronizando, setSincronizando] = useState(false);
  const [statusSync, setStatusSync] = useState<StatusSincronizacao | null>(null);
  const [pendentesPorFluxo, setPendentesPorFluxo] = useState<Partial<Record<Fluxo, number>>>({});
  const [reparosEmAberto, setReparosEmAberto] = useState(0);
  const { notificar } = useToast();
  const cobrinha = useCliquesSecretos();

  async function atualizarStatusSync() {
    if (ehGestor) setStatusSync(await statusSincronizacao());
  }

  async function atualizarPendentes() {
    try {
      const todas = await buscarTransferenciasPendentes();
      const contagem: Partial<Record<Fluxo, number>> = {};
      for (const t of todas) {
        contagem[t.fluxo] = (contagem[t.fluxo] ?? 0) + 1;
      }
      setPendentesPorFluxo(contagem);
    } catch {
      // So um atalho visual (contador nas abas) - a tela de cada fluxo ja
      // mostra erro de verdade, com retry, se isso falhar de novo por la.
    }
  }

  // Contador de reparos externos em aberto - endpoint separado dos
  // "pendentesPorFluxo" acima, que e especificamente sobre transferencias
  // entre A4/B2 (nao existe pra reparo externo). So faz sentido pra quem tem
  // um armazem fixo (gestor sem armazem fixo nao usa essa tela).
  async function atualizarReparosEmAberto() {
    if (!armazem) {
      setReparosEmAberto(0);
      return;
    }
    try {
      const abertos = await buscarReparosEmAberto(armazem.id);
      setReparosEmAberto(abertos.length);
    } catch {
      // Mesmo atalho visual - so um contador, a tela de Reparo Externo mostra
      // erro de verdade com retry se isso falhar de novo por la.
    }
  }

  // O retry automatico de verdade agora roda no backend (loop em `lib.rs`,
  // independente de sessao/gestor - ver docs/ARQUITETURA.md), entao esta
  // funcao so cobre o clique manual do botao "Sincronizar agora".
  async function handleSincronizar() {
    setSincronizando(true);
    const resultado = await sincronizarAgora();
    setSincronizando(false);
    notificar(
      resultado.ok ? resultado.mensagem ?? 'Sincronizado.' : resultado.error ?? 'Falha ao sincronizar.',
      resultado.ok ? 'sucesso' : 'erro'
    );
    await atualizarStatusSync();
    await atualizarPendentes();
  }

  // So atualiza o retrato local (sem rede) periodicamente - quem de fato
  // tenta sincronizar com o Turso e o loop de backend, nao esta tela.
  useEffect(() => {
    if (!ehGestor) return;
    atualizarStatusSync();
    const intervalo = setInterval(atualizarStatusSync, INTERVALO_STATUS_SYNC_MS);
    return () => clearInterval(intervalo);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ehGestor]);

  // Contador de transferencias pendentes nas abas - pra qualquer usuario (nao
  // so gestor), ja que quem recebe fisicamente e confirma e o conferente.
  useEffect(() => {
    atualizarPendentes();
    atualizarReparosEmAberto();
    const intervalo = setInterval(() => {
      atualizarPendentes();
      atualizarReparosEmAberto();
    }, INTERVALO_PENDENTES_MS);
    return () => clearInterval(intervalo);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [armazem?.id]);

  return (
    <div className="pagina">
      <header className="topo">
        <div className="topo-marca">
          <img src={logoEcoviva} alt="Ecoviva" onClick={cobrinha.registrarClique} />
          <div>
            <h1>Controle de Armazem {armazem ? `(${armazem.codigo})` : ''}</h1>
            <p className="subtitulo" style={{ margin: 0 }}>
              Ola, {usuario.nome} ({usuario.papel})
            </p>
          </div>
        </div>
        <div className="somente-tela" style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          {ehGestor && (
            <>
              {statusSync && statusSync.pendentes > 0 && (
                <span className="subtitulo" title={statusSync.ultimo_erro ?? ''}>
                  {statusSync.pendentes} pendente{statusSync.pendentes > 1 ? 's' : ''}
                  {statusSync.com_erro > 0 ? ` (${statusSync.com_erro} com erro)` : ''}
                </span>
              )}
              <button className="info" onClick={handleSincronizar} disabled={sincronizando}>
                {sincronizando ? <IconSpinner size={15} /> : null}
                {sincronizando ? 'Sincronizando...' : 'Sincronizar agora'}
              </button>
            </>
          )}
          {versao && <span className="pilula-versao">v{versao}</span>}
          <button className="secundario" onClick={onSair}>
            <IconLogout size={15} />
            Sair
          </button>
        </div>
      </header>

      <nav className="abas somente-tela" style={{ marginBottom: 20 }}>
        <button
          className={`aba-lancamentos${aba === 'lancamentos' ? ' ativo' : ''}`}
          onClick={() => setAba('lancamentos')}
          aria-current={aba === 'lancamentos' ? 'page' : undefined}
        >
          <IconCaixa size={15} />
          Saida de Armazem
          {!!pendentesPorFluxo.saida_armazem && (
            <span className="badge badge-notificacao" title="Transferencias aguardando confirmacao">
              {pendentesPorFluxo.saida_armazem}
            </span>
          )}
        </button>
        <button
          className={`aba-montagem${aba === 'montagem' ? ' ativo' : ''}`}
          onClick={() => setAba('montagem')}
          aria-current={aba === 'montagem' ? 'page' : undefined}
        >
          <IconAjuste size={15} />
          Montagem
          {!!pendentesPorFluxo.peca_montagem && (
            <span className="badge badge-notificacao" title="Transferencias aguardando confirmacao">
              {pendentesPorFluxo.peca_montagem}
            </span>
          )}
        </button>
        <button
          className={`aba-sac${aba === 'sac' ? ' ativo' : ''}`}
          onClick={() => setAba('sac')}
          aria-current={aba === 'sac' ? 'page' : undefined}
        >
          <IconChat size={15} />
          SAC
        </button>
        <button
          className={`aba-reparo${aba === 'reparo_externo' ? ' ativo' : ''}`}
          onClick={() => setAba('reparo_externo')}
          aria-current={aba === 'reparo_externo' ? 'page' : undefined}
        >
          <IconFerramenta size={15} />
          Reparo Externo
          {!!reparosEmAberto && (
            <span className="badge badge-notificacao" title="Pecas aguardando retorno do tecnico">
              {reparosEmAberto}
            </span>
          )}
        </button>
        <button
          className={`aba-historico${aba === 'historico' ? ' ativo' : ''}`}
          onClick={() => setAba('historico')}
          aria-current={aba === 'historico' ? 'page' : undefined}
        >
          <IconRelogio size={15} />
          Historico
        </button>
        {ehGestor && (
          <button
            className={`aba-usuarios${aba === 'usuarios' ? ' ativo' : ''}`}
            onClick={() => setAba('usuarios')}
            aria-current={aba === 'usuarios' ? 'page' : undefined}
          >
            <IconUsuarios size={15} />
            Usuarios
          </button>
        )}
      </nav>

      <main className={`conteudo-aba conteudo-aba-${aba}`}>
        {aba === 'lancamentos' && (
          <Lancamentos
            usuario={usuario}
            armazem={armazem}
            armazens={armazens}
            onTransferenciaConfirmada={atualizarPendentes}
          />
        )}
        {aba === 'montagem' && (
          <Montagem
            usuario={usuario}
            armazem={armazem}
            armazens={armazens}
            onTransferenciaConfirmada={atualizarPendentes}
          />
        )}
        {aba === 'sac' && <Sac usuario={usuario} armazem={armazem} />}
        {aba === 'reparo_externo' && (
          <ReparoExterno usuario={usuario} armazem={armazem} onReparoAtualizado={atualizarReparosEmAberto} />
        )}
        {aba === 'historico' && <Historico usuario={usuario} armazem={armazem} />}
        {aba === 'usuarios' && ehGestor && <Usuarios armazens={armazens} />}
      </main>
      {cobrinha.ativo && <CobrinhaSecreta onFechar={cobrinha.fechar} />}
    </div>
  );
}
