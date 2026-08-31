import { FormEvent, useState } from 'react';
import type { Armazem } from '../types';
import { setupPrimeiroUsuario } from '../lib/api';
import AuthCard from '../components/AuthCard';

interface Props {
  armazens: Armazem[];
  onConcluido: () => void;
  versao?: string;
}

export default function Setup({ armazens, onConcluido, versao }: Props) {
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
    <AuthCard
      titulo="Configuracao inicial"
      subtitulo="Este computador ainda nao tem nenhum usuario cadastrado. Crie a primeira conta (gestor) para comecar a usar o sistema."
      versao={versao}
      erro={erro}
      enviando={enviando}
      textoBotao="Criar conta e continuar"
      textoBotaoEnviando="Criando..."
      onSubmit={handleSubmit}
    >
      <label>
        Nome completo
        <input value={nome} onChange={(e) => setNome(e.target.value)} required autoFocus />
      </label>

      <label>
        Usuario de acesso
        <input
          value={login}
          onChange={(e) => setLogin(e.target.value.trim())}
          autoComplete="username"
          required
        />
      </label>

      <label>
        Armazem deste computador
        <select value={armazemId ?? ''} onChange={(e) => setArmazemId(Number(e.target.value))}>
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
          autoComplete="new-password"
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
          autoComplete="new-password"
          minLength={6}
          required
        />
      </label>
    </AuthCard>
  );
}
