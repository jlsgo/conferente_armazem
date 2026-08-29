import type { Fechamento, Movimento, VarianteFechamento } from '../types';
import { agoraLocalTexto, formatarDataHora } from './data';
import { motivoSacTexto, situacaoInfo } from './situacao';

type Variante = VarianteFechamento;

/** Texto "2x categoria (descricao) - observacao [enviado: N]" por item,
 * unido por " + " - usado tanto no fechamento impresso quanto nos exports
 * CSV/XLSX, pra nao arriscar as duas versoes mostrarem itens diferentes pro
 * mesmo fechamento. */
export function itensTexto(m: Movimento): string {
  return m.itens
    .map((it) => {
      const base = `${it.quantidade}x ${it.categoria}${it.descricao ? ' (' + it.descricao + ')' : ''}${it.observacao ? ' - ' + it.observacao : ''}`;
      const divergente = it.quantidade_enviada != null && it.quantidade_enviada !== it.quantidade;
      return divergente ? `${base} [enviado: ${it.quantidade_enviada}]` : base;
    })
    .join(' + ');
}

export function qtdTotal(m: Movimento): number {
  return m.itens.reduce((s, it) => s + it.quantidade, 0);
}

/**
 * Colunas do fechamento diario, espelhando exatamente o que
 * `FechamentoImpressao.tsx` renderiza por variante - usado pra gerar CSV/XLSX
 * com o mesmo conteudo da versao impressa.
 */
export function colunasFechamento(variante: Variante): {
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
        m.contraparte || '-',
        itensTexto(m),
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
        itensTexto(m),
        String(qtdTotal(m)),
        m.itens.map((it) => it.condicao).filter(Boolean).join(', ') || '-',
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
      m.contraparte || '-',
      itensTexto(m),
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
