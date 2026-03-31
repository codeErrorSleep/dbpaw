# Testing Guide

This document describes the testing strategy and how to run tests in DbPaw.

## Test Architecture

DbPaw uses a **layered testing approach**:

```
┌─────────────────────────────────┐
│  Frontend (React/TypeScript)     │  ← Unit tests (*.unit.test.ts)
│  - Components, utilities, libs   │  ← Service tests (*.service.test.ts)
├─────────────────────────────────┤
│  Tauri Commands (Rust)           │  ← Command integration tests
│  - connection.rs, query.rs, etc. │     (*_command_integration.rs)
├─────────────────────────────────┤
│  Database Drivers (Rust)         │  ← Driver integration tests
│  - MysqlDriver, PostgresDriver   │     (*_integration.rs)
└─────────────────────────────────┘
```

## Quick Start

### Run All Tests
```bash
bun run test:all
```

### Run Specific Test Types
```bash
# Frontend tests only
bun run test:unit          # Pure logic tests (*.unit.test.ts)
bun run test:service       # Service layer tests (*.service.test.ts)

# Backend tests only
bun run test:rust:unit     # Rust unit tests

# Integration tests (requires Docker)
bun run test:integration   # All databases
IT_DB=mysql bun run test:integration      # MySQL only
IT_DB=postgres bun run test:integration   # PostgreSQL only
```

### Quick Validation (Pre-commit)
```bash
bun run test:smoke   # typecheck + lint + rust:check + unit + service + rust:unit
```

### CI Full Suite
```bash
bun run test:ci      # smoke + all integration tests
```

## Change → Validation Mapping

When you modify files, run the appropriate test suite:

| Directory Changed | Tests to Run | Command |
|------------------|--------------|---------|
| `src/components/`, `src/lib/` | TypeScript unit tests | `bun run test:unit` |
| `src/services/` | Service tests | `bun run test:service` |
| `src-tauri/src/db/drivers/` | Driver integration tests | `IT_DB=mysql bun run test:integration` |
| `src-tauri/src/commands/` | Command integration tests | `IT_DB=mysql bun run test:integration` |
| Cross-layer changes | Full suite | `bun run test:all` |

## Integration Test Details

### Environment Variables

- `IT_DB` - Which database to test: `mysql`, `postgres`, `mariadb`, `mssql`, `clickhouse`, `sqlite`, `duckdb`, `all`
- `IT_REUSE_LOCAL_DB=1` - Reuse existing local database (faster for development)
- `IT_CONTAINER_PREFIX` - Custom container name prefix (default: `dbpaw-it-$$-`)

### Examples

```bash
# Test single database (faster iteration)
IT_DB=mysql bun run test:integration

# Reuse local database (no Docker container startup)
IT_REUSE_LOCAL_DB=1 IT_DB=mysql bun run test:integration

# Test all databases (CI mode)
IT_DB=all bun run test:integration
```

### Manual Test Execution

```bash
# Run specific test file
cargo test --manifest-path src-tauri/Cargo.toml \
  --test mysql_integration -- --ignored --nocapture --test-threads=1

# Run specific test case
cargo test --manifest-path src-tauri/Cargo.toml \
  --test mysql_command_integration test_mysql_command_test_connection_success \
  -- --ignored --nocapture --test-threads=1
```

## Test Coverage by Database

### Legend
- ✅ Fully covered (driver + command + stateful tests)
- 🟢 Good coverage (driver + command tests)
- 🟡 Partial (driver tests only)
- ❌ Missing

| Database | Driver Tests | Command Tests | Stateful Command Tests | Notes |
|----------|-------------|---------------|----------------------|-------|
| **MySQL** | ✅ | ✅ | ✅ | Complete coverage |
| **PostgreSQL** | ✅ | ✅ | ✅ | Complete coverage |
| **MariaDB** | ✅ | ✅ | ⏳ | Driver + Command (stateful pending) |
| **SQL Server** | ✅ | ✅ | ⏳ | Driver + Command (stateful pending) |
| **ClickHouse** | ✅ | ✅ | ⏳ | Driver + Command (stateful pending) |
| **SQLite** | ✅ | ✅ | ⏳ | Driver + Command (stateful pending) |
| **DuckDB** | ✅ | ✅ | ⏳ | Driver + Command (stateful pending) |
| **TiDB** | N/A | N/A | N/A | Uses MySQL driver |

## Test File Naming Conventions

### TypeScript Tests
- `*.unit.test.ts` - Pure unit tests (no external dependencies)
- `*.service.test.ts` - Service layer tests (may use mocks)
- `*.test.ts` - General tests (deprecated, prefer specific suffixes)

### Rust Tests
- `src-tauri/tests/<db>_integration.rs` - Driver layer tests (direct driver calls)
- `src-tauri/tests/<db>_command_integration.rs` - Command layer tests (ephemeral connections)
- `src-tauri/tests/<db>_stateful_command_integration.rs` - Stateful command tests (saved connections)
- `src-tauri/tests/common/<db>_context.rs` - Test helpers and Docker setup

## Adding Tests for a New Database

When adding support for a new database, follow this checklist:

### 1. Create Test Context
```bash
src-tauri/tests/common/<db>_context.rs
```
- Docker container setup
- Connection form builder
- Retry helpers

### 2. Create Driver Integration Tests
```bash
src-tauri/tests/<db>_integration.rs
```

**Minimum P0 tests:**
- [ ] `test_<db>_integration_flow` - Basic CRUD flow
- [ ] `test_<db>_get_table_data_supports_pagination_sort_filter_and_order_by`
- [ ] `test_<db>_get_table_data_rejects_invalid_sort_column`
- [ ] `test_<db>_table_structure_and_schema_overview`
- [ ] `test_<db>_metadata_includes_indexes_and_foreign_keys`
- [ ] `test_<db>_boolean_and_json_type_mapping_regression`
- [ ] `test_<db>_error_handling_for_sql_error`

**Recommended P1 tests:**
- [ ] `test_<db>_transaction_commit_and_rollback`
- [ ] `test_<db>_execute_query_reports_affected_rows_for_update_delete`
- [ ] `test_<db>_batch_insert_and_batch_execute_flow`
- [ ] `test_<db>_large_text_and_blob_round_trip`
- [ ] `test_<db>_concurrent_connections_can_query`
- [ ] `test_<db>_view_can_be_listed_and_queried`
- [ ] `test_<db>_connection_failure_with_wrong_password`
- [ ] `test_<db>_connection_timeout_or_unreachable_host_error`

### 3. Create Command Integration Tests
```bash
src-tauri/tests/<db>_command_integration.rs
```

**Minimum P0 commands:**
- [ ] `test_<db>_command_test_connection_success`
- [ ] `test_<db>_command_test_connection_invalid_password_returns_error`
- [ ] `test_<db>_command_list_tables_by_conn_contains_created_table`
- [ ] `test_<db>_command_list_databases_contains_target_db`
- [ ] `test_<db>_command_execute_by_conn_select_returns_rows`
- [ ] `test_<db>_command_execute_by_conn_invalid_sql_returns_error`
- [ ] `test_<db>_command_execute_by_conn_insert_affects_rows`
- [ ] `test_<db>_command_get_table_data_by_conn_pagination_works`

### 4. Create Stateful Command Tests
```bash
src-tauri/tests/<db>_stateful_command_integration.rs
```

**Covered areas:**
- Connection CRUD lifecycle
- Database creation/listing with saved connections
- Query execution with connection IDs
- Metadata operations with connection IDs
- SQL execution logging

### 5. Update Test Script
Edit `scripts/test-integration.sh`:

```bash
# Add new database case
<db>)
  run_integration_test "<db>_integration"
  run_integration_test "<db>_command_integration"
  run_integration_test "<db>_stateful_command_integration"
  ;;

# Add to 'all' case
all)
  ...
  run_integration_test "<db>_integration"
  run_integration_test "<db>_command_integration"
  run_integration_test "<db>_stateful_command_integration"
  ...
  ;;
```

## Test Best Practices

### General
- Use `#[ignore]` for integration tests (only run explicitly)
- Use `--test-threads=1` to avoid database race conditions
- Clean up all test data after each test
- Use unique names for temporary objects (include timestamp)

### Assertions
- Don't just assert `is_ok()` - verify actual data
- For errors, assert error message is non-empty
- Check for specific error prefixes: `[CONN_FAILED]`, `[VALIDATION_ERROR]`, etc.
- Verify row counts, column names, and data values

### Docker/Testcontainers
- Use `IT_REUSE_LOCAL_DB=1` for faster local development
- Container names must be unique (use prefix + timestamp)
- Always wait for port availability after container start
- Use retry logic for connection establishment

### Examples

```rust
// ❌ Bad - only checks is_ok()
let result = driver.execute_query(sql).await;
assert!(result.is_ok());

// ✅ Good - verifies actual data
let result = driver.execute_query(sql).await
    .expect("query should succeed");
assert_eq!(result.row_count, 1);
assert_eq!(result.data[0]["name"].as_str(), Some("DbPaw"));

// ❌ Bad - generic error check
assert!(result.is_err());

// ✅ Good - verifies error content
let error = result.err().unwrap();
assert!(!error.trim().is_empty());
assert!(error.contains("[CONN_FAILED]"));
```

## CI Integration

### Current GitHub Actions Workflow

The `.github/workflows/ci.yml` runs:
1. TypeScript type checking
2. Frontend linting
3. Rust cargo check
4. Frontend unit tests
5. Frontend service tests
6. Rust unit tests
7. **Full integration test matrix** (all databases)

### Optimization Opportunities

Consider splitting CI into:
- **PR checks** (fast feedback): `test:smoke` + MySQL integration only
- **Nightly** (full coverage): All databases
- **Conditional** (smart): Full database matrix only when `src-tauri/src/db/` changes

## Troubleshooting

### "Container name already exists"
```bash
# Clean up containers
docker ps -a --filter "name=dbpaw-it-" | grep dbpaw-it- | awk '{print $1}' | xargs -r docker rm -f
```

### "Port already in use"
```bash
# Use reuse mode
IT_REUSE_LOCAL_DB=1 IT_DB=mysql bun run test:integration
```

### "Test hangs/timeouts"
- Check Docker daemon is running
- Verify port is not blocked by firewall
- Increase wait timeout in `*_context.rs`

### "Connection refused"
- Wait longer for database readiness (increase retry count)
- Check container logs: `docker logs <container-name>`
- Verify credentials match container environment

## Performance Tips

### For Local Development
```bash
# 1. Use reuse mode (fastest)
IT_REUSE_LOCAL_DB=1 IT_DB=mysql bun run test:integration

# 2. Run specific test file
cargo test --manifest-path src-tauri/Cargo.toml \
  --test mysql_command_integration -- --ignored --nocapture

# 3. Run specific test case
cargo test --manifest-path src-tauri/Cargo.toml \
  test_mysql_command_test_connection_success -- --ignored
```

### For CI
```bash
# Run only changed database
IT_DB=mysql bun run test:integration

# Cache Docker images
# Add to .github/workflows/ci.yml:
# - uses: actions/cache@v3
#   with:
#     path: /var/lib/docker
```

## Future Improvements

### P0 (High Priority)
- [ ] Add command integration tests for MariaDB, MSSQL, ClickHouse, SQLite, DuckDB
- [ ] Document test data setup patterns
- [ ] Create test helper library for common assertions

### P1 (Medium Priority)
- [ ] Add frontend component tests (Playwright or Vitest)
- [ ] Split CI into fast/slow test suites
- [ ] Add performance benchmarks for query execution
- [ ] Create test coverage reports

### P2 (Nice to Have)
- [ ] Add visual regression tests for UI components
- [ ] Create load testing scenarios
- [ ] Add mutation testing for critical paths
- [ ] Set up automatic test generation for new drivers
