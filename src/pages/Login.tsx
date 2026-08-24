import { FormEvent, useState } from 'react';
import type { Usuario } from '../types';
import { login as autenticar } from '../lib/api';

interface Props {
  onLogin: (usuario: Usuario) => void;
}

export default function Login({ onLogin }: Props) {
  const [login, setLogin] = useState('');
  const [senha, setSenha] = useState('');
  const [erro, setErro] = useState('');
  const [enviando, setEnviando] = useState(false);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setErro('');
    setEnviando(true);
    const resultado = await autenticar({ login, senha });
    setEnviando(false);

    if (!resultado.ok || !resultado.usuario) {
      setErro(resultado.error ?? 'Nao foi possivel entrar.');
      return;
    }

    onLogin(resultado.usuario);
  }

  return (
    <div className="tela-centralizada">
      <form className="cartao" onSubmit={handleSubmit}>
        <h1>Ecoviva</h1>
        <p className="subtitulo">Controle de entradas e saidas do armazem</p>

        <label>
          Usuario
          <input
            value={login}
            onChange={(e) => setLogin(e.target.value.trim())}
            required
            autoFocus
          />
        </label>

        <label>
          Senha
          <input type="password" value={senha} onChange={(e) => setSenha(e.target.value)} required />
        </label>

        {erro && <p className="erro">{erro}</p>}

        <button type="submit" disabled={enviando}>
          {enviando ? 'Entrando...' : 'Entrar'}
        </button>
      </form>
    </div>
  );
}
