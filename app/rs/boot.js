// 러스트(WASM) 화면을 불러와서, 기다리던 JS 이름들(window.Notices 등)에 연결해요.
// money.js와 money_bg.wasm은 web/build.sh가 만들어요.
import init, * as rs from "./money.js?v=7";

try {
  await init({ module_or_path: new URL("./money_bg.wasm?v=7", import.meta.url) });
  // 돈(지갑 줄 세우기)을 가장 먼저: 다른 화면이 지갑을 써요
  window.Money.__ready({ init: rs.money_init, startWallet: rs.money_start_wallet, walletOp: rs.money_wallet_op, claim: rs.money_claim, log: rs.money_log, render: rs.money_render });
  // 사업 탭을 먼저: 상점·인벤토리가 상품 그림(pkv)을 같이 써요
  window.Business.__ready({ init: rs.business_init, refresh: rs.business_refresh, lab: rs.business_lab, pkv: rs.business_pkv, kindSvg: rs.business_kind_svg });
  window.Notices.__ready({ init: rs.notices_init, refresh: rs.notices_refresh, check: rs.notices_check });
  window.Inventory.__ready({ init: rs.inventory_init, refresh: rs.inventory_refresh });
  window.Manage.__ready({ init: rs.manage_init, refresh: rs.manage_refresh });
  window.Shop.__ready({ init: rs.shop_init, refresh: rs.shop_refresh });
  window.Work.__ready({ init: rs.work_init, refresh: rs.work_refresh, factoryInto: rs.work_factory_into });
} catch (err) {
  console.warn("러스트 화면을 불러오지 못했어요:", err);
}
