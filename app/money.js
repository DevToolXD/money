/* 돈 탭(잔액, 이자 상품, 거래내역, 그래프, 카드 순서, 지갑 쓰기 줄 세우기)은 러스트(WASM)로 옮겼어요: web/crates/ui/src/money/ (이름만 먼저 만들어 둬요, app/rs-shim.js) */
window.rsShim("Money", ["init", "startWallet", "walletOp", "claim", "log", "render"]);
