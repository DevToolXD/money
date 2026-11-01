#!/bin/sh
# 화면 시험 전체 돌리기: 에뮬레이터(인증 9099, Firestore 8081)와 정적 서버(8765)를 띄우고, 시험마다 데이터를 비워요.
# 쓰는 법: cd tests && npm run build && EMU=1 node build.mjs && sh e2e/run.sh   (특정 시험만: sh e2e/run.sh notice cats)
set -u
cd "$(dirname "$0")"
mkdir -p shots
ROOT="$(cd ../.. && pwd)"
if ! curl -s -o /dev/null http://127.0.0.1:8081/; then
  (cd "$ROOT" && nohup npx --prefix tests firebase emulators:start --only auth,firestore --project money-sim-36ed8 > /tmp/money-emu.log 2>&1 &)
  for i in $(seq 1 60); do sleep 2; curl -s -o /dev/null http://127.0.0.1:8081/ && break; done
fi
if ! curl -s -o /dev/null http://127.0.0.1:8765/index.html; then
  (cd "$ROOT" && nohup python3 -m http.server 8765 --bind 127.0.0.1 > /tmp/money-static.log 2>&1 &)
  sleep 2
fi
reset() {
  curl -s -X DELETE "http://127.0.0.1:9099/emulator/v1/projects/money-sim-36ed8/accounts" > /dev/null
  curl -s -X DELETE "http://127.0.0.1:8081/emulator/v1/projects/money-sim-36ed8/databases/(default)/documents" > /dev/null
}
LIST="${*:-$(ls *.js | sed 's/\.js$//')}"
fail=0
for t in $LIST; do
  reset
  if node "$t.js" > "shots/$t.log" 2>&1; then echo "통과  $t  ($(tail -1 shots/$t.log))"; else echo "실패  $t  (shots/$t.log)"; fail=1; fi
done
exit $fail
