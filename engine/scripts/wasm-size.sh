#!/bin/sh
# Build a crate for wasm32-unknown-unknown, run wasm-bindgen and wasm-opt -Oz,
# print the size and fail above the limit (NFR-02).
# Usage: scripts/wasm-size.sh [package] [limit-bytes]
set -eu
cd "$(dirname "$0")/.."
pkg=${1:-wasm-probe}
limit=${2:-1572864} # 1.5 MB
target=1048576      # 1 MB
lib=$(echo "$pkg" | tr - _)
out=target/wasm-size
cargo build -q -p "$pkg" --release --target wasm32-unknown-unknown
rm -rf "$out" && mkdir -p "$out"
wasm-bindgen --target web --out-dir "$out" "target/wasm32-unknown-unknown/release/$lib.wasm"
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
  --enable-mutable-globals "$out/${lib}_bg.wasm" -o "$out/$lib.opt.wasm"
size=$(wc -c < "$out/$lib.opt.wasm" | tr -d ' ')
gz=$(gzip -9 -c "$out/$lib.opt.wasm" | wc -c | tr -d ' ')
echo "$pkg: $size bytes ($((size / 1024)) KB), gzip $((gz / 1024)) KB"
if [ "$size" -gt "$limit" ]; then
  echo "FAIL: above the $limit byte limit" >&2
  exit 1
elif [ "$size" -gt "$target" ]; then
  echo "WARN: above the 1 MB target, log it in PROGRESS.md" >&2
fi
