//! 자판이 올라왔을 때: 아이폰은 자판이 올라와도 화면 틀(100dvh)이 줄지 않아서, 입력창이 자판 위로 붙지 않고 붕 떠 있었어요.
//! 실제로 보이는 크기(visualViewport)를 따라 앱 틀의 높이와 위치를 맞춰서, 입력창이 늘 자판 바로 위에 붙어요.

use std::cell::Cell;

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

thread_local! {
    static PENDING: Cell<bool> = const { Cell::new(false) };
}

fn vv() -> Option<JsValue> {
    let w = web_sys::window()?;
    Reflect::get(&w, &"visualViewport".into()).ok().filter(|v| v.is_object())
}
fn num(o: &JsValue, k: &str) -> f64 {
    Reflect::get(o, &k.into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn apply() {
    PENDING.set(false);
    let (Some(v), Some(w), Some(root)) = (vv(), web_sys::window(), crate::dom::doc().document_element()) else { return };
    let full = w.inner_height().ok().and_then(|x| x.as_f64()).unwrap_or(0.0);
    let (h, top) = (num(&v, "height"), num(&v, "offsetTop"));
    let st = root.unchecked_ref::<web_sys::HtmlElement>().style();
    // 자판이 올라와 보이는 높이가 확 줄었을 때만 맞춰요 (주소창이 줄었다 늘었다 하는 정도는 그대로 둬요)
    if h > 0.0 && full - h > 120.0 {
        let _ = st.set_property("--vvh", &format!("{h}px"));
        let _ = st.set_property("--vvt", &format!("{top}px"));
        crate::dom::toggle(&root, "kb-open", true);
    } else {
        let _ = st.remove_property("--vvh");
        let _ = st.remove_property("--vvt");
        crate::dom::toggle(&root, "kb-open", false);
    }
}
fn schedule() {
    if PENDING.replace(true) {
        return;
    }
    let cb = Closure::once_into_js(apply);
    let _ = web_sys::window().map(|w| w.request_animation_frame(cb.unchecked_ref()));
}

pub fn init() {
    let Some(v) = vv() else { return };
    let Ok(t) = v.dyn_into::<web_sys::EventTarget>() else { return };
    for ev in ["resize", "scroll"] {
        crate::dom::on(&t, ev, |_| schedule());
    }
    // 자판이 닫힐 때 아이폰이 남겨 두는 스크롤을 되돌려요
    crate::dom::on(&crate::dom::doc(), "focusout", |_| {
        let cb = Closure::once_into_js(|| {
            if let Some(w) = web_sys::window() {
                w.scroll_to_with_x_and_y(0.0, 0.0);
            }
            apply();
        });
        let _ = web_sys::window().map(|w| w.set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 120));
    });
}
