import { useEffect, useState } from 'react';
import type { Armazem, StatusSincronizacao, Usuario } from '../types';
import Lancamentos from './Lancamentos';
import Montagem from './Montagem';
import Sac from './Sac';
import Historico from './Historico';
import Usuarios from './Usuarios';
import logoEcoviva from '../assets/ecoviva-logo.png';
import { sincronizarAgora, statusSincronizacao } from '../lib/api';

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
  const [mensagemSync, setMensagemSync] = useState('');
  const [statusSync, setStatusSync] = useState<StatusSincronizacao | null>(null);

  async function atualizarStatusSync() {
    if (ehGestor) setStatusSync(await statusSincronizacao());
  }

  async function handleSincronizar() {
    setSincronizando(true);
    setMensagemSync('');
    const resultado = await sincronizarAgora();
    setSincronizando(false);
    setMensagemSync(resultado.ok ? resultado.mensagem ?? '' : resultado.error ?? 'Falha ao sincronizar.');
    await atualizarStatusSync();
  }

  useEffect(() => {
    if (!ehGestor) return;
    atualizarStatusSync();
    const intervalo = setInterval(() => {
      handleSincronizar();
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
              {mensagemSync && <span className="subtitulo">{mensagemSync}</span>}
              {!mensagemSync && statusSync && statusSync.pendentes > 0 && (
                <span className="subtitulo" title={statusSync.ultimo_erro ?? ''}>
                  {statusSync.pendentes} pendente{statusSync.pendentes > 1 ? 's' : ''}
                  {statusSync.com_erro > 0 ? ` (${statusSync.com_erro} com erro)` : ''}
                </span>
              )}
              <button className="info" onClick={handleSincronizar} disabled={sincronizando}>
                {sincronizando ? 'Sincronizando...' : 'Sincronizar agora'}
              </button>
            </>
          )}
          <button className="secundario" onClick={onSair}>
            Sair
          </button>
        </div>
      </header>

      <nav className="abas somente-tela" style={{ marginBottom: 20 }}>
        <button
          className={`aba-lancamentos${aba === 'lancamentos' ? ' ativo' : ''}`}
          onClick={() => setAba('lancamentos')}
        >
          Saida de Armazem
        </button>
        <button
          className={`aba-montagem${aba === 'montagem' ? ' ativo' : ''}`}
          onClick={() => setAba('montagem')}
        >
          Montagem
        </button>
        <button className={`aba-sac${aba === 'sac' ? ' ativo' : ''}`} onClick={() => setAba('sac')}>
          SAC
        </button>
        <button
          className={`aba-historico${aba === 'historico' ? ' ativo' : ''}`}
          onClick={() => setAba('historico')}
        >
          Historico
        </button>
        {ehGestor && (
          <button
            className={`aba-usuarios${aba === 'usuarios' ? ' ativo' : ''}`}
            onClick={() => setAba('usuarios')}
          >
            Usuarios
          </button>
        )}
      </nav>

      <main className={`conteudo-aba conteudo-aba-${aba}`}>
        {aba === 'lancamentos' && <Lancamentos usuario={usuario} armazem={armazem} />}
        {aba === 'montagem' && <Montagem usuario={usuario} armazem={armazem} armazens={armazens} />}
        {aba === 'sac' && <Sac usuario={usuario} armazem={armazem} />}
        {aba === 'historico' && <Historico usuario={usuario} />}
        {aba === 'usuarios' && ehGestor && <Usuarios armazens={armazens} />}
      </main>
    </div>
  );
}
