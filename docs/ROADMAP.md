# Roadmap

Registro do que já está pronto e do que vem a seguir, para retomar o trabalho sem
precisar reconstruir o contexto. Atualize esta lista ao concluir/replanejar uma sprint —
não deixe ela ficar desatualizada.

## Feito

- Fundacao Tauri + Rust + SQLite (login, migrations, testes, CI, `docs/ARQUITETURA.md`).
- Fluxo **Saida do Armazem** (veiculos: scooter/triciclo/patinete) completo: lancamento
  de pedido com multiplos itens, lista do dia, total automatico.
- Repositorio no GitHub (`jlsgo/conferente_armazem`), CI verde em Linux e Windows.
- **Sprint 1**: fechamento do dia (trava os lancamentos e gera uma visao de impressao em
  A4 via `window.print()` do proprio app, sem depender de nenhuma lib de PDF — ver
  `docs/ARQUITETURA.md`); cadastro de mais usuarios (tela restrita a `papel = 'gestor'`,
  checado no backend); icone do instalador trocado pela marca Ecoviva; horario do
  lancamento nao reseta mais sozinho entre pedidos do mesmo lote.

## Pendente do Sprint 1 (ficou pra depois)

- **Correcao apos o fechamento**: hoje, depois de fechado, o dia fica travado por
  completo — nao existe ainda um "lancamento de ajuste/estorno" pra corrigir um erro
  descoberto depois do fechamento. Por enquanto a unica saida e um gestor reabrir o caso
  manualmente no banco. Vale planejar isso antes do fechamento do dia virar habito nas
  duas pontas (A4 e B2).

## Sprint 2 — Os outros dois fluxos

O schema (`fluxo IN ('peca_montagem', 'sac')`) e o backend generico ja suportam os dois;
falta so a tela:

- **Peca para Montagem (B2 → A4)**: entrada/saida de pecas soltas, com condicao
  (boa/defeito/sucata). Tela parecida com a de Lancamentos, mas para `categoria = 'peca'`
  e sem o campo `numero_pedido` em destaque.
- **SAC**: protocolo + garantia/venda + valor (so quando venda) + itens de peca.

## Sprint 3 — Distribuicao real

- Gerar e testar o instalador Windows de verdade (`.msi`/`.exe` via `cargo tauri build`
  no CI `windows-latest`, ou numa maquina Windows) — ate agora so validamos que o app
  compila e roda no Linux deste ambiente.
- Backup automatico local (copia diaria do arquivo `ecoviva-armazem.db` para outra
  pasta/pendrive) — nenhum backup existe hoje alem do proprio arquivo SQLite.
- Instalar nos PCs reais de A4 e B2 e acompanhar o primeiro uso das conferentes.

## Sprint 4 — Preparar para mais de um armazem "conversarem"

Depende de existir alguma janela de conectividade (ver conversa anterior — ainda nao
confirmado se ha internet em algum ponto do dia):

- Sincronizacao oportunista: quando o PC detectar internet, envia os lancamentos novos
  para um backup/consolidacao central.
- Usar `armazem_destino_id` / `transferencia_origem_id` (ja no schema, sem logica ainda)
  para o check-in de confirmacao entre B2 e A4 que o usuario descreveu: quem libera uma
  peca registra a saida, quem recebe do outro lado confirma a entrada, fechando o ciclo
  e evitando extravio no trajeto.
- Painel consolidado (visao dos dois armazens juntos) para gestao.

## Depois disso

- Relatorios e exportacao para Excel.
- Escalar o mesmo instalador para novos armazens, se a empresa abrir mais.

## Decisoes que ja foram tomadas (nao reabrir sem motivo novo)

- Sem controle de saldo de estoque — e um livro de movimentacao/auditoria, nao um
  sistema de estoque.
- Sem catalogo de produto fixo — categoria (lista curta) + descricao livre com
  autocomplete.
- Uma unica `Mutex<Connection>` no backend, nao um pool.
- App 100% local por PC; qualquer sincronizacao e oportunista, nunca uma dependencia
  para o uso diario.
- Sem biblioteca de geracao de PDF em Rust — o fechamento do dia e impresso direto do
  app via `window.print()` (CSS `@media print`, tamanho A4), evitando dependencias
  transitivas desnecessarias (uma lib como `genpdf`/`printpdf` traria ~30 crates extras,
  varios antigos).
