# AGENTS.md

This file is an **immunity system**. Every rule below exists because an agent
once made this mistake. When you find a new failure mode, add a rule here so
it never happens again.

## Rust / Cargo

- After modifying any `.rs` file, always run `cargo check` before declaring
  done. TypeScript compilation alone does not catch Rust errors.
- In a dirty worktree, do not run broad `cargo fmt` for a narrow Rust change.
  Format only the Rust files touched by the task so unrelated user WIP is not
  rewritten.
- Structured errors must cross the backend from the inside out. New or modified
  service/internal code should return `Result<T, AppError>` from
  `src-tauri/src/error.rs`; Tauri commands return `Result<T, AppError>` directly
  — `AppError` implements `Serialize` and is sent as a structured JSON object
  `{code, message, hint, category}` to the frontend. Do **not** use
  `.map_err(String::from)` at the command boundary. Database drivers are
  still migrating, so do not introduce new string-tag protocols like
  `[VALIDATION_ERROR]`, `[UNSUPPORTED]`, or `[ERR-1001]`; when touching driver
  code, migrate toward `DbError` or `AppError` instead of parsing strings.
- When adding a new database driver, do **three** things — not two, not four:
  1. Add the module in `src-tauri/src/db/drivers/<name>.rs`
  2. Add the `pub mod <name>;` declaration in `src-tauri/src/db/drivers/mod.rs`
  3. Register it in the `connect()` match arms and imports in
     `src-tauri/src/db/drivers/registry.rs`
- When a driver's type is MySQL-family (mariadb, tidb, starrocks, doris),
  update `is_mysql_family_driver()` in `src-tauri/src/db/drivers/registry.rs`
  if the new driver belongs there.
- Oracle tests require the Oracle Instant Client installed locally. The
  `scripts/test-integration.sh` script detects this via `DYLD_LIBRARY_PATH` and
  common paths. Integration tests for Oracle will be **skipped** if the client
  is missing — do not try to "fix" the test to run without it.
- Error messages for connection failures **must** use `conn_failed_error()` in
  `src-tauri/src/db/drivers/mod.rs`. This provides context-aware hints (TLS,
  auth, network). Raw error strings confuse users.
- Integration tests are marked `#[ignore]` and require `IT_DB=<name>` env
  variable to run. They also need Docker. Do not remove `#[ignore]` to "make
  tests pass" in CI.

- SQLite 迁移必须基于真实 schema 校验（表/列是否存在），不能只信 `schema_migrations` 记录。
  2026-08 旧代码把无记录 + 有 connections 表的库一次性标记为"全部已应用"却未执行，
  导致用户库缺 `auth_source` 列、`redis_command_logs` 表，启动即报 "no such column"。
  迁移判断逻辑统一放 `src-tauri/src/db/migrations.rs::migration_applied`，新增迁移时同步加检查分支。

## Frontend / TypeScript

- `src/services/api/core.ts` is the **only** file that calls `invoke()`. Never
  call `@tauri-apps/api/core` invoke directly from components or other services.
  Domain wrappers live in `src/services/api/{query,metadata,redis,...}.ts`.
- `invoke()` in `src/services/api/core.ts` has a single typed overload
  constrained by `CommandMap`. Never write `invoke<ReturnType>(...)` — the
  return type is inferred from the command name. Explicit generics bypass
  type checking by falling through to an unconstrained overload.
- When a Tauri command's parameter types change, update **both**:
  - The Rust `#[tauri::command]` signature
  - The corresponding TypeScript wrapper in `src/services/api/`
- Mock mode (`VITE_USE_MOCK=true`) is for rapid UI iteration. The mock
  implementations live in `src/services/mocks/`. When adding a new API
  method to `api/`, always add a corresponding mock entry.
- i18n locale files are TypeScript, not JSON. After adding a new locale file
  in `src/lib/i18n/locales/`, register it in `src/lib/i18n/index.ts`.
- Until the i18n entry point composes the split domain locale modules, update
  translation keys in both `locales/{en,zh}.ts` and the matching
  `locales/{en,zh}/<domain>.ts` files. The app currently imports the monolithic
  files, so changing only a split domain file has no runtime effect.
- The keyboard-shortcut registry lives in `src/lib/shortcuts/defaults.ts`.
  The `ShortcutsProvider` in `src/contexts/ShortcutsContext.tsx` is the
  ONLY component allowed to call `getSetting`/`saveSetting` for the
  `shortcuts.v1` store key (mirrors the api.ts-invokes-Tauri rule above).
  Components must read bindings via `useShortcutBinding(id)`,
  `useShortcutBindings()`, or `useShortcutMatcher()`, never directly
  from the store. When adding a new shortcut, add an entry to
  `SHORTCUT_DEFAULTS`, a label under `settings.shortcuts.label.*` in
  every locale, and wire the matcher call in the appropriate handler
  site — do not hard-code the key.
- Icon-only buttons that trigger user actions must have an accessible name
  (`aria-label`, and usually matching `title`) from i18n text. Without this,
  Playwright flow tests have to click by position and can miss the exact
  "button exists but crashes when clicked" regression they are meant to catch.
- Effects that set React state must not depend on unstable callback identities
  unless the callback is deliberately memoized. For "read latest callback but
  do not re-run" cases, store the callback in a ref. When updating `Set`/`Map`
  state inside effects, return the previous object if the key/value is already
  present so a no-op does not trigger a render loop.

## Database Drivers

- Every driver implements the `DatabaseDriver` trait re-exported by
  `src-tauri/src/db/drivers/mod.rs`.
  The trait has required methods (`connect`, `list_databases`, `list_tables`,
  `get_table_structure`, `get_table_metadata`, `get_table_ddl`,
  `get_table_data`, `get_table_data_chunk`, `execute_query`,
  `get_schema_overview`, `close`) and optional ones (`list_routines`,
  `get_routine_ddl`, `execute_query_with_id`).
- Do not add driver-specific connection logic outside the driver module.
  SSH tunneling is handled transparently in the connection layer, not in
  individual drivers.
- SQL statement splitting lives in `src-tauri/src/db/sql/splitter.rs`
  (`split_sql_statements`, `first_sql_keyword`). `src-tauri/src/db/drivers/mod.rs`
  only re-exports splitter helpers as a compatibility layer. Do not reimplement
  SQL parsing in individual drivers.
- `TableInfo.type` is a logical object category consumed by sidebar grouping,
  not a database-specific storage engine. Drivers must return the categories
  expected by the tree (`table`/`BASE TABLE` and the driver's view categories);
  expose engine details through table metadata instead.

## Testing

- Rust integration tests follow three levels:
  - `<db>_integration.rs` — direct driver method testing
  - `<db>_command_integration.rs` — ephemeral connection commands
  - `<db>_stateful_command_integration.rs` — saved connection workflows
  All three must be added to `scripts/test-integration.sh` when adding a new
  database.
- Integration tests run with `--test-threads=1`. Do not change this —
  database containers are shared and parallel tests collide.
- Use `IT_REUSE_LOCAL_DB=1` to skip container creation during local
  development iterations.

## Release / Packaging

- The GitHub release workflow's macOS updater path only requires the signed
  `.app.tar.gz` updater bundle. Do not let optional DMG generation block CI
  unless the release intentionally needs a DMG asset; use `tauri build
  --bundles app` for macOS updater builds.

## General

- `CLAUDE.md` is a **table of contents** — it points to deeper docs but does
  not duplicate them. If you need architecture details, read `docs/architecture.md`.
  If you need commands, read `docs/commands.md`.
- Do not add instructions to `CLAUDE.md` that apply only in a specific
  context. Add them here in `AGENTS.md` instead, or in a skill file under
  `.claude/skills/`.
- When you encounter a new failure mode during this session, add an entry to
  this file. The file grows as the project's institutional knowledge grows.

## Hot Files Governance

The following files are modification hotspots (most commits in last 30 days).
Before modifying them, check line count. If over threshold, suggest extracting
logic before adding more.

| File | Threshold | Extraction strategy |
|------|-----------|---------------------|
| `src/components/business/DataGrid/TableView.tsx` | 500 lines | Extract to hooks in `tableView/hooks/` |
| `src/components/business/Sidebar/ConnectionList.tsx` | 500 lines | Split tree state, dialog state, action handlers |
| `src-tauri/src/commands/query.rs` | 600 lines | Extract helpers to `db/` or `services/` |
| `src-tauri/src/commands/metadata.rs` | 600 lines | Extract helpers to `db/` or `services/` |
| `src-tauri/src/lib.rs` | 500 lines | Move command registration to module-level helpers |
| `src/lib/i18n/locales/*.ts` | 800 lines | Split by domain, re-export from index |

**Rules:**
1. When modifying a hot file, first check `wc -l`. If over threshold, inform the
   user and suggest extraction targets before proceeding.
2. Do not add new top-level functions or handlers to hot files. Extract to
   hooks, helpers, or service modules first, then import.
3. For `TableView.tsx`: only orchestration and layout belong here. State logic,
   event handlers, and data transformations go in `tableView/hooks/`.
4. For `ConnectionList.tsx`: split into `useTreeState`, `useDialogState`,
   `useConnectionActions` hooks.
5. For `lib.rs`: prefer `commands::<module>::register(builder)` pattern over
   listing every command inline.
