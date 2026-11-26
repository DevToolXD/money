//! 자판이 올라왔을 때: 아이폰은 자판이 올라와도 화면 틀(100dvh)이 줄지 않아서, 입력창이 자판 위로 붙지 않고 붕 떠 있었어요.
//! 실제로 보이는 크기(visualViewport)를 따라 앱 틀의 높이와 위치를 맞춰서, 입력창이 늘 자판 바로 위에 붙어요.

use std::cell::Cell;

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

thread_local! {
    /// 자판이 없을 때 본 가장 큰 화면 높이. iOS 26처럼 자판이 올라오면 innerHeight까지 같이 줄어드는 경우에도 자판을 알아채요
    static BASE: Cell<f64> = const { Cell::new(0.0) };
}

/// 글을 쓰는 칸에 초점이 있나 (자판이 올라와 있을 수 있는 때)
fn typing() -> bool {
    crate::dom::doc().active_element().is_some_and(|a| {
        let tag = a.tag_name();
        (tag == "INPUT" && !matches!(a.get_attribute("type").as_deref(), Some("checkbox" | "radio" | "button" | "submit" | "range" | "file"))) || tag == "TEXTAREA" || a.get_attribute("contenteditable").is_some_and(|v| v != "false")
    })
}

fn vv() -> Option<JsValue> {
    let w = web_sys::window()?;
    Reflect::get(&w, &"visualViewport".into()).ok().filter(|v| v.is_object())
}
fn num(o: &JsValue, k: &str) -> f64 {
    Reflect::get(o, &k.into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn apply() {
    let (Some(v), Some(w), Some(root)) = (vv(), web_sys::window(), crate::dom::doc().document_element()) else { return };
    let full = w.inner_height().ok().and_then(|x| x.as_f64()).unwrap_or(0.0);
    let (h, top) = (num(&v, "height"), num(&v, "offsetTop"));
    let st = root.unchecked_ref::<web_sys::HtmlElement>().style();
    let typing = typing();
    if !typing {
        BASE.set(BASE.get().max(full).max(h));
    }
    let base = BASE.get().max(full);
    // 자판이 올라와 보이는 높이가 확 줄었을 때만 맞춰요 (주소창이 줄었다 늘었다 하는 정도는 그대로 둬요).
    // innerHeight가 같이 줄어드는 iOS 26도, 자판이 없을 때의 높이(base)와 비교해서 알아채요
    if h > 0.0 && (full - h > 120.0 || (typing && base - h > 120.0)) {
        let _ = st.set_property("--vvh", &format!("{h}px"));
        let _ = st.set_property("--vvt", &format!("{top}px"));
        crate::dom::toggle(&root, "kb-open", true);
    } else {
        let _ = st.remove_property("--vvh");
        let _ = st.remove_property("--vvt");
        crate::dom::toggle(&root, "kb-open", false);
    }
}
/// 크기 알림이 오면 바로 맞춰요. 다음 프레임으로 미루면 자판이 올라오는 동안 한 프레임씩 늦어서 판이 아래로 튀었다 올라와요
fn schedule() {
    apply();
}

pub fn init() {
    let Some(v) = vv() else { return };
    let Ok(t) = v.dyn_into::<web_sys::EventTarget>() else { return };
    for ev in ["resize", "scroll"] {
        crate::dom::on(&t, ev, |_| schedule());
    }
    crate::dom::on(&crate::dom::doc(), "focusin", |_| {
        schedule();
        // 자판이 다 올라온 뒤에 한 번 더 맞춰요 (자판이 올라오는 동안에는 크기 알림이 늦게 오기도 해요)
        let cb = Closure::once_into_js(|| {
            schedule();
            // 로그인 판처럼 글칸이 가운데쯤 있는 곳은, 자판에 가리지 않게 초점 간 칸을 보이는 곳으로 올려요
            if let Some(a) = crate::dom::doc().active_element() {
                if a.closest(".login-body, .sheet, .ci-panel").ok().flatten().is_some() {
                    if let Ok(f) = Reflect::get(&a, &"scrollIntoView".into()).and_then(|f| f.dyn_into::<js_sys::Function>()) {
                        let _ = f.call1(&a, &crate::dom::obj(&[("block", "center".into())]));
                    }
                }
            }
        });
        let _ = web_sys::window().map(|w| w.set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 350));
    });
    if let Some(w) = web_sys::window() {
        crate::dom::on(&w, "resize", |_| schedule());
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
