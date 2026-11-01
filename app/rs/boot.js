// 러스트(WASM) 화면을 불러와서, 기다리던 JS 이름들(window.Notices 등)에 연결해요.
// money.js와 money_bg.wasm은 web/build.sh가 만들어요.
import init, * as rs from "./money.js?v=2";

try {
  await init({ module_or_path: new URL("./money_bg.wasm?v=2", import.meta.url) });
  window.Notices.__ready({ init: rs.notices_init, refresh: rs.notices_refresh, check: rs.notices_check });
  window.Inventory.__ready({ init: rs.inventory_init, refresh: rs.inventory_refresh });
} catch (err) {
  console.warn("러스트 화면을 불러오지 못했어요:", err);
}
