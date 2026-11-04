/* 메시지 탭(대화 목록, 대화방, 새 대화·단톡방, 배경화면, 사진·그리기·투표·토큰 선물)은 러스트(WASM)로 옮겼어요: web/crates/ui/src/chat/ (이름만 먼저 만들어 둬요, app/rs-shim.js) */
window.rsShim("Chat", ["init", "start", "render", "leave"]);
