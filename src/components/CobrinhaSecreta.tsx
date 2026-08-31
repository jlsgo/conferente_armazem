import { useEffect, useRef, useState } from 'react';

interface Props {
  onFechar: () => void;
}

interface Ponto {
  x: number;
  y: number;
}

const TAMANHO_GRADE = 16;
const VELOCIDADE_MS = 150;

function posicaoAleatoria(): Ponto {
  return {
    x: Math.floor(Math.random() * TAMANHO_GRADE),
    y: Math.floor(Math.random() * TAMANHO_GRADE),
  };
}

const DIRECOES: Record<string, Ponto> = {
  ArrowUp: { x: 0, y: -1 },
  ArrowDown: { x: 0, y: 1 },
  ArrowLeft: { x: -1, y: 0 },
  ArrowRight: { x: 1, y: 0 },
};

/**
 * Easter egg: cobrinha simples, sem estilo elaborado - so pra ser uma
 * pausa engracada, escondida atras de 5 cliques na logo (useCliquesSecretos).
 * Sem persistencia de recorde de proposito, e so diversao passageira.
 */
export default function CobrinhaSecreta({ onFechar }: Props) {
  const [cobra, setCobra] = useState<Ponto[]>([{ x: 8, y: 8 }]);
  const [comida, setComida] = useState<Ponto>(() => posicaoAleatoria());
  const [gameOver, setGameOver] = useState(false);
  const direcaoRef = useRef<Ponto>({ x: 1, y: 0 });
  const proximaDirecaoRef = useRef<Ponto>({ x: 1, y: 0 });

  useEffect(() => {
    function aoTeclar(e: KeyboardEvent) {
      if (e.key === 'Escape') {
        onFechar();
        return;
      }
      const nova = DIRECOES[e.key];
      if (!nova) return;
      e.preventDefault();
      const atual = direcaoRef.current;
      if (atual.x + nova.x === 0 && atual.y + nova.y === 0) return;
      proximaDirecaoRef.current = nova;
    }
    window.addEventListener('keydown', aoTeclar);
    return () => window.removeEventListener('keydown', aoTeclar);
  }, [onFechar]);

  useEffect(() => {
    if (gameOver) return;
    const intervalo = window.setInterval(() => {
      direcaoRef.current = proximaDirecaoRef.current;
      setCobra((atual) => {
        const cabeca = atual[0];
        const novaCabeca = {
          x: cabeca.x + direcaoRef.current.x,
          y: cabeca.y + direcaoRef.current.y,
        };

        const bateuParede =
          novaCabeca.x < 0 || novaCabeca.x >= TAMANHO_GRADE || novaCabeca.y < 0 || novaCabeca.y >= TAMANHO_GRADE;
        const bateuNoProprioCorpo = atual.some((p) => p.x === novaCabeca.x && p.y === novaCabeca.y);
        if (bateuParede || bateuNoProprioCorpo) {
          setGameOver(true);
          return atual;
        }

        const comeu = novaCabeca.x === comida.x && novaCabeca.y === comida.y;
        const novoCorpo = [novaCabeca, ...atual];
        if (comeu) {
          setComida(posicaoAleatoria());
        } else {
          novoCorpo.pop();
        }
        return novoCorpo;
      });
    }, VELOCIDADE_MS);
    return () => window.clearInterval(intervalo);
  }, [comida, gameOver]);

  function reiniciar() {
    setCobra([{ x: 8, y: 8 }]);
    setComida(posicaoAleatoria());
    direcaoRef.current = { x: 1, y: 0 };
    proximaDirecaoRef.current = { x: 1, y: 0 };
    setGameOver(false);
  }

  const celulas: JSX.Element[] = [];
  for (let y = 0; y < TAMANHO_GRADE; y++) {
    for (let x = 0; x < TAMANHO_GRADE; x++) {
      const ehCabeca = cobra[0].x === x && cobra[0].y === y;
      const ehCorpo = !ehCabeca && cobra.some((p) => p.x === x && p.y === y);
      const ehComida = comida.x === x && comida.y === y;
      celulas.push(
        <div
          key={`${x}-${y}`}
          className={
            ehCabeca
              ? 'cobrinha-celula cobrinha-cabeca'
              : ehCorpo
                ? 'cobrinha-celula cobrinha-corpo'
                : ehComida
                  ? 'cobrinha-celula cobrinha-comida'
                  : 'cobrinha-celula'
          }
        />,
      );
    }
  }

  return (
    <div className="cobrinha-overlay" onClick={onFechar}>
      <div className="cobrinha-cartao" onClick={(e) => e.stopPropagation()}>
        <p className="subtitulo">🐍 Modo secreto! Use as setas. Pontos: {cobra.length - 1}</p>
        <div className="cobrinha-grade" style={{ gridTemplateColumns: `repeat(${TAMANHO_GRADE}, 1fr)` }}>
          {celulas}
        </div>
        {gameOver && (
          <div className="cobrinha-fim">
            <p>Bateu! Pontos: {cobra.length - 1}</p>
            <button type="button" onClick={reiniciar}>
              Jogar de novo
            </button>
          </div>
        )}
        <p className="subtitulo">Esc ou clique fora pra fechar</p>
      </div>
    </div>
  );
}
