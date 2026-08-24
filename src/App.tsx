import { useEffect, useState } from 'react';
import type { AppStatus, Usuario } from './types';
import { getStatus } from './lib/api';
import Setup from './pages/Setup';
import Login from './pages/Login';
import Dashboard from './pages/Dashboard';

export default function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [usuario, setUsuario] = useState<Usuario | null>(null);
  const [loading, setLoading] = useState(true);

  async function refreshStatus() {
    const s = await getStatus();
    setStatus(s);
  }

  useEffect(() => {
    refreshStatus().finally(() => setLoading(false));
  }, []);

  if (loading || !status) {
    return (
      <div className="tela-centralizada">
        <p>Carregando...</p>
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
      onSair={() => setUsuario(null)}
    />
  );
}
