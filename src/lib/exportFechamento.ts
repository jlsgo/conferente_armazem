import type { Armazem, Fechamento, Movimento, VarianteFechamento } from '../types';
import { agoraLocalTexto, formatarDataHora } from './data';
import { colunaColeta, itensResumoTexto, motivoSacTexto, situacaoInfo } from './situacao';

type Variante = VarianteFechamento;

export function qtdTotal(m: Movimento): number {
  return m.itens.reduce((s, it) => s + it.quantidade, 0);
}

/** Resultado do conserto (fluxo `reparo_externo`) - so preenchido na
 * entrada, vazio na saida. Compartilhado entre `FechamentoImpressao.tsx` e
 * `Historico.tsx` pra nao arriscar as duas telas mostrarem coisas
 * diferentes pro mesmo movimento. */
export function resultadoReparoTexto(m: Movimento): string {
  return m.itens.map((it) => it.condicao).filter(Boolean).join(', ') || '-';
}

/**
 * Colunas do fechamento diario, espelhando exatamente o que
 * `FechamentoImpressao.tsx` renderiza por variante - usado pra gerar CSV/XLSX
 * com o mesmo conteudo da versao impressa.
 */
export function colunasFechamento(
  variante: Variante,
  armazens: Armazem[],
  todos: Movimento[]
): {
  cabecalhos: string[];
  linha: (m: Movimento) => string[];
} {
  if (variante === 'armazem') {
    return {
      cabecalhos: [
        'Nº',
        'Horario',
        'Pedido',
        'Coleta',
        'Itens',
        'Qtd.',
        'Quem retirou',
        'Observacoes',
        'Registrado por',
        'Situacao',
      ],
      linha: (m) => [
        String(m.numero),
        m.hora,
        (m.numero_pedido || '-') + (!m.retirada_completa ? ' (parcial)' : ''),
        colunaColeta(m, armazens),
        itensResumoTexto(m, todos),
        String(qtdTotal(m)),
        m.quem_retirou || '-',
        m.observacoes || '-',
        m.usuario_nome,
        situacaoInfo(m).texto,
      ],
    };
  }

  if (variante === 'montagem') {
    return {
      cabecalhos: ['Nº', 'Horario', 'Direcao', 'Itens', 'Qtd.', 'Condicao', 'Registrado por', 'Situacao'],
      linha: (m) => [
        String(m.numero),
        m.hora,
        m.tipo === 'saida' ? 'Saida B2' : 'Entrada B2',
        itensResumoTexto(m, todos),
        String(qtdTotal(m)),
        resultadoReparoTexto(m),
        m.usuario_nome,
        situacaoInfo(m).texto,
      ],
    };
  }

  if (variante === 'reparo_externo') {
    return {
      cabecalhos: ['Nº', 'Horario', 'Tecnico/Oficina', 'Itens', 'Qtd.', 'Resultado', 'Registrado por', 'Situacao'],
      linha: (m) => [
        String(m.numero),
        m.hora,
        m.contraparte || '-',
        itensResumoTexto(m, todos),
        String(qtdTotal(m)),
        resultadoReparoTexto(m),
        m.usuario_nome,
        situacaoInfo(m).texto,
      ],
    };
  }

  return {
    cabecalhos: ['Nº', 'Horario', 'Protocolo', 'Coleta', 'Itens', 'Qtd.', 'Motivo', 'Registrado por', 'Situacao'],
    linha: (m) => [
      String(m.numero),
      m.hora,
      m.numero_pedido || '-',
      colunaColeta(m, armazens),
      itensResumoTexto(m, todos),
      String(qtdTotal(m)),
      motivoSacTexto(m),
      m.usuario_nome,
      situacaoInfo(m).texto,
    ],
  };
}

/**
 * Rodape de auditoria incluido em todo export (CSV/XLSX) do fechamento -
 * texto deliberadamente neutro ("referencia"), sem prometer prova
 * criptografica inviolavel: diferente do PDF impresso, CSV/XLSX sao
 * editaveis por natureza (e o proprio motivo desse export existir). Quem
 * garante integridade de verdade e a cadeia de hash no backend.
 */
export function rodapeAuditoria(fechamento: Fechamento): { rotulo: string; valor: string }[] {
  return [
    { rotulo: 'Sistema', valor: 'Ecoviva - Sistema de Controle de Armazens' },
    { rotulo: 'Hash de auditoria (referencia)', valor: fechamento.hash_integridade.slice(0, 16) + '...' },
    { rotulo: 'Fechado em', valor: formatarDataHora(fechamento.criado_em) },
    { rotulo: 'Exportado em', valor: formatarDataHora(agoraLocalTexto()) },
  ];
}
