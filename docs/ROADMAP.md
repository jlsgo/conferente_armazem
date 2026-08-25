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
- **Sprint 2**: sessao real no backend (`AppState.sessao`) — `login`/`logout` passam a
  controlar quem esta autenticado; `criar_movimento`, `fechar_dia` e `criar_usuario`
  nao aceitam mais `usuario_id`/`solicitante_id` vindo do payload, usam a sessao.
  Validacao de turno/montagem/condicao, limite de texto (500 caracteres) e quantidade
  (100.000), e checagem de usuario/armazem ativos e correspondentes em toda escrita.
  Hash de auditoria (`calcular_hash`) agora cobre todos os campos do movimento (nao so
  uma fracao) e ha uma rotina `verificar_cadeia` que detecta alteracao direta no banco
  (com teste). Estorno append-only (`estornar_movimento`, usa a coluna `estornado_de`
  que ja existia no schema) — corrige um lancamento mesmo com o dia fechado, exige
  gestor + justificativa, e o fechamento mostra `total_estornado`/`total_liquido`
  calculados na hora (sem editar o registro do fechamento). UI: botao "Estornar" para
  gestores em `Lancamentos.tsx`, botao "Fechar o dia" some para conferentes.
- **Sprint 3**: telas de `Montagem.tsx` (fluxo `peca_montagem`: entrada/saida de peca
  solta no galpao B2, condicao boa/defeito/sucata obrigatoria por item, sem
  `numero_pedido`) e `Sac.tsx` (fluxo `sac`: protocolo, coleta, garantia/venda com
  valor obrigatorio so quando venda, `tipo` fixo em `entrada` sem toggle pra reduzir
  campo pro conferente preencher). Validacao das duas regras no backend
  (`domain::movimentos::validar_novo_movimento`), nao so na tela. `Movimento` (Rust e
  TS) passou a expor `motivo`/`valor_centavos`, que faltavam pra tela do SAC.
  `FechamentoImpressao` ganhou uma prop `variante` (`armazem`/`montagem`/`sac`) pra
  imprimir cada fluxo com as colunas certas. Aba de navegacao no `Dashboard.tsx` agora
  aparece pra qualquer usuario logado (antes so gestor via nav).
- **Sprint 4 (parte executavel)**: `Lancamentos.tsx` ganhou os campos que ja existiam
  no banco mas nao tinham UI — `codigo_rastreio`, `observacoes` do movimento e
  `observacao` por item — refletidos na tabela do dia e na impressao do fechamento
  (que virou paisagem, retrato nao cabia mais colunas). Backup automatico
  (`db::backup::backup_automatico`, feature `backup` do `rusqlite` via Online Backup
  API) roda a cada abertura do app, um arquivo por dia em `backups/`, retencao de 14
  dias — procedimento de restauracao documentado em `docs/ARQUITETURA.md`. Workflow
  `.github/workflows/build-installer.yml` (novo, separado do `ci.yml`) gera o
  instalador Windows (`.msi`/`.exe`) sob demanda via `workflow_dispatch` ou tag `v*`.
- **Polimento visual + aba de Historico**: logo da Ecoviva no login/setup/cabecalho
  (`src/assets/ecoviva-logo.png`, mesmo icone do instalador); hover/focus em
  botoes e campos, badges coloridos de situacao (`src/lib/situacao.ts`, compartilhado
  entre as 4 telas), tabelas com zebra striping e scroll horizontal em vez de
  quebrar o layout. Nova aba **Historico** (`src/pages/Historico.tsx`) — busca
  lancamentos de qualquer dia (nao so hoje) por periodo, cliente/coleta e numero de
  pedido, com o mesmo botao de estornar das outras telas; backend em
  `domain::movimentos::buscar_historico` (SQL com filtros opcionais via `?N IS NULL
  OR ...`, limite de 500 linhas, sem paginacao ainda). Dados de teste pra inspecionar
  tudo isso: `src-tauri/examples/seed_dev_data.rs` (`cargo run --example
  seed_dev_data`, idempotente pra usuarios, so avisa e pula lancamento que colidir
  com dia ja fechado).

- **Testes de seguranca e erros**: 12 testes novos (54 -> 66) fechando lacunas que a
  suite anterior nao cobria: estorno por gestor de outro armazem (isolamento entre
  A4/B2), estorno por usuario desativado depois de ja ter sido cadastrado, estorno de
  lancamento inexistente, `buscar_movimento`/quantidade negativa/armazem destino
  inativo, e no `auth.rs` - login de usuario desativado (mesma mensagem generica de um
  login inexistente, sem vazar que a conta existe), login com senha vazia, cadastro de
  usuario por um `solicitante_id` que nao existe, e confirmacao de que a senha nunca e
  gravada em texto puro e usa salt diferente por conta (`domain/auth.rs`). Novo teste em
  `domain/errors.rs` trava que `AppError::Database` nunca vaze o texto interno de um
  erro de SQL (nome de coluna/tabela) para a mensagem mostrada na tela.

- **Exportacao CSV na aba Historico**: botao "Exportar CSV" ao lado de "Buscar" em
  `Historico.tsx`, baixa os resultados filtrados nas mesmas colunas ja mostradas na
  tabela (varia por fluxo). 100% client-side (`src/lib/csv.ts`), sem lib nova e sem
  precisar de capability `fs` do Tauri — usa Blob + `<a download>`, mesma familia de
  solucao do `window.print()` do fechamento. BOM UTF-8 e `;` como separador para abrir
  certo no Excel em Windows/PT-BR (senao acento quebra e numero espalha em colunas
  erradas).

- **Endurecimento pra producao (leitura protegida, backup externo, sync v1)**: apos
  avaliar "esta pronto pra producao?", 3 frentes fechadas com o usuario e implementadas:
  (1) `listar_movimentos_do_dia`, `buscar_historico`, `buscar_fechamento_do_dia`,
  `sugestoes_descricao` e `listar_usuarios` agora exigem sessao e (as tres primeiras)
  conferem que o `armazem_id` pedido bate com o do usuario logado
  (`domain::movimentos::autorizar_leitura`, `domain::auth::listar_usuarios_como_gestor`)
  — antes so os comandos de escrita tinham essa checagem, um comando Tauri chamado
  direto vazava dado de outro armazem. (2) Backup externo pra pendrive/HD
  (`backup_externo.txt` na pasta de dados, `db::backup::backup_externo`, mesma retencao
  de 14 dias do backup local) com restauracao testada de ponta a ponta
  (`db::backup::verificar_backup_valido` + teste de integracao que faz backup, apaga o
  banco original, restaura, e confere dados **e** cadeia de hash intactos). (3)
  Sincronizacao oportunista v1 com o Turso/libSQL (envio unidirecional, ver
  `docs/ARQUITETURA.md`) — `db::sync`, comando `sincronizar_agora` (botao no
  `Dashboard.tsx`, gestor-only) e tentativa automatica em segundo plano na abertura do
  app; a logica local e testada, o passo de rede exige credenciais Turso reais do
  usuario pra validar (documentado). 85 testes Rust (66 -> 85).

## Sprint 4 (resto) — Distribuicao real

Fora do que da pra fazer neste ambiente (sem Windows real, sem acesso aos PCs de
A4/B2):

- Baixar e testar de verdade o instalador gerado pelo `build-installer.yml` numa
  maquina Windows.
- Instalar nos PCs reais de A4 e B2 e acompanhar o primeiro uso das conferentes (piloto
  em paralelo com a planilha, conforme a "Decisao atual" do plano de melhorias).

## Sprint 5 — Mais de um armazem "conversarem" (resto)

Confirmado que ha internet real (mesmo que intermitente) nos dois PCs, e a fundacao de
sync (envio unidirecional pro Turso) ja esta em "Feito" acima. Falta:

- Usar `armazem_destino_id` / `transferencia_origem_id` (ja no schema, sem logica ainda)
  para o check-in de confirmacao entre B2 e A4 que o usuario descreveu: quem libera uma
  peca registra a saida, quem recebe do outro lado confirma a entrada, fechando o ciclo
  e evitando extravio no trajeto. Constroi em cima de `movimentos_consolidados` (a
  tabela remota que a v1 do sync ja envia) — o lado que recebe le de la e escreve uma
  confirmacao de volta.
- Painel consolidado (visao dos dois armazens juntos) para gestao, lendo
  `movimentos_consolidados` no Turso.
- **Importacao de historico**: pedir os XLSX/ODS originais se existirem (os PDFs em
  `modelos_antigos/` quebram coluna e tem registros inconsistentes, entao ficam so como
  arquivo de referencia); definir mapeamento de colunas por tipo de planilha; importar
  somente depois de validacao humana dos totais.

## Depois disso

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
