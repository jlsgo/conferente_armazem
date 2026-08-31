import { useState } from 'react';
import type { Armazem, Fechamento, Movimento, VarianteFechamento } from '../types';
import { motivoSacTexto, resumoMovimentos, situacaoInfo } from '../lib/situacao';
import { agoraLocalTexto, formatarData, formatarDataArquivo, formatarDataHora } from '../lib/data';
import { paraCsv, baixarCsv } from '../lib/csv';
import { baixarXlsx } from '../lib/xlsx';
import { colunasFechamento, itensTexto, qtdTotal, resultadoReparoTexto, rodapeAuditoria } from '../lib/exportFechamento';
import logoEcoviva from '../assets/ecoviva-logo.png';

type Variante = VarianteFechamento;

interface Props {
  armazem: Armazem | undefined;
  data: string;
  fechamento: Fechamento;
  lancamentos: Movimento[];
  variante?: Variante;
}

const TITULOS: Record<Variante, string> = {
  armazem: 'Controle de Saidas de Armazem',
  montagem: 'Controle de Pecas para Montagem',
  sac: 'Controle de Saidas do SAC',
  reparo_externo: 'Controle de Reparo Externo',
};

// Mesma cor usada no friso de cada aba do menu (ver global.css, "Cor por
// aba") - repetida aqui como friso no topo da folha impressa, pra identificar
// de relance de qual fluxo e o documento mesmo fora do sistema.
const CORES_VARIANTE: Record<Variante, string> = {
  armazem: 'var(--cor-lancamentos-escuro)',
  montagem: 'var(--cor-montagem-escuro)',
  sac: 'var(--cor-sac-escuro)',
  reparo_externo: 'var(--cor-reparo-escuro)',
};

// Largura (%) de cada coluna na impressao, na mesma ordem dos <th> abaixo -
// sem isso o layout automatico espreme Coleta/Itens pra caber Observacoes,
// forcando quebra de linha em quase toda linha e estourando pra 2-3 paginas.
const LARGURAS_COLUNAS: Record<Variante, number[]> = {
  armazem: [3, 6, 8, 13, 24, 4, 8, 18, 8, 8],
  montagem: [4, 7, 10, 38, 5, 10, 14, 12],
  sac: [3, 6, 9, 15, 26, 4, 13, 12, 12],
  reparo_externo: [4, 7, 13, 38, 5, 10, 13, 10],
};

export default function FechamentoImpressao({
  armazem,
  data,
  fechamento,
  lancamentos,
  variante = 'armazem',
}: Props) {
  const [exportandoXlsx, setExportandoXlsx] = useState(false);
  const nomesResponsaveis = Array.from(new Set(lancamentos.map((m) => m.usuario_nome)));
  const responsaveis = nomesResponsaveis.join(', ');
  // Quem assina fisicamente a folha: um bloco por conferente que lancou algo
  // no dia. Se por algum motivo nao houver lancamento (fechamento de um dia
  // vazio), cai pra quem fechou o dia.
  const assinantes = nomesResponsaveis.length > 0 ? nomesResponsaveis : [fechamento.usuario_nome];
  const totalGeral = lancamentos.reduce(
    (soma, m) => soma + (m.estornado_de ? -1 : 1) * qtdTotal(m),
    0
  );

  // Resumo pra preencher com informacao util o espaco que sobra em dias com
  // poucos lancamentos, em vez da folha terminar em branco logo apos a
  // tabela.
  const { porSituacao, porCategoria } = resumoMovimentos(lancamentos);

  function nomeBase(extensao: string): string {
    return `fechamento_${variante}_${armazem?.codigo ?? 'armazem'}_${formatarDataArquivo(data)}.${extensao}`;
  }

  function handleExportarCsv() {
    const { cabecalhos, linha } = colunasFechamento(variante);
    const linhas = lancamentos.map(linha);
    const rodape = rodapeAuditoria(fechamento);
    const linhasComRodape = [...linhas, [], ...rodape.map((r) => [r.rotulo, r.valor])];
    baixarCsv(nomeBase('csv'), paraCsv(cabecalhos, linhasComRodape));
  }

  async function handleExportarXlsx() {
    setExportandoXlsx(true);
    try {
      const { cabecalhos, linha } = colunasFechamento(variante);
      const linhas = lancamentos.map(linha);
      const rodape = rodapeAuditoria(fechamento);
      await baixarXlsx(nomeBase('xlsx'), [
        { nome: 'Fechamento', cabecalhos, linhas },
        { nome: 'Auditoria', cabecalhos: ['Item', 'Valor'], linhas: rodape.map((r) => [r.rotulo, r.valor]) },
      ]);
    } finally {
      setExportandoXlsx(false);
    }
  }

  return (
    <section className="cartao area-impressao" style={{ borderTop: `4px solid ${CORES_VARIANTE[variante]}` }}>
      <div className="cabecalho-impressao">
        <h2>
          <img src={logoEcoviva} alt="Ecoviva" className="logo-impressao" />
          {TITULOS[variante]}
        </h2>
        <div className="ficha-campos">
          <div>
            <span className="ficha-rotulo">Armazem</span>
            <span className="ficha-valor">{armazem ? `${armazem.nome} (${armazem.codigo})` : '-'}</span>
          </div>
          <div>
            <span className="ficha-rotulo">Data</span>
            <span className="ficha-valor">{formatarData(data)}</span>
          </div>
          <div>
            <span className="ficha-rotulo">Fechado em</span>
            <span className="ficha-valor">
              {formatarDataHora(fechamento.criado_em)} (por {fechamento.usuario_nome})
            </span>
          </div>
          <div>
            <span className="ficha-rotulo">Responsavel(is)</span>
            <span className="ficha-valor">{responsaveis || '-'}</span>
          </div>
        </div>
      </div>

      <div className="tabela-scroll">
      <table>
        <colgroup>
          {LARGURAS_COLUNAS[variante].map((largura, i) => (
            <col key={i} style={{ width: `${largura}%` }} />
          ))}
        </colgroup>
        <thead>
          <tr>
            <th>Nº</th>
            <th>Horario</th>
            {variante === 'armazem' && (
              <>
                <th>Pedido</th>
                <th>Coleta</th>
              </>
            )}
            {variante === 'montagem' && <th>Direcao</th>}
            {variante === 'sac' && (
              <>
                <th>Protocolo</th>
                <th>Coleta</th>
              </>
            )}
            {variante === 'reparo_externo' && <th>Tecnico/Oficina</th>}
            <th>Itens</th>
            <th>Qtd.</th>
            {variante === 'armazem' && <th>Quem retirou</th>}
            {variante === 'montagem' && <th>Condicao</th>}
            {variante === 'sac' && <th>Motivo</th>}
            {variante === 'reparo_externo' && <th>Resultado</th>}
            {variante === 'armazem' && <th>Observacoes</th>}
            <th>Registrado por</th>
            <th>Situacao</th>
          </tr>
        </thead>
        <tbody>
          {lancamentos.map((m) => (
            <tr key={m.id}>
              <td>{m.numero}</td>
              <td>{m.hora}</td>
              {variante === 'armazem' && (
                <>
                  <td>
                    {m.numero_pedido || '-'}
                    {!m.retirada_completa && ' (parcial)'}
                  </td>
                  <td>{m.contraparte || '-'}</td>
                </>
              )}
              {variante === 'montagem' && <td>{m.tipo === 'saida' ? 'Saida B2' : 'Entrada B2'}</td>}
              {variante === 'sac' && (
                <>
                  <td>{m.numero_pedido || '-'}</td>
                  <td>{m.contraparte || '-'}</td>
                </>
              )}
              {variante === 'reparo_externo' && <td>{m.contraparte || '-'}</td>}
              <td>{itensTexto(m)}</td>
              <td>{qtdTotal(m)}</td>
              {variante === 'armazem' && <td>{m.quem_retirou || '-'}</td>}
              {variante === 'montagem' && <td>{resultadoReparoTexto(m)}</td>}
              {variante === 'sac' && <td>{motivoSacTexto(m)}</td>}
              {variante === 'reparo_externo' && <td>{resultadoReparoTexto(m)}</td>}
              {variante === 'armazem' && <td>{m.observacoes || '-'}</td>}
              <td>{m.usuario_nome}</td>
              <td>
                <span className={situacaoInfo(m).classe}>{situacaoInfo(m).texto}</span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      </div>

      <p className="total-fechamento">
        <strong>{totalGeral}</strong> unidades no total ({lancamentos.length} pedidos)
      </p>
      {fechamento.total_estornado > 0 && (
        <p className="rodape-tabela">
          Ajuste por estorno: -{fechamento.total_estornado} unidades. Total liquido do dia:{' '}
          <strong>{fechamento.total_liquido}</strong>.
        </p>
      )}

      <div className="resumo-fechamento">
        <span>
          <strong>Por situacao:</strong>{' '}
          {Object.entries(porSituacao)
            .map(([s, n]) => `${s}: ${n}`)
            .join(' · ') || '-'}
        </span>
        <span>
          <strong>Por categoria:</strong>{' '}
          {Object.entries(porCategoria)
            .map(([c, n]) => `${n}x ${c}`)
            .join(' · ') || '-'}
        </span>
      </div>

      <div className="area-assinaturas">
        {assinantes.map((nome) => (
          <div className="assinatura" key={nome}>
            <div className="linha-assinatura" />
            <p>Assinatura - {nome}</p>
          </div>
        ))}
      </div>

      <div className="rodape-documento">
        <span>Ecoviva - Sistema de Controle de Armazens</span>
        <span>hash de auditoria: {fechamento.hash_integridade.slice(0, 16)}...</span>
        <span>
          Documento impresso em:{' '}
          {formatarDataHora(agoraLocalTexto())}
        </span>
      </div>

      <div className="somente-tela" style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
        <button onClick={() => window.print()}>Imprimir / Salvar como PDF</button>
        <button className="secundario" onClick={handleExportarCsv}>
          Exportar CSV
        </button>
        <button className="secundario" onClick={handleExportarXlsx} disabled={exportandoXlsx}>
          {exportandoXlsx ? 'Exportando...' : 'Exportar XLSX'}
        </button>
      </div>
      <p className="somente-tela rodape-tabela">
        Se a caixa de impressao nao mostrar automaticamente "Paisagem", selecione manualmente antes de
        imprimir.
      </p>
    </section>
  );
}
