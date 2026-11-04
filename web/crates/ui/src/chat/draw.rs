//! 그리기: 손가락으로 그려서 보내요 (펜 색 5가지, 되돌리기·다시하기)

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::{open_chat, queue_send, toast};
use crate::dom::{self, by_id};

const COLORS: [&str; 5] = ["#111111", "#0a7cff", "#ff3b30", "#34c759", "#ff9f0a"];

#[derive(Clone)]
struct Stroke {
    color: String,
    w: f64,
    pts: Vec<(f64, f64)>,
}
thread_local! {
    static STROKES: RefCell<Vec<Stroke>> = const { RefCell::new(Vec::new()) };
    static REDO: RefCell<Vec<Stroke>> = const { RefCell::new(Vec::new()) };
    static INK: RefCell<String> = RefCell::new(COLORS[0].to_string());
    static DPR: Cell<f64> = const { Cell::new(1.0) };
    static DRAWING: Cell<bool> = const { Cell::new(false) };
}
fn canvas() -> Option<web_sys::HtmlCanvasElement> {
    by_id("draw-canvas")?.dyn_into().ok()
}
fn c2d(c: &web_sys::HtmlCanvasElement) -> Option<web_sys::CanvasRenderingContext2d> {
    c.get_context("2d").ok().flatten()?.dyn_into().ok()
}
fn paint_all() {
    let Some(c) = canvas() else { return };
    let Some(x) = c2d(&c) else { return };
    let dpr = DPR.get();
    x.clear_rect(0.0, 0.0, c.width() as f64, c.height() as f64);
    x.set_line_cap("round");
    x.set_line_join("round");
    STROKES.with(|s| {
        for s in s.borrow().iter() {
            x.set_stroke_style_str(&s.color);
            x.set_fill_style_str(&s.color);
            x.set_line_width(s.w * dpr);
            let p = &s.pts;
            if p.len() == 1 {
                x.begin_path();
                let _ = x.arc(p[0].0, p[0].1, (s.w * dpr) / 2.0, 0.0, 7.0);
                x.fill();
                continue;
            }
            x.begin_path();
            x.move_to(p[0].0, p[0].1);
            for i in 1..p.len() - 1 {
                x.quadratic_curve_to(p[i].0, p[i].1, (p[i].0 + p[i + 1].0) / 2.0, (p[i].1 + p[i + 1].1) / 2.0);
            }
            let l = p[p.len() - 1];
            x.line_to(l.0, l.1);
            x.stroke();
        }
    });
}
pub fn open() {
    let (Some(el), Some(c)) = (by_id("draw"), canvas()) else { return };
    el.set_hidden(false);
    STROKES.with(|s| s.borrow_mut().clear());
    REDO.with(|s| s.borrow_mut().clear());
    dom::set_hidden("palette", true);
    let dpr = web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0).clamp(1.0, 2.0);
    DPR.set(dpr);
    leptos::prelude::request_animation_frame(move || {
        c.set_width((el.client_width() as f64 * dpr).round() as u32);
        c.set_height((el.client_height() as f64 * dpr).round() as u32);
        paint_all();
        let _ = el.class_list().add_1("is-open");
    });
}
fn close() {
    let Some(el) = by_id("draw") else { return };
    let _ = el.class_list().remove_1("is-open");
    leptos::prelude::set_timeout(move || el.set_hidden(true), std::time::Duration::from_millis(280));
}
fn point(c: &web_sys::HtmlCanvasElement, e: &web_sys::PointerEvent) -> (f64, f64) {
    let r = c.get_bounding_client_rect();
    let dpr = DPR.get();
    ((e.client_x() as f64 - r.left()) * dpr, (e.client_y() as f64 - r.top()) * dpr)
}
async fn ok() {
    let Some(c) = canvas() else { return };
    if STROKES.with(|s| s.borrow().is_empty()) {
        return close();
    }
    let k = (720.0 / c.width() as f64).min(1.0);
    let Ok(out) = dom::doc().create_element("canvas") else { return };
    let out: web_sys::HtmlCanvasElement = out.unchecked_into();
    out.set_width((c.width() as f64 * k).round() as u32);
    out.set_height((c.height() as f64 * k).round() as u32);
    if let Some(o) = c2d(&out) {
        o.set_fill_style_str("#fff");
        o.fill_rect(0.0, 0.0, out.width() as f64, out.height() as f64);
        let _ = o.draw_image_with_html_canvas_element_and_dw_and_dh(&c, 0.0, 0.0, out.width() as f64, out.height() as f64);
    }
    let mut img = out.to_data_url_with_type("image/png").unwrap_or_default();
    let mut q = 0.8;
    while img.len() > 130000 && q > 0.3 {
        img = out.to_data_url_with_type_and_encoder_options("image/jpeg", &JsValue::from_f64(q)).unwrap_or_default();
        q -= 0.1;
    }
    if img.len() > 130000 {
        return toast("그림이 너무 커요. 조금 줄여서 다시 보내 주세요");
    }
    close();
    let Some(chat) = open_chat() else { return };
    let (id, other) = (chat.id.clone(), chat.other.clone());
    let payload = dom::obj(&[("type", "draw".into()), ("img", JsValue::from_str(&img))]);
    if queue_send(move || cloud::call_now("send", &[JsValue::from_str(&id), JsValue::from_str(&other), payload, JsValue::from_str("그림")])).await.is_err() {
        toast("그림을 보내지 못했어요");
    }
}

pub fn init() {
    if let Some(p) = by_id("palette") {
        p.set_inner_html(&COLORS.iter().enumerate().map(|(i, c)| format!(r#"<button type="button" data-c="{c}" style="background:{c}" aria-label="색 {}"></button>"#, i + 1)).collect::<String>());
        let p2 = p.clone();
        dom::on(&p, "click", move |e| {
            if let Some(b) = dom::closest(&e, "[data-c]") {
                INK.with(|i| *i.borrow_mut() = dom::data(&b, "c"));
                p2.set_hidden(true);
            }
        });
    }
    if let Some(b) = by_id("draw-pen") {
        dom::on(&b, "click", |_| {
            if let Some(p) = by_id("palette") {
                p.set_hidden(!p.hidden());
            }
        });
    }
    if let Some(c) = canvas() {
        let c2 = c.clone();
        dom::on(&c, "pointerdown", move |e| {
            let Some(pe) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
            dom::set_hidden("palette", true);
            let _ = c2.set_pointer_capture(pe.pointer_id());
            let pt = point(&c2, pe);
            STROKES.with(|s| s.borrow_mut().push(Stroke { color: INK.with(|i| i.borrow().clone()), w: 5.0, pts: vec![pt] }));
            REDO.with(|s| s.borrow_mut().clear());
            DRAWING.set(true);
            paint_all();
        });
        let c3 = c.clone();
        dom::on(&c, "pointermove", move |e| {
            if !DRAWING.get() {
                return;
            }
            let Some(pe) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
            let pt = point(&c3, pe);
            STROKES.with(|s| {
                if let Some(l) = s.borrow_mut().last_mut() {
                    l.pts.push(pt);
                }
            });
            paint_all();
        });
        for t in ["pointerup", "pointercancel"] {
            dom::on(&c, t, |_| DRAWING.set(false));
        }
    }
    if let Some(b) = by_id("draw-undo") {
        dom::on(&b, "click", |_| {
            if let Some(s) = STROKES.with(|s| s.borrow_mut().pop()) {
                REDO.with(|r| r.borrow_mut().push(s));
                paint_all();
            }
        });
    }
    if let Some(b) = by_id("draw-redo") {
        dom::on(&b, "click", |_| {
            if let Some(s) = REDO.with(|r| r.borrow_mut().pop()) {
                STROKES.with(|x| x.borrow_mut().push(s));
                paint_all();
            }
        });
    }
    if let Some(b) = by_id("draw-close") {
        dom::on(&b, "click", |_| close());
    }
    if let Some(b) = by_id("draw-ok") {
        dom::on(&b, "click", |_| leptos::task::spawn_local(ok()));
    }
}
