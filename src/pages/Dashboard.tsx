import type { Armazem, Usuario } from '../types';
import Lancamentos from './Lancamentos';

interface Props {
  usuario: Usuario;
  armazem: Armazem | undefined;
  onSair: () => void;
}

export default function Dashboard({ usuario, armazem, onSair }: Props) {
  return (
    <div className="pagina">
      <header className="topo">
        <div>
          <h1>Ecoviva - Controle de Armazem {armazem ? `(${armazem.codigo})` : ''}</h1>
          <p className="subtitulo">
            Ola, {usuario.nome} ({usuario.papel})
          </p>
        </div>
        <button className="secundario" onClick={onSair}>
          Sair
        </button>
      </header>

      <main>
        <Lancamentos usuario={usuario} />
      </main>
    </div>
  );
}
