//! 앱 안 알림: 새 메시지가 오면 아이폰 알림처럼 위쪽에 유리 판이 내려와요 (사람 동그라미, 이름, 글, "지금").
//! 누르면 그 대화가 열리고, 위로 밀거나 4초가 지나면 올라가 사라져요.

use std::cell::{Cell, RefCell};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::dom;

thread_local! {
    static EL: RefCell<Option<web_sys::HtmlElement>> = const { RefCell::new(None) };
    static TIMER: Cell<i32> = const { Cell::new(0) };
    static OPEN_ID: RefCell<String> = const { RefCell::new(String::new()) };
    static START_Y: Cell<Option<f64>> = const { Cell::new(None) };
}

fn win() -> web_sys::Window {
    web_sys::window().expect("window")
}

fn el() -> Option<web_sys::HtmlElement> {
    if let Some(e) = EL.with(|e| e.borrow().clone()) {
        return Some(e);
    }
    let app = dom::app_el()?;
    let e: web_sys::HtmlElement = dom::doc().create_element("div").ok()?.dyn_into().ok()?;
    e.set_class_name("note-banner lg-card");
    let _ = e.set_attribute("role", "status");
    let _ = e.set_attribute("aria-live", "polite");
    e.set_hidden(true);
    let _ = app.append_child(&e);
    dom::on(&e, "click", |_| {
        let id = OPEN_ID.with(|o| o.borrow().clone());
        hide();
        if !id.is_empty() {
            crate::chat::thread::open(&id);
        }
    });
    dom::on(&e, "pointerdown", |ev| START_Y.set(js_sys::Reflect::get(&ev, &"clientY".into()).ok().and_then(|v| v.as_f64())));
    dom::on(&e, "pointerup", |ev| {
        let y = js_sys::Reflect::get(&ev, &"clientY".into()).ok().and_then(|v| v.as_f64());
        if let (Some(a), Some(b)) = (START_Y.take(), y) {
            if a - b > 24.0 {
                ev.prevent_default();
                hide(); // 위로 밀면 바로 올라가요
            }
        }
    });
    EL.with(|x| *x.borrow_mut() = Some(e.clone()));
    Some(e)
}

/// 알림 띄우기
pub fn show(chat_id: &str, name: &str, photo: &str, text: &str) {
    let Some(e) = el() else { return };
    OPEN_ID.with(|o| *o.borrow_mut() = chat_id.to_string());
    e.set_inner_html(&format!(
        r#"<span class="nb-av">{}<i class="nb-app" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M4 12.31 20 5 13.66 21 10.13 13.88Z"/><path d="M10.13 13.88 20 5"/></svg></i></span><span class="nb-main"><b>{}</b><span>{}</span></span><span class="nb-time">지금</span>"#,
        dom::avatar_html(name, photo, "avatar contact"),
        dom::esc(name),
        dom::esc(&dom::cut(text, 80))
    ));
    e.set_hidden(false);
    let _ = e.class_list().remove_1("out");
    // 다시 그려서 내려오는 움직임이 처음부터 보이게 해요
    let _ = e.offset_width();
    let _ = e.class_list().add_1("in");
    dom::vibrate(12);
    system(chat_id, name, text);
    win().clear_timeout_with_handle(TIMER.get());
    let cb = Closure::once_into_js(hide);
    TIMER.set(win().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 4000).unwrap_or(0));
}

pub fn hide() {
    let Some(e) = EL.with(|e| e.borrow().clone()) else { return };
    if e.hidden() {
        return;
    }
    let _ = e.class_list().remove_1("in");
    let _ = e.class_list().add_1("out");
    let e2 = e.clone();
    let cb = Closure::once_into_js(move || {
        if e2.class_list().contains("out") {
            e2.set_hidden(true);
        }
    });
    let _ = win().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 420);
}

/* ---------- 휴대폰 알림 (앱이 뒤에 있거나 다른 화면일 때) ---------- */

use js_sys::Reflect;

fn get(o: &JsValue, k: &str) -> JsValue {
    Reflect::get(o, &JsValue::from_str(k)).unwrap_or(JsValue::UNDEFINED)
}
fn call(o: &JsValue, f: &str, args: &[JsValue]) -> JsValue {
    let Ok(func) = get(o, f).dyn_into::<js_sys::Function>() else { return JsValue::UNDEFINED };
    func.apply(o, &args.iter().cloned().collect::<js_sys::Array>()).unwrap_or(JsValue::UNDEFINED)
}
/// 알림 권한 ("granted", "denied", "default", 없으면 "")
pub fn permission() -> String {
    get(&get(&js_sys::global(), "Notification"), "permission").as_string().unwrap_or_default()
}
fn sw() -> JsValue {
    get(&win().navigator().into(), "serviceWorker")
}
/// 서비스 워커를 등록하고, 알림을 누르면 그 대화를 열어요
pub fn init_system() {
    let c = sw();
    if c.is_undefined() {
        return;
    }
    let _ = call(&c, "register", &[JsValue::from_str("sw.js")]);
    let cb = Closure::<dyn FnMut(JsValue)>::new(|e: JsValue| {
        let d = get(&e, "data");
        if get(&d, "type").as_string().as_deref() == Some("open-chat") {
            if let Some(id) = get(&d, "chat").as_string().filter(|s| !s.is_empty()) {
                crate::chat::thread::open(&id);
            }
        }
    })
    .into_js_value();
    let _ = call(&c, "addEventListener", &[JsValue::from_str("message"), cb]);
}
/// 켜기: 권한을 물어요 (버튼을 누른 순간에만 물을 수 있어요). 허락되면 true
pub async fn ask_permission() -> bool {
    let n = get(&js_sys::global(), "Notification");
    if n.is_undefined() {
        return false;
    }
    let p = call(&n, "requestPermission", &[]);
    let r = match p.dyn_into::<js_sys::Promise>() {
        Ok(p) => wasm_bindgen_futures::JsFuture::from(p).await.ok().and_then(|v| v.as_string()).unwrap_or_default(),
        Err(_) => permission(),
    };
    r == "granted"
}
/// 알림을 켜 두었고, 앱이 안 보이고(뒤에 있음), 권한이 있으면 휴대폰 알림을 띄워요
fn system(chat_id: &str, name: &str, text: &str) {
    if dom::store_get("notify-on", "") != "1" || permission() != "granted" || !dom::doc().hidden() {
        return;
    }
    let c = sw();
    if c.is_undefined() {
        return;
    }
    let (chat_id, title, body) = (chat_id.to_string(), name.to_string(), dom::cut(text, 120));
    wasm_bindgen_futures::spawn_local(async move {
        let ready = get(&c, "ready");
        let Ok(p) = ready.dyn_into::<js_sys::Promise>() else { return };
        let Ok(reg) = wasm_bindgen_futures::JsFuture::from(p).await else { return };
        let opts = dom::obj(&[
            ("body", JsValue::from_str(&body)),
            ("icon", JsValue::from_str("assets/icons/icon-192.png")),
            ("badge", JsValue::from_str("assets/icons/icon-192.png")),
            ("tag", JsValue::from_str(&format!("chat-{chat_id}"))),
            ("data", dom::obj(&[("chat", JsValue::from_str(&chat_id))])),
        ]);
        let _ = call(&reg, "showNotification", &[JsValue::from_str(&title), opts]);
    });
}
