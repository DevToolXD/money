#!/bin/sh
# 러스트 상품 그림(web/crates/goods)이 예전 JS와 글자 하나까지 같은지 봐요.
# 예전 JS는 git에서 꺼내요 (기본: 러스트로 옮기기 전 커밋 3f73866)
set -e
cd "$(dirname "$0")/../.."
REV=${1:-3f73866}
T=$(mktemp -d)
git show "$REV:app/shapes.js" > "$T/shapes.js"
git show "$REV:app/biz.js" > "$T/biz.js"
node tests/goods/dump-js.js "$T/shapes.js" "$T/biz.js" > "$T/js.txt"
(cd web && cargo run -q -p goods --example dump) > "$T/rs.txt"
if cmp -s "$T/js.txt" "$T/rs.txt"; then
  echo "같아요: $(wc -l < "$T/rs.txt")줄"
else
  diff "$T/js.txt" "$T/rs.txt" | head -20 | cut -c1-400
  echo "달라요: $(diff "$T/js.txt" "$T/rs.txt" | grep -c '^<')줄"
  exit 1
fi
