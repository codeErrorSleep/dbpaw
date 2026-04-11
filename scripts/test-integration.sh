#!/usr/bin/env bash
set -euo pipefail

it_db="${IT_DB:-all}"
it_reuse_local_db="${IT_REUSE_LOCAL_DB:-0}"
it_container_prefix="${IT_CONTAINER_PREFIX:-dbpaw-it-$$-}"
export IT_CONTAINER_PREFIX="${it_container_prefix}"

cleanup_it_containers() {
  if [[ "${it_reuse_local_db}" == "1" ]]; then
    return 0
  fi
  if ! command -v docker >/dev/null 2>&1; then
    return 0
  fi

  local ids
  ids="$(docker ps -aq --filter "name=${it_container_prefix}" || true)"
  if [[ -n "${ids}" ]]; then
    echo "[cleanup] removing leftover integration containers: ${it_container_prefix}*"
    docker rm -f ${ids} >/dev/null 2>&1 || true
  fi
}

cleanup_it_containers
trap cleanup_it_containers EXIT

run_integration_test() {
  local test_name="$1"
  echo "[run] integration test: ${test_name} (IT_REUSE_LOCAL_DB=${it_reuse_local_db})"
  cargo test \
    --manifest-path src-tauri/Cargo.toml \
    --test "${test_name}" -- --ignored --nocapture --test-threads=1
}

case "${it_db}" in
  mysql)
    run_integration_test "mysql_integration"
    run_integration_test "mysql_command_integration"
    run_integration_test "mysql_stateful_command_integration"
    ;;
  starrocks)
    run_integration_test "starrocks_integration"
    run_integration_test "starrocks_command_integration"
    ;;
  mariadb)
    run_integration_test "mariadb_integration"
    run_integration_test "mariadb_command_integration"
    ;;
  postgres)
    run_integration_test "postgres_integration"
    run_integration_test "postgres_command_integration"
    run_integration_test "postgres_stateful_command_integration"
    ;;
  clickhouse)
    run_integration_test "clickhouse_integration"
    run_integration_test "clickhouse_command_integration"
    ;;
  mssql)
    run_integration_test "mssql_integration"
    run_integration_test "mssql_command_integration"
    ;;
  duckdb)
    run_integration_test "duckdb_integration"
    run_integration_test "duckdb_command_integration"
    ;;
  sqlite)
    run_integration_test "sqlite_integration"
    run_integration_test "sqlite_command_integration"
    ;;
  oracle)
    run_integration_test "oracle_integration"
    run_integration_test "oracle_command_integration"
    ;;
  all)
    run_integration_test "mysql_integration"
    run_integration_test "mysql_command_integration"
    run_integration_test "mysql_stateful_command_integration"
    run_integration_test "mariadb_integration"
    run_integration_test "mariadb_command_integration"
    run_integration_test "postgres_integration"
    run_integration_test "postgres_command_integration"
    run_integration_test "postgres_stateful_command_integration"
    run_integration_test "clickhouse_integration"
    run_integration_test "clickhouse_command_integration"
    run_integration_test "mssql_integration"
    run_integration_test "mssql_command_integration"
    run_integration_test "duckdb_integration"
    run_integration_test "duckdb_command_integration"
    run_integration_test "sqlite_integration"
    run_integration_test "sqlite_command_integration"
    run_integration_test "oracle_integration"
    run_integration_test "oracle_command_integration"
    ;;
  *)
    echo "[error] Invalid IT_DB='${it_db}'. Expected one of: mysql|starrocks|mariadb|postgres|clickhouse|mssql|duckdb|sqlite|oracle|all"
    exit 1
    ;;
esac
