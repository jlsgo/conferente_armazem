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
  matching the three paper control sheets the client actually uses. Only
  `saida_armazem` has a UI screen so far (`src/pages/Lancamentos.tsx`); `peca_montagem`
  (parts released from warehouse B2 to assembly at A4) and `sac` (warranty/sale part
  returns) are backend-ready but have no frontend yet.
- **Forward-compat for a future cross-warehouse check-in**: `movimentos` has
  `armazem_destino_id` and `transferencia_origem_id` (both nullable, unused today). These
  exist so that a future "confirm receipt at the destination warehouse" flow (to prevent
  loss/theft of parts in transit between B2 and A4) doesn't require a destructive
  migration — no confirmation logic is implemented yet, and it depends on cross-PC sync
  that doesn't exist yet either (this app is 100% local per machine right now).
- **No inventory/stock balance tracking.** This system is a movement log/audit trail
  (who did what, when, how many), not a stock-level system — confirmed explicitly with
  the client. Don't add "available stock" validation or reporting without re-confirming
  scope.
- **Audit trail**: every `movimentos` row stores `hash_integridade`, a SHA-256 chained
  over the previous row's hash plus this row's essential fields
  (`domain::movimentos::calcular_hash`). Nothing currently verifies the chain or exposes
  it in the UI — it's there for a future tamper-detection/closing-the-day feature.
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
