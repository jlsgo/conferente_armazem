import { useCallback, useRef, useState } from 'react';

/**
 * Contador de cliques com janela de tempo - usado pro easter egg da cobrinha
 * (clicar 5x na logo). Zera sozinho se o usuario parar de clicar por um
 * tempo, pra nao acumular cliques espacados de dias diferentes.
 */
export function useCliquesSecretos(cliquesNecessarios = 5, janelaMs = 1500) {
  const [ativo, setAtivo] = useState(false);
  const contagemRef = useRef(0);
  const timeoutRef = useRef<number | null>(null);

  const registrarClique = useCallback(() => {
    contagemRef.current += 1;
    if (timeoutRef.current !== null) window.clearTimeout(timeoutRef.current);
    timeoutRef.current = window.setTimeout(() => {
      contagemRef.current = 0;
    }, janelaMs);

    if (contagemRef.current >= cliquesNecessarios) {
      contagemRef.current = 0;
      setAtivo(true);
    }
  }, [cliquesNecessarios, janelaMs]);

  const fechar = useCallback(() => setAtivo(false), []);

  return { ativo, registrarClique, fechar };
}
