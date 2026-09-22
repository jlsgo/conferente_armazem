import { createContext, FormEvent, ReactNode, useCallback, useContext, useEffect, useState } from 'react';

interface OpcoesDialogoBase {
  titulo?: string;
  textoConfirmar?: string;
  textoCancelar?: string;
  /** Usa o botao vermelho (mesma classe de "estornar"/"fechar o dia") pra acoes dificeis de desfazer. */
  perigo?: boolean;
}

interface OpcoesConfirmar extends OpcoesDialogoBase {}

interface OpcoesPerguntar extends OpcoesDialogoBase {
  placeholder?: string;
}

type EstadoDialogo =
  | { tipo: 'confirmar'; mensagem: string; opcoes: OpcoesConfirmar; resolver: (valor: boolean) => void }
  | { tipo: 'perguntar'; mensagem: string; opcoes: OpcoesPerguntar; resolver: (valor: string | null) => void }
  | null;

interface DialogoContextValue {
  confirmar: (mensagem: string, opcoes?: OpcoesConfirmar) => Promise<boolean>;
  perguntar: (mensagem: string, opcoes?: OpcoesPerguntar) => Promise<string | null>;
}

const DialogoContext = createContext<DialogoContextValue | null>(null);

/**
 * Substitui window.confirm/window.prompt: o dialogo nativo do SO ignora
 * completamente os tokens de cor do app (fica uma caixa branca solta mesmo
 * no tema escuro) e nao tem como estilizar. Usado nas acoes mais sensiveis
 * do sistema (estornar, fechar o dia), entao mantem a mesma semantica de
 * retorno (false/null em caso de cancelamento) pra nao mudar o comportamento
 * dos call sites, so a apresentacao.
 */
export function DialogoProvider({ children }: { children: ReactNode }) {
  const [estado, setEstado] = useState<EstadoDialogo>(null);

  const confirmar = useCallback((mensagem: string, opcoes: OpcoesConfirmar = {}) => {
    return new Promise<boolean>((resolver) => {
      setEstado({ tipo: 'confirmar', mensagem, opcoes, resolver });
    });
  }, []);

  const perguntar = useCallback((mensagem: string, opcoes: OpcoesPerguntar = {}) => {
    return new Promise<string | null>((resolver) => {
      setEstado({ tipo: 'perguntar', mensagem, opcoes, resolver });
    });
  }, []);

  function fechar(valor: boolean | string | null) {
    setEstado((atual) => {
      if (!atual) return null;
      if (atual.tipo === 'confirmar') atual.resolver(valor as boolean);
      else atual.resolver(valor as string | null);
      return null;
    });
  }

  return (
    <DialogoContext.Provider value={{ confirmar, perguntar }}>
      {children}
      {estado && <CaixaDialogo estado={estado} onFechar={fechar} />}
    </DialogoContext.Provider>
  );
}

export function useDialogo(): DialogoContextValue {
  const ctx = useContext(DialogoContext);
  if (!ctx) throw new Error('useDialogo precisa estar dentro de um DialogoProvider.');
  return ctx;
}

function CaixaDialogo({
  estado,
  onFechar,
}: {
  estado: NonNullable<EstadoDialogo>;
  onFechar: (valor: boolean | string | null) => void;
}) {
  const [valor, setValor] = useState('');

  function cancelar() {
    onFechar(estado.tipo === 'confirmar' ? false : null);
  }

  useEffect(() => {
    function aoTeclar(e: KeyboardEvent) {
      if (e.key === 'Escape') cancelar();
    }
    window.addEventListener('keydown', aoTeclar);
    return () => window.removeEventListener('keydown', aoTeclar);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function handleSubmit(e: FormEvent) {
    e.preventDefault();
    onFechar(estado.tipo === 'confirmar' ? true : valor);
  }

  return (
    <div
      className="dialogo-fundo"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) cancelar();
      }}
    >
      <form className="dialogo-cartao" role="alertdialog" aria-modal="true" onSubmit={handleSubmit}>
        {estado.opcoes.titulo && <h3>{estado.opcoes.titulo}</h3>}
        <p>{estado.mensagem}</p>
        {estado.tipo === 'perguntar' && (
          // eslint-disable-next-line jsx-a11y/no-autofocus
          <textarea
            autoFocus
            rows={3}
            value={valor}
            onChange={(e) => setValor(e.target.value)}
            placeholder={estado.opcoes.placeholder}
          />
        )}
        <div className="dialogo-acoes">
          <button type="button" className="secundario" onClick={cancelar}>
            {estado.opcoes.textoCancelar ?? 'Cancelar'}
          </button>
          <button
            type="submit"
            className={estado.opcoes.perigo ? 'perigo' : undefined}
            // eslint-disable-next-line jsx-a11y/no-autofocus
            autoFocus={estado.tipo === 'confirmar'}
          >
            {estado.opcoes.textoConfirmar ?? 'Confirmar'}
          </button>
        </div>
      </form>
    </div>
  );
}
