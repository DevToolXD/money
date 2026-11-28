//! 밀어서 답장: 말풍선을 오른쪽으로 밀면 아이폰 메시지처럼 그 글에 답장해요.
//! 밀면 말풍선이 손가락을 따라 오른쪽으로 움직이며 왼쪽에 답장 화살표가 나타나고, 충분히 밀고 놓으면 입력줄 위에 "OO에게 답장" 줄이 생겨요.
//! 보내면 글 말풍선 위에 답장하는 글의 앞부분(80자)이 인용돼요 (서버 규칙: 메시지 're').

use std::cell::{Cell, RefCell};

use wasm_bindgen::JsCast;

use super::{open_chat, thread};
use crate::dom::{self, by_id, esc};

const PULL: f64 = 64.0; // 이만큼 밀고 놓으면 답장
const MAX: f64 = 84.0;
const X_SVG: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18"/></svg>"#;

#[derive(Default)]
struct Drag {
    id: i32,
    x0: f64,
    y0: f64,
    dx: f64,
    row: Option<web_sys::HtmlElement>,
    horizontal: bool,
    armed: bool,
}

thread_local! {
    static REPLY: RefCell<Option<String>> = const { RefCell::new(None) };
    static DRAG: RefCell<Option<Drag>> = const { RefCell::new(None) };
    static BOUND: Cell<bool> = const { Cell::new(false) };
}

/// 보낼 답장 인용 (보내면서 한 번 가져가요)
pub fn take() -> Option<String> {
    let r = REPLY.with(|r| r.borrow_mut().take());
    if r.is_some() {
        remove_bar();
    }
    r
}

/// 대화를 나가거나 다른 대화로 바뀌면 답장 줄을 치워요
pub fn clear() {
    REPLY.with(|r| *r.borrow_mut() = None);
    remove_bar();
}

fn remove_bar() {
    if let Some(b) = by_id("reply-bar") {
        b.remove();
    }
}

fn set(who: &str, quote: &str) {
    let q: String = dom::cut(quote, 80);
    if q.trim().is_empty() {
        return;
    }
    REPLY.with(|r| *r.borrow_mut() = Some(q.clone()));
    remove_bar();
    let Some(f) = by_id("composer") else { return };
    let _ = f.insert_adjacent_html(
        "beforebegin",
        &format!(
            r#"<div class="reply-bar" id="reply-bar"><span class="rb-main"><b>{} 답장</b><span>{}</span></span><button type="button" class="rb-x" id="reply-x" aria-label="답장 취소">{X_SVG}</button></div>"#,
            esc(&dom::cut(who, 20)),
            esc(&q)
        ),
    );
    if let Some(x) = by_id("reply-x") {
        dom::on(&x, "click", |_| clear());
    }
    if let Some(i) = by_id("composer-input") {
        let _ = i.focus();
    }
}

/// 이 줄(.mrow)의 글: (누구에게, 앞부분)
fn quote_of(row: &web_sys::HtmlElement) -> Option<(String, String)> {
    let c = open_chat()?;
    let i: usize = dom::qs(row, "[data-i]")?.get_attribute("data-i")?.parse().ok()?;
    let m = c.msgs.get(i)?;
    let who = if m.me { "나".to_string() } else if c.group && !m.by.is_empty() { m.by.clone() } else { c.name.clone() };
    Some((who, thread::preview(&m.raw)))
}

fn set_pull(row: &web_sys::HtmlElement, dx: f64) {
    let line = dom::qs(row, ".line");
    let eff = (dx * 0.62).clamp(0.0, MAX);
    if let Some(l) = line {
        let st = l.style();
        let _ = st.set_property("--rx", &format!("{eff}px"));
        let _ = st.set_property("--rp", &format!("{:.2}", (eff / (PULL * 0.62)).min(1.0)));
        let _ = l.class_list().add_1("swp");
    }
}
fn release(row: &web_sys::HtmlElement) {
    if let Some(l) = dom::qs(row, ".line") {
        let st = l.style();
        let _ = st.remove_property("--rx");
        let _ = st.remove_property("--rp");
        let _ = l.class_list().remove_1("swp");
    }
}

pub fn init() {
    if BOUND.replace(true) {
        return;
    }
    let Some(b) = by_id("bubbles") else { return };
    dom::on(&b, "pointerdown", |e| {
        let Some(p) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
        if p.pointer_type() == "mouse" && p.button() != 0 {
            return;
        }
        // 화면 왼쪽 끝에서 시작한 건 뒤로 가기 끌기예요
        if p.client_x() < 24 {
            return;
        }
        let Some(row) = dom::closest(&e, ".mrow") else { return };
        // 버튼(받기·투표·재생)을 누른 건 답장이 아니에요
        if dom::closest(&e, "button, a, input").is_some() {
            return;
        }
        DRAG.with(|d| *d.borrow_mut() = Some(Drag { id: p.pointer_id(), x0: p.client_x() as f64, y0: p.client_y() as f64, row: Some(row), ..Drag::default() }));
    });
    dom::on(&b, "pointermove", |e| {
        let Some(p) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
        DRAG.with(|d| {
            let mut d = d.borrow_mut();
            let Some(g) = d.as_mut() else { return };
            if g.id != p.pointer_id() {
                return;
            }
            let (dx, dy) = (p.client_x() as f64 - g.x0, p.client_y() as f64 - g.y0);
            if !g.horizontal {
                if dy.abs() > 10.0 && dy.abs() > dx.abs() {
                    *d = None; // 위아래로 넘기는 중
                    return;
                }
                if dx > 10.0 && dx > dy.abs() * 1.3 {
                    g.horizontal = true;
                } else {
                    return;
                }
            }
            e.prevent_default();
            g.dx = dx.max(0.0);
            let armed = g.dx >= PULL;
            if armed && !g.armed {
                dom::vibrate(8); // 여기서 놓으면 답장이라는 작은 진동
            }
            g.armed = armed;
            if let Some(r) = &g.row {
                set_pull(r, g.dx);
            }
        });
    });
    for ev in ["pointerup", "pointercancel"] {
        dom::on(&b, ev, move |e| {
            let g = DRAG.with(|d| d.borrow_mut().take());
            let Some(g) = g else { return };
            let Some(row) = g.row else { return };
            let go = g.horizontal && g.armed && e.type_() == "pointerup";
            release(&row);
            if go {
                if let Some((who, q)) = quote_of(&row) {
                    set(&who, &q);
                }
            }
        });
    }
}
