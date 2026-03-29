#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: scripts/db-onboard.sh <db> [--skip-gate] [--skip-matrix]"
  exit 1
fi

db="$1"
shift || true

skip_gate=0
skip_matrix=0

for arg in "$@"; do
  case "$arg" in
    --skip-gate)
      skip_gate=1
      ;;
    --skip-matrix)
      skip_matrix=1
      ;;
    *)
      echo "[error] unknown option: $arg"
      echo "Usage: scripts/db-onboard.sh <db> [--skip-gate] [--skip-matrix]"
      exit 1
      ;;
  esac
done

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${root_dir}"

context_file="src-tauri/tests/common/${db}_context.rs"
integration_file="src-tauri/tests/${db}_integration.rs"
command_file="src-tauri/tests/${db}_command_integration.rs"
stateful_file="src-tauri/tests/${db}_stateful_command_integration.rs"
tracker_file="docs/zh/Development/MYSQL_TEST_COVERAGE_GAP_TRACKER.md"

echo "[step] scaffold check: ${db}"
missing=0
for file in "${context_file}" "${integration_file}" "${command_file}" "${stateful_file}"; do
  if [[ ! -f "${file}" ]]; then
    echo "[missing] ${file}"
    missing=1
  else
    echo "[ok] ${file}"
  fi
done

if [[ ${missing} -ne 0 ]]; then
  echo "[error] scaffold is incomplete for '${db}'."
  echo "[hint] finish scaffold first, then rerun scripts/db-onboard.sh ${db}"
  exit 1
fi

if [[ ${skip_gate} -eq 0 ]]; then
  echo "[step] gate syntax check"
  bash -n scripts/test-integration.sh

  echo "[step] compile smoke: ${db}_integration"
  cargo test --manifest-path src-tauri/Cargo.toml --test "${db}_integration" --no-run

  echo "[step] compile smoke: ${db}_command_integration"
  cargo test --manifest-path src-tauri/Cargo.toml --test "${db}_command_integration" --no-run

  echo "[step] compile smoke: ${db}_stateful_command_integration"
  cargo test --manifest-path src-tauri/Cargo.toml --test "${db}_stateful_command_integration" --no-run

  echo "[step] integration gate run: IT_DB=${db}"
  IT_DB="${db}" bash scripts/test-integration.sh
else
  echo "[skip] gate run skipped by --skip-gate"
fi

if [[ ${skip_matrix} -eq 0 ]]; then
  echo "[step] matrix sync check"
  test_count="$(rg -n "async fn test_${db}_" src-tauri/tests --glob "*.rs" || true)"
  test_count="$(printf "%s\n" "${test_count}" | sed '/^$/d' | wc -l | tr -d ' ')"
  echo "[info] detected test functions for ${db}: ${test_count}"
  if [[ -f "${tracker_file}" ]]; then
    tracker_hits="$(rg -n "test_${db}_" "${tracker_file}" || true)"
    tracker_hits="$(printf "%s\n" "${tracker_hits}" | sed '/^$/d' | wc -l | tr -d ' ')"
    if [[ "${tracker_hits}" -eq 0 ]]; then
      echo "[warn] tracker has no '${db}' test entries yet: ${tracker_file}"
      echo "[next] sync capability matrix and command coverage sections for '${db}'"
    else
      echo "[ok] tracker already contains ${tracker_hits} '${db}' test entries"
    fi
  else
    echo "[warn] tracker file not found: ${tracker_file}"
  fi
else
  echo "[skip] matrix sync check skipped by --skip-matrix"
fi

echo "[done] db onboarding pipeline finished for '${db}'"
