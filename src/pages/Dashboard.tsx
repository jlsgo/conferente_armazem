import { useState } from 'react';
import type { Armazem, Usuario } from '../types';
import Lancamentos from './Lancamentos';
import Usuarios from './Usuarios';

interface Props {
  usuario: Usuario;
  armazem: Armazem | undefined;
  armazens: Armazem[];
  onSair: () => void;
}

type Aba = 'lancamentos' | 'usuarios';

export default function Dashboard({ usuario, armazem, armazens, onSair }: Props) {
  const [aba, setAba] = useState<Aba>('lancamentos');
  const ehGestor = usuario.papel === 'gestor';

  return (
    <div className="pagina">
      <header className="topo">
        <div>
          <h1>Ecoviva - Controle de Armazem {armazem ? `(${armazem.codigo})` : ''}</h1>
          <p className="subtitulo">
            Ola, {usuario.nome} ({usuario.papel})
          </p>
        </div>
        <button className="secundario somente-tela" onClick={onSair}>
          Sair
        </button>
      </header>

      {ehGestor && (
        <nav className="abas somente-tela" style={{ marginBottom: 20 }}>
          <button className={aba === 'lancamentos' ? 'ativo' : ''} onClick={() => setAba('lancamentos')}>
            Lancamentos
          </button>
          <button className={aba === 'usuarios' ? 'ativo' : ''} onClick={() => setAba('usuarios')}>
            Usuarios
          </button>
        </nav>
      )}

      <main>
        {aba === 'lancamentos' && <Lancamentos usuario={usuario} armazem={armazem} />}
        {aba === 'usuarios' && ehGestor && <Usuarios usuarioLogado={usuario} armazens={armazens} />}
      </main>
    </div>
  );
}
