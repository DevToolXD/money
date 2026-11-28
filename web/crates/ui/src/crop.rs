//! 사진 조정 화면: 프로필 사진을 고르면 까만 화면에 사진이 뜨고, 손가락으로 끌어 옮기고 두 손가락으로 모아 크기를 맞춰요.
//! 가운데 정사각형 틀 안이 프로필 사진이 되고(틀 밖은 어둡게 보여요), 위쪽 "저장"을 누르면 그 부분만 잘라 돌려줘요. 뒤로 가기는 취소예요.

use std::cell::RefCell;

use js_sys::{Function, Promise};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::dom::{self, by_id};

const BACK: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M15 5l-7 7 7 7"/></svg>"#;
const MAX_ZOOM: f64 = 6.0;

struct Crop {
    img: web_sys::HtmlImageElement,
    url: String,
    nw: f64,
    nh: f64,
    /// 틀 (무대 안 위치와 한 변)
    fx: f64,
    fy: f64,
    f: f64,
    s: f64,
    tx: f64,
    ty: f64,
    min_s: f64,
    ptrs: Vec<(i32, f64, f64)>,
    /// 손가락 수가 바뀔 때의 기준: 이동, 크기, 가운데, 거리
    base: (f64, f64, f64, (f64, f64), f64),
    done: Function,
}

thread_local! {
    static C: RefCell<Option<Crop>> = const { RefCell::new(None) };
}

fn clamp(c: &mut Crop) {
    c.s = c.s.clamp(c.min_s, c.min_s * MAX_ZOOM);
    let (w, h) = (c.nw * c.s, c.nh * c.s);
    // 틀이 늘 사진으로 가득 차게 (틀 밖으로 사진 끝이 들어오지 않게)
    c.tx = c.tx.min(c.fx).max(c.fx + c.f - w);
    c.ty = c.ty.min(c.fy).max(c.fy + c.f - h);
}
fn paint(c: &Crop) {
    let _ = c.img.style().set_property("transform", &format!("translate3d({}px,{}px,0) scale({})", c.tx, c.ty, c.s));
}
fn mid_dist(p: &[(i32, f64, f64)]) -> ((f64, f64), f64) {
    match p {
        [a] => ((a.1, a.2), 0.0),
        [a, b, ..] => (((a.1 + b.1) / 2.0, (a.2 + b.2) / 2.0), (a.1 - b.1).hypot(a.2 - b.2).max(1.0)),
        _ => ((0.0, 0.0), 0.0),
    }
}
fn rebase(c: &mut Crop) {
    let (m, d) = mid_dist(&c.ptrs);
    c.base = (c.tx, c.ty, c.s, m, d);
}
fn local(stage: &web_sys::HtmlElement, e: &web_sys::PointerEvent) -> (f64, f64) {
    let r = stage.get_bounding_client_rect();
    (e.client_x() as f64 - r.left(), e.client_y() as f64 - r.top())
}

fn close(result: JsValue) {
    let c = C.with(|c| c.borrow_mut().take());
    if let Some(c) = c {
        let _ = web_sys::Url::revoke_object_url(&c.url);
        let _ = c.done.call1(&JsValue::NULL, &result);
    }
    if let Some(e) = by_id("crop") {
        e.remove();
    }
}

/// 틀 안의 부분을 잘라 data 주소로 (캡을 넘으면 작게)
fn cut() -> Option<String> {
    C.with(|c| {
        let c = c.borrow();
        let c = c.as_ref()?;
        let (sx, sy, sw) = ((c.fx - c.tx) / c.s, (c.fy - c.ty) / c.s, c.f / c.s);
        for out in [240.0, 200.0, 160.0, 128.0] {
            let cv: web_sys::HtmlCanvasElement = dom::doc().create_element("canvas").ok()?.unchecked_into();
            cv.set_width(out as u32);
            cv.set_height(out as u32);
            let ctx = cv.get_context("2d").ok().flatten()?.dyn_into::<web_sys::CanvasRenderingContext2d>().ok()?;
            ctx.draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(&c.img, sx, sy, sw, sw, 0.0, 0.0, out, out).ok()?;
            let mut q = 0.85;
            while q >= 0.4 {
                if let Ok(d) = cv.to_data_url_with_type_and_encoder_options("image/jpeg", &JsValue::from_f64(q)) {
                    if d.len() <= 36000 {
                        return Some(d);
                    }
                }
                q -= 0.09;
            }
        }
        None
    })
}

/// 사진을 조정해요. 저장하면 잘라 낸 사진(data 주소), 뒤로 가면 None
pub async fn edit(file: web_sys::File) -> Option<String> {
    if !file.type_().starts_with("image/") || file.size() > 25.0 * 1024.0 * 1024.0 {
        return None;
    }
    let url = web_sys::Url::create_object_url_with_blob(&file).ok()?;
    let img = web_sys::HtmlImageElement::new().ok()?;
    let loaded = Promise::new(&mut |res, rej| {
        img.set_onload(Some(&res));
        img.set_onerror(Some(&rej));
    });
    img.set_src(&url);
    if JsFuture::from(loaded).await.is_err() {
        let _ = web_sys::Url::revoke_object_url(&url);
        return None;
    }
    let (nw, nh) = (img.natural_width() as f64, img.natural_height() as f64);
    if nw < 1.0 || nh < 1.0 {
        let _ = web_sys::Url::revoke_object_url(&url);
        return None;
    }
    if let Some(e) = by_id("crop") {
        e.remove();
    }
    let root = dom::doc().body()?;
    let host = dom::doc().create_element("div").ok()?;
    host.set_inner_html(&format!(
        r#"<div class="crop" id="crop" role="dialog" aria-modal="true" aria-label="사진 조정"><div class="crop-top"><button type="button" class="crop-back" id="crop-back" aria-label="뒤로">{BACK}</button><button type="button" class="crop-save" id="crop-save">저장</button></div><div class="crop-stage" id="crop-stage"><div class="crop-frame" id="crop-frame"></div></div></div>"#
    ));
    let el: web_sys::HtmlElement = host.first_element_child()?.dyn_into().ok()?;
    let _ = root.append_child(&el);
    let stage = by_id("crop-stage")?;
    // 사진은 틀보다 앞(뒤)에 깔려야 해서, 무대 맨 앞에 넣어요
    img.set_class_name("crop-img");
    let _ = img.style().set_property("width", &format!("{nw}px"));
    let _ = img.style().set_property("height", &format!("{nh}px"));
    let _ = img.set_attribute("alt", "");
    let _ = img.set_attribute("draggable", "false");
    let _ = stage.insert_before(&img, stage.first_child().as_ref());
    let r = stage.get_bounding_client_rect();
    let (w, h) = (r.width(), r.height());
    let f = w.min(h);
    let (fx, fy) = ((w - f) / 2.0, (h - f) / 2.0);
    if let Some(fr) = by_id("crop-frame") {
        let st = fr.style();
        let _ = st.set_property("left", &format!("{fx}px"));
        let _ = st.set_property("top", &format!("{fy}px"));
        let _ = st.set_property("width", &format!("{f}px"));
        let _ = st.set_property("height", &format!("{f}px"));
    }
    let min_s = (f / nw).max(f / nh);

    let mut res_fn: Option<Function> = None;
    let promise = Promise::new(&mut |res, _| res_fn = Some(res));
    let done = res_fn?;
    let mut c = Crop { img: img.clone(), url, nw, nh, fx, fy, f, s: min_s, tx: 0.0, ty: 0.0, min_s, ptrs: Vec::new(), base: (0.0, 0.0, min_s, (0.0, 0.0), 0.0), done };
    c.tx = fx + (f - nw * min_s) / 2.0;
    c.ty = fy + (f - nh * min_s) / 2.0;
    clamp(&mut c);
    paint(&c);
    C.with(|x| *x.borrow_mut() = Some(c));

    if let Some(b) = by_id("crop-back") {
        dom::on(&b, "click", |_| close(JsValue::NULL));
    }
    if let Some(b) = by_id("crop-save") {
        dom::on(&b, "click", |_| match cut() {
            Some(d) => close(JsValue::from_str(&d)),
            None => {
                crate::shell::toast("사진을 저장하지 못했어요");
            }
        });
    }
    let st2 = stage.clone();
    dom::on(&stage, "pointerdown", move |e| {
        let Some(p) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
        let _ = st2.set_pointer_capture(p.pointer_id());
        let (x, y) = local(&st2, p);
        C.with(|c| {
            if let Some(c) = c.borrow_mut().as_mut() {
                c.ptrs.retain(|q| q.0 != p.pointer_id());
                c.ptrs.push((p.pointer_id(), x, y));
                rebase(c);
            }
        });
    });
    let st3 = stage.clone();
    dom::on(&stage, "pointermove", move |e| {
        let Some(p) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
        let (x, y) = local(&st3, p);
        C.with(|c| {
            let mut c = c.borrow_mut();
            let Some(c) = c.as_mut() else { return };
            let Some(q) = c.ptrs.iter_mut().find(|q| q.0 == p.pointer_id()) else { return };
            q.1 = x;
            q.2 = y;
            let (m, d) = mid_dist(&c.ptrs);
            let (bx, by, bs, bm, bd) = c.base;
            let s = if c.ptrs.len() >= 2 && bd > 0.0 { (bs * d / bd).clamp(c.min_s, c.min_s * MAX_ZOOM) } else { bs };
            // 손가락 아래의 사진 자리가 그대로 손가락 아래에 있게
            let k = s / bs;
            c.s = s;
            c.tx = m.0 - k * (bm.0 - bx);
            c.ty = m.1 - k * (bm.1 - by);
            clamp(c);
            paint(c);
        });
    });
    for ev in ["pointerup", "pointercancel"] {
        dom::on(&stage, ev, |e| {
            let Some(p) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
            C.with(|c| {
                if let Some(c) = c.borrow_mut().as_mut() {
                    c.ptrs.retain(|q| q.0 != p.pointer_id());
                    rebase(c);
                }
            });
        });
    }
    // 컴퓨터에서는 휠로 크기를 맞춰요
    let st4 = stage.clone();
    dom::on_active(&stage, "wheel", move |e| {
        e.prevent_default();
        let Some(we) = e.dyn_ref::<web_sys::WheelEvent>() else { return };
        let r = st4.get_bounding_client_rect();
        let (x, y) = (we.client_x() as f64 - r.left(), we.client_y() as f64 - r.top());
        C.with(|c| {
            let mut c = c.borrow_mut();
            let Some(c) = c.as_mut() else { return };
            let s = (c.s * if we.delta_y() < 0.0 { 1.12 } else { 1.0 / 1.12 }).clamp(c.min_s, c.min_s * MAX_ZOOM);
            let k = s / c.s;
            c.tx = x - k * (x - c.tx);
            c.ty = y - k * (y - c.ty);
            c.s = s;
            clamp(c);
            paint(c);
            rebase(c);
        });
    });

    let v = JsFuture::from(promise).await.ok()?;
    v.as_string()
}
