#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_DIR="${1:-$ROOT_DIR/../yozora/fixtures}"
DST_DIR="$ROOT_DIR/fixtures"

if [[ ! -d "$SRC_DIR" ]]; then
  echo "source fixtures dir not found: $SRC_DIR" >&2
  exit 1
fi

mkdir -p "$DST_DIR"
mkdir -p "$DST_DIR/custom" "$DST_DIR/gfm"
rsync -a --delete "$SRC_DIR/custom/" "$DST_DIR/custom/"
rsync -a --delete "$SRC_DIR/gfm/" "$DST_DIR/gfm/"

echo "synced fixtures: $SRC_DIR -> $DST_DIR"
