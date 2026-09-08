# Docs historicos

Os tres arquivos desta pasta sao planos/diagnosticos escritos antes de Montagem, SAC,
sincronizacao A4/B2 e o fechamento unificado existirem — hoje totalmente implementados
(ver `docs/ARQUITETURA.md` e `docs/ROADMAP.md`, que e o registro atualizado do que ja
foi feito). Ficam aqui como referencia do raciocinio original, nao como fonte de verdade
do estado atual do sistema:

- `plano de melhorias.md` e `plano-melhorias-prompt-original.md` — diagnostico inicial
  de paridade com as planilhas de papel (quase identicos; o segundo tem a mais a spec
  original da impressao unificada do fechamento, ja implementada em
  `FechamentoImpressao.tsx`/`exportFechamento.ts`).
- `plano de melhorias futuras.md` — proposta original da sincronizacao com backend
  central e painel web, ja implementada via Turso (`db::sync`) e o painel estatico
  somente-leitura em `painel/` (ver a secao "Painel web somente-leitura" em
  `ARQUITETURA.md`).
