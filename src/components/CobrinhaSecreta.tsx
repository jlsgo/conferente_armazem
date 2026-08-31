import { FormEvent, useEffect, useRef, useState } from 'react';

interface Props {
  onFechar: () => void;
}

interface Ponto {
  x: number;
  y: number;
}

interface Recorde {
  nome: string;
  pontos: number;
}

const TAMANHO_GRADE = 16;
const VELOCIDADE_MS = 150;
const PONTUACAO_MINIMA_PARA_RECORDE = 13;
const MAX_RECORDES = 5;
const CHAVE_RECORDES = 'ecoviva-cobrinha-recordes';

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

// Recordes ficam so no localStorage deste PC (nao sincroniza entre A4/B2) -
// e uma brincadeira local entre quem usa esta maquina, nao um dado real do
// negocio.
function carregarRecordes(): Recorde[] {
  try {
    const bruto = window.localStorage.getItem(CHAVE_RECORDES);
    const lista = bruto ? JSON.parse(bruto) : [];
    return Array.isArray(lista) ? lista : [];
  } catch {
    return [];
  }
}

function salvarRecordes(recordes: Recorde[]) {
  try {
    window.localStorage.setItem(CHAVE_RECORDES, JSON.stringify(recordes));
  } catch {
    // Storage bloqueado/aba privada - o jogo continua, so sem persistir.
  }
}

/**
 * Easter egg: cobrinha simples, sem estilo elaborado - so pra ser uma
 * pausa engracada, escondida atras de 5 cliques na logo (useCliquesSecretos).
 */
export default function CobrinhaSecreta({ onFechar }: Props) {
  const [cobra, setCobra] = useState<Ponto[]>([{ x: 8, y: 8 }]);
  const [comida, setComida] = useState<Ponto>(() => posicaoAleatoria());
  const [gameOver, setGameOver] = useState(false);
  const [recordes, setRecordes] = useState<Recorde[]>(() => carregarRecordes());
  const [nomeInput, setNomeInput] = useState('');
  const [nomeSalvo, setNomeSalvo] = useState(false);
  const [recordeQuebradoDe, setRecordeQuebradoDe] = useState<string | null>(null);
  const direcaoRef = useRef<Ponto>({ x: 1, y: 0 });
  const proximaDirecaoRef = useRef<Ponto>({ x: 1, y: 0 });

  const pontos = cobra.length - 1;
  const elegivelParaRecorde = gameOver && !nomeSalvo && pontos > PONTUACAO_MINIMA_PARA_RECORDE;

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

  function registrarNome(e: FormEvent) {
    e.preventDefault();
    const nome = nomeInput.trim().toUpperCase().slice(0, 4) || 'ANON';
    const liderAnterior = recordes[0];
    const novaLista = [...recordes, { nome, pontos }].sort((a, b) => b.pontos - a.pontos).slice(0, MAX_RECORDES);
    setRecordes(novaLista);
    salvarRecordes(novaLista);
    setNomeSalvo(true);
    if (liderAnterior && pontos > liderAnterior.pontos && liderAnterior.nome !== nome) {
      setRecordeQuebradoDe(liderAnterior.nome);
    }
  }

  function reiniciar() {
    setCobra([{ x: 8, y: 8 }]);
    setComida(posicaoAleatoria());
    direcaoRef.current = { x: 1, y: 0 };
    proximaDirecaoRef.current = { x: 1, y: 0 };
    setGameOver(false);
    setNomeInput('');
    setNomeSalvo(false);
    setRecordeQuebradoDe(null);
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
        <p className="subtitulo">🐍 Modo secreto! Use as setas. Pontos: {pontos}</p>
        <div className="cobrinha-grade" style={{ gridTemplateColumns: `repeat(${TAMANHO_GRADE}, 1fr)` }}>
          {celulas}
        </div>
        {gameOver && (
          <div className="cobrinha-fim">
            {elegivelParaRecorde ? (
              <form onSubmit={registrarNome} className="cobrinha-form-recorde">
                <p>Novo recorde! Pontos: {pontos} - suas iniciais:</p>
                <input
                  value={nomeInput}
                  onChange={(e) => setNomeInput(e.target.value.toUpperCase().slice(0, 4))}
                  maxLength={4}
                  autoFocus
                  placeholder="ABCD"
                />
                <button type="submit">Salvar</button>
              </form>
            ) : (
              <>
                <p>Bateu! Pontos: {pontos}</p>
                {recordeQuebradoDe && <p>Voce tirou {recordeQuebradoDe} do topo!</p>}
                <button type="button" onClick={reiniciar}>
                  Jogar de novo
                </button>
              </>
            )}
          </div>
        )}
        {recordes.length > 0 && (
          <div className="cobrinha-recordes">
            <p className="subtitulo">Recordes desta maquina:</p>
            <ol>
              {recordes.map((r, i) => (
                <li key={i}>
                  {r.nome} - {r.pontos}
                </li>
              ))}
            </ol>
          </div>
        )}
        <p className="subtitulo">Esc ou clique fora pra fechar</p>
      </div>
    </div>
  );
}
