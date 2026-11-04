//! 시작점: JS 쪽(`app/rs/boot.js`)이 불러 가는 함수들이에요.
//! 예전 JS 모듈과 같은 이름(`window.Notices` 등)으로 연결돼서, 나머지 JS는 바꾸지 않아도 돼요.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// 공지: 설정의 공지 화면에 목록을 붙여요. (ctx는 예전 JS와 같은 모양으로 받지만, 다리는 window.cloud를 바로 써요)
#[wasm_bindgen]
pub fn notices_init(_ctx: JsValue) {
    ui::notices::init();
}

/// 공지 화면을 열 때 목록을 다시 읽어요
#[wasm_bindgen]
pub fn notices_refresh() {
    ui::notices::refresh();
}

/// 아직 안 본 공지를 전체 화면으로 보여줘요
#[wasm_bindgen]
pub fn notices_check() {
    ui::notices::check();
}

/// 인벤토리: ctx(app.js가 주는 도움 묶음)를 받아 #inv-root에 붙여요
#[wasm_bindgen]
pub fn inventory_init(ctx: JsValue) {
    ui::inventory::init(ui::ctx::Ctx(ctx));
}

/// 인벤토리를 서버에서 다시 읽어요
#[wasm_bindgen]
pub fn inventory_refresh() {
    ui::inventory::refresh();
}

/// 관리 (오너만): #manage-root에 붙여요
#[wasm_bindgen]
pub fn manage_init(ctx: JsValue) {
    ui::manage::init(ui::ctx::Ctx(ctx));
}

/// 관리 화면을 열 때 올린 공지를 다시 읽어요
#[wasm_bindgen]
pub fn manage_refresh() {
    ui::manage::refresh();
}

/// 상점: #shop-root에 붙여요
#[wasm_bindgen]
pub fn shop_init(ctx: JsValue) {
    ui::shop::init(ui::ctx::Ctx(ctx));
}

/// 상점 목록을 다시 읽어요
#[wasm_bindgen]
pub fn shop_refresh() {
    ui::shop::refresh();
}

/// 일 탭: #work-root에 붙여요
#[wasm_bindgen]
pub fn work_init(ctx: JsValue) {
    ui::work::init(ui::ctx::Ctx(ctx));
}

/// 일 탭을 다시 그려요 (force면 서버에서 다시 읽어요)
#[wasm_bindgen]
pub fn work_refresh(force: Option<bool>) {
    ui::work::refresh(force.unwrap_or(false));
}

/// 사업 탭의 식품 공장 카드를 el 안에 그려요
#[wasm_bindgen]
pub fn work_factory_into(el: JsValue, cid: String) {
    if let Ok(el) = el.dyn_into::<web_sys::HtmlElement>() {
        ui::factory::factory_into(el, cid);
    }
}

/// 사업 탭: 판매 목록, 창고, 직원 채용, 연구실, 팔 상품 정하기, 블록 옮기기
#[wasm_bindgen]
pub fn business_init(ctx: JsValue) {
    ui::biz::init(ui::ctx::Ctx(ctx));
}

/// 판매 목록과 연구실을 다시 읽어요
#[wasm_bindgen]
pub fn business_refresh() {
    ui::biz::refresh();
}

/// 연구실 숫자만 다시 읽어요
#[wasm_bindgen]
pub fn business_lab() {
    ui::biz::lab_refresh();
}

/// 상품 그림 HTML (상점·인벤토리·판매 목록이 같이 써요)
#[wasm_bindgen]
pub fn business_pkv(p: JsValue, mini: Option<bool>, co: JsValue) -> String {
    ui::biz::pkv(&p, mini.unwrap_or(false), &co)
}

/// 회사 종류 그림
#[wasm_bindgen]
pub fn business_kind_svg(k: String) -> String {
    goods::catalog::kind_svg(&k)
}
