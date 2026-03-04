#!/usr/bin/env bash
set -euo pipefail

files=()
while IFS= read -r file; do
  files+=("$file")
done < <(find src -type f -name "*.service.test.ts" | sort)

if [[ ${#files[@]} -eq 0 ]]; then
  echo "[skip] no service test files found (*.service.test.ts)"
  exit 0
fi

echo "[run] service tests (${#files[@]} files)"
bun test "${files[@]}"
