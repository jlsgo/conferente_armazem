import { useEffect, useState } from 'react';

export type Tema = 'claro' | 'escuro';

const CHAVE_ARMAZENAMENTO = 'ecoviva-tema';

function lerTemaSalvo(): Tema | null {
  try {
    const salvo = localStorage.getItem(CHAVE_ARMAZENAMENTO);
    return salvo === 'claro' || salvo === 'escuro' ? salvo : null;
  } catch {
    // Sem acesso a localStorage (raro, mas nao deve travar o app) - segue
    // sem lembrar a escolha entre uma sessao e outra.
    return null;
  }
}

/**
 * Tema claro/escuro do app inteiro. Comeca no que o Windows/SO do PC ja usa
 * (`prefers-color-scheme`) - a maioria dos PCs de armazem provavelmente esta
 * no padrao do SO - mas uma escolha manual do conferente sempre vence e fica
 * salva neste PC (`localStorage`, por maquina - nao sincroniza entre A4/B2).
 */
export function useTema() {
  const [tema, setTema] = useState<Tema>(() => {
    const salvo = lerTemaSalvo();
    if (salvo) return salvo;
    return window.matchMedia?.('(prefers-color-scheme: dark)').matches ? 'escuro' : 'claro';
  });

  useEffect(() => {
    document.documentElement.setAttribute('data-tema', tema);
  }, [tema]);

  function alternar() {
    setTema((atual) => {
      const novo = atual === 'claro' ? 'escuro' : 'claro';
      try {
        localStorage.setItem(CHAVE_ARMAZENAMENTO, novo);
      } catch {
        // Ignora - so afeta lembrar a escolha na proxima abertura.
      }
      return novo;
    });
  }

  return { tema, alternar };
}
