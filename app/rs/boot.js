// 러스트(WASM) 화면을 불러와서, 기다리던 JS 이름들(window.Notices 등)에 연결해요.
// money.js와 money_bg.wasm은 web/build.sh가 만들어요.
import init, { notices_init, notices_refresh, notices_check } from "./money.js?v=1";

try {
  await init({ module_or_path: new URL("./money_bg.wasm?v=1", import.meta.url) });
  window.Notices.__ready({ init: notices_init, refresh: notices_refresh, check: notices_check });
} catch (err) {
  console.warn("러스트 화면을 불러오지 못했어요:", err);
}
