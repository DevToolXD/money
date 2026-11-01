//! 시작점: JS 쪽(`app/rs/boot.js`)이 불러 가는 함수들이에요.
//! 예전 JS 모듈과 같은 이름(`window.Notices` 등)으로 연결돼서, 나머지 JS는 바꾸지 않아도 돼요.

use wasm_bindgen::prelude::*;

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
