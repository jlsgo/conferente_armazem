import { FormEvent, useEffect, useRef, useState } from 'react';
import { cobrinhaListarRecordes, cobrinhaRegistrarRecorde } from '../lib/api';
import type { RecordeCobrinha } from '../types';

interface Props {
  onFechar: () => void;
  /** Codigo do armazem ('A4'/'B2') de quem esta logado - so passado pela
   * instancia pos-login (Dashboard.tsx). Presente = liga o placar unificado
   * (Turso, A4 x B2); ausente (instancia da tela de login, AuthCard.tsx) =
   * placar continua 100% local, sem nenhuma chamada ao backend. */
  armazemCodigo?: string;
}

interface Ponto {
  x: number;
  y: number;
}

interface RecordeLocal {
  nome: string;
  pontos: number;
}

const TAMANHO_GRADE = 16;
const VELOCIDADE_MS = 150;
const PONTUACAO_MINIMA_PARA_RECORDE = 13;
const MAX_RECORDES = 5;
const CHAVE_RECORDES = 'ecoviva-cobrinha-recordes';

// "Motivacao" ironica pra quem nao bateu recorde - so pra rir, sorteada a
// cada game over (ver `mensagemFim`). Se um dia isso incomodar alguem, e so
// apagar este array e o texto que o usa.
const MENSAGENS_SEM_RECORDE = [
  'Bateu na parede. A vida e assim mesmo.',
  'Reflita sobre suas escolhas enquanto a cobra reinicia.',
  'Culpa do teclado, com certeza.',
  'Grande jogada. Digna de um quadro la no fundo do galpao.',
  'Voce e oficialmente melhor que a cobrinha de ontem. Parabens, eu acho.',
  'Isso foi... uma pontuacao. Tecnicamente.',
  'A parede nao teve culpa dessa vez. Ou teve?',
  'Impressionante como sempre da pra piorar amanha.',
  'A cobrinha confiou em voce. Ela vai superar.',
];

function mensagemSemRecorde(): string {
  return MENSAGENS_SEM_RECORDE[Math.floor(Math.random() * MENSAGENS_SEM_RECORDE.length)];
}

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

// Fallback local (localStorage deste PC) - usado sempre na tela de login
// (sem `armazemCodigo`) e como rede de seguranca pos-login se o placar
// unificado nao estiver disponivel (sync nao configurado, sem internet).
function carregarRecordesLocais(): RecordeLocal[] {
  try {
    const bruto = window.localStorage.getItem(CHAVE_RECORDES);
    const lista = bruto ? JSON.parse(bruto) : [];
    return Array.isArray(lista) ? lista : [];
  } catch {
    return [];
  }
}

function salvarRecordesLocais(recordes: RecordeLocal[]) {
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
export default function CobrinhaSecreta({ onFechar, armazemCodigo }: Props) {
  const [cobra, setCobra] = useState<Ponto[]>([{ x: 8, y: 8 }]);
  const [comida, setComida] = useState<Ponto>(() => posicaoAleatoria());
  const [gameOver, setGameOver] = useState(false);
  const [recordesLocais, setRecordesLocais] = useState<RecordeLocal[]>(() => carregarRecordesLocais());
  // `null` = ainda nao carregou (ou falhou) - nesse caso a tela usa
  // `recordesLocais` como se nao houvesse placar unificado nenhum.
  const [recordesUnificados, setRecordesUnificados] = useState<RecordeCobrinha[] | null>(null);
  const [nomeInput, setNomeInput] = useState('');
  const [nomeSalvo, setNomeSalvo] = useState(false);
  const [mensagemFim, setMensagemFim] = useState('');
  const [recordeQuebradoDe, setRecordeQuebradoDe] = useState<string | null>(null);
  const [comemorarRecorde, setComemorarRecorde] = useState(false);
  const direcaoRef = useRef<Ponto>({ x: 1, y: 0 });
  const proximaDirecaoRef = useRef<Ponto>({ x: 1, y: 0 });
  // Melhor pontuacao desta sessao (aba aberta) - `null` = ainda nao jogou
  // nenhuma vez, pra nao mostrar "seu melhor da sessao" logo na primeira
  // partida (nao ha sessao anterior pra bater). Deliberadamente sem
  // persistencia/identidade (as iniciais de 4 letras nao identificam a
  // pessoa de forma confiavel entre partidas/maquinas) - só um placar
  // informal enquanto o jogo fica aberto.
  const melhorSessaoRef = useRef<number | null>(null);

  const pontos = cobra.length - 1;
  const elegivelParaRecorde = gameOver && !nomeSalvo && pontos > PONTUACAO_MINIMA_PARA_RECORDE;
  const unificadoDisponivel = !!armazemCodigo && recordesUnificados !== null;
  const recordesExibidos: { nome: string; pontos: number; armazemCodigo?: string }[] = unificadoDisponivel
    ? recordesUnificados!.map((r) => ({ nome: r.nome, pontos: r.pontos, armazemCodigo: r.armazem_codigo }))
    : recordesLocais;

  useEffect(() => {
    if (!armazemCodigo) return;
    let cancelado = false;
    cobrinhaListarRecordes()
      .then((lista) => {
        if (!cancelado) setRecordesUnificados(lista);
      })
      .catch(() => {
        // Sem sync/sem internet - a tela ja cai sozinha pro placar local
        // (recordesUnificados continua null).
      });
    return () => {
      cancelado = true;
    };
  }, [armazemCodigo]);

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
          const pontosFinal = atual.length - 1;
          const ehRecordeDaSessao =
            melhorSessaoRef.current !== null && pontosFinal > melhorSessaoRef.current;
          melhorSessaoRef.current = Math.max(melhorSessaoRef.current ?? 0, pontosFinal);
          setGameOver(true);
          setMensagemFim(ehRecordeDaSessao ? 'Seu melhor desta sessao! 🔥' : mensagemSemRecorde());
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

  async function registrarNome(e: FormEvent) {
    e.preventDefault();
    const nome = nomeInput.trim().toUpperCase().slice(0, 4) || 'ANON';
    const liderAnterior = recordesExibidos[0];
    setNomeSalvo(true);
    // "Bater o recorde" = virar o novo nº1, mesmo que seja a propria pessoa
    // superando a marca anterior dela - diferente de `recordeQuebradoDe`
    // (so faz sentido o texto "voce tirou X do topo" quando X e outra pessoa).
    setComemorarRecorde(!liderAnterior || pontos > liderAnterior.pontos);
    if (liderAnterior && pontos > liderAnterior.pontos && liderAnterior.nome !== nome) {
      setRecordeQuebradoDe(liderAnterior.nome);
    }

    if (armazemCodigo) {
      try {
        const novaLista = await cobrinhaRegistrarRecorde(pontos, nome);
        setRecordesUnificados(novaLista);
        return;
      } catch {
        // Sync nao configurado/sem internet nesta hora - nao perde a
        // pontuacao, so cai pro placar local desta maquina.
      }
    }
    const novaListaLocal = [...recordesLocais, { nome, pontos }]
      .sort((a, b) => b.pontos - a.pontos)
      .slice(0, MAX_RECORDES);
    setRecordesLocais(novaListaLocal);
    salvarRecordesLocais(novaListaLocal);
  }

  function reiniciar() {
    setCobra([{ x: 8, y: 8 }]);
    setComida(posicaoAleatoria());
    direcaoRef.current = { x: 1, y: 0 };
    proximaDirecaoRef.current = { x: 1, y: 0 };
    setGameOver(false);
    setNomeInput('');
    setNomeSalvo(false);
    setMensagemFim('');
    setRecordeQuebradoDe(null);
    setComemorarRecorde(false);
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
                {comemorarRecorde && (
                  <p className="cobrinha-parabens" aria-hidden="true">
                    🎉🏆🎉
                  </p>
                )}
                <p>Bateu! Pontos: {pontos}</p>
                {comemorarRecorde && <p>Novo recorde geral! Parabens! 🎉</p>}
                {recordeQuebradoDe && <p>Voce tirou {recordeQuebradoDe} do topo!</p>}
                {!comemorarRecorde && mensagemFim && <p className="subtitulo">{mensagemFim}</p>}
                <button type="button" onClick={reiniciar}>
                  Jogar de novo
                </button>
              </>
            )}
          </div>
        )}
        {recordesExibidos.length > 0 && (
          <div className="cobrinha-recordes">
            <p className="subtitulo">{unificadoDisponivel ? 'Recordes gerais (A4 x B2):' : 'Recordes desta maquina:'}</p>
            <ol>
              {recordesExibidos.map((r, i) => (
                <li key={i}>
                  {r.nome}
                  {r.armazemCodigo ? ` (${r.armazemCodigo})` : ''} - {r.pontos}
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
