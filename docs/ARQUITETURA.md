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
                 cadeia de hash de auditoria (SHA-256).
  errors.rs      AppError (thiserror) - unica fonte de mensagens de erro mostradas
                 na tela; nunca vaza detalhe de SQL para o frontend.

db/         Abertura da conexao SQLite, pragmas (WAL, foreign_keys), aplicacao das
            migrations e seed dos dois armazens (A4/B2).

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

## Preparado para sincronizacao futura, sem implementa-la agora

`movimentos` ja tem `armazem_destino_id` e `transferencia_origem_id` (ambos opcionais).
Eles existem para permitir, no futuro, um fluxo de "check-in" entre armazens (ex.: B2
libera uma peca, A4 confirma o recebimento quando ela chega) sem precisar de uma
migration destrutiva depois — mas nenhuma logica de confirmacao ou sincronizacao entre
computadores esta implementada ainda. Hoje o sistema e 100% local a cada PC.

## Migrations

Arquivos SQL numerados em `src-tauri/migrations/`, aplicados por `rusqlite_migration` a
cada abertura do banco (`db::abrir`). Para mudar o schema, **nunca edite uma migration ja
existente** — crie um novo arquivo `000N_descricao.sql` com o proximo numero.

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
