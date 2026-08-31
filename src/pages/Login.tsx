import { FormEvent, useState } from 'react';
import type { Usuario } from '../types';
import { login as autenticar } from '../lib/api';
import AuthCard from '../components/AuthCard';

interface Props {
  onLogin: (usuario: Usuario) => void;
  versao?: string;
}

export default function Login({ onLogin, versao }: Props) {
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
    <AuthCard
      titulo="Ecoviva"
      subtitulo="Controle de entradas e saidas do armazem"
      versao={versao}
      erro={erro}
      enviando={enviando}
      textoBotao="Entrar"
      textoBotaoEnviando="Entrando..."
      onSubmit={handleSubmit}
    >
      <label>
        Usuario
        <input
          value={login}
          onChange={(e) => setLogin(e.target.value.trim())}
          autoComplete="username"
          required
          autoFocus
        />
      </label>

      <label>
        Senha
        <input
          type="password"
          value={senha}
          onChange={(e) => setSenha(e.target.value)}
          autoComplete="current-password"
          required
        />
      </label>
    </AuthCard>
  );
}
