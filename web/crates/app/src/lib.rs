//! 시작점: `app/rs/boot.js`가 WASM을 받은 뒤 `shell_start()` 하나만 불러요.
//! 화면 틀·탭 연결·로그인 흐름은 `ui::shell`, 각 화면은 `ui`의 모듈들이에요 (docs/rust-migration-plan.md).

use wasm_bindgen::prelude::*;

/// 앱 전체 시작: 화면 틀, 탭 바, 모든 화면을 붙이고 로그인 흐름을 시작해요
#[wasm_bindgen]
pub fn shell_start() {
    ui::shell::start();
}
