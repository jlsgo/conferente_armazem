import type { Movimento } from '../types';
import { resumoMovimentos } from '../lib/situacao';

interface Props {
  lancamentos: Movimento[];
}

/**
 * Faixa discreta com o resumo do dia ate agora (antes do fechamento) - pro
 * conferente se autoconferir ("bati o numero certo de pedidos hoje?") sem
 * precisar abrir o painel do gestor ou esperar o fechamento pra ver algo alem
 * da lista crua. So contadores de texto, de proposito - graficos/analytics
 * mais ricos ja existem no painel (`painel/index.html`), aqui o foco continua
 * sendo o formulario de lancamento.
 */
export default function ResumoDoDia({ lancamentos }: Props) {
  if (lancamentos.length === 0) return null;

  const { totalLancamentos, totalUnidades, porSituacao, porCategoria } = resumoMovimentos(lancamentos);

  return (
    <div className="resumo-do-dia">
      <span>
        <strong>Hoje:</strong> {totalLancamentos} lancamento{totalLancamentos !== 1 ? 's' : ''} · {totalUnidades}{' '}
        unidades
      </span>
      <span>
        {Object.entries(porSituacao)
          .map(([s, n]) => `${s}: ${n}`)
          .join(' · ')}
      </span>
      <span>
        {Object.entries(porCategoria)
          .map(([c, n]) => `${n}x ${c}`)
          .join(' · ')}
      </span>
    </div>
  );
}
