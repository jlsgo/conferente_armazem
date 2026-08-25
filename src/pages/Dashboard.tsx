import { useState } from 'react';
import type { Armazem, Usuario } from '../types';
import Lancamentos from './Lancamentos';
import Montagem from './Montagem';
import Sac from './Sac';
import Historico from './Historico';
import Usuarios from './Usuarios';
import logoEcoviva from '../assets/ecoviva-logo.png';

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
        <button className="secundario somente-tela" onClick={onSair}>
          Sair
        </button>
      </header>

      <nav className="abas somente-tela" style={{ marginBottom: 20 }}>
        <button className={aba === 'lancamentos' ? 'ativo' : ''} onClick={() => setAba('lancamentos')}>
          Saida de Armazem
        </button>
        <button className={aba === 'montagem' ? 'ativo' : ''} onClick={() => setAba('montagem')}>
          Montagem
        </button>
        <button className={aba === 'sac' ? 'ativo' : ''} onClick={() => setAba('sac')}>
          SAC
        </button>
        <button className={aba === 'historico' ? 'ativo' : ''} onClick={() => setAba('historico')}>
          Historico
        </button>
        {ehGestor && (
          <button className={aba === 'usuarios' ? 'ativo' : ''} onClick={() => setAba('usuarios')}>
            Usuarios
          </button>
        )}
      </nav>

      <main>
        {aba === 'lancamentos' && <Lancamentos usuario={usuario} armazem={armazem} />}
        {aba === 'montagem' && <Montagem usuario={usuario} armazem={armazem} />}
        {aba === 'sac' && <Sac usuario={usuario} armazem={armazem} />}
        {aba === 'historico' && <Historico usuario={usuario} />}
        {aba === 'usuarios' && ehGestor && <Usuarios armazens={armazens} />}
      </main>
    </div>
  );
}
