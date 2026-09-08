import { useEffect, useState } from 'react';
import type { AppStatus, Usuario } from './types';
import { getStatus, logout } from './lib/api';
import Setup from './pages/Setup';
import Login from './pages/Login';
import Dashboard from './pages/Dashboard';
import Carregando from './components/Carregando';
import { useTema } from './hooks/useTema';

export default function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [usuario, setUsuario] = useState<Usuario | null>(null);
  const [loading, setLoading] = useState(true);
  const [erro, setErro] = useState('');
  // Chamado aqui (nao dentro de Dashboard) pra aplicar o tema salvo/do SO ja
  // nas telas de Setup/Login tambem, antes de existir sessao - Dashboard so
  // recebe de volta o valor e o alternador pra mostrar o botao no cabecalho.
  const { tema, alternar: alternarTema } = useTema();

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
        <p className="erro" role="alert">{erro || 'Nao foi possivel iniciar o aplicativo.'}</p>
        <button type="button" onClick={refreshStatus}>
          Tentar novamente
        </button>
      </div>
    );
  }

  if (status.precisa_configurar_primeiro_usuario) {
    return <Setup armazens={status.armazens} onConcluido={refreshStatus} versao={status.versao} />;
  }

  if (!usuario) {
    return <Login onLogin={setUsuario} versao={status.versao} />;
  }

  const armazem = status.armazens.find((a) => a.id === usuario.armazem_id);

  return (
    <Dashboard
      usuario={usuario}
      armazem={armazem}
      armazens={status.armazens}
      versao={status.versao}
      tema={tema}
      onAlternarTema={alternarTema}
      onSair={() => {
        logout().finally(() => setUsuario(null));
      }}
    />
  );
}
