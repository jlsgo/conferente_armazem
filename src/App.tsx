import { useEffect, useState } from 'react';
import type { AppStatus, Usuario } from './types';
import { getStatus, logout } from './lib/api';
import Setup from './pages/Setup';
import Login from './pages/Login';
import Dashboard from './pages/Dashboard';
import Carregando from './components/Carregando';

export default function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [usuario, setUsuario] = useState<Usuario | null>(null);
  const [loading, setLoading] = useState(true);
  const [erro, setErro] = useState('');

  async function refreshStatus() {
    setErro('');
    try {
      const s = await getStatus();
      setStatus(s);
    } catch (err) {
      setErro(typeof err === 'string' ? err : 'Nao foi possivel iniciar o aplicativo.');
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    refreshStatus();
  }, []);

  if (loading) {
    return (
      <div className="tela-centralizada">
        <Carregando />
      </div>
    );
  }

  if (erro || !status) {
    return (
      <div className="tela-centralizada">
        <p className="erro">{erro || 'Nao foi possivel iniciar o aplicativo.'}</p>
        <button type="button" onClick={refreshStatus}>
          Tentar novamente
        </button>
      </div>
    );
  }

  if (status.precisa_configurar_primeiro_usuario) {
    return <Setup armazens={status.armazens} onConcluido={refreshStatus} />;
  }

  if (!usuario) {
    return <Login onLogin={setUsuario} />;
  }

  const armazem = status.armazens.find((a) => a.id === usuario.armazem_id);

  return (
    <Dashboard
      usuario={usuario}
      armazem={armazem}
      armazens={status.armazens}
      onSair={() => {
        logout().finally(() => setUsuario(null));
      }}
    />
  );
}
