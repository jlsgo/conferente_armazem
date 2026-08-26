# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Offline-first desktop app (Tauri v2 + Rust + SQLite, React/TypeScript frontend) that
replaces the paper/Excel warehouse in/out control sheets used by Ecoviva's conferentes
at warehouses A4 and B2. See `docs/ARQUITETURA.md` (Portuguese) for the full
architecture write-up and the reasoning behind key decisions — read it before making
structural changes; this file only summarizes what's needed to work day to day.

## Commands

Frontend (run from repo root):
- `npm install` — install JS deps
- `npm run dev` — `tauri dev`, opens the desktop window with hot-reload
- `npm run build:renderer` — `vite build` (frontend only, outputs to `dist/`)
- `npx tsc --noEmit` — type-check the frontend
- `npm run dist` — `tauri build`, produces the platform installer

Backend (run from `src-tauri/`):
- `cargo test` — unit tests colocated with the code (`domain/*.rs`, `#[cfg(test)] mod
  tests`) plus the end-to-end test in `tests/movimentos_test.rs`
- `cargo test <substring>` — run a single test, e.g. `cargo test rejeita_login_com_senha_errada`
- `cargo clippy --all-targets --all-features -- -D warnings` — lint (warnings fail)
- `cargo fmt` / `cargo fmt --check` — format / verify formatting
- `cargo check` — fast type-check without a full build

CI (`.github/workflows/ci.yml`) runs all of the above (fmt check, clippy, test, `tsc
--noEmit`, frontend build) on `ubuntu-latest` and `windows-latest` for every push/PR to
`main`. Windows is the real deployment target; Linux is what this dev environment can
actually build and run for local testing.

## Architecture

- **`src-tauri/src/domain/`** holds all business logic and validation as plain functions
  over `&rusqlite::Connection` — no Tauri types involved, so it's tested directly with an
  in-memory SQLite DB (`db::abrir_em_memoria()`), no mocks needed. `auth.rs` (Argon2
  hashing, login) and `movimentos.rs` (create/list a "pedido" with N items, audit hash
  chain, description autocomplete) are the two modules.
- **`src-tauri/src/commands/`** are thin `#[tauri::command]` wrappers: pull `State<AppState>`,
  call into `domain::*`, return `Result<T, AppError>`. No business logic belongs here.
- **`src-tauri/src/state.rs`**: `AppState` holds a single `Mutex<Connection>` — deliberately
  not a connection pool (r2d2/deadpool). This is a local single/dual-user desktop app, not
  a server; a pool would be unneeded complexity. Revisit only if real write contention
  shows up.
- **`src-tauri/src/db/mod.rs`**: opens the SQLite file in the OS app-data dir, sets
  `journal_mode=WAL` and `foreign_keys=ON`, runs migrations, and seeds the two fixed
  warehouses (`A4`, `B2`) if empty. There is **no user seeding** — first run always goes
  through the Setup screen (`App.tsx` checks `precisa_configurar_primeiro_usuario` from
  the `get_status` command).
- **Migrations** live in `src-tauri/migrations/*.sql`, numbered, applied via
  `rusqlite_migration`. Never edit an already-committed migration — add a new numbered
  file for schema changes.
- **No product catalog.** `movimento_itens` has a fixed-set `categoria`
  (`scooter`/`triciclo`/`patinete`/`peca`) plus a free-text, optional `descricao`.
  Autocomplete suggestions come from `domain::movimentos::sugestoes_descricao`, which
  queries distinct past `descricao` values for that category — not a maintained lookup
  table. This was a deliberate simplification after the client clarified the product
  variety (scooters/tricycles/patinetes/parts) was too large for a fixed catalog, and
  that vehicle orders already have full detail in an external tool, referenced here by
  `numero_pedido`.
- **Three `fluxo` values exist in the schema** (`saida_armazem`, `peca_montagem`, `sac`),
  matching the three paper control sheets the client actually uses, each with its own
  screen: `src/pages/Lancamentos.tsx` (vehicles), `src/pages/Montagem.tsx` (loose parts
  released from warehouse B2 to assembly at A4 — condition boa/defeito/sucata required
  per item, validated in `domain::movimentos::validar_novo_movimento`), and
  `src/pages/Sac.tsx` (warranty/sale part returns — `motivo` required
  garantia/venda, `valor_centavos` required only when venda, also validated
  domain-side, not just in the form). All three share `commands/movimento_commands.rs`
  and `commands/fechamento_commands.rs` — the domain layer was already generic per
  `fluxo` before these screens existed.
- **Cross-warehouse transfer + receipt confirmation (A4 ↔ B2)**: implemented, and generic
  across `fluxo` — a `saida` with `armazem_destino_id` set (either `saida_armazem` for
  vehicles, from `Lancamentos.tsx`, or `peca_montagem` for loose parts, from
  `Montagem.tsx`) is a pending transfer; each screen's `<TransferenciasChegando>`
  component (`src/components/TransferenciasChegando.tsx`) polls
  `buscar_transferencias_pendentes`/`confirmar_recebimento` and filters the result to its
  own `fluxo` client-side, so a vehicle transfer is confirmed from Saida de Armazem and a
  part transfer from Montagem. `transferencia_origem_id` (in the schema since the start)
  turned out not to fit: it's an FK to a row in the *local* table, but the original send
  lives on the other armazem's PC. The real mechanism uses `recebido_de_armazem_codigo`/
  `recebido_de_id_origem` (migration `0004_transferencias.sql`, no FK — a composite key
  into `movimentos_consolidados` on Turso) plus `db::sync::TransferenciaPendente.fluxo`
  so `confirmar_recebimento` records the confirmation under the *same* fluxo as the
  original send instead of a hardcoded one. `validar_quantidades_recebidas` rejects
  receiving more than was sent (never trusts the frontend), accepts less as a legitimate
  divergence. This depends on the Turso sync described below — see `docs/ARQUITETURA.md`.
- **No inventory/stock balance tracking.** This system is a movement log/audit trail
  (who did what, when, how many), not a stock-level system — confirmed explicitly with
  the client. Don't add "available stock" validation or reporting without re-confirming
  scope.
- **Audit trail**: every `movimentos` row stores `hash_integridade`, a SHA-256 chained
  over the previous row's hash plus every field of this row, including item-level
  fields (`domain::movimentos::calcular_hash`/`CamposHash`) — a direct `UPDATE` on any
  covered column breaks the chain. `domain::movimentos::verificar_cadeia` walks the
  table and returns the first row whose stored hash no longer matches; it's
  domain-only today (no Tauri command/UI), covered by unit tests.
- **Session-based authorization**: `AppState.sessao` (set by the `login` command,
  cleared by `logout`) is the only source of "who is doing this" for
  `criar_movimento`, `fechar_dia`, `criar_usuario` and `estornar_movimento` — none of
  them accept a `usuario_id`/`solicitante_id` from the JS payload anymore. Every
  write also re-fetches the user from the DB (`auth::buscar_usuario_ativo`) and
  checks `ativo` plus (when the user has a fixed `armazem_id`) that it matches the
  armazem being written to; `fechar_dia` additionally requires `papel = 'gestor'`.
- **Correction after closing**: `domain::movimentos::estornar_movimento` appends a
  new row (`status = 'estorno'`, `estornado_de` pointing at the original) instead of
  editing anything — it deliberately bypasses the "day is closed" guard, since that's
  the only way to correct a mistake found after closing. Requires `papel = 'gestor'`
  and a non-empty justification. `fechamentos::buscar_fechamento` computes
  `total_estornado`/`total_liquido` live from current data — the stored `fechamentos`
  row itself is never rewritten.
- **Errors**: `domain::errors::AppError` (thiserror) has a custom `Serialize` impl that
  turns every variant into the plain Portuguese string shown directly in the UI. Keep
  error messages user-safe — never let raw `rusqlite::Error`/SQL text reach a variant's
  `Display` output.
- **Frontend ↔ backend**: `src/lib/api.ts` wraps `@tauri-apps/api`'s `invoke()`, matching
  the Rust command names and payload shapes (JSON field names are snake_case, mirroring
  the Rust structs directly — no camelCase conversion in the payloads). Tauri commands
  with more than one top-level scalar argument (e.g. `listar_movimentos_do_dia`) are
  declared with `#[tauri::command(rename_all = "snake_case")]` — without it, Tauri
  expects camelCase argument keys from JS by default, which would silently break calls
  built with the snake_case convention used everywhere else in this codebase.
- **Security**: `src-tauri/capabilities/default.json` only grants `core:default` — no
  `fs`, `shell`, `http`, or `dialog` plugin permissions. Adding a new Tauri plugin
  requires an explicit capability entry, not just a `Cargo.toml` dependency.
