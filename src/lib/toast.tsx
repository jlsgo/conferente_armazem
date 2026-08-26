import { createContext, ReactNode, useCallback, useContext, useRef, useState } from 'react';
import { IconAlerta, IconCheckCircle, IconInfoCircle, IconX } from '../components/Icon';

type TipoToast = 'erro' | 'sucesso' | 'info';

interface ToastItem {
  id: number;
  tipo: TipoToast;
  mensagem: string;
}

interface ToastContextValue {
  notificar: (mensagem: string, tipo?: TipoToast) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

const DURACAO_MS: Record<TipoToast, number> = {
  erro: 8000,
  sucesso: 4000,
  info: 5000,
};

const ICONE: Record<TipoToast, (props: { size?: number }) => JSX.Element> = {
  erro: IconAlerta,
  sucesso: IconCheckCircle,
  info: IconInfoCircle,
};

export function ToastProvider({ children }: { children: ReactNode }) {
  const [itens, setItens] = useState<ToastItem[]>([]);
  const proximoId = useRef(1);

  const remover = useCallback((id: number) => {
    setItens((atual) => atual.filter((t) => t.id !== id));
  }, []);

  const notificar = useCallback(
    (mensagem: string, tipo: TipoToast = 'info') => {
      const id = proximoId.current++;
      setItens((atual) => [...atual, { id, tipo, mensagem }]);
      setTimeout(() => remover(id), DURACAO_MS[tipo]);
    },
    [remover]
  );

  return (
    <ToastContext.Provider value={{ notificar }}>
      {children}
      <div className="toast-viewport somente-tela">
        {itens.map((item) => {
          const Icone = ICONE[item.tipo];
          return (
            <div key={item.id} className={`toast toast-${item.tipo}`} role="status">
              <Icone size={18} />
              <p>{item.mensagem}</p>
              <button
                type="button"
                className="toast-fechar"
                aria-label="Fechar aviso"
                onClick={() => remover(item.id)}
              >
                <IconX size={14} />
              </button>
            </div>
          );
        })}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast(): ToastContextValue {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error('useToast precisa estar dentro de um ToastProvider.');
  return ctx;
}
