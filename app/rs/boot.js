// 앱 전체가 러스트(WASM)예요: web/crates/. 이 파일은 WASM을 받아서 시작만 해요.
// money.js와 money_bg.wasm은 web/build.sh가 만들어요. (탭 바·뒤 화면 사진·아이콘은 app/tabbar.js, backdrop.js, icons.js, 서버 연결은 app/firebase.js)
import init, * as rs from "./money.js?v=71";

try {
  await init({ module_or_path: new URL("./money_bg.wasm?v=71", import.meta.url) });
  rs.shell_start();
} catch (err) {
  console.warn("러스트 화면을 불러오지 못했어요:", err);
}
