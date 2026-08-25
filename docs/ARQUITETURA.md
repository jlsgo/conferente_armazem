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

## Sincronizacao com Turso (v1: envio unidirecional)

Cada PC (A4 e B2) continua sendo 100% local e funcional offline — a sincronizacao e um
extra oportunista, nunca uma dependencia pra uso diario. `db::sync` envia os lancamentos
novos (`movimentos.sincronizado_em IS NULL`) para uma tabela consolidada
(`movimentos_consolidados`) num banco [Turso](https://turso.tech) (SQLite gerenciado na
nuvem, free tier), chaveada por `(armazem_codigo, id_origem)` — um upsert idempotente,
seguro se a rede cair no meio do envio. Isso acontece:
- Automaticamente, em segundo plano, toda vez que o app abre (`.setup()` em `lib.rs`) —
  falha (sem internet, por exemplo) so grava um aviso no log, nunca trava o app.
- Sob demanda, pelo botao "Sincronizar agora" no cabecalho (so gestor), que chama o
  comando `sincronizar_agora`.

**Configurar numa maquina** (uma vez, por PC): crie uma conta em turso.tech, instale a
CLI (`curl -sSfL https://get.tur.so/install.sh | bash`), rode
`turso db create ecoviva-armazem` e `turso db tokens create ecoviva-armazem`, e grave a
URL (`libsql://...`) e o token em duas linhas no arquivo `turso.txt`, na mesma pasta do
banco local (`<diretorio de dados do usuario>/turso.txt`). Sem esse arquivo, a
sincronizacao e pulada silenciosamente — comportamento identico a nao ter configurado.

**O que esta fora do escopo desta v1** (proxima fatia, nao implementada ainda): a
confirmacao de recebimento entre armazens (ex.: B2 libera uma peca, A4 confirma quando
ela chega) usando `movimentos.armazem_destino_id`/`transferencia_origem_id` (ambos ja no
schema, sem logica ainda) e um painel consolidado de leitura sobre
`movimentos_consolidados`. Esta v1 so da visibilidade cruzada (a tabela remota) e a base
de envio sobre a qual essa confirmacao sera construida.

`db::sync::ler_config_turso`/`movimentos_pendentes`/`marcar_sincronizado` sao puramente
locais e cobertos por teste automatizado. O passo de rede (`enviar_para_turso`) so pode
ser validado de ponta a ponta com uma conta/banco Turso real configurada — nao ha como
testar isso automaticamente sem credenciais de verdade.

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
