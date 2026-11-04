/* 사업 탭(판매 목록, 창고, 직원 채용, 연구실, 팔 상품 정하기, 블록 옮기기)과 상품 그림은 러스트(WASM)로 옮겼어요:
   web/crates/ui/src/biz/, web/crates/goods/ (이름만 먼저 만들어 둬요, app/rs-shim.js) */
window.rsShim("Business", ["init", "refresh", "lab", "pkv", "kindSvg"]);
