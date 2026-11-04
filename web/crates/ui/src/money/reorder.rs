//! 꾹 눌러서 카드 순서 바꾸기 (잔액 칸은 고정). 450ms 누르면 들어 올리고, 손을 따라 옮기고, 놓으면 이 기기에 순서를 저장해요

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::dom;

struct St {
    el: web_sys::HtmlElement,
    x0: f64,
    y0: f64,
    y: f64,
    active: bool,
    grab: f64,
    timer: i32,
    raf: i32,
}
thread_local! {
    static ST: RefCell<Option<St>> = const { RefCell::new(None) };
    static SWALLOW: Cell<bool> = const { Cell::new(false) };
    static ROOT: RefCell<Option<web_sys::HtmlElement>> = const { RefCell::new(None) };
    static LOOP: RefCell<Option<Closure<dyn FnMut()>>> = const { RefCell::new(None) };
}
/// 끌어서 옮긴 직후에는 카드를 연 것으로 치지 않아요
pub fn swallow() -> bool {
    SWALLOW.get()
}
fn win() -> web_sys::Window {
    web_sys::window().expect("window")
}
fn scr() -> Option<web_sys::HtmlElement> {
    dom::doc().query_selector(".screen[data-screen=\"money\"]").ok().flatten()?.dyn_into().ok()
}
fn root() -> Option<web_sys::HtmlElement> {
    ROOT.with(|r| r.borrow().clone())
}
fn content_y(y: f64) -> f64 {
    let Some(s) = scr() else { return y };
    y - s.get_bounding_client_rect().top() + s.scroll_top() as f64
}
fn children(root: &web_sys::HtmlElement) -> Vec<web_sys::HtmlElement> {
    let c = root.children();
    (0..c.length()).filter_map(|i| c.item(i)?.dyn_into().ok()).collect()
}
fn place() {
    ST.with(|s| {
        if let Some(st) = s.borrow().as_ref() {
            let _ = st.el.style().set_property("transform", &format!("translateY({}px) scale(1.03)", content_y(st.y) - st.grab - st.el.offset_top() as f64));
        }
    });
}
fn reorder() {
    let Some(root) = root() else { return };
    let Some((el, y, grab)) = ST.with(|s| s.borrow().as_ref().map(|s| (s.el.clone(), s.y, s.grab))) else { return };
    let sibs: Vec<_> = children(&root).into_iter().filter(|c| *c != el).collect();
    let center = content_y(y) - grab + el.offset_height() as f64 / 2.0;
    let before = sibs.iter().find(|s| center < s.offset_top() as f64 + s.offset_height() as f64 / 2.0).cloned();
    let next: Option<web_sys::HtmlElement> = el.next_element_sibling().and_then(|n| n.dyn_into().ok());
    if before == next {
        return;
    }
    let tops: Vec<i32> = sibs.iter().map(|s| s.offset_top()).collect();
    let _ = root.insert_before(&el, before.as_ref().map(|b| b.as_ref()));
    for (s, top) in sibs.iter().zip(tops) {
        let d = top - s.offset_top();
        if d == 0 {
            continue;
        }
        let st = s.style();
        let _ = st.set_property("transition", "none");
        let _ = st.set_property("transform", &format!("translateY({d}px)"));
        let _ = s.offset_height();
        let _ = st.set_property("transition", "transform 0.3s cubic-bezier(0.22, 1, 0.36, 1)");
        let _ = st.set_property("transform", "");
    }
}
fn frame() {
    let active = ST.with(|s| s.borrow().as_ref().is_some_and(|s| s.active));
    if !active {
        return;
    }
    if let Some(s) = scr() {
        let r = s.get_bounding_client_rect();
        let y = ST.with(|st| st.borrow().as_ref().map(|s| s.y).unwrap_or(0.0));
        let speed = if y < r.top() + 120.0 {
            -10
        } else if y > r.bottom() - 170.0 {
            10
        } else {
            0
        };
        if speed != 0 {
            s.set_scroll_top(s.scroll_top() + speed);
        }
    }
    place();
    reorder();
    let id = LOOP.with(|l| l.borrow().as_ref().and_then(|c| win().request_animation_frame(c.as_ref().unchecked_ref()).ok())).unwrap_or(0);
    ST.with(|s| {
        if let Some(s) = s.borrow_mut().as_mut() {
            s.raf = id;
        }
    });
}
fn end() {
    let Some(s) = ST.with(|s| s.borrow_mut().take()) else { return };
    win().clear_timeout_with_handle(s.timer);
    if !s.active {
        return;
    }
    let _ = win().cancel_animation_frame(s.raf);
    let el = s.el;
    let st = el.style();
    let _ = st.set_property("transition", "transform 0.35s cubic-bezier(0.22, 1, 0.36, 1), box-shadow 0.35s");
    let _ = st.set_property("transform", "");
    let _ = el.class_list().remove_1("lifting");
    let el2 = el.clone();
    leptos::prelude::set_timeout(
        move || {
            let _ = el2.style().set_property("transition", "");
        },
        std::time::Duration::from_millis(380),
    );
    if let Some(root) = root() {
        dom::store_set("money-order", &children(&root).iter().map(|c| dom::data(c, "id")).collect::<Vec<_>>().join(","));
    }
    leptos::prelude::set_timeout(|| SWALLOW.set(false), std::time::Duration::from_millis(120));
}

pub fn init(root: web_sys::HtmlElement) {
    ROOT.with(|r| *r.borrow_mut() = Some(root.clone()));
    LOOP.with(|l| *l.borrow_mut() = Some(Closure::<dyn FnMut()>::new(frame)));
    dom::on(&root, "pointerdown", |e| {
        let Some(pe) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
        let Some(el) = dom::closest(&e, ".movable") else { return };
        if ST.with(|s| s.borrow().is_some()) || (pe.pointer_type() == "mouse" && pe.button() != 0) {
            return;
        }
        let (x, y) = (pe.client_x() as f64, pe.client_y() as f64);
        let lift = Closure::once_into_js(move || {
            let el = ST.with(|s| {
                let mut s = s.borrow_mut();
                let st = s.as_mut()?;
                st.active = true;
                st.grab = content_y(st.y) - st.el.offset_top() as f64;
                Some(st.el.clone())
            });
            let Some(el) = el else { return };
            SWALLOW.set(true);
            dom::vibrate(14);
            let _ = el.style().set_property("transition", "box-shadow 0.25s");
            let _ = el.class_list().add_1("lifting");
            frame();
        });
        let timer = win().set_timeout_with_callback_and_timeout_and_arguments_0(lift.unchecked_ref(), 450).unwrap_or(0);
        ST.with(|s| *s.borrow_mut() = Some(St { el, x0: x, y0: y, y, active: false, grab: 0.0, timer, raf: 0 }));
    });
    dom::on(&win(), "pointermove", |e| {
        let Some(pe) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
        let (x, y) = (pe.client_x() as f64, pe.client_y() as f64);
        let cancel = ST.with(|s| {
            let mut s = s.borrow_mut();
            let Some(st) = s.as_mut() else { return false };
            st.y = y;
            !st.active && (x - st.x0).hypot(y - st.y0) > 10.0
        });
        if cancel {
            if let Some(s) = ST.with(|s| s.borrow_mut().take()) {
                win().clear_timeout_with_handle(s.timer);
            }
        }
    });
    dom::on(&win(), "pointerup", |_| end());
    dom::on(&win(), "pointercancel", |_| end());
    // 끌어올린 뒤에는 화면이 같이 스크롤되지 않게 막아요
    dom::on_active(&dom::doc(), "touchmove", |e| {
        if ST.with(|s| s.borrow().as_ref().is_some_and(|s| s.active)) {
            e.prevent_default();
        }
    });
}
