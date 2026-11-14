#!/bin/bash
# Builds Lemon Studio for the browser into ../../studio-lemon (or $1): plain static files.
# Needs: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli --version 0.2.129
# Optional: wasm-opt (binaryen) for a smaller file.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:-../studio-lemon}"
PROFILE="${PROFILE:-web}"
cargo build --target wasm32-unknown-unknown -p os-app --profile "$PROFILE"
DIR=$PROFILE; [ "$PROFILE" = dev ] && DIR=debug
mkdir -p "$OUT/pkg"
wasm-bindgen --target web --no-typescript --remove-name-section --out-dir "$OUT/pkg" "target/wasm32-unknown-unknown/$DIR/lemonstudio.wasm"
if command -v wasm-opt >/dev/null && [ "${OPT:-1}" = 1 ]; then
  wasm-opt -O2 --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals --enable-reference-types "$OUT/pkg/lemonstudio_bg.wasm" -o "$OUT/pkg/lemonstudio_bg.wasm"
fi
cp web/static/* "$OUT/"
cp LICENSE "$OUT/LICENSE.txt"
cp THIRD-PARTY-NOTICES.txt "$OUT/THIRD-PARTY-NOTICES.txt"
ls -la "$OUT" "$OUT/pkg"
