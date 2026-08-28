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

## Auditoria de cenarios de entrada/saida (Feito)

Pedido do usuario: "testa todas as hipoteses de entrada e saida e ve se precisamos de
mais campos". Mapeada cada combinacao fluxo x tipo contra `validar_novo_movimento` (que
ja aceita `entrada`/`saida` livremente em qualquer fluxo) e contra o que cada tela
realmente deixa o conferente fazer. 106 testes Rust ao final (102 -> 106).

- **Bug real encontrado e corrigido**: `Montagem.tsx` tinha perdido a opcao de
  "Entrada" manual - o commit que adicionou a transferencia entre armazens (`0c4c042`)
  reescreveu o formulario em torno do seletor de destino (so se aplica a saida) e o
  `handleSubmit` ficou mandando `tipo: 'saida'` fixo, sem nenhuma decisao documentada
  dizendo que isso era proposital. Resultado: nao existia forma de registrar peca
  solta chegando no B2 por compra direta de fornecedor (so via confirmacao de
  transferencia vinda de A4). Restaurado o alternador Entrada/Saida (independente do
  seletor de destino, que so aparece quando tipo=saida) - confirmado com o usuario
  antes de mexer.
- **SAC ganhou uma segunda etapa (saida)**: antes so registrava a entrada (devolucao
  do cliente, garantia/venda). Confirmado com o usuario que existe saida real -
  "entregue/devolvida ao cliente" e "descarte/sucata" (nao existe "devolvida ao
  fabricante" hoje). Sem coluna nova no banco: `domain::movimentos::validar_novo_movimento`
  agora escolhe o conjunto de `motivo` valido por `tipo`
  (`MOTIVOS_SAC_ENTRADA_VALIDOS` = garantia/venda, `MOTIVOS_SAC_SAIDA_VALIDOS` =
  entregue/descarte) - uma saida com motivo de entrada (ou vice-versa) e rejeitada.
  `Sac.tsx` ganhou o mesmo alternador Entrada/Saida das outras telas.
- **Consolidado texto de motivo do SAC**: existiam 3 copias quase identicas da logica
  "venda -> mostra valor, garantia -> texto fixo" (`Sac.tsx`, `FechamentoImpressao.tsx`,
  `Historico.tsx`). Unificado em `motivoSacTexto` (`src/lib/situacao.ts`, ao lado de
  `situacaoInfo`), agora cobrindo os 4 motivos (garantia/venda/entregue/descarte) num
  lugar so - corrige de quebra uma pequena inconsistencia de formatacao de valor que
  existia entre a tela e a impressao.
- **Ajuste menor em Lancamentos**: os campos "Coleta" e "Quem retirou" apareciam
  igual pra `tipo=entrada`, onde "quem retirou" nao faz sentido (nada foi retirado,
  algo chegou). "Quem retirou" some pra entrada; "Coleta" muda de rotulo pra
  "Fornecedor / origem" nesse caso.

## Versao 0.2.0

Bump de `0.1.0` pra `0.2.0` (`package.json`, `Cargo.toml`, `tauri.conf.json` + lockfiles
regenerados) - a versao ficava parada desde o inicio do projeto, sem nenhuma tag/release
criada, mesmo depois de varias sprints inteiras. Passou a valer a pena marcar porque o
instalador carrega a versao no nome do arquivo (`..._0.2.0_x64_en-US.msi`) - sem o bump,
nao dava pra saber pelo nome do instalador se um PC de A4/B2 ja tinha a transferencia
entre armazens ou o SAC com saida. `LEIA-ME.txt` do pendrive atualizado com os nomes de
arquivo novos.

## Polimentos pre-producao: impressao, exportacao e painel (Feito)

Pedido do usuario com um PDF real de fechamento anexado, mostrando cabecalho de tabela
ilegivel ("QUEM RETIROOUBSERVACOES"). Virou o pacote de polimento pra fechar a versao
0.3.0.

- **Bug real corrigido**: `th { white-space: nowrap; }` (regra base de tela,
  `global.css:436`) vazava pro `@media print` sem ser resetado - em colunas estreitas
  (8% de largura pra "Registrado por"/"Situacao"), o texto do cabecalho nao quebrava
  linha e vazava visualmente pra celula vizinha. Corrigido com `white-space: normal`
  em `.area-impressao thead th`. Reverificado com 40 lancamentos sinteticos via Chrome
  headless (`--print-to-pdf`): cabecalho legivel, paisagem A4 mantida, ainda cabe numa
  pagina so.
- **Exportacao CSV/XLSX do fechamento diario**: novos botoes na tela de fechamento
  (Lancamentos/Montagem/SAC) e um botao XLSX a mais no Historico (que ja tinha CSV).
  `src/lib/exportFechamento.ts` centraliza as colunas por `fluxo` (espelhando
  `FechamentoImpressao.tsx`) e o rodape de auditoria (`rodapeAuditoria` - hash de
  referencia + timestamps, texto deliberadamente sem prometer inviolabilidade, ja que
  CSV/XLSX sao editaveis por natureza). `src/lib/xlsx.ts` gera o arquivo inteiramente
  em memoria (SheetJS `xlsx@0.18.5`) e baixa via `Blob`/`<a download>`, igual ao CSV -
  nenhuma capability nova no Tauri. `xlsx@0.18.5` tem uma vulnerabilidade alta no
  `npm audit` (poluicao de prototipo/ReDoS), mas o proprio aviso do fabricante diz que
  so afeta quem *le* arquivo externo - nosso uso e so escrita. Avaliado trocar por
  `exceljs` e descartado: 98 pacotes com dependencias obsoletas e vulnerabilidades
  moderadas proprias, pior no total.
- **Logo no fechamento impresso**: `ecoviva-logo.png` no cabecalho do PDF (so na
  versao impressa - CSV/XLSX ficam so com texto, SheetJS Community nao embute imagem
  de forma confiavel).
- **Filtros novos no painel** (`painel/index.html`): intervalo de datas (De/Ate), tipo,
  situacao e busca livre. A coluna "Situacao" da tabela, que mostrava o `status` cru do
  banco, passou a usar a mesma derivacao ENTRADA/BAIXA/ESTORNO de
  `src/lib/situacao.ts` (portada inline, o arquivo e standalone sem import).
- **Insights no painel**: card com total de movimentos/unidades, transferencias
  pendentes e taxa de estorno do periodo filtrado, mais dois graficos de barra (SVG
  inline, sem lib externa) de volume por dia e top categorias - deliberadamente so
  analytics de movimentacao, sem nenhuma nocao de saldo/estoque (regra ja documentada
  no CLAUDE.md).

## Versao 0.3.0

Bump de `0.2.0` pra `0.3.0` junto com o pacote de polimento acima - mesmo motivo do
bump anterior (nome do instalador no pendrive precisa refletir o que mudou).

## SAC: saida tambem aceita Garantia/Venda (Feito)

Pedido do usuario: a saida do SAC (`Sac.tsx`, "Entregue ou descarte") tambem precisava
das opcoes Garantia e Venda, alem de Entregue/Descarte. `MOTIVOS_SAC_SAIDA_VALIDOS`
(`domain/movimentos.rs`) passou de 2 pra 4 valores (`entregue`, `descarte`, `garantia`,
`venda`) - so a saida ganhou as opcoes novas, a entrada continua so com
garantia/venda (nao foi pedido o inverso). `valor_centavos` obrigatorio quando
`motivo = venda` ja nao era condicionado a `tipo` no backend, so precisou soltar a
mesma trava no frontend (`Sac.tsx`, campo so aparecia se `tipo === 'entrada'`).
`motivoSacTexto` (`situacao.ts`) ja cobria os 4 motivos independente do tipo, entao
Historico/exportacao/impressao do fechamento nao precisaram de mudanca. 108 testes
Rust (106 -> 108).

## Exportacao consolidada do fechamento do dia (Feito)

Pedido do usuario: poder exportar os 3 fluxos (Saida de Armazem/Montagem/SAC) de uma
vez so, num arquivo so, separados por secao - em vez de precisar abrir cada tela e
exportar uma de cada vez. Implementado como um botao no cabecalho do `Dashboard.tsx`
("Exportar fechamento do dia", `src/components/FechamentoConsolidado.tsx`), disponivel
pra qualquer usuario (mesma visibilidade dos exports individuais ja existentes) - abre
um mini-painel com campo de data (qualquer dia, nao so hoje) e os botoes Exportar
CSV/XLSX.

- **Nao virou um 4º "fechar o dia"**: decisao deliberada pra nao duplicar o conceito de
  fechamento que ja existe por fluxo - o export consolidado so agrega o que ja foi
  fechado. `buscarSecoesConsolidadas` (`src/lib/exportConsolidado.ts`) busca o
  fechamento dos 3 fluxos pra data escolhida (`buscar_fechamento_do_dia`) e so inclui no
  arquivo os que retornaram um fechamento de verdade (fluxo sem lancamento naquele dia,
  ou ainda em aberto, fica de fora silenciosamente) - se nenhum dos 3 estava fechado,
  mostra aviso em vez de gerar arquivo vazio.
- **XLSX**: uma aba por fluxo fechado (reaproveita `baixarXlsx`, que ja suportava
  multiplas abas desde a v0.3.0), cada uma com seu proprio rodape de auditoria.
- **CSV**: como CSV nao tem conceito de aba, as secoes ficam uma abaixo da outra no
  mesmo arquivo, separadas por um subtitulo (`=== SAIDA DE ARMAZEM ===`) e linha em
  branco. Achado no caminho: `paraCsv` (`src/lib/csv.ts`) prependia um BOM UTF-8 a cada
  chamada - concatenar blocos direto teria repetido o BOM no meio do arquivo. Extraida
  `linhasParaCsv` (mesma logica, sem BOM) pra montar cada secao, com o BOM aplicado uma
  unica vez no arquivo final.
- Reaproveita 100% as colunas por fluxo ja centralizadas em `colunasFechamento`
  (`exportFechamento.ts`, da v0.3.0) - nao duplica logica de formatacao de linha.

## Fechamento impresso mais profissional (Feito)

Pedido do usuario com um PDF real anexado (dia com so 3 lancamentos): o CSS de
impressao (`global.css`) foi calibrado pra caber ~40 lancamentos numa folha - otimo num
dia cheio, mas num dia com poucos lancamentos sobrava muita folha em branco e o
cabecalho/tabela ficavam desproporcionalmente pequenos. 4 melhorias implementadas em
`FechamentoImpressao.tsx`/`global.css`:

- **Rodape fixo no fim da folha**: `.area-impressao` virou flex-column com
  `min-height: 200mm` (altura util do A4 paisagem, 210mm - 2x5mm de margem do `@page`)
  em `@media print`; `.rodape-documento` (nome do sistema, hash de auditoria,
  "Documento impresso em") usa `margin-top: auto` pra ficar sempre ancorado no fim da
  pagina - em dias cheios o conteudo so cresce alem do minimo, igual ja acontecia
  antes.
- **Bloco de resumo** (`.resumo-fechamento`) entre a tabela e a assinatura: contadores
  "Por situacao" (ENTRADA/BAIXA/ESTORNO) e "Por categoria" (soma de quantidade por
  categoria de item) - preenche com informacao util o espaco que sobraria em branco.
- **Moldura + cabecalho em formato de ficha**: borda fina ao redor de toda a folha
  (`border: 1px solid var(--texto)`) mais um friso colorido no topo na cor do fluxo
  (`CORES_VARIANTE`, mesma paleta do "friso no topo" ja usado nas abas do menu -
  Saida de Armazem/Montagem/SAC cada um com sua cor). Cabecalho trocou de texto corrido
  por uma grade rotulo:valor (`.ficha-campos`: Armazem/Data/Fechado em/Responsavel(is)
  em campos separados).
- **Uma linha de assinatura por responsavel**: antes sempre uma linha generica
  ("Assinatura da conferente responsavel"), mesmo com 2+ conferentes tendo lancado
  algo no dia. `area-assinaturas` agora renderiza um bloco "Assinatura - {nome}" por
  conferente distinto que apareceu nos lancamentos do dia (cai pro nome de quem fechou
  o dia se por algum motivo nao houver lancamento).

**Verificacao**: reproduzido o CSS compilado (`vite build`) num harness estatico com
3 e com 42 lancamentos sinteticos, impresso via Chrome headless
(`--print-to-pdf`) - as duas primeiras rodadas de ajuste estouraram 42 linhas pra uma
2ª pagina (rotulo de assinatura + rodape sobrando ~8mm); reduzido padding da moldura
(3mm -> 2mm) e a margem/fonte do bloco de assinaturas pra recuperar esse espaco - 42
lancamentos (acima do "~40" original) voltou a caber numa unica folha, e o dia de 3
lancamentos ficou com cabecalho/tabela/resumo no topo e o rodape ancorado embaixo, sem
mais a folha "morrer" em branco no meio.

## Faixa de insights ao vivo pro conferente (Feito)

Pedido do usuario: dar mais insights pros conferentes com os dados do dia, nao so pro
gestor (que ja tem o painel `painel/index.html`). Decisao tomada com o usuario: manter
simples - contadores de texto discretos, sem grafico, pra nao competir com o
formulario de lancamento (o foco real da tela).

- `ResumoDoDia.tsx` - faixa no topo de `Lancamentos.tsx`/`Montagem.tsx`/`Sac.tsx` (antes
  do fechamento do dia) com "Hoje: N lancamentos · M unidades", contagem por situacao
  (ENTRADA/BAIXA/ESTORNO) e por categoria - os mesmos numeros que ja apareciam so
  depois de fechar o dia (`FechamentoImpressao`), agora visiveis ao vivo enquanto o
  conferente ainda esta lancando. Fica oculta se ainda nao houver lancamento no dia (sem
  poluir a tela vazia).
- `resumoMovimentos` extraida pra `src/lib/situacao.ts`, compartilhada entre
  `ResumoDoDia.tsx` e o resumo do fechamento impresso (`FechamentoImpressao.tsx`) - a
  mesma conta que antes vivia duplicada inline num dos dois lugares.
- So aparece na visao "dia aberto"; no dia ja fechado o resumo equivalente ja
  mostrado por `FechamentoImpressao` cobre o mesmo papel, entao nao duplica.

## Transferencias pendentes mais evidentes + opcao "Outro" nos selects (Feito)

Pedido do usuario a partir de um print do painel do gestor (`painel/index.html`,
secao "Transferencias pendentes de confirmacao") - queria essa visibilidade tambem
dentro do app de cada conferente, nao so no painel web separado, e clicavel pra ir
direto na aba certa. Junto, pediu pra selects em geral ganharem uma opcao "Outro" que
cai em observacoes quando o caso nao e o normal. 115 testes Rust ao final (108 -> 115).

- **Contador nas abas do menu**: `Dashboard.tsx` busca `buscarTransferenciasPendentes()`
  uma vez (nao gestor-only - quem recebe fisicamente e confirma e o conferente),
  agrupa por `fluxo` e mostra um badge amarelo com a contagem direto nos botoes "Saida
  de Armazem"/"Montagem" (`.badge-notificacao`, `global.css`) - atualiza sozinho a cada
  60s e logo apos "Sincronizar agora". Clicar na aba ja leva pra tela certa (o
  componente `TransferenciasChegando.tsx` que ja existia continua sendo onde a
  confirmacao de fato acontece).
- **Opcao "Outro"**: decisao tomada com o usuario, campo por campo (alguns exigiram
  ensinar o backend a aceitar o valor, outros ficaram so no frontend):
  - `categoria` do item (`scooter`/`triciclo`/`patinete`/`peca`) ganhou `outro` - o
    usuario confirmou explicitamente que queria isso mesmo sendo uma decisao anterior
    documentada (evitar catalogo de produto) - `outro` continua uma lista curta fixa,
    nao virou catalogo aberto, so que agora exige `observacao` preenchida
    (`domain::movimentos::validar_novo_movimento`).
  - `condicao` da peca (`peca_montagem`, obrigatoria) e `motivo` do SAC (obrigatorio)
    tambem ganharam `outro` real, aceito pelo backend, com a mesma exigencia de
    descricao (`observacao` do item pra condicao, `observacoes` do movimento pra
    motivo - `Sac.tsx` ganhou um campo Observacoes que nao existia antes, so pra isso).
  - `montagem` do item (`montado`/`caixa`, sempre opcional) NAO ganhou um valor
    `outro` no backend - na UI "Outro" so limpa o campo pra `null` e pede a descricao
    em observacao, ja que o campo em si nunca foi obrigatorio.
  - `motivoSacTexto` (`situacao.ts`) mostra "Outro - {observacoes}" em vez de so
    "Outro" nas tabelas/impressao/exportacao, reaproveitando o mesmo texto livre que a
    validacao exigiu no lancamento.

## Auditoria pre-v1.0: revisao de codigo + correcoes (Feito)

Pedido do usuario: sistema maduro, hora de planejar a v1.0 "definitiva pra producao,
sem bug nenhum". Rodada `/code-review` (nivel high, multi-agente) sobre o diff
acumulado da sessao (~750 linhas, 13 arquivos + 4 novos) antes de dar qualquer nota.
115 testes Rust seguem passando, `clippy`/`fmt`/`tsc` limpos.

**Bugs reais corrigidos**:
- **`resumoMovimentos` contava estorno em dobro**: `estornar_movimento` copia a
  quantidade original sem inverter o sinal (so `estornado_de` marca a reversao) - o
  resumo novo (`ResumoDoDia.tsx` ao vivo e `.resumo-fechamento` na folha impressa)
  somava as duas linhas positivas, mostrando por ex. "8x scooter" no resumo bem acima
  de "Total geral: 0 unidades" no mesmo dia/folha. Corrigido com o mesmo sinal
  `estornado_de ? -1 : 1` que `totalGeral` ja usava.
- **CSV consolidado** tinha uma linha em branco indevida entre o subtitulo da secao e
  seu proprio cabecalho (`exportConsolidado.ts`).
- **3 mensagens de erro desatualizadas** no backend (`domain::movimentos.rs`) -
  motivo ausente no SAC (entrada e saida) e condicao ausente na Montagem citavam so
  as opcoes antigas, sem "outro", mesmo ja aceitando o valor.
- **Badge de transferencias pendentes** nas abas nao atualizava na hora ao confirmar
  um recebimento de dentro da propria aba (so no proximo poll de 60s) -
  `Lancamentos.tsx`/`Montagem.tsx` agora avisam o `Dashboard.tsx` via
  `onTransferenciaConfirmada`.
- **"Nº 3 - pedido - - 3 un."**: texto com dois tracos seguidos quando o lancamento
  nao tem numero de pedido (ex.: transferencia). Corrigido pra omitir o trecho
  "pedido" inteiro nesse caso.

**Polimento de codigo** (sem mudanca de comportamento): `itemPrecisaObservacao`
(duplicada em `Lancamentos.tsx`/`Montagem.tsx`) e a checagem "outro exige detalhe"
(duplicada 2x em `domain::movimentos.rs`) foram extraidas pra funcoes compartilhadas
(`src/lib/outro.ts`, `exigir_detalhe_para_outro`); `.badge-notificacao` passou a
compor com a classe base `.badge` em vez de reescrever as mesmas propriedades.

**Investigado e descartado**: `usuario.armazem_id as number` (usado em varias telas,
inclusive a nova `FechamentoConsolidado`) e tecnicamente inseguro pra um "gestor sem
armazem fixo" (existe e e testado na camada de dominio), mas `Setup.tsx`/`Usuarios.tsx`
sempre exigem selecionar um armazem real - esse estado nunca e alcancavel pela UI hoje,
entao nao foi alterado.

## Polimento pre-v1.0 (Feito, 2026-08-27)

Decisao do usuario: horario sempre local (nunca UTC), e um unico gestor por enquanto
(sem feature de segundo gestor/continuidade).

- **UTC -> horario local em todo timestamp exibido**: `criado_em` (`movimentos` e
  `fechamentos`) usava o `DEFAULT (datetime('now'))` do SQLite, que e UTC - migration
  nao editada (regra do projeto), em vez disso as 3 INSERTs afetadas (`criar_movimento`,
  `estornar_movimento` em `domain/movimentos.rs`, `fechar_dia` em
  `domain/fechamentos.rs`) passaram a gravar `criado_em` explicitamente via
  `datetime('now', 'localtime')`. `criado_em` nao faz parte de `CamposHash`/hash de
  auditoria, entao mudar como e populado nao afeta a cadeia. No frontend, os 3 lugares
  que geravam "Exportado em"/"Documento impresso em" com `new Date().toISOString()`
  (UTC) passaram a usar `agoraLocalTexto()` (novo helper em `src/lib/data.ts`, usa
  getters locais do `Date`). O "Horario" digitado pela conferente ja era local, sem
  mudanca. `sync_proxima_tentativa`/`enviado_em`/`sincronizado_em` (bookkeeping interno
  do sync, nunca exibido) ficaram como estavam - so timestamps que aparecem pra
  conferente/gestor foram tocados.
- **Segundo gestor**: decisao tomada — fica so o Jhon por enquanto. Item removido da
  lista de "deixado de fora, revisitar" abaixo; nao e mais uma pendencia em aberto.
- **Botao "Exportar fechamento do dia" removido**: a exportacao consolidada (3 fluxos
  juntos, `FechamentoConsolidado.tsx`/`exportConsolidado.ts`) nao estava sendo usada no
  dia a dia — removida (componente, lib e `IconDownload`, que so ela usava). A
  exportacao por fluxo individual (Historico) continua.
- **Estorno liberado pro conferente**: mesma mudanca que `fechar_dia` (acima), aplicada
  a `estornar_movimento` — usa `autorizar_movimento` em vez de checar `papel`. Qualquer
  conferente ativo pode corrigir (estornar) um lancamento do proprio armazem, antes ou
  depois do dia fechado. A coluna "Acoes"/botao "Estornar" (tabela do dia aberto e o
  painel "Corrigir um lancamento deste dia" apos o fechamento) deixou de ser
  `{ehGestor && ...}` nas 3 telas — `ehGestor` ficou sem uso e foi removido delas.
  `papel = 'gestor'` agora so gate `criar_usuario`.
- **Cores fixas saida/entrada**: o alternador saida/entrada nas 3 telas usava a mesma
  cor generica de "aba ativa" (dependia da posicao pra saber qual estava selecionado).
  Agora saida e sempre vermelha e entrada sempre verde (convencao de extrato bancario),
  reconhecivel sem ler o texto (`--cor-saida`/`--cor-entrada` em `global.css`).

## v0.4.0 (Feito)

Sessao de "analisa o sistema e planeja melhorias" (2026-08-27): revisado o codigo atual
contra o roadmap. P0-P2 do plano original ja estavam feitos; os itens abaixo foram
escolhidos pelo usuario como prioridade, entre uma lista maior de candidatos (ver
secoes seguintes pros que ficaram de fora por ora). Implementados numa sessao seguinte
("continue as proximas sprints ate terminar todas", 2026-08-27).

- **Paginacao no Historico**: `buscar_historico` tinha um limite fixo de 500 linhas
  sem paginacao — uma busca ampla cortava resultado silenciosamente sem avisar que
  havia mais dados. Agora `buscar_historico` recebe `offset`, busca
  `LIMITE_HISTORICO + 1` linhas (LIMIT+OFFSET) e devolve `ResultadoHistorico { movimentos,
  tem_mais }` — `tem_mais` vem de ter recebido a linha extra, sem COUNT separado.
  `Historico.tsx` acumula localmente ao clicar "Carregar mais" (offset =
  `resultados.length`); uma nova busca (troca de fluxo/filtro) reseta pra offset 0.
- **Protecao contra forca bruta no login**: `domain::auth::login` agora libera 3
  tentativas erradas sem penalidade, depois bloqueia a conta progressivamente
  (1/5/15/30min, 60min da 5a em diante — `calcular_bloqueio_minutos`, mesmo formato do
  backoff de sync mas reescrito em `domain` pra nao criar uma dependencia de `domain`
  em `db`). Colunas `tentativas_falhas`/`bloqueado_ate` em `usuarios` (migration
  `0008_lockout_login.sql`), novo `AppError::ContaBloqueada` distinto de
  `CredenciaisInvalidas`. Login certo zera o contador.

**Deixado de fora por ora, revisitar se necessario**:
- Importacao de historico antigo (XLSX/ODS) — Sprint 5 resto, so relevante se ainda ha
  valor real nos dados antigos.
- Testes automatizados de frontend (hoje so `tsc --noEmit` do lado React) — baixo risco
  dado o tamanho do app.

## Capricho de UX/UI (Feito o essencial, 2026-08-27)

Pedido do usuario: dedicar uma sprint a polimento visual/de usabilidade — tanto do app
(Tauri) quanto do painel web somente-leitura (`painel/index.html`,
jlsgo.github.io/conferente_armazem). O painel era o item mais cru (paleta improvisada,
sem dark mode de verdade apesar de declarar `color-scheme: light dark`, badge de
situacao so no estorno) - foi o foco desta passada:

- **Paleta harmonizada com o app**: `painel/index.html` passou a usar os mesmos tons
  (`--verde`/`--erro`/`--azul` etc.) de `src/styles/global.css`, em vez de uma paleta
  proxima mas ligeiramente diferente.
- **Dark mode de verdade**: `@media (prefers-color-scheme: dark)` redefinindo os tokens
  - antes so o `color-scheme: light dark` no `:root` (que so afeta scrollbar/form
  controls nativos do browser, nao a pagina em si) estava la, sem nenhum override real.
  Verificado visualmente (headless Chrome, claro e escuro) antes de commitar.
  Cabecalho com gradiente + icone, cards com sombra, tabela com hover/zebra sutil,
  campos de filtro com foco visivel.
- **Badge de situacao consistente**: `situacaoBadge()` so envolvia ESTORNO num
  `<span class="badge">`; ENTRADA/BAIXA eram texto puro. Agora as 3 usam as mesmas
  classes `.badge-entrada`/`.badge-baixa`/`.badge-estorno` que o app usa em
  `src/lib/situacao.ts` (mesmas cores tambem) - quem olha os dois reconhece o mesmo
  vocabulario visual.
- **Favicon**: emoji de caixa via data URI (sem asset novo, mantendo o arquivo unico
  sem build step).

No app (Tauri) em si, o trabalho de consistencia ja avancou nas sessoes anteriores
(insights por tela, campo Outro, "Fechar o dia"/estorno liberados pro conferente,
cores fixas saida/entrada) - nao foi identificado nenhum ponto especifico adicional
que doesse o suficiente pra justificar mexer mais agora. Revisitar se o usuario
apontar uma tela especifica que incomoda no dia a dia.

## v1.0.0 - primeira versao de producao (2026-08-27)

Todos os itens planejados pra essa rodada final (v0.4.0 + capricho de UX/UI + estorno
liberado pro conferente) fechados nessa mesma sessao ("continue as proximas sprints ate
terminar todas"). Instalador Windows gerado (`build-installer.yml`) e copiado pra
`~/Downloads/PARA-O-PENDRIVE-ECOVIVA` (fora do repo - pasta de staging local, nao
versionada), pronto pra instalar em A4 e B2. Dados de teste (local desta maquina +
Turso compartilhado) resetados antes do build - `usuarios`/`armazens` preservados,
zero movimentos/fechamentos no dia 1 de producao de verdade.

## Depois disso

- Escalar o mesmo instalador para novos armazens, se a empresa abrir mais.

## Sprints do painel web (planejadas com o usuario, 2026-08-28)

Pedido do usuario apos ver o v1.0.0 rodando: melhor visibilidade/filtros/insights no
painel, horario de Brasilia, mais destaque pro numero do pedido, "pagina moderna,
segura e com dados extremamente confiaveis". Plano dividido em sprints, aprovado pelo
usuario ("sim, de maneira profissional, separando por sprints") - cada uma vira um
commit proprio.

### Sprint 1: confiabilidade dos dados (Feito)

Prioridade 1 porque sem isso nenhum insight novo seria confiavel.

- **Horario de Brasilia de verdade em `enviado_em`**: a coluna "Sincronizado" do
  painel usava um timestamp gerado com `datetime('now')` rodando *no proprio servidor
  do Turso* (nuvem - sempre UTC, nunca o fuso de quem esta operando), nao dava pra
  corrigir so trocando pra `'localtime'` la porque seria o fuso do servidor, nao de
  Brasilia. Corrigido calculando o horario local **no PC de origem**
  (`db::sync::agora_local`, mesma ideia do `criado_em`) e mandando pronto pro Turso
  como parametro (`enviado_em` saiu de `SQL_UPSERT` como literal `datetime('now')` pra
  virar `?23` bind) - `enviar_para_turso` ganhou um parametro `enviado_em_local`, os 3
  pontos que chamam (`lib.rs` no startup, `sincronizar_agora`, `confirmar_recebimento`)
  atualizados. Verificado contra o Turso real: linha nova mostrou horario batendo com
  o relogio do SO, nao mais ~3h adiantado.
- **Filtros aplicados na consulta SQL, nao so no que ja tinha sido baixado**: antes o
  painel buscava sempre as ultimas 300 linhas e filtrava tudo (tabela E os cards de
  "insights") em cima desse recorte fixo - um filtro de 60 dias podia mostrar um
  resumo incompleto sem avisar direito. Agora armazem/fluxo/data/tipo/situacao viram
  `WHERE` de verdade na consulta ao Turso (sempre com bind de parametro, nunca
  concatenando o valor no SQL, embora os campos ja venham restritos por `<select>`/
  `<input type=date>` no HTML), com o teto subindo pra 2000 linhas e um aviso visivel
  no proprio cartao de resumo (nao so no rodape da tabela) quando o filtro tem mais
  linhas que isso. So a busca por texto livre (pedido/nome/item) continua client-side
  (rapida, sem round-trip a cada tecla).
- **Bug de verdade encontrado ao portar a logica**: `calcularPendentes` do painel so
  considerava `fluxo === "peca_montagem"`, nunca `saida_armazem` - transferencias de
  veiculo pendentes nunca apareciam no painel (apareciam certo no app). Corrigido
  reescrevendo a deteccao de pendentes inteira em SQL, portada de
  `db::sync::SQL_PENDENTES_RECEBIMENTO` (a mesma logica de 2 `NOT EXISTS` - nao
  confirmada, nao estornada do lado de quem enviou - ja usada e testada no app real),
  em vez de reimplementar em JS. Passou a rodar independente do filtro da tabela (uma
  transferencia B2->A4 pendente tem que aparecer mesmo filtrando "so A4" ou "so B2").
- **Bug de hoisting pego testando contra o Turso real antes de commitar**: as novas
  `var LIMITE_LINHAS`/`COLUNAS_MOVIMENTO`/`ultimosFiltrados` foram declaradas depois do
  bloco de autenticacao - numa visita repetida na mesma sessao (`jaAutenticado` ja
  `true`), o carregamento chama `atualizar()` de forma sincrona antes do motor de JS
  chegar nessas linhas, entao vinham `undefined` (virava `LIMIT NaN`/`SELECT
  undefined` na consulta). Movidas pra antes do bloco de autenticacao. So foi pego
  porque o teste rodou contra o Turso de verdade (via servidor HTTP local + Chrome
  headless) em vez de so ler o codigo - viraria um bug em producao pra qualquer
  usuario que reabrisse a aba na mesma sessao.
- Bonus pequeno (calculo ja estava ali): cards de resumo ganharam "Entrada"/"Saida"
  separados, nao so "Unidades" total.

### Sprint 2: sincronizacao por armazem (Feito)

- Novo cartao "Sincronizacao por armazem" logo abaixo dos filtros: um pill por
  armazem (`A4`/`B2`, lista fixa) mostrando ha quanto tempo cada um mandou dado pro
  Turso pela ultima vez (`MAX(enviado_em) GROUP BY armazem_codigo`), com um ponto
  colorido de frescor (verde <15min, amarelo <2h, vermelho acima disso ou nunca
  sincronizou). Antes so existia um "atualizado as HH:MM" global no cabecalho, que
  nao dizia nada sobre um armazem especifico estar desatualizado - se o B2 ficasse
  dias sem internet, o painel mostrava o dado velho dele como se fosse atual, sem
  avisar. Verificado contra o Turso real (um armazem com dado fresco, outro que nunca
  sincronizou) antes de commitar.

### Sprint 3: visibilidade do numero do pedido (Feito)

- Campo "Numero do pedido" proprio, primeiro na barra de filtros (antes da busca
  generica), com destaque visual (borda/texto verde) - filtra no SQL
  (`numero_pedido LIKE ?`, com bind de parametro) com um debounce de 400ms pra nao
  disparar uma consulta a cada tecla. A busca generica ("Nome ou item") deixou de
  tambem casar com pedido, pra nao ter dois campos fazendo a mesma coisa de jeito
  ambiguo.
- Coluna "Pedido" na tabela ganhou destaque (negrito + cor verde) - reconhecivel de
  relance em vez de se misturar com o resto da linha. Fixar a coluna ao rolar
  horizontal (`position: sticky`) ficou de fora por ora: com colunas de largura
  variavel o calculo do offset fica fragil, e o ganho e menor que o risco de quebrar
  visualmente - revisitar so se o usuario pedir depois de usar.
- Verificado contra o Turso real: filtro por "3893" corretamente trouxe so a linha
  com esse pedido, escondendo as outras.

### Sprints seguintes (planejadas, ainda nao feitas)

- **Sprint 4**: mais insights - comparativo A4 x B2 lado a lado, participacao por
  fluxo (Saida de Armazem/Montagem/SAC), filtro por responsavel, atalhos de periodo
  (hoje/7d/30d).
- **Sprint 5**: confiabilidade visivel - comunicar na propria tela que e um espelho
  somente-leitura (token read-only ja e verdade, so nao aparece pro usuario), talvez
  um indicativo de integridade reaproveitando a cadeia de hash que o app ja calcula.

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
