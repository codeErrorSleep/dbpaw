#!/usr/bin/env bash
set -euo pipefail

files=()
while IFS= read -r file; do
  files+=("$file")
done < <(find src -type f -name "*.unit.test.ts" | sort)

if [[ ${#files[@]} -eq 0 ]]; then
  echo "[skip] no unit test files found (*.unit.test.ts)"
  exit 0
fi

echo "[run] unit tests (${#files[@]} files)"
bun test "${files[@]}"
