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
- **Confirmacao de recebimento entre A4 e B2**: a tela de Montagem agora libera as 4
  categorias (nao so peca) e ganhou um seletor de destino ao registrar uma saida —
  "outro armazem" (marca `armazem_destino_id` sozinho, so existem os dois) ou "outro
  destino" (ex.: tecnico externo pra reparo de bateria/modulo/motor, com o
  codigo/serie de cada peca no campo Observacao). Uma nova secao no topo da tela
  mostra "transferencias aguardando confirmacao" — busca ao vivo no Turso o que foi
  endereçado ao meu armazem e ainda nao foi confirmado nem estornado do lado de quem
  enviou; "Confirmar recebimento" copia os itens automaticamente (sem redigitar) e
  sincroniza na hora. Descoberta no caminho: `transferencia_origem_id` (no schema
  desde o inicio) nao servia pra isso — e FK pra linha *na mesma tabela local*, e o
  envio original vive no PC do outro armazem; a solucao usa colunas novas sem FK
  (`recebido_de_armazem_codigo`/`recebido_de_id_origem`, migration
  `0004_transferencias.sql`) guardando a chave composta que ja identifica a linha no
  Turso — detalhes em `docs/ARQUITETURA.md`. Verificado de ponta a ponta contra o
  Turso real (simulando os dois PCs), incluindo o caso de estorno cancelar a
  pendencia do outro lado. 88 testes Rust (85 -> 88).

- **Um unico usuario gestor, cadastro restrito a conferente**: `criar_usuario_como_gestor`
  (usado pela tela `Usuarios.tsx`) agora rejeita `papel = 'gestor'` no backend — so da pra
  cadastrar conferente por ali, entao nao tem como a equipe criar um segundo gestor sem
  querer (testado em `gestor_nao_pode_cadastrar_outro_gestor`). `Usuarios.tsx` perdeu o
  seletor de Papel (sempre manda `conferente`). O bootstrap inicial (`setup_primeiro_usuario`,
  tela de Setup no primeiro uso) continua criando gestor livremente — e o unico jeito
  legitimo de existir um gestor, e so roda quando o banco esta vazio. 89 testes Rust
  (88 -> 89). Banco local de desenvolvimento resetado nessa mesma leva: sem lancamentos/
  fechamentos de teste, usuario unico `jhon` (gestor, sem armazem fixo pra poder fechar/
  estornar nos dois).

## Sprint 4 (resto) — Distribuicao real

O instalador (`.msi`/`.exe`) ja foi gerado pelo `build-installer.yml`, baixado e deixado
pronto pra copiar (pasta `PARA-O-PENDRIVE-ECOVIVA` na Area de Trabalho, junto com
`turso.txt` configurado e um `LEIA-ME.txt` em linguagem simples). Falta so o que exige
acesso fisico aos PCs reais, fora deste ambiente:

- Instalar de verdade numa maquina Windows (validar que o instalador roda e abre).
- Instalar nos PCs reais de A4 e B2, criar as contas de conferente reais pela tela
  Usuarios (logado como `jhon`) e acompanhar o primeiro uso (piloto em paralelo com a
  planilha, conforme a "Decisao atual" do plano de melhorias).

## Sprint 5 — Mais de um armazem "conversarem" (resto)

Confirmado que ha internet real (mesmo que intermitente) nos dois PCs. A sincronizacao
(envio unidirecional) e a confirmacao de recebimento entre B2 e A4 ja estao em "Feito"
acima. O painel consolidado (visao dos dois armazens juntos), que estava listado aqui
como pendente, ja foi entregue pelo Sprint 7 (`painel/index.html`, item 3) - so falta
mesmo:

- **Importacao de historico**: pedir os XLSX/ODS originais se existirem (os PDFs em
  `modelos_antigos/` quebram coluna e tem registros inconsistentes, entao ficam so como
  arquivo de referencia); definir mapeamento de colunas por tipo de planilha; importar
  somente depois de validacao humana dos totais.

## Sprint 6 — Resiliencia de erro no frontend (Feito)

Item que ficou pendente do plano de melhorias original (P2 "Erros e recuperacao no
frontend"). Duplicidade de envio ja estava coberta (todo formulario desabilita o botao
com `enviando` durante o request). Revisado o resto:

- `App.tsx` ja tratava o caso de `getStatus()` falhar na inicializacao — tela de erro
  explicita com "Tentar novamente" em vez de ficar preso em "Carregando..." (nao
  precisou de mudanca, so a checagem).
- `Historico.tsx` ja tratava falha na busca (`erro` + retry via re-submit do formulario).
- Os 4 pontos que realmente vazavam falha silenciosa (promise sem `catch`, lista so
  ficava vazia sem explicacao): `garantirSugestoes` (autocomplete de descricao) em
  `Lancamentos.tsx`, `Montagem.tsx` e `Sac.tsx`, e `carregarPendentes` ("transferencias
  aguardando confirmacao" em `Montagem.tsx`, o mais provavel de falhar por depender do
  Turso). Corrigido: as sugestoes agora mostram um aviso leve e nao-bloqueante
  ("Nao foi possivel carregar as sugestoes... Pode digitar normalmente") em vez de
  falhar calada; as pendentes ganharam estado de erro proprio com botao "Tentar
  novamente", sem bloquear o resto da tela de Montagem.

## Sprint 7 — Painel do administrador em tempo quase real (Feito)

Baseado em `plano de melhorias futuras.md`. Reduzido a 3 fatias sem precisar de API
propria nem WebSocket (o app ja fala direto com o Turso) — todas implementadas:

1. **Fila de sincronizacao com estado**: `movimentos` ganhou `sync_tentativas`,
   `sync_erro`, `sync_proxima_tentativa` (migration `0005_sync_retry.sql`).
   `enviar_para_turso` agora devolve `ResultadoSincronizacao { enviados, falhas }` em
   vez de so uma lista de sucesso; uma falha por linha e registrada com backoff
   progressivo (1/5/15/30/60 min, `calcular_backoff_minutos`) — a linha some da
   proxima tentativa ate o backoff passar (`movimentos_pendentes` filtra por
   `sync_proxima_tentativa`). Novo comando `status_sincronizacao` (gestor-only) mostra
   pendentes/com-erro no `Dashboard.tsx`, que agora tambem re-tenta sozinho a cada 5
   minutos (antes so no clique manual ou abertura do app).
2. **Divergencia de transferencia**: `movimento_itens` ganhou `quantidade_enviada`
   (migration `0006_divergencia_recebimento.sql`, coberta pela cadeia de hash). Ao
   confirmar recebimento (`Montagem.tsx`), o conferente ve a quantidade enviada por
   item com um campo editavel (pre-preenchido) pra registrar quanto chegou de verdade;
   `domain::movimentos::validar_quantidades_recebidas` rejeita receber mais do que foi
   enviado (nunca confia no frontend), aceita menos (divergencia legitima, fica
   registrada nos dois campos pra auditoria/painel).
3. **Painel web somente-leitura**: `painel/index.html`, arquivo unico sem build step,
   `fetch()` direto no Turso HTTP Pipeline API (`POST {url}/v2/pipeline`) com um token
   **somente-leitura** (`turso db tokens create --read-only` — testado que o escopo e
   reforcado pelo servidor: uma tentativa de `INSERT` com esse token e rejeitada com
   `BLOCKED`, nao e so uma convencao do cliente). Publicado via GitHub Pages
   (`.github/workflows/deploy-painel.yml`, dispara em push que toque `painel/**`) —
   URL e token entram no HTML so no deploy, via secrets do repo
   (`TURSO_PAINEL_URL`/`TURSO_PAINEL_TOKEN`), nunca commitados. Gate de senha simples
   (hash SHA-256 no arquivo, nao a senha em texto puro) so pra afastar acesso casual —
   a protecao real e o token ser somente-leitura. GitHub Pages exige repo publico no
   plano gratuito (nao suporta Pages em repo privado sem GitHub Pro) — o repo foi
   tornado publico pra isso; confirmado que nenhum segredo jamais foi commitado
   (`turso.txt` sempre viveu fora do repo).

97 testes Rust (89 -> 97: backoff, fila respeitando `sync_proxima_tentativa`,
`validar_quantidades_recebidas`). Verificado de ponta a ponta contra o Turso real
(envio, divergencia aceita/rejeitada, limpeza dos dados de teste).

## Ajustes finais pre-producao (Feito)

Rodada de acerto fino depois do Sprint 7, com o cliente ja testando de verdade.
101 testes Rust ao final (97 -> 101).

- **Restricao de cadastro de gestor**: `criar_usuario_como_gestor` rejeita
  `papel = 'gestor'` explicitamente — so existe um gestor hoje (Jhon), cadastrado
  fora da tela; a tela "Usuarios" so cria conferentes.
- **Retirada parcial em Saida de Armazem**: `movimentos` ganhou `retirada_completa`
  (migration `0007_retirada_parcial.sql`, coberta pela cadeia de hash — 19 campos no
  `CamposHash` agora). `verificar_retirada_pendente` avisa o conferente, ao digitar
  um numero de pedido ja usado no mesmo armazem, se a retirada anterior ficou
  marcada como parcial; aparece como badge na tabela, marcador "(parcial)" no
  historico/CSV/impressao do fechamento.
- **Cor por aba**: cada fluxo (Saida de Armazem, Montagem, SAC, Historico, Usuarios)
  tem uma cor propria no menu e um friso no topo do primeiro cartao da pagina —
  reduz o risco de a conferente lancar no fluxo errado sem perceber.
- **Impressao do fechamento recalibrada pra caber ~40 lancamentos numa folha A4**:
  o layout automatico da tabela espremia Coleta/Itens pra caber Observacoes,
  forcando quebra de linha em quase toda linha (3 paginas pro que cabia numa folha
  no Excel antigo). `colgroup` com largura fixa por coluna (calibrado por variante)
  + fonte/padding reduzidos + remocao do padding de tela (`32px`) que a area de
  impressao herdava sem necessidade — testado gerando a impressao real (CSS
  compilado do projeto) com 40 linhas via Chrome headless antes de fixar os
  valores. A data/hora de fechamento, que tinha ficado escondida junto com o hash
  de auditoria numa linha minuscula, agora tem linha propria no mesmo tamanho de
  Data/Responsavel(is); o CSV do Historico ganhou a mesma informacao numa coluna
  "Fechado em".
- **Bug real de compatibilidade Windows**: um arquivo commitado com `:` no nome
  quebrava `git checkout` no Windows (caractere reservado no sistema de arquivos) —
  pego pelo CI (`windows-latest` falhando com "invalid path"), corrigido renomeando
  o arquivo. Lembrete pra qualquer commit futuro: nunca usar `: * ? " < > |` em
  nome de arquivo, o alvo real de instalacao e Windows.

## Modernizacao de UI/UX e transferencia entre armazens em Saida de Armazem (Feito)

Rodada pedida pelo usuario: "deixar o sistema mais bonito e moderno", tratar todos os
erros que ainda escapavam calados, e levar a transferencia entre armazens (que so
existia em Montagem) tambem pra Saida de Armazem, ja que veiculo/caixa tambem se
movimenta entre A4 e B2. 102 testes Rust ao final (101 -> 102).

- **Design/UI**: paleta refinada (cinzas mais neutros, sombras em camadas), icones SVG
  inline (`src/components/Icon.tsx`, sem lib externa) nas abas do menu e em botoes-chave,
  listras zebradas nas tabelas, spinner animado nos estados de carregamento
  (`src/components/Carregando.tsx`) no lugar de so texto, e um sistema de notificacoes
  flutuantes (`src/lib/toast.tsx`, `ToastProvider`/`useToast`) pra avisos de fundo
  nao-bloqueantes (falha ao carregar sugestao, falha de sync) — antes viravam banner
  fixo na tela ou nem apareciam.
- **Auditoria completa de tratamento de erro** (nao so os pontos ja conhecidos):
  revisado todo `await` de chamada ao backend em todas as 8 paginas do frontend contra o
  comportamento real de cada funcao em `src/lib/api.ts` (quais rejeitam a promise vs
  quais ja devolvem `{ok:false}`/`null`/`[]` internamente). Achados reais corrigidos: (1)
  `buscarTransferenciasPendentes` engolia qualquer erro e sempre resolvia com `[]`,
  entao o tratamento de erro adicionado na rodada anterior em `Montagem.tsx` nunca
  disparava — corrigido na raiz, a funcao agora deixa o erro real (rede/IPC) propagar,
  ja que "sem sync configurado" ja volta `Ok([])` direto do Rust; (2) `Usuarios.tsx`
  podia ficar preso em "Carregando..." pra sempre se a listagem falhasse — ganhou erro +
  retry; (3) exportar CSV no Historico falhava calado (estado resetava mas sem
  mensagem) — ganhou mensagem de erro. Tambem auditados os `unwrap()`/`expect()`/
  `panic!` do backend fora de teste: so existe um, o boilerplate padrao do Tauri em
  `lib.rs`, nada de risco real.
- **Transferencia entre armazens tambem em Saida de Armazem**: o backend da
  transferencia (Sprint acima) estava com o fluxo fixo em `peca_montagem` em tres
  lugares (`db::sync::SQL_PENDENTES_RECEBIMENTO` filtrava so esse fluxo,
  `TransferenciaPendente` nem carregava o campo `fluxo`, e `confirmar_recebimento`
  criava a entrada de confirmacao sempre como `peca_montagem`) — generalizado pra
  tambem aceitar `saida_armazem`, com o fluxo da transferencia original viajando ate a
  confirmacao. `Lancamentos.tsx` ganhou um seletor "Cliente/coleta" vs "Transferir para
  {outro armazem}" (so aparece em `tipo=saida`, com `outroArmazem` calculado a partir do
  novo prop `armazens`); numero do pedido fica opcional nesse caso (decisao confirmada
  com o usuario - uma transferencia interna nao tem numero de pedido no sistema
  externo, igual ja acontecia em Montagem) e os campos Coleta/Quem retirou/retirada
  parcial somem (nao se aplicam a uma transferencia). A secao "transferencias chegando"
  foi extraida pra um componente compartilhado
  (`src/components/TransferenciasChegando.tsx`, recebe `fluxo` e filtra a lista
  client-side) usado tanto por Montagem quanto por Lancamentos, pra nao duplicar as
  ~130 linhas de logica (busca, confirmar, quantidade editavel por item). Decisao
  tomada com o usuario: **nao** trazer a opcao "outro destino externo" (tecnico) pra
  Saida de Armazem - so a transferencia entre os dois armazens mesmo.

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
