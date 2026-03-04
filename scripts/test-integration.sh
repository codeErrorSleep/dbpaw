#!/usr/bin/env bash
set -euo pipefail

run_integration_test() {
  local db_name="$1"
  local test_name="$2"
  local run_flag="$3"

  if [[ "${run_flag}" != "1" ]]; then
    echo "[skip] ${db_name} integration test (set ${db_name}=1 to enable)"
    return 0
  fi

  echo "[run] ${db_name} integration test: ${test_name}"
  cargo test \
    --manifest-path src-tauri/Cargo.toml \
    --test "${test_name}" \
    -- --ignored --nocapture
}

run_integration_test "RUN_MYSQL_IT" "mysql_integration" "${RUN_MYSQL_IT:-0}"
run_integration_test "RUN_POSTGRES_IT" "postgres_integration" "${RUN_POSTGRES_IT:-0}"
run_integration_test "RUN_SQLITE_IT" "sqlite_integration" "${RUN_SQLITE_IT:-0}"
run_integration_test "RUN_MSSQL_IT" "mssql_integration" "${RUN_MSSQL_IT:-0}"
run_integration_test "RUN_CLICKHOUSE_IT" "clickhouse_integration" "${RUN_CLICKHOUSE_IT:-0}"
