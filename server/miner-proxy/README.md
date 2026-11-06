# 코인 탭 채굴 프록시 (Cloud Run)

브라우저는 모네로 풀(stratum TCP)에 직접 붙을 수 없어서, 이 작은 서버가 WebSocket ↔ TCP를 이어 줘요.
채굴한 모네로는 모두 `WALLET` 주소로 가요. 토큰 보상(계정 하나에 100초마다 1 토큰)은 앱의 Firestore 규칙이 정해요.

## 올리기 (한 번만)

```sh
cd server/miner-proxy
gcloud run deploy money-miner --source . --region asia-northeast3 --allow-unauthenticated \
  --timeout 3600 --session-affinity --min-instances 0 --max-instances 3 \
  --set-env-vars ORIGINS=https://사이트주소
```

나오는 주소(`https://money-miner-xxxx.a.run.app`)를 `wss://money-miner-xxxx.a.run.app`로 바꿔서
`web/crates/ui/src/coin.rs`의 `PROXY`에 넣고, `index.html` CSP의 `connect-src`에 같은 주소를 더한 뒤 `sh web/build.sh`로 다시 빌드해요.

환경 변수: `POOL_HOST`(기본 pool.supportxmr.com), `POOL_PORT`(3333), `WALLET`, `ORIGINS`(허락할 사이트 주소, 쉼표), `MAX_CONN`.
