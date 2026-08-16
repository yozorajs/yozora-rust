#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REFERENCE_ROOT="${1:-${YOZORA_REFERENCE_ROOT:-}}"

if [[ -z "$REFERENCE_ROOT" ]]; then
  echo "usage: $0 <yozora-reference-root>" >&2
  echo "or set YOZORA_REFERENCE_ROOT" >&2
  exit 2
fi

cd "$ROOT_DIR"
node script/reference-test-inventory.mjs \
  --reference-root "$REFERENCE_ROOT" \
  --output test-parity/reference-tests.json \
  --check
cargo test -p yozora-suitecases --test reference_test_parity
