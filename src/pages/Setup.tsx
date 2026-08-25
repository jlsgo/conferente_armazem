import { FormEvent, useState } from 'react';
import type { Armazem } from '../types';
import { setupPrimeiroUsuario } from '../lib/api';
import logoEcoviva from '../assets/ecoviva-logo.png';

interface Props {
  armazens: Armazem[];
  onConcluido: () => void;
}

export default function Setup({ armazens, onConcluido }: Props) {
  const [nome, setNome] = useState('');
  const [login, setLogin] = useState('');
  const [senha, setSenha] = useState('');
  const [confirmarSenha, setConfirmarSenha] = useState('');
  const [armazemId, setArmazemId] = useState<number | null>(armazens[0]?.id ?? null);
  const [erro, setErro] = useState('');
  const [enviando, setEnviando] = useState(false);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setErro('');

    if (senha !== confirmarSenha) {
      setErro('As senhas nao conferem.');
      return;
    }

    setEnviando(true);
    const resultado = await setupPrimeiroUsuario({
      nome,
      login,
      senha,
      armazem_id: armazemId,
    });
    setEnviando(false);

    if (!resultado.ok) {
      setErro(resultado.error ?? 'Nao foi possivel concluir o cadastro.');
      return;
    }

    onConcluido();
  }

  return (
    <div className="tela-centralizada">
      <form className="cartao" onSubmit={handleSubmit}>
        <img src={logoEcoviva} alt="Ecoviva" className="logo-ecoviva" />
        <h1>Configuracao inicial</h1>
        <p className="subtitulo">
          Este computador ainda nao tem nenhum usuario cadastrado. Crie a primeira conta
          (gestor) para comecar a usar o sistema.
        </p>

        <label>
          Nome completo
          <input value={nome} onChange={(e) => setNome(e.target.value)} required autoFocus />
        </label>

        <label>
          Usuario de acesso
          <input value={login} onChange={(e) => setLogin(e.target.value.trim())} required />
        </label>

        <label>
          Armazem deste computador
          <select
            value={armazemId ?? ''}
            onChange={(e) => setArmazemId(Number(e.target.value))}
          >
            {armazens.map((a) => (
              <option key={a.id} value={a.id}>
                {a.codigo} - {a.nome}
              </option>
            ))}
          </select>
        </label>

        <label>
          Senha
          <input
            type="password"
            value={senha}
            onChange={(e) => setSenha(e.target.value)}
            minLength={6}
            required
          />
        </label>

        <label>
          Confirmar senha
          <input
            type="password"
            value={confirmarSenha}
            onChange={(e) => setConfirmarSenha(e.target.value)}
            minLength={6}
            required
          />
        </label>

        {erro && <p className="erro">{erro}</p>}

        <button type="submit" disabled={enviando}>
          {enviando ? 'Criando...' : 'Criar conta e continuar'}
        </button>
      </form>
    </div>
  );
}
