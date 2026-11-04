//! 끝에서 부드럽게 되돌아오기: 메시지를 맨 아래보다 더 내리려고 하면 막히지 않고 저항을 받으며 조금 따라오다가,
//! 손을 떼면 부드럽게 제자리로 돌아와요. 내용(자식들)만 옮겨요 (--ob 값), 스크롤 상자는 그대로예요.

use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

const MAX: f64 = 120.0; // 아무리 당겨도 이만큼까지만 따라와요

// 당긴 거리 → 보이는 거리 (처음엔 거의 따라오고, 멀어질수록 무거워져요)
fn rubber(d: f64) -> f64 {
    MAX * (1.0 - 1.0 / (d * 0.55 / MAX + 1.0))
}
fn at_bottom(el: &HtmlElement) -> bool {
    el.scroll_top() as f64 + el.client_height() as f64 >= el.scroll_height() as f64 - 1.0
}
fn y_of(e: &web_sys::Event) -> Option<f64> {
    let t = e.dyn_ref::<web_sys::TouchEvent>()?.touches().get(0)?;
    Some(t.client_y() as f64)
}
fn put(el: &HtmlElement, px: f64, back: bool) {
    let st = el.style();
    // 돌아올 때만 움직임을 켜요 (화면이 원래 가진 움직임은 app.css의 .ob-back이 같이 적어 둬요)
    let _ = el.class_list().toggle_with_force("ob-back", back);
    if px == 0.0 {
        let _ = st.remove_property("translate");
    } else {
        let _ = st.set_property("translate", &format!("0 {}px", -px));
    }
}

/// 이 스크롤 상자에 붙여요
pub fn attach(el: &HtmlElement) {
    let _ = el.class_list().add_1("bounce");
    let start: Rc<Cell<Option<f64>>> = Rc::new(Cell::new(None)); // 맨 아래에 닿은 순간의 손가락 위치
    let shown = Rc::new(Cell::new(0.0));
    {
        let (el2, start) = (el.clone(), start.clone());
        crate::dom::on(el, "touchstart", move |e| {
            start.set(if at_bottom(&el2) { y_of(&e) } else { None });
        });
    }
    {
        let (el2, start, shown) = (el.clone(), start.clone(), shown.clone());
        crate::dom::on_active(el, "touchmove", move |e| {
            let Some(y) = y_of(&e) else { return };
            if !at_bottom(&el2) {
                start.set(None);
                if shown.get() != 0.0 {
                    shown.set(0.0);
                    put(&el2, 0.0, false);
                }
                return;
            }
            // 스크롤하다가 맨 아래에 닿으면 그 자리부터 세요
            let s0 = match start.get() {
                Some(v) => v,
                None => {
                    start.set(Some(y));
                    y
                }
            };
            let d = s0 - y; // 위로 밀면 +: 아래 내용을 더 보려는 것
            if d <= 0.0 {
                if shown.get() != 0.0 {
                    shown.set(0.0);
                    put(&el2, 0.0, false);
                }
                return;
            }
            e.prevent_default();
            let v = rubber(d);
            shown.set(v);
            put(&el2, v, false);
        });
    }
    for ev in ["touchend", "touchcancel"] {
        let (el2, start, shown) = (el.clone(), start.clone(), shown.clone());
        crate::dom::on(el, ev, move |_| {
            start.set(None);
            if shown.get() != 0.0 {
                shown.set(0.0);
                put(&el2, 0.0, true);
            }
        });
    }
}
