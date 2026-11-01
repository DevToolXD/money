#!/bin/sh
# 러스트 화면을 WASM으로 만들어 app/rs/ 에 놓아요 (index.html이 app/rs/boot.js로 불러요).
# 필요한 것: rustup target add wasm32-unknown-unknown, cargo install wasm-bindgen-cli --version 0.2.130
# wasm-opt(크기 줄이기)는 tests/node_modules 의 binaryen을 써요 (cd tests && npm install).
set -eu
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown -p money-app
OUT=../app/rs
mkdir -p "$OUT"
wasm-bindgen --target web --no-typescript --out-dir "$OUT" --out-name money target/wasm32-unknown-unknown/release/money_app.wasm
OPT=../tests/node_modules/.bin/wasm-opt
if [ -x "$OPT" ]; then
  "$OPT" -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals --enable-reference-types -o "$OUT/money_bg.wasm" "$OUT/money_bg.wasm"
else
  echo "wasm-opt가 없어서 크기 줄이기를 건너뛰어요 (cd tests && npm install)"
fi
echo "만들었어요: $(du -h "$OUT/money_bg.wasm" | cut -f1) $OUT/money_bg.wasm"
