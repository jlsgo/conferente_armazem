import { FormEvent, useEffect, useState } from 'react';
import type { Armazem, QuebraCadeia, Usuario } from '../types';
import { criarUsuario, listarUsuarios, verificarIntegridade } from '../lib/api';
import Carregando from '../components/Carregando';

interface Props {
  armazens: Armazem[];
}

export default function Usuarios({ armazens }: Props) {
  const [usuarios, setUsuarios] = useState<Usuario[]>([]);
  const [carregando, setCarregando] = useState(true);
  const [erroCarregamento, setErroCarregamento] = useState('');

  const [nome, setNome] = useState('');
  const [login, setLogin] = useState('');
  const [senha, setSenha] = useState('');
  const [armazemId, setArmazemId] = useState<number | null>(armazens[0]?.id ?? null);
  const [erro, setErro] = useState('');
  const [sucesso, setSucesso] = useState('');
  const [enviando, setEnviando] = useState(false);

  const [verificando, setVerificando] = useState(false);
  const [erroIntegridade, setErroIntegridade] = useState('');
  // undefined = ainda nao verificou nessa sessao da tela; null = verificou e
  // esta intacta; QuebraCadeia = achou uma linha divergente.
  const [quebra, setQuebra] = useState<QuebraCadeia | null | undefined>(undefined);

  async function carregar() {
    setCarregando(true);
    setErroCarregamento('');
    try {
      setUsuarios(await listarUsuarios());
    } catch (err) {
      setErroCarregamento(
        typeof err === 'string' ? err : 'Nao foi possivel carregar os usuarios cadastrados.'
      );
    } finally {
      setCarregando(false);
    }
  }

  useEffect(() => {
    carregar();
  }, []);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setErro('');
    setSucesso('');
    setEnviando(true);

    const resultado = await criarUsuario({
      nome,
      login,
      senha,
      armazem_id: armazemId,
      papel: 'conferente',
    });
    setEnviando(false);

    if (!resultado.ok) {
      setErro(resultado.error ?? 'Nao foi possivel cadastrar o usuario.');
      return;
    }

    setSucesso(`Usuario "${login}" cadastrado.`);
    setNome('');
    setLogin('');
    setSenha('');
    await carregar();
  }

  async function handleVerificarIntegridade() {
    setErroIntegridade('');
    setVerificando(true);
    const resultado = await verificarIntegridade();
    setVerificando(false);

    if (!resultado.ok) {
      setErroIntegridade(resultado.error ?? 'Nao foi possivel verificar a integridade.');
      return;
    }
    setQuebra(resultado.quebra ?? null);
  }

  return (
    <div>
      <section className="cartao">
        <h2>Nova conferente / usuario</h2>
        <form onSubmit={handleSubmit}>
          <div className="grade-formulario">
            <label>
              Nome completo
              <input value={nome} onChange={(e) => setNome(e.target.value)} required />
            </label>

            <label>
              Usuario de acesso
              {/* "off", nao "username": quem digita aqui e o gestor criando o
                  login de OUTRA pessoa, nao o proprio - nao queremos que o
                  navegador tente autopreencher com a conta do gestor. */}
              <input
                value={login}
                onChange={(e) => setLogin(e.target.value.trim())}
                autoComplete="off"
                required
              />
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
              Armazem
              <select value={armazemId ?? ''} onChange={(e) => setArmazemId(Number(e.target.value))}>
                {armazens.map((a) => (
                  <option key={a.id} value={a.id}>
                    {a.codigo} - {a.nome}
                  </option>
                ))}
              </select>
            </label>
          </div>

          <p className="subtitulo">
            Todo novo cadastro entra como conferente. So existe um usuario gestor no sistema.
          </p>

          {erro && <p className="erro" role="alert">{erro}</p>}
          {sucesso && <p className="sucesso">{sucesso}</p>}

          <button type="submit" disabled={enviando}>
            {enviando ? 'Cadastrando...' : 'Cadastrar usuario'}
          </button>
        </form>
      </section>

      <section className="cartao">
        <h2>Integridade da auditoria</h2>
        <p className="subtitulo">
          Confere se algum lancamento foi alterado direto no banco, fora do fluxo normal do
          app (a trilha de auditoria e uma cadeia de hash - ver <code>docs/ARQUITETURA.md</code>).
          Pode demorar alguns segundos num banco grande.
        </p>

        {erroIntegridade && <p className="erro" role="alert">{erroIntegridade}</p>}
        {quebra === null && (
          <p className="sucesso">Cadeia intacta - nenhuma alteracao fora do fluxo normal encontrada.</p>
        )}
        {quebra && (
          <p className="erro" role="alert">
            Quebra encontrada no lancamento nº {quebra.movimento_id}
            {quebra.numero_pedido ? ` (pedido ${quebra.numero_pedido})` : ''} - algum campo foi
            alterado direto no banco.
          </p>
        )}

        <button type="button" onClick={handleVerificarIntegridade} disabled={verificando}>
          {verificando ? 'Verificando...' : 'Verificar integridade'}
        </button>
      </section>

      <section className="cartao">
        <h2>Usuarios cadastrados</h2>
        {carregando ? (
          <Carregando />
        ) : erroCarregamento ? (
          <div>
            <p className="erro" role="alert">{erroCarregamento}</p>
            <button type="button" onClick={carregar}>
              Tentar novamente
            </button>
          </div>
        ) : (
          <table>
            <thead>
              <tr>
                <th>Nome</th>
                <th>Usuario</th>
                <th>Armazem</th>
                <th>Papel</th>
              </tr>
            </thead>
            <tbody>
              {usuarios.map((u) => (
                <tr key={u.id}>
                  <td>{u.nome}</td>
                  <td>{u.login}</td>
                  <td>{armazens.find((a) => a.id === u.armazem_id)?.codigo ?? '-'}</td>
                  <td>{u.papel}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
    </div>
  );
}
