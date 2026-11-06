//! 메뉴 버튼(탭 바 오른쪽 줄 세 개): 누르면 아이폰 메시지 + 메뉴와 같은 움직임으로 창이 버튼에서 자라나요.
//! 누르는 순간 버튼이 1.35배로 부풀고, 창은 버튼 자리에서 버튼 크기로 시작해 약 0.3초에 살짝 되튀며 커져요.
//! 버튼의 줄 세 개는 창 안에서 흐려지며 사라지고, 닫을 때는 되튐 없이 0.18초에 버튼으로 빨려 들어가요.
//! 탭 바(WebGL)가 이 버튼을 고르지 않게, 누름은 nav에서 먼저(캡처 단계) 받아서 멈춰요.

use std::cell::Cell;
use wasm_bindgen::JsCast;

use crate::dom::{self, by_id};

thread_local! {
    static OPEN: Cell<bool> = const { Cell::new(false) };
    static SEQ: Cell<u32> = const { Cell::new(0) };
}

const ITEMS: [(&str, &str, &str); 5] = [
    ("coin", "코인", r#"<circle cx="12" cy="12" r="8.5"/><path d="M12 7.5v9M14.6 9.4c-.5-.8-1.5-1.2-2.6-1.2-1.5 0-2.6.8-2.6 1.9 0 2.6 5.4 1.3 5.4 3.9 0 1.1-1.2 1.9-2.8 1.9-1.2 0-2.2-.5-2.7-1.3"/>"#),
    ("game", "게임", r#"<path d="M7.5 8h9a4.5 4.5 0 0 1 4.4 5.4l-.6 3a2.4 2.4 0 0 1-4 1.3L14.6 16H9.4l-1.7 1.7a2.4 2.4 0 0 1-4-1.3l-.6-3A4.5 4.5 0 0 1 7.5 8z"/><path d="M8 11v3M6.5 12.5h3"/><path d="M15.5 12.3h.01M17.5 13.6h.01"/>"#),
    ("account", "프로필", r#"<circle cx="12" cy="8.5" r="3.8"/><path d="M4.8 20c.8-3.6 3.6-5.6 7.2-5.6s6.4 2 7.2 5.6"/>"#),
    ("settings", "설정", r#"<circle cx="12" cy="12" r="3"/><path d="M12 3.5v2.2M12 18.3v2.2M3.5 12h2.2M18.3 12h2.2M6 6l1.6 1.6M16.4 16.4L18 18M6 18l1.6-1.6M16.4 7.6L18 6"/>"#),
    ("youtube", "유튜브", r#"<rect x="3" y="6" width="18" height="12" rx="3.5"/><path d="M10.5 9.5v5l4.2-2.5z"/>"#),
];

fn svg(d: &str) -> String {
    format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{d}</svg>"#)
}
fn btn() -> Option<web_sys::HtmlElement> {
    dom::doc().query_selector(".tabbar .tab-account").ok().flatten()?.dyn_into().ok()
}

fn root() -> Option<web_sys::HtmlElement> {
    if let Some(e) = by_id("qm") {
        return Some(e);
    }
    let app = dom::app_el()?;
    let el: web_sys::HtmlElement = dom::doc().create_element("div").ok()?.dyn_into().ok()?;
    el.set_id("qm");
    el.set_class_name("qm-wrap");
    el.set_hidden(true);
    let rows: String = ITEMS.iter().map(|(k, t, d)| format!(r#"<button type="button" class="qm-row" data-go="{k}" role="menuitem"><span class="qm-ic">{}</span><span>{t}</span></button>"#, svg(d))).collect();
    el.set_inner_html(&format!(
        r#"<div class="qm" role="menu" aria-label="메뉴"><div class="qm-in">{rows}</div><span class="qm-ghost" aria-hidden="true">{}</span></div>"#,
        svg(r#"<path d="M6 7.5h12M6 12h12M6 16.5h12"/>"#)
    ));
    let _ = app.append_child(&el);
    dom::on(&el, "click", |e| {
        if let Some(b) = dom::closest(&e, "[data-go]") {
            let to = dom::data(&b, "go");
            close();
            crate::shell::go_screen(&to);
            if to == "coin" {
                crate::coin::open();
            }
            return;
        }
        if dom::closest(&e, ".qm").is_none() {
            close();
        }
    });
    Some(el)
}

/// 창을 버튼 자리에 맞춰요: 오른쪽 아래 모서리를 버튼과 맞추고, 버튼 크기에서 시작해요
fn place(el: &web_sys::HtmlElement) {
    let (Some(b), Some(app), Some(qm)) = (btn(), dom::app_el(), el.query_selector(".qm").ok().flatten().and_then(|q| q.dyn_into::<web_sys::HtmlElement>().ok())) else { return };
    let r = b.get_bounding_client_rect();
    let a = app.get_bounding_client_rect();
    let st = qm.style();
    let _ = st.set_property("right", &format!("{}px", a.right() - r.right()));
    let _ = st.set_property("bottom", &format!("{}px", a.bottom() - r.top() + 10.0));
    let w = qm.offset_width() as f64;
    let h = qm.offset_height() as f64;
    let _ = st.set_property("--bw", &format!("{}px", r.width()));
    let _ = st.set_property("--sx", &format!("{}", (r.width() / w.max(1.0)).min(1.0)));
    let _ = st.set_property("--sy", &format!("{}", (r.height() / h.max(1.0)).min(1.0)));
}

pub fn open() {
    let Some(el) = root() else { return };
    if OPEN.replace(true) {
        return;
    }
    let seq = SEQ.get() + 1;
    SEQ.set(seq);
    el.set_hidden(false);
    let _ = el.class_list().remove_2("on", "closing");
    place(&el);
    let _ = el.offset_width();
    if let Some(b) = btn() {
        let _ = b.class_list().add_1("qm-press");
    }
    dom::vibrate(8);
    // 버튼이 부푼 다음 (약 2프레임) 창이 자라나요
    wasm_bindgen_futures::spawn_local(async move {
        dom::sleep(50).await;
        if SEQ.get() == seq {
            let _ = el.class_list().add_1("on");
        }
        dom::sleep(160).await;
        if let Some(b) = btn() {
            let _ = b.class_list().remove_1("qm-press");
        }
    });
}

pub fn close() {
    let Some(el) = by_id("qm") else { return };
    if !OPEN.replace(false) {
        return;
    }
    let seq = SEQ.get() + 1;
    SEQ.set(seq);
    let _ = el.class_list().add_1("closing");
    let _ = el.class_list().remove_1("on");
    wasm_bindgen_futures::spawn_local(async move {
        dom::sleep(200).await;
        if SEQ.get() == seq {
            let _ = el.class_list().remove_1("closing");
            el.set_hidden(true);
        }
    });
}

/// 탭 바의 메뉴 버튼 누름을 탭 바보다 먼저 받아요
pub fn init(nav: &web_sys::HtmlElement) {
    for ev in ["pointerdown", "pointerup", "pointermove", "click", "keydown"] {
        let cb = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(move |e: web_sys::Event| {
            let Some(t) = e.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) else { return };
            let on_btn = t.closest(".tab-account").ok().flatten().is_some()
                || e.dyn_ref::<web_sys::MouseEvent>().zip(btn()).is_some_and(|(m, b)| {
                    let r = b.get_bounding_client_rect();
                    let (x, y) = (m.client_x() as f64, m.client_y() as f64);
                    x >= r.left() && x <= r.right() && y >= r.top() && y <= r.bottom()
                });
            if !on_btn {
                return;
            }
            if ev == "keydown" {
                let k = e.dyn_ref::<web_sys::KeyboardEvent>().map(|k| k.key()).unwrap_or_default();
                if k != "Enter" && k != " " {
                    return;
                }
            }
            e.stop_immediate_propagation();
            e.prevent_default();
            if ev == "click" || ev == "keydown" {
                if OPEN.get() {
                    close();
                } else {
                    open();
                }
            }
        })
        .into_js_value();
        let o = web_sys::AddEventListenerOptions::new();
        o.set_capture(true);
        let _ = nav.add_event_listener_with_callback_and_add_event_listener_options(ev, cb.unchecked_ref(), &o);
    }
    if let Some(b) = btn() {
        let _ = b.set_attribute("aria-label", "메뉴");
        let _ = b.set_attribute("aria-haspopup", "menu");
    }
}
