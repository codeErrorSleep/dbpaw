#!/usr/bin/env bash
set -euo pipefail

it_db="${IT_DB:-all}"
it_reuse_local_db="${IT_REUSE_LOCAL_DB:-0}"
it_container_prefix="${IT_CONTAINER_PREFIX:-dbpaw-it-$$-}"
export IT_CONTAINER_PREFIX="${it_container_prefix}"

oracle_client_detected() {
  local dyld_path="${DYLD_LIBRARY_PATH:-}"
  if [[ -n "${dyld_path}" ]]; then
    local old_ifs="${IFS}"
    IFS=':'
    for dir in ${dyld_path}; do
      if [[ -f "${dir}/libclntsh.dylib" ]]; then
        IFS="${old_ifs}"
        return 0
      fi
    done
    IFS="${old_ifs}"
  fi

  [[ -f "/opt/homebrew/lib/libclntsh.dylib" ]] && return 0
  [[ -f "/usr/local/lib/libclntsh.dylib" ]] && return 0
  [[ -f "${HOME}/lib/libclntsh.dylib" ]] && return 0

  return 1
}

print_oracle_preflight_notice() {
  echo "[oracle] Oracle integration tests require a local Oracle instance and Oracle Instant Client."

  if [[ "${it_reuse_local_db}" != "1" ]]; then
    echo "[oracle] IT_REUSE_LOCAL_DB=1 is not enabled; Oracle tests will be skipped."
    echo "[oracle] Real run example:"
    echo "          IT_REUSE_LOCAL_DB=1 ORACLE_HOST=127.0.0.1 ORACLE_PORT=1521 ORACLE_USER=system ORACLE_PASSWORD=... ORACLE_SERVICE=FREE IT_DB=oracle bun run test:integration"
    return 0
  fi

  if [[ -z "${ORACLE_PASSWORD:-}" ]]; then
    echo "[oracle] ORACLE_PASSWORD is not set; Oracle tests may skip during preflight."
  fi

  if ! oracle_client_detected; then
    echo "[oracle] Oracle Instant Client was not detected from DYLD_LIBRARY_PATH/common paths."
    echo "[oracle] If tests skip with DPI-1047, install Instant Client and export DYLD_LIBRARY_PATH to the directory containing libclntsh.dylib."
  fi
}

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

case "${it_db}" in
  oracle|all)
    print_oracle_preflight_notice
    ;;
esac

run_integration_test() {
  local test_name="$1"
  echo "[run] integration test: ${test_name} (IT_REUSE_LOCAL_DB=${it_reuse_local_db})"
  local ignored_flag="--ignored"
  # redis_integration tests no longer use #[ignore]
  if [[ "${test_name}" == "redis_integration" ]]; then
    ignored_flag=""
  fi
  cargo test \
    --manifest-path src-tauri/Cargo.toml \
    --test "${test_name}" -- ${ignored_flag} --nocapture --test-threads=1
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
  doris)
    run_integration_test "doris_integration"
    run_integration_test "doris_command_integration"
    ;;
  mariadb)
    run_integration_test "mariadb_integration"
    run_integration_test "mariadb_command_integration"
    run_integration_test "mariadb_stateful_command_integration"
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
    run_integration_test "sqlite_stateful_command_integration"
    ;;
  oracle)
    run_integration_test "oracle_integration"
    run_integration_test "oracle_command_integration"
    ;;
  redis)
    if [[ "${it_reuse_local_db}" == "1" && -z "${REDIS_CLUSTER_HOSTS:-}" ]]; then
      echo "[redis] IT_REUSE_LOCAL_DB=1 detected but REDIS_CLUSTER_HOSTS is not set."
      echo "[redis] Cluster tests will be skipped. To run them, start the test environment first:"
      echo "[redis]   docker compose -f docker-compose.redis.yml up -d --wait"
    fi
    run_integration_test "redis_integration"
    ;;
  all)
    run_integration_test "mysql_integration"
    run_integration_test "mysql_command_integration"
    run_integration_test "mysql_stateful_command_integration"
    run_integration_test "mariadb_integration"
    run_integration_test "mariadb_command_integration"
    run_integration_test "mariadb_stateful_command_integration"
    run_integration_test "doris_integration"
    run_integration_test "doris_command_integration"
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
    run_integration_test "sqlite_stateful_command_integration"
    run_integration_test "oracle_integration"
    run_integration_test "oracle_command_integration"
    run_integration_test "redis_integration"
    ;;
  *)
    echo "[error] Invalid IT_DB='${it_db}'. Expected one of: mysql|starrocks|doris|mariadb|postgres|clickhouse|mssql|duckdb|sqlite|oracle|redis|all"
    exit 1
    ;;
esac
