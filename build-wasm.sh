#!/usr/bin/env bash
set -euo pipefail

echo "Building WASM (release)..."
cargo build -p mdx2md-wasm --target wasm32-unknown-unknown --release

WASM_FILE=$(find target -path "*/wasm32-unknown-unknown/release/mdx2md_wasm.wasm" | head -1)
if [ -z "$WASM_FILE" ]; then
  echo "Error: WASM file not found"
  exit 1
fi

echo "Generating JS bindings..."
mkdir -p pkg
wasm-bindgen --target web --out-dir pkg "$WASM_FILE"

echo "Copying package.json and README..."
cp npm/package.json pkg/package.json
cp README.md pkg/README.md

echo "Done. Output in pkg/"
ls -lh pkg/
