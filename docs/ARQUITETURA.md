# Arquitetura

Sistema desktop **offline-first** para as conferentes dos armazens Ecoviva registrarem
entradas e saidas (patinetes, scooters, triciclos e pecas), substituindo as planilhas em
papel/Excel. Roda inteiramente no computador de cada armazem, sem depender de internet.

## Stack

- **Backend**: Rust + [Tauri v2](https://tauri.app). Um unico binario nativo por
  plataforma (Windows e Linux hoje; macOS funciona pela mesma base, sem trabalho extra).
- **Banco de dados**: SQLite (via `rusqlite`, com o driver compilado junto ao app —
  `features = ["bundled"]"`, sem exigir SQLite instalado no PC). Um arquivo por
  computador, em `<diretorio de dados do usuario>/ecoviva-armazem.db`.
- **Frontend**: React + TypeScript, empacotado com Vite e servido pelo proprio Tauri
  (sem servidor HTTP externo).

## Por que Tauri (e nao Electron)

O protótipo inicial foi feito em Electron. Foi trocado por Tauri ainda no começo do
projeto porque: o instalador final fica muito menor, o backend em Rust é memory-safe, o
sistema de `capabilities` do Tauri aplica menor privilégio de forma declarativa
(`src-tauri/capabilities/default.json` — nenhum plugin de `fs`, `shell`, `http` ou
`dialog` habilitado, so os comandos custom da aplicacao), e a arvore de dependencias e
muito menor (o `npm audit` do projeto em Electron acusava 15 vulnerabilidades, 1 critica;
com Tauri, 0).

## Modulos (`src-tauri/src`)

```
domain/     Regras de negocio puras. Nao conhece Tauri nem SQL de UI - so recebe uma
            &Connection/&mut Connection e retorna Result<T, AppError>. E aqui que fica
            toda validacao. Testado com banco SQLite em memoria (rapido, sem mocks).
  auth.rs        hash/verify de senha (Argon2), criar usuario, login.
  movimentos.rs  criar pedido com N itens, listar o dia, sugestoes de descricao,
                 cadeia de hash de auditoria (SHA-256), autorizacao de leitura por
                 sessao/armazem (`autorizar_leitura`) alem da de escrita.
  errors.rs      AppError (thiserror) - unica fonte de mensagens de erro mostradas
                 na tela; nunca vaza detalhe de SQL para o frontend.

db/         Abertura da conexao SQLite, pragmas (WAL, foreign_keys), aplicacao das
            migrations e seed dos dois armazens (A4/B2).
  backup.rs      backup automatico diario (local e externo) com retencao, e checagem
                 de integridade de um arquivo de backup (ver secao propria abaixo).
  sync.rs        sincronizacao oportunista com o Turso (ver secao propria abaixo).

commands/   Wrappers finos com #[tauri::command]. So extraem o State, chamam
            domain::* e devolvem o Result. Nao tem logica de negocio aqui de proposito
            - é a camada que fala com o frontend via IPC.

state.rs    AppState: uma unica Mutex<Connection> (ver "Por que nao um pool" abaixo).
```

O frontend (`src/`) chama tudo atraves de `src/lib/api.ts`, que envolve
`@tauri-apps/api` `invoke()` com tipos TypeScript espelhando as structs Rust
(`src/types.ts`). As paginas (`src/pages/*.tsx`) nao conhecem `invoke` diretamente.

## Por que nao ha catalogo de produtos

O catalogo fixo (modelo+cor com chave estrangeira obrigatoria) foi tentado e descartado:
a saida real cobre scooters, triciclos, patinetes e pecas — variedade grande demais pra
manter uma lista fechada, e pedidos de veiculo ja tem o detalhe completo em outra
ferramenta da empresa (o numero do pedido). Por isso `movimento_itens.descricao` e texto
livre opcional, com sugestoes de autocompletar vindas dos proprios lancamentos anteriores
(`domain::movimentos::sugestoes_descricao`), nao de uma tabela mantida a parte.

## Por que uma unica conexao (Mutex) em vez de um pool

E uma app desktop de uso local, tipicamente 1-2 conferentes por armazem digitando por
vez — nao um servidor web com centenas de conexoes simultaneas. Um pool (`r2d2` ou
similar) so adicionaria uma dependencia e complexidade sem beneficio real nesse cenario.
Se um dia isso mudar (varios usuarios simultaneos no mesmo PC, por exemplo), trocar por
um pool e uma mudanca isolada em `state.rs`.

## Fechamento do dia: sem biblioteca de PDF

`domain::fechamentos` trava (`status = 'fechado'`) os lancamentos do dia e grava um
resumo (`fechamentos`, hash SHA-256 encadeado sobre os hashes dos movimentos daquele
dia). Nao existe geracao de arquivo PDF pelo backend: a tela de fechamento
(`src/components/FechamentoImpressao.tsx`) e sempre renderizada ao vivo a partir dos
movimentos (ja garantidos imutaveis pelo fechamento) e impressa com `window.print()` do
proprio webview, usando `@media print` para o layout A4. Isso foi uma escolha
deliberada: uma lib de geracao de PDF em Rust (`genpdf`, que usa `printpdf`) traz cerca
de 30 dependencias transitivas, algumas antigas (`time` 0.2, `stdweb`), o oposto do que
se busca com "menor superficie de ataque". Como toda impressora e a maioria dos sistemas
operacionais ja oferecem "salvar como PDF" no proprio dialogo de impressao, isso cobre a
necessidade de exportar sem nenhuma dependencia nova.

## Sincronizacao com Turso e confirmacao de recebimento entre armazens

Cada PC (A4 e B2) continua sendo 100% local e funcional offline — a sincronizacao e um
extra oportunista, nunca uma dependencia pra uso diario. `db::sync` envia os lancamentos
novos (`movimentos.sincronizado_em IS NULL`) para uma tabela consolidada
(`movimentos_consolidados`) num banco [Turso](https://turso.tech) (SQLite gerenciado na
nuvem, free tier), chaveada por `(armazem_codigo, id_origem)` — um upsert idempotente,
seguro se a rede cair no meio do envio. Isso acontece:
- **Automaticamente, em loop de segundo plano** (`lib.rs`, `tauri::async_runtime::spawn`
  dentro de `.setup()`): tenta uma vez na abertura do app e depois a cada 1 minuto,
  pela vida inteira do processo, chamando `db::sync::tentar_sincronizar_uma_vez`. Isto
  **nao depende de sessao/login nem de papel** — roda igual com um conferente logado,
  com ninguem logado (tela de login), ou com um gestor — de proposito: antes, o retry
  periodico so existia no frontend (`Dashboard.tsx`) e era gestor-only, entao um PC
  onde so um conferente trabalha o dia inteiro nunca reenviava nada depois da tentativa
  inicial. `turso.txt` e relido a cada iteracao do loop (nao so uma vez no boot), entao
  configurar sincronizacao numa maquina ja aberta funciona sem reiniciar.
- Sob demanda, pelo botao "Sincronizar agora" no cabecalho (so gestor), que chama o
  comando `sincronizar_agora` — um disparo imediato, complementar ao loop de fundo.
- Logo apos confirmar o recebimento de uma transferencia (ver abaixo), pra fechar o
  ciclo sem esperar o proximo sync automatico.

Uma falha de **conexao total** (sem internet, Turso fora do ar, token expirado) e
tratada igual a uma falha por linha: `db::sync::conectar_turso` isola os passos de
conexao, e se falharem, `enviar_para_turso` devolve `Ok(ResultadoSincronizacao)` com
todo o lote em `falhas` em vez de `Err` — antes disso, uma queda total nunca era
gravada via `marcar_falha_sincronizacao`, e `status_sincronizacao` continuava mostrando
"0 com erro" mesmo depois de dias sem sincronizar de verdade.

**Configurar numa maquina** (uma vez, por PC): crie uma conta em turso.tech, instale a
CLI (`curl -sSfL https://get.tur.so/install.sh | bash`), rode
`turso db create ecoviva-armazem` e `turso db tokens create ecoviva-armazem`, e grave a
URL (`libsql://...`) e o token em duas linhas no arquivo `turso.txt`, na mesma pasta do
banco local (`<diretorio de dados do usuario>/turso.txt`). Sem esse arquivo, a
sincronizacao e pulada silenciosamente — comportamento identico a nao ter configurado.
**O mesmo arquivo `turso.txt` serve pros dois PCs** (A4 e B2 compartilham o mesmo banco
Turso, so o `armazem_codigo` de cada linha muda).

### Confirmacao de recebimento (transferencias entre A4 e B2)

A tela de Montagem registra a saida de peca/scooter montado do galpao com um destino:
"outro armazem" (padrao) ou "outro destino" (ex.: tecnico externo pra conserto de
bateria/modulo/motor — nesse caso o codigo/serie de cada peca vai no campo Observacao
do item, e "pra quem" no campo `contraparte`, sem precisar de coluna nova). Quando o
destino e o outro armazem, `armazem_destino_id` e preenchido sozinho (so existem dois
armazens, sem pergunta extra) e a linha aparece, depois de sincronizada, na tela do
outro armazem como "aguardando confirmacao" — o conferente de la clica "Confirmar
recebimento" (comando `confirmar_recebimento`) e os itens sao copiados automaticamente,
sem redigitar nada.

**Descoberta durante a implementacao**: `transferencia_origem_id INTEGER REFERENCES
movimentos(id)` (no schema desde o inicio, pensado exatamente pra isso) acabou nao
servindo — e uma FK pra uma linha *na mesma tabela local*, e o envio original vive no
banco de **outro PC**; o id local de um PC nao tem relacao com o id local do outro, e o
FK (com `foreign_keys=ON`) rejeitaria o insert. A coluna fica no schema sem uso (nao foi
removida — pode servir pra um caso same-DB futuro). Em vez dela, `movimentos` ganhou
`recebido_de_armazem_codigo`/`recebido_de_id_origem` (migration `0004_transferencias.sql`,
sem FK), guardando a mesma chave composta `(armazem_codigo, id_origem)` que ja identifica
uma linha em `movimentos_consolidados`.

A tabela remota tambem precisou crescer depois de ja estar em uso com dados reais —
`db::sync::SQL_ALTER_TABELA_REMOTA` roda `ALTER TABLE ... ADD COLUMN` a cada sincronizacao
(erro ignorado de proposito: a unica forma de falhar e a coluna ja existir). A consulta de
"o que esta pendente pra mim" (`buscar_pendentes_recebimento`) exclui tanto o que ja foi
confirmado quanto o que foi estornado do lado de quem enviou (dois `NOT EXISTS`), e
`confirmar_recebimento` busca a transferencia de novo no Turso pela chave — nunca confia
nos itens que o frontend mandar de volta — e confere que ela estava mesmo endereçada ao
armazem de quem esta confirmando antes de aceitar.

### Recusa de recebimento (v2.1.2)

Alem de "Confirmar recebimento", `<TransferenciasChegando>` tambem oferece "Recusar
recebimento" (peca errada, avariada, etc.), com justificativa obrigatoria. Desenhada
pra reaproveitar ao maximo o mecanismo de confirmacao existente, sem tabela nova nem
mudanca de schema:

- `domain::movimentos::recusar_recebimento` grava uma entrada normal via
  `criar_movimento` (mesma cadeia de hash, checagem de dia fechado, autorizacao — ao
  contrario do estorno, que bypassa a trava de dia fechado de proposito por corrigir o
  passado, uma recusa e um evento novo acontecendo hoje, entao nao ganha esse passe
  livre), marcada com `motivo = MOTIVO_RECUSA_RECEBIMENTO` ("recusado") — um sentinela
  interno, nao um motivo de SAC de verdade. So faz sentido junto com
  `recebido_de_armazem_codigo` preenchido, que ja pula a validacao de motivo do SAC
  (mesma regra que ja existia pra `confirmar_recebimento`), entao nunca colide com um
  motivo real. A justificativa fica em `observacoes`, mesmo padrao de
  `estornar_movimento`.
- `SQL_PENDENTES_RECEBIMENTO` **nao precisou mudar**: o `NOT EXISTS` que ja existia
  (`recebido_de_armazem_codigo`/`recebido_de_id_origem`) nao olha pro `status`/`motivo`,
  entao uma recusa ja some da lista de pendentes de quem recebeu de graca.
- `db::sync::SQL_MINHAS_TRANSFERENCIAS_RECUSADAS`/`buscar_minhas_transferencias_recusadas`
  e o espelho pro lado de quem enviou: busca `movimentos_consolidados` onde
  `recebido_de_armazem_codigo` = meu codigo, `motivo = 'recusado'`, e eu ainda nao
  estornei o lancamento original (mesmo `NOT EXISTS` de `estornado_de` de
  `SQL_PENDENTES_RECEBIMENTO`) — sem essa ultima parte, o aviso nunca sumiria mesmo
  depois de corrigido. `<TransferenciasRecusadas>` (espelho de
  `<TransferenciasChegando>`, mas nas telas de quem envia) mostra isso; a acao e usar o
  botao **Estornar que ja existe** no lancamento original (ja exige justificativa, ja
  auditado) — nao ha um botao "corrigir" dedicado, de proposito, pra nao duplicar
  logica de correcao ja existente.
- `situacaoInfo` (`src/lib/situacao.ts`) ganhou um badge "RECUSADO" (`motivo ===
  'recusado' && recebido_de_armazem_codigo`), mesma cor de aviso de `.badge-parcial` —
  aparece na propria lista de lancamentos do dia de quem recusou, ja que a entrada de
  recusa e um lancamento normal como qualquer outro.

### Bug do `numero_pedido`/`observacoes` perdidos (v2.1.1/v2.1.2)

`db::sync::ler_config_turso`/`movimentos_pendentes`/`marcar_sincronizado` e a logica pura
de parsing (`linha_para_transferencia`) sao cobertas por teste automatizado. As strings
SQL usadas contra o Turso (`SQL_UPSERT`, `SQL_PENDENTES_RECEBIMENTO`,
`SQL_BUSCAR_TRANSFERENCIA_POR_CHAVE`) tambem sao testadas — SQLite generico, nada
especifico de libsql, entao rodam contra um `rusqlite` em memoria dentro de
`db::sync::tests` sem precisar de conta Turso real. Foi assim que se pegou dois bugs
reais da mesma classe: `numero_pedido` (v2.1.1) e depois `observacoes` do movimento
(v2.1.2) — os dois eram gravados certinho no envio, mas `SQL_PENDENTES_RECEBIMENTO`/
`SQL_BUSCAR_TRANSFERENCIA_POR_CHAVE` esqueciam de selecionar a coluna, entao quem
recebia nunca via o dado (nem no `numero_pedido` do pedido, nem nas instrucoes
importantes que quem envia as vezes escreve em Observacoes) — so um teste que roda o
SQL de verdade pega esse tipo de erro. `confirmar_recebimento` tambem passou a anexar
a observacao original na entrada confirmada (`"Recebido de X. Observacao de quem
enviou: ..."`), pra ela sobreviver no historico de quem recebeu, nao so ficar visivel
enquanto a transferencia estava pendente. O que so pode ser validado de ponta a ponta e a camada de
rede/autenticacao em si (`Builder::new_remote`, HTTP, credenciais) — `tests/
sync_turso_real_test.rs` cobre isso, `#[ignore]` por padrao (a CI nunca toca rede),
rodado manualmente contra um banco Turso descartavel:

```bash
turso db create ecoviva-armazem-teste          # uma vez so
turso db tokens create ecoviva-armazem-teste    # gera um token novo quando precisar

TURSO_TESTE_URL=libsql://... TURSO_TESTE_TOKEN=... \
  cargo test --test sync_turso_real_test -- --ignored
```

**Nunca aponte isso pro banco de producao** (`ecoviva-armazem`) — o teste escreve dados
fabricados. Use sempre um banco `-teste` separado, criado com o mesmo `turso db create`.
Antes da v2.1.1, esse passo so dava pra fazer manualmente simulando os dois PCs contra
o Turso real (envio de B2, confirmacao em A4, estorno em B2 sumindo do pendente em A4) —
agora e um teste repetivel, e serve de base pra validar qualquer feature nova de sync
(ex.: recusa de recebimento) sem arriscar dado de teste no Turso de producao.

### Fila de sincronizacao com retry (Sprint 7)

O envio pro Turso era "melhor esforco, uma tentativa so" — uma falha por linha era
simplesmente ignorada (nem registrada), so seria tentada de novo na proxima chamada
inteira de `enviar_para_turso`. `movimentos` ganhou `sync_tentativas`, `sync_erro`,
`sync_proxima_tentativa` (migration `0005_sync_retry.sql`); `enviar_para_turso` agora
devolve `ResultadoSincronizacao { enviados, falhas }` em vez de so uma lista de
sucesso, e cada falha e registrada via `marcar_falha_sincronizacao` com um backoff
progressivo (`calcular_backoff_minutos`: 1/5/15/30 min, fixo em 60 min a partir da 5a
tentativa) — a linha some de `movimentos_pendentes` ate esse horario passar, pra nao
martelar o Turso repetidamente numa falha persistente. Quem re-tenta sozinho a cada 5
minutos e o loop de backend descrito acima (`lib.rs`), independente de quem esta
logado; `Dashboard.tsx` so re-le esse retrato local (`status_sincronizacao`,
gestor-only) no mesmo intervalo, sem disparar rede — ver "Versao 2.0.0" no
`docs/ROADMAP.md` pra por que isso mudou (retry preso a uma sessao de gestor era a
causa raiz de transferencias nao chegarem no outro armazem).

### Divergencia de quantidade na confirmacao de recebimento (Sprint 7)

`confirmar_recebimento` copiava os itens da transferencia 1:1 — nao havia como o
conferente que recebe registrar que chegou menos do que foi enviado.
`movimento_itens` ganhou `quantidade_enviada` (migration
`0006_divergencia_recebimento.sql`, coberta pela cadeia de hash via `ItemHash`). Na
tela de Montagem, cada item de uma transferencia pendente mostra um campo editavel
pre-preenchido com a quantidade enviada; `domain::movimentos::validar_quantidades_recebidas`
(pura, testada) rejeita receber mais do que foi enviado — nunca confia no frontend pra
isso, mesma logica ja usada pra `armazem_destino_codigo` — mas aceita receber menos
(divergencia legitima), gravando os dois valores (`quantidade` = recebido,
`quantidade_enviada` = enviado) pra auditoria e pro painel.

### Painel web somente-leitura (Sprint 7)

`painel/index.html` e um site estatico de arquivo unico (sem build step, JS puro) que
le `movimentos_consolidados` direto do Turso via o **HTTP Pipeline API**
(`POST {url}/v2/pipeline`, `Authorization: Bearer <token>` — o mesmo protocolo que o
`libsql` do backend usa, so que aqui e um `fetch()` puro, sem client library). O token
embutido no site e gerado com `turso db tokens create <db> --read-only` — testado que
o escopo e reforcado pelo *servidor*, nao e so uma convencao do cliente: uma tentativa
de `INSERT` com esse token volta `{"error":{"code":"BLOCKED", ...}}`. Isso importa
porque o painel fica **publico** (GitHub Pages nao restringe por colaborador do repo
no plano gratuito) — a unica protecao real e essa (o gate de senha no `sessionStorage`,
com hash SHA-256 no arquivo em vez da senha em texto puro, so afasta acesso casual;
qualquer um que veja o codigo-fonte pode tentar forcar o hash offline).

Publicado via `.github/workflows/deploy-painel.yml`, que dispara em todo push a `main`
que toque `painel/**` — a URL e o token **nunca ficam commitados**: entram no HTML so
no momento do deploy, substituindo os placeholders `__TURSO_PAINEL_URL__`/
`__TURSO_PAINEL_TOKEN__` a partir de secrets do repo (`TURSO_PAINEL_URL`/
`TURSO_PAINEL_TOKEN`). **GitHub Pages exige repo publico no plano gratuito** — repos
privados precisam de GitHub Pro; o repo foi tornado publico especificamente por isso
(confirmado antes que nenhum segredo jamais foi commitado — `turso.txt` sempre viveu
fora do repo, no diretorio de dados do usuario).

## Migrations

Arquivos SQL numerados em `src-tauri/migrations/`, aplicados por `rusqlite_migration` a
cada abertura do banco (`db::abrir`). Para mudar o schema, **nunca edite uma migration ja
existente** — crie um novo arquivo `000N_descricao.sql` com o proximo numero.

## Backup automatico (local e externo) e restauracao

`db::backup::backup_automatico` roda uma vez a cada abertura real do app (dentro do
`.setup()` em `lib.rs`, logo apos `db::abrir`) e grava uma copia do banco em
`<diretorio de dados do usuario>/backups/ecoviva-armazem-<AAAA-MM-DD>.db` — um arquivo
por dia (roda de novo no mesmo dia so sobrescreve), mantendo os ultimos 14 dias. Usa a
Online Backup API do SQLite (`Connection::backup`, feature `backup` do `rusqlite`), que
lida corretamente com o modo WAL — diferente de simplesmente copiar o arquivo `.db` no
sistema de arquivos, que poderia perder escritas ainda so no `-wal`. Falha no backup
**nao impede o app de abrir**, so grava um aviso no log.

**Backup externo (pendrive/HD)**: se existir um arquivo `backup_externo.txt` na pasta de
dados (uma linha, com o caminho de destino — ex.: `D:\EcovivaBackups`), o app tambem
grava a copia do dia la, logo apos o backup local, com a mesma retencao de 14 dias
(`db::backup::backup_externo`). Sem esse arquivo, ou com a unidade desconectada no
momento, esse passo e pulado silenciosamente — o backup local continua acontecendo
normalmente de qualquer forma. Configurar isso e manual (crie o arquivo apontando pro
caminho da unidade removivel no PC).

**Para restaurar um backup** (recuperar de um problema, ou trocar de computador):
1. Feche o app completamente.
2. Localize a pasta de dados (no Windows, normalmente
   `%APPDATA%\com.ecoviva.controlearmazem\`) e a subpasta `backups/` dentro dela (ou a
   pasta configurada em `backup_externo.txt`, se for restaurar a partir do backup
   externo).
3. Antes de sobrescrever qualquer coisa, valide o arquivo escolhido com
   `db::backup::verificar_backup_valido` — confere que as tabelas essenciais existem e
   que `PRAGMA integrity_check` esta limpo, pra nao trocar um banco bom por um
   corrompido.
4. Copie o arquivo do dia desejado por cima de `ecoviva-armazem.db` (no mesmo
   diretorio, um nivel acima de `backups/`).
5. Abra o app normalmente.

Esse ciclo completo (gravar backup, apagar o banco original, copiar o backup por cima,
reabrir) e coberto por um teste de integracao automatizado
(`db::backup::tests::restaura_backup_e_dados_e_cadeia_de_hash_sobrevivem`) que confere
nao so que os dados voltam, mas que a cadeia de hash de auditoria
(`domain::movimentos::verificar_cadeia`) continua intacta depois da restauracao.

## Rodando localmente

```bash
npm install
npm run dev          # tauri dev - abre a janela com hot-reload do frontend
```

```bash
cd src-tauri
cargo test                                          # testes de dominio + integracao
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt                                           # ou --check no CI
```

## Adicionando um novo comando

1. Escreva a funcao de dominio em `domain/*.rs` (com testes no mesmo arquivo, modulo
   `#[cfg(test)] mod tests`).
2. Exponha um wrapper fino em `commands/*.rs` com `#[tauri::command]`.
3. Registre em `tauri::generate_handler![...]` dentro de `lib.rs`.
4. Adicione a chamada tipada em `src/lib/api.ts` e os tipos correspondentes em
   `src/types.ts`.

## CI

`.github/workflows/ci.yml` roda em `ubuntu-latest` e `windows-latest` (o alvo real de
distribuicao e Windows): `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`,
`tsc --noEmit` e o build do frontend. So executa de fato quando o repositorio tiver um
remote no GitHub.
