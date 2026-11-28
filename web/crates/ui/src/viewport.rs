//! 자판이 올라왔을 때: 아이폰은 자판이 올라와도 화면 틀(100dvh)이 줄지 않아서, 입력창이 자판 위로 붙지 않고 붕 떠 있었어요.
//! 실제로 보이는 크기(visualViewport)를 따라 앱 틀의 높이와 위치를 맞춰서, 입력창이 늘 자판 바로 위에 붙어요.

use std::cell::Cell;

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

thread_local! {
    /// 자판이 없을 때 본 가장 큰 화면 높이. iOS 26처럼 자판이 올라오면 innerHeight까지 같이 줄어드는 경우에도 자판을 알아채요
    static BASE: Cell<f64> = const { Cell::new(0.0) };
    /// 지난번에 본 자판 높이 (저장해 둬요). 글칸을 누르는 순간 이만큼 먼저 줄여서, 아이폰이 화면을 위로 밀어 올릴 일이 없게 해요
    static KB_EST: Cell<f64> = const { Cell::new(-1.0) };
    /// 이 시각까지는 자판이 올라오는 중이라, 자판이 반쯤 올라온 크기 알림은 무시해요
    static SETTLE_UNTIL: Cell<f64> = const { Cell::new(0.0) };
}

const SETTLE_MS: f64 = 700.0;

fn kb_est() -> f64 {
    if KB_EST.get() < 0.0 {
        KB_EST.set(crate::dom::store_get("kb-h", "0").parse::<f64>().ok().filter(|v| (150.0..=600.0).contains(v)).unwrap_or(0.0));
    }
    KB_EST.get()
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
    let now = js_sys::Date::now();
    let settling = now < SETTLE_UNTIL.get();
    if h > 0.0 && (full - h > 120.0 || (typing && base - h > 120.0)) {
        // 자판이 올라오는 중에는 반쯤 올라온 크기로 앱 틀이 커졌다 줄었다 하지 않게, 먼저 맞춰 둔 크기보다 큰 것은 무시해요
        let est_h = base - kb_est();
        if settling && kb_est() > 0.0 && h > est_h + 40.0 {
            return;
        }
        let _ = st.set_property("--vvh", &format!("{h}px"));
        // 아이폰은 자판이 올라오면 보이는 화면(visual viewport)을 위로 밀어 올려요. offsetTop만큼 앱 틀을 내려서 보이는 자리에 맞춰요
        // (화면 틀이 같이 줄어드는 기기에서도 offsetTop을 안 따라가면 앱이 보이는 곳 위로 올라가 하얀 화면만 남아요)
        let _ = st.set_property("--vvt", &format!("{top}px"));
        crate::dom::toggle(&root, "kb-open", true);
        // 자판이 다 올라온 뒤의 높이를 기억해요 (다음에 글칸을 누를 때 먼저 맞춰 두려고)
        if !settling && typing {
            let kb = (base - h).round();
            if (150.0..=600.0).contains(&kb) && (kb - KB_EST.get()).abs() > 2.0 {
                KB_EST.set(kb);
                crate::dom::store_set("kb-h", &kb.to_string());
            }
        }
    } else if settling && kb_est() > 0.0 {
        // 자판이 아직 안 올라온 알림이에요. 먼저 맞춰 둔 크기를 그대로 둬요
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
        // 글칸을 누른 순간, 지난번에 본 자판 높이만큼 앱 틀을 먼저 줄여요. 입력줄이 처음부터 자판이 올라올 자리 위에 있어서
        // 아이폰이 화면을 위로 밀어 올렸다 내리는 일(버벅임)이 없어요. 실제 자판 높이가 오면 맞춰요
        if typing() && kb_est() > 0.0 {
            if let Some(w) = web_sys::window() {
                let full = w.inner_height().ok().and_then(|x| x.as_f64()).unwrap_or(0.0);
                let base = BASE.get().max(full);
                if base > 0.0 {
                    if let Some(root) = crate::dom::doc().document_element() {
                        let st = root.unchecked_ref::<web_sys::HtmlElement>().style();
                        let _ = st.set_property("--vvh", &format!("{}px", base - kb_est()));
                        let _ = st.set_property("--vvt", "0px");
                        crate::dom::toggle(&root, "kb-open", true);
                        SETTLE_UNTIL.set(js_sys::Date::now() + SETTLE_MS);
                        let cb = Closure::once_into_js(apply);
                        let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), SETTLE_MS as i32 + 60);
                    }
                }
            }
        }
        schedule();
        // 자판이 다 올라온 뒤에 한 번 더 맞춰요 (자판이 올라오는 동안에는 크기 알림이 늦게 오기도 해요)
        let cb = Closure::once_into_js(|| {
            schedule();
            // 글칸이 가운데쯤 있는 판(로그인, 글쓰기, 거래·놀기 글쓰기, 정보 판)은 자판에 가리지 않게 초점 간 칸을 보이는 곳으로 올려요.
            // 대화 입력줄과 릴스 댓글줄은 늘 맨 아래 붙어 있어서 건드리지 않아요
            if let Some(a) = crate::dom::doc().active_element() {
                let fixed_bar = a.closest("#composer, .rc-bar").ok().flatten().is_some();
                if !fixed_bar && a.closest(".login-body, .sheet, .ci-panel, .sns-panel, .pl-form").ok().flatten().is_some() {
                    if let Ok(f) = Reflect::get(&a, &"scrollIntoView".into()).and_then(|f| f.dyn_into::<js_sys::Function>()) {
                        let _ = f.call1(&a, &crate::dom::obj(&[("block", "center".into()), ("behavior", "smooth".into())]));
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
