//! 사업 탭: 블록(상품·연구실·직원 채용·공장·창고)을 450ms 꾹 누르면 들어 올려 위아래로 옮겨요. 순서는 이 기기에 저장해요

use std::cell::RefCell;

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::dom::{self, by_id};

const KEY: &str = "pv-bizOrder";

struct Press {
    blk: web_sys::HtmlElement,
    x: f64,
    y: f64,
    t: i32,
}
struct Drag {
    blk: web_sys::HtmlElement,
    start_y: f64,
    last_y: f64,
    scroll0: f64,
    iv: i32,
    _cb: Closure<dyn FnMut()>,
}
thread_local! {
    static PRESS: RefCell<Option<Press>> = const { RefCell::new(None) };
    static DRAG: RefCell<Option<Drag>> = const { RefCell::new(None) };
    static MARKER: RefCell<Option<web_sys::Element>> = const { RefCell::new(None) };
}

fn win() -> web_sys::Window {
    web_sys::window().expect("window")
}
fn blocks(dash: &web_sys::Element) -> Vec<web_sys::HtmlElement> {
    dom::qsa(dash, ":scope > .biz-blk")
}
/// localStorage (개인 브라우징이면 없을 수 있어요)
fn storage() -> Option<JsValue> {
    Reflect::get(&win(), &JsValue::from_str("localStorage")).ok().filter(|s| !s.is_null() && !s.is_undefined())
}
fn load() -> Vec<String> {
    let Some(s) = storage() else { return Vec::new() };
    let v = Reflect::get(&s, &JsValue::from_str("getItem"))
        .ok()
        .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
        .and_then(|f| f.call1(&s, &JsValue::from_str(KEY)).ok())
        .and_then(|v| v.as_string())
        .unwrap_or_default();
    js_sys::JSON::parse(if v.is_empty() { "[]" } else { &v })
        .ok()
        .filter(|a| a.is_array())
        .map(|a| js_sys::Array::from(&a).iter().filter_map(|x| x.as_string()).collect())
        .unwrap_or_default()
}
fn save(dash: &web_sys::Element) {
    let Some(s) = storage() else { return };
    let arr = js_sys::Array::new();
    for b in blocks(dash) {
        arr.push(&JsValue::from_str(&dom::data(&b, "blk")));
    }
    let Ok(txt) = js_sys::JSON::stringify(&arr) else { return };
    if let Ok(f) = Reflect::get(&s, &JsValue::from_str("setItem")).and_then(|f| f.dyn_into::<js_sys::Function>()) {
        let _ = f.call2(&s, &JsValue::from_str(KEY), &txt);
    }
}
fn apply_order(dash: &web_sys::Element) {
    let saved = load();
    let mut list: Vec<(usize, usize, web_sys::HtmlElement)> = blocks(dash)
        .into_iter()
        .enumerate()
        .map(|(i, el)| {
            let k = dom::data(&el, "blk");
            (saved.iter().position(|s| *s == k).unwrap_or(100), i, el)
        })
        .collect();
    list.sort_by_key(|(rank, i, _)| (*rank, *i));
    for (_, _, el) in list {
        let _ = dash.append_child(&el);
    }
}

pub fn init() {
    let Some(dash) = by_id("biz-dash") else { return };
    apply_order(&dash);
    let d2 = dash.clone();
    dom::on(&dash, "pointerdown", move |e| {
        let Some(pe) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
        if pe.button() > 0 || dom::closest(&e, "button, input, textarea, select, a, [data-take]").is_some() {
            return;
        }
        let Some(blk) = dom::closest(&e, ".biz-blk") else { return };
        if blk.parent_element().as_ref() != Some(&*d2) {
            return;
        }
        let (x, y) = (pe.client_x() as f64, pe.client_y() as f64);
        let (b2, dash3) = (blk.clone(), d2.clone());
        let cb = Closure::once_into_js(move || lift(&dash3, b2, y));
        let t = win().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 450).unwrap_or(0);
        PRESS.with(|p| *p.borrow_mut() = Some(Press { blk, x, y, t }));
    });
    let d3 = dash.clone();
    dom::on_active(&dom::doc(), "pointermove", move |e| {
        let Some(pe) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
        let (cx, cy) = (pe.client_x() as f64, pe.client_y() as f64);
        let dragging = DRAG.with(|d| d.borrow().is_some());
        if !dragging {
            PRESS.with(|p| {
                let mut p = p.borrow_mut();
                if let Some(pr) = p.as_ref() {
                    if (cx - pr.x).hypot(cy - pr.y) > 8.0 {
                        win().clear_timeout_with_handle(pr.t);
                        *p = None;
                    }
                }
            });
            return;
        }
        e.prevent_default();
        DRAG.with(|d| {
            if let Some(d) = d.borrow_mut().as_mut() {
                d.last_y = cy;
                let _ = d.blk.style().set_property("transform", &format!("translateY({}px) scale(1.02)", cy - d.start_y + win().scroll_y().unwrap_or(0.0) - d.scroll0));
            }
        });
        place_marker(&d3);
    });
    let d4 = dash.clone();
    let end = move |_e: web_sys::Event| {
        if let Some(pr) = PRESS.with(|p| p.borrow_mut().take()) {
            win().clear_timeout_with_handle(pr.t);
            let _ = pr.blk;
        }
        let Some(d) = DRAG.with(|d| d.borrow_mut().take()) else { return };
        win().clear_interval_with_handle(d.iv);
        let _ = d.blk.class_list().remove_1("is-lift");
        let _ = d.blk.style().set_property("transform", "");
        if let Some(m) = MARKER.with(|m| m.borrow_mut().take()) {
            if m.parent_node().is_some() {
                let _ = d4.insert_before(&d.blk, Some(&m));
            }
            m.remove();
        }
        save(&d4);
    };
    let end2 = end.clone();
    dom::on(&dom::doc(), "pointerup", end);
    dom::on(&dom::doc(), "pointercancel", end2);
}

/// 들어 올린 블록의 위치에 맞춰 끼울 자리(빨간 선)를 정해요
fn place_marker(dash: &web_sys::Element) {
    let Some(m) = MARKER.with(|m| m.borrow().clone()) else { return };
    let Some(blk) = DRAG.with(|d| d.borrow().as_ref().map(|d| d.blk.clone())) else { return };
    let r0 = blk.get_bounding_client_rect();
    let cy = r0.top() + r0.height() / 2.0;
    let before = blocks(dash).into_iter().filter(|o| *o != blk).find(|o| {
        let r = o.get_bounding_client_rect();
        cy < r.top() + r.height() / 2.0
    });
    match before {
        Some(b) => {
            let _ = dash.insert_before(&m, Some(&b));
        }
        None => {
            let _ = dash.append_child(&m);
        }
    }
}

fn lift(dash: &web_sys::Element, blk: web_sys::HtmlElement, y: f64) {
    if let Some(pr) = PRESS.with(|p| p.borrow_mut().take()) {
        win().clear_timeout_with_handle(pr.t);
    }
    let _ = blk.class_list().add_1("is-lift");
    let Ok(marker) = dom::doc().create_element("div") else { return };
    marker.set_class_name("biz-drop");
    let _ = blk.after_with_node_1(&marker);
    MARKER.with(|m| *m.borrow_mut() = Some(marker));
    // 화면 가장자리에 가면 천천히 내려가거나 올라가요 (손을 가만히 둬도)
    let dash2 = dash.clone();
    let cb = Closure::<dyn FnMut()>::new(move || {
        let vh = win().inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
        let Some((last_y, start_y, scroll0, blk)) = DRAG.with(|d| d.borrow().as_ref().map(|d| (d.last_y, d.start_y, d.scroll0, d.blk.clone()))) else { return };
        if last_y > vh - 90.0 {
            win().scroll_by_with_x_and_y(0.0, 12.0);
        } else if last_y < 90.0 {
            win().scroll_by_with_x_and_y(0.0, -12.0);
        } else {
            return;
        }
        let _ = blk.style().set_property("transform", &format!("translateY({}px) scale(1.02)", last_y - start_y + win().scroll_y().unwrap_or(0.0) - scroll0));
        place_marker(&dash2);
    });
    let iv = win().set_interval_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), 50).unwrap_or(0);
    DRAG.with(|d| *d.borrow_mut() = Some(Drag { blk, start_y: y, last_y: y, scroll0: win().scroll_y().unwrap_or(0.0), iv, _cb: cb }));
}
