#!/usr/bin/env bash
set -euo pipefail

./build-wasm.sh
cd pkg && npm publish --access public
