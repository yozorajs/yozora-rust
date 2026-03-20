#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

cargo test -p yozora-suitecases --test suitecases_custom_subset
cargo test -p yozora-suitecases --test suitecases_smoke
cargo test -p yozora-suitecases --test suitecases_upstream_subset

for profile in yozora gfm gfm_ex yozora_inline_math_backtick_required; do
  echo "[fixture-batch] profile=$profile"
  YOZORA_PARSER_PROFILE="$profile" \
  YOZORA_ASSERT_LEVEL="L2" \
  YOZORA_FAIL_ON_DIFF="1" \
  cargo test -p yozora-suitecases --test suitecases_upstream_batch_report -- --ignored
 done
