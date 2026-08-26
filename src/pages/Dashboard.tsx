import { useEffect, useState } from 'react';
import type { Armazem, StatusSincronizacao, Usuario } from '../types';
import Lancamentos from './Lancamentos';
import Montagem from './Montagem';
import Sac from './Sac';
import Historico from './Historico';
import Usuarios from './Usuarios';
import logoEcoviva from '../assets/ecoviva-logo.png';
import { sincronizarAgora, statusSincronizacao } from '../lib/api';
import { IconAjuste, IconCaixa, IconChat, IconLogout, IconRelogio, IconSpinner, IconUsuarios } from '../components/Icon';
import { useToast } from '../lib/toast';

const INTERVALO_RETRY_SYNC_MS = 5 * 60 * 1000;

interface Props {
  usuario: Usuario;
  armazem: Armazem | undefined;
  armazens: Armazem[];
  onSair: () => void;
}

type Aba = 'lancamentos' | 'montagem' | 'sac' | 'historico' | 'usuarios';

export default function Dashboard({ usuario, armazem, armazens, onSair }: Props) {
  const [aba, setAba] = useState<Aba>('lancamentos');
  const ehGestor = usuario.papel === 'gestor';

  const [sincronizando, setSincronizando] = useState(false);
  const [statusSync, setStatusSync] = useState<StatusSincronizacao | null>(null);
  const { notificar } = useToast();

  async function atualizarStatusSync() {
    if (ehGestor) setStatusSync(await statusSincronizacao());
  }

  async function handleSincronizar(manual: boolean) {
    setSincronizando(true);
    const resultado = await sincronizarAgora();
    setSincronizando(false);
    if (manual || !resultado.ok) {
      notificar(
        resultado.ok ? resultado.mensagem ?? 'Sincronizado.' : resultado.error ?? 'Falha ao sincronizar.',
        resultado.ok ? 'sucesso' : 'erro'
      );
    }
    await atualizarStatusSync();
  }

  useEffect(() => {
    if (!ehGestor) return;
    atualizarStatusSync();
    const intervalo = setInterval(() => {
      handleSincronizar(false);
    }, INTERVALO_RETRY_SYNC_MS);
    return () => clearInterval(intervalo);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ehGestor]);

  return (
    <div className="pagina">
      <header className="topo">
        <div className="topo-marca">
          <img src={logoEcoviva} alt="Ecoviva" />
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
              <button className="info" onClick={() => handleSincronizar(true)} disabled={sincronizando}>
                {sincronizando ? <IconSpinner size={15} /> : null}
                {sincronizando ? 'Sincronizando...' : 'Sincronizar agora'}
              </button>
            </>
          )}
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
        >
          <IconCaixa size={15} />
          Saida de Armazem
        </button>
        <button
          className={`aba-montagem${aba === 'montagem' ? ' ativo' : ''}`}
          onClick={() => setAba('montagem')}
        >
          <IconAjuste size={15} />
          Montagem
        </button>
        <button className={`aba-sac${aba === 'sac' ? ' ativo' : ''}`} onClick={() => setAba('sac')}>
          <IconChat size={15} />
          SAC
        </button>
        <button
          className={`aba-historico${aba === 'historico' ? ' ativo' : ''}`}
          onClick={() => setAba('historico')}
        >
          <IconRelogio size={15} />
          Historico
        </button>
        {ehGestor && (
          <button
            className={`aba-usuarios${aba === 'usuarios' ? ' ativo' : ''}`}
            onClick={() => setAba('usuarios')}
          >
            <IconUsuarios size={15} />
            Usuarios
          </button>
        )}
      </nav>

      <main className={`conteudo-aba conteudo-aba-${aba}`}>
        {aba === 'lancamentos' && <Lancamentos usuario={usuario} armazem={armazem} armazens={armazens} />}
        {aba === 'montagem' && <Montagem usuario={usuario} armazem={armazem} armazens={armazens} />}
        {aba === 'sac' && <Sac usuario={usuario} armazem={armazem} />}
        {aba === 'historico' && <Historico usuario={usuario} />}
        {aba === 'usuarios' && ehGestor && <Usuarios armazens={armazens} />}
      </main>
    </div>
  );
}
