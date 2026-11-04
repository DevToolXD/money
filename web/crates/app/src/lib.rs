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

/// 돈 탭: 잔액, 이자 상품, 거래내역, 카드 순서 (지갑 숫자는 window.__bank 에 비춰 둬요)
#[wasm_bindgen]
pub fn money_init(ctx: JsValue) {
    ui::money::init(ui::ctx::Ctx(ctx));
}

/// 서버 지갑을 듣기 시작해요 (로그인한 뒤)
#[wasm_bindgen]
pub fn money_start_wallet() {
    ui::money::start_wallet();
}

/// 지갑 쓰기 줄 세우기: f(약속을 돌려주는 함수)를 2.2초 간격으로 불러요
#[wasm_bindgen]
pub fn money_wallet_op(f: js_sys::Function) -> js_sys::Promise {
    ui::money::wallet_op(f)
}

/// 쌓인 이자를 정산해요
#[wasm_bindgen]
pub fn money_claim() -> js_sys::Promise {
    ui::money::claim()
}

/// 거래 한 줄 남기기 ({t, name, cat, icon, amt})
#[wasm_bindgen]
pub fn money_log(e: JsValue) {
    ui::money::log(&e);
}

/// 돈 탭 숫자를 다시 써요 (바뀐 것만)
#[wasm_bindgen]
pub fn money_render() {
    ui::money::render();
}

/// 메시지 탭: 대화 목록, 대화방, 새 대화·단톡방, 사진·그리기·투표·토큰 선물
#[wasm_bindgen]
pub fn chat_init(ctx: JsValue) {
    ui::chat::init(ui::ctx::Ctx(ctx));
}

/// 로그인한 뒤 대화 목록을 실시간으로 듣기 시작해요
#[wasm_bindgen]
pub fn chat_start() {
    ui::chat::start();
}

/// 대화 목록을 다시 그려요
#[wasm_bindgen]
pub fn chat_render() {
    ui::chat::render();
}

/// 다른 탭으로 가면 열린 대화를 닫아요
#[wasm_bindgen]
pub fn chat_leave(tab: String) {
    ui::chat::leave(&tab);
}
