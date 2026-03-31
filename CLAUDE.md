# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

DbPaw is a cross-platform database client built with Tauri v2, supporting PostgreSQL, MySQL, MariaDB, TiDB, SQLite, SQL Server, ClickHouse, and DuckDB. The architecture separates frontend (React/TypeScript) from backend (Rust) with communication via Tauri commands.

## Essential Commands

### Development
- `bun install` - Install dependencies
- `bun dev:mock` - Frontend-only development with mock data (recommended for UI work)
- `bun tauri dev` - Full Tauri app with Rust backend (for end-to-end testing)
- `bun tauri build` - Production build

### Testing
For comprehensive testing documentation, see [TESTING.md](TESTING.md).

Quick reference:
- `bun run test:all` - Run all tests (unit, service, Rust, integration)
- `bun run test:unit` - Run TypeScript unit tests (files: `*.unit.test.ts`)
- `bun run test:service` - Run service layer tests (files: `*.service.test.ts`)
- `bun run test:rust:unit` - Run Rust unit tests (`cargo test --lib`)
- `bun run test:integration` - Run database integration tests (requires Docker)
  - `IT_DB=mysql bun run test:integration` - Test specific database
  - `IT_DB=all bun run test:integration` - Test all databases
  - `IT_REUSE_LOCAL_DB=1` - Reuse existing local database containers
- `bun run test:smoke` - Quick validation (typecheck, lint, unit tests)
- `bun run test:ci` - Full CI test suite

### Code Quality
- `bun run typecheck` - TypeScript type checking
- `bun run lint` - Lint TypeScript/JSON files with Prettier
- `bun run lint:rust` - Check Rust code (`cargo check`)
- `bun run format` - Format TypeScript files with Prettier

### Website
- `bun run website:dev` - Run Astro marketing site locally
- `bun run website:build` - Build marketing site

## Architecture

### Frontend (React + TypeScript)

**Directory Structure:**
- `src/components/ui/` - Shadcn/UI components (base UI primitives)
- `src/components/business/` - Business logic components:
  - `Editor/` - SQL editor (Monaco/CodeMirror)
  - `DataGrid/` - Query results and table data display
  - `Sidebar/` - Connection/database tree navigation
  - `Metadata/` - Table structure and schema views
  - `SqlLogs/` - Query execution history
- `src/components/settings/` - Settings dialogs
- `src/services/` - Tauri API wrapper and mocks
- `src/lib/` - Utilities (i18n, keyboard shortcuts, validation)
- `src/theme/` - Theme registry and management

**Key Patterns:**
- All Tauri backend calls go through `src/services/api.ts` which provides:
  - Mock mode (`VITE_USE_MOCK=true`) for frontend-only development
  - Type-safe wrappers around Tauri `invoke()` commands
  - Runtime detection (`isTauri()`) to handle non-Tauri environments
- Path alias: `@/` maps to `./src/`
- i18n: Files in `src/lib/i18n/locales/` (en, zh, ja supported)

### Backend (Rust + Tauri)

**Core Modules:**
- `src-tauri/src/commands/` - Tauri command handlers (exposed to frontend):
  - `connection.rs` - Connection CRUD and testing
  - `query.rs` - Query execution and cancellation
  - `metadata.rs` - Schema inspection (tables, structures, DDL)
  - `storage.rs` - Saved queries persistence
  - `ai.rs` - AI provider management and chat
  - `transfer.rs` - Import/export operations
- `src-tauri/src/db/` - Database layer:
  - `drivers/` - Per-database implementations (postgres, mysql, clickhouse, mssql, sqlite, duckdb)
  - `pool_manager.rs` - Connection pooling with bb8
  - `local.rs` - SQLite database for app metadata
- `src-tauri/src/state.rs` - Global app state (local DB + pool manager)
- `src-tauri/src/ssh.rs` - SSH tunnel support
- `src-tauri/src/ai/` - AI provider integration (OpenAI-compatible APIs)
- `src-tauri/src/models/` - Shared data types
- `src-tauri/src/error.rs` - Error handling

**Key Patterns:**
- All database drivers implement `DatabaseDriver` trait (see `src-tauri/src/db/drivers/mod.rs`)
- Connection pooling: Each database connection gets a managed pool via `PoolManager`
- State management: `AppState` holds `local_db` (SQLite) and `pool_manager`
- SSH tunneling: Transparent port forwarding for remote database access
- Error messages: Use `conn_failed_error()` to provide context-aware hints (TLS issues, auth failures, network problems)

### Testing Strategy

DbPaw uses a **3-layer testing approach** (see [TESTING.md](TESTING.md) for full details):

```
Frontend Layer    → Unit tests (*.unit.test.ts) + Service tests (*.service.test.ts)
Tauri Commands    → Command integration tests (*_command_integration.rs)
Database Drivers  → Driver integration tests (*_integration.rs)
```

**Test Coverage by Database:**
- ✅ **MySQL & PostgreSQL**: Complete (driver + command + stateful tests)
- 🟢 **MariaDB, MSSQL, ClickHouse, SQLite, DuckDB**: Driver + command tests (stateful tests pending)

**TypeScript Tests:**
- Unit tests: `*.unit.test.ts` - Pure logic, no external dependencies
- Service tests: `*.service.test.ts` - Mock-based service layer tests
- Test runner: Bun's built-in test runner

**Rust Tests:**
- Unit tests: `#[test]` in source files, run with `cargo test --lib`
- Driver integration tests: `src-tauri/tests/<db>_integration.rs` - Direct driver method testing
- Command integration tests: `src-tauri/tests/<db>_command_integration.rs` - Ephemeral connection commands
- Stateful command tests: `src-tauri/tests/<db>_stateful_command_integration.rs` - Saved connection workflows
- All integration tests:
  - Use testcontainers for real database instances
  - Marked with `#[ignore]` (only run explicitly)
  - Environment: `IT_DB` (mysql/postgres/mariadb/clickhouse/mssql/sqlite/duckdb/all)
  - Helpers in `src-tauri/tests/common/` provide database context setup

### Database Driver Development

When adding/modifying database drivers:
1. Implement `DatabaseDriver` trait in `src-tauri/src/db/drivers/<driver>.rs`
2. Add to driver enum in `src-tauri/src/db/drivers/mod.rs`
3. Handle driver-specific connection strings and options
4. Use `conn_failed_error()` for user-friendly connection error messages
5. Add integration tests in `src-tauri/tests/<driver>_integration.rs`
6. Update `scripts/test-integration.sh` to include new driver

### Common Patterns

**Frontend-Backend Communication:**
```typescript
// Frontend
import { api } from '@/services/api';
const result = await api.execute_query(connectionId, database, sql);
```

```rust
// Backend
#[tauri::command]
async fn execute_query(
    state: State<'_, AppState>,
    connection_id: i64,
    database: Option<String>,
    sql: String,
) -> Result<QueryResult, String> {
    // Implementation
}
```

**Mock Development:**
- Use `bun dev:mock` for rapid UI iteration without Rust compilation
- Mocks defined in `src/services/mocks.ts`
- Useful for working on: themes, UI components, layouts, i18n

**SSH Tunneling:**
- Handled transparently in connection layer
- SSH config in connection form, tunnel established before database connection
- Port forwarding lifetime managed with connection pool

## Build System

- Frontend: Vite with React plugin and TailwindCSS
- Backend: Cargo with sqlx (compile-time SQL checking disabled), tiberius (SQL Server), bb8 (pooling)
- Platform toolchain: Follows Tauri v2 prerequisites (see https://tauri.app/start/prerequisites/)
- Package manager: Bun (preferred) or npm/pnpm

## CI/GitHub Actions

- `.github/workflows/ci.yml` - Main CI pipeline
- Tests run on: Ubuntu (Linux), macOS, Windows
- Integration tests use Docker containers (testcontainers)
- Release builds triggered on tags

## Translation/i18n

- Framework: i18next + react-i18next
- Locale files: `src/lib/i18n/locales/*.ts` (TypeScript, not JSON)
- Supported: English (en), Chinese (zh), Japanese (ja)
- To add language: Create locale file and register in `src/lib/i18n/index.ts`
