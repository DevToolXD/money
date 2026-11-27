//! 지도 탭: 일반 지도 (CARTO Voyager 타일, OpenStreetMap 자료, 키 없음. 한국은 한글 지명). 출처만 작게 적어요.
//! 위치 권한은 묻지 않아요 (처음엔 서울, 마지막으로 본 자리를 기억해요).
//! 렉 없이: 끌기·핀치·두 번 톡 확대는 지도 덩어리 하나에 transform만 걸고, 손을 떼거나 많이 벗어날 때만 타일을 다시 깔아요.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::dom::{self, by_id};

const TILE: f64 = 256.0;
const MIN_Z: i32 = 3;
const MAX_Z: i32 = 19;
const PAD: f64 = 256.0; // 화면 밖으로 이만큼 더 깔아 둬서, 끌거나 줄일 때 빈칸이 안 보여요

#[derive(Default, Clone, Copy)]
struct Tf {
    tx: f64,
    ty: f64,
    s: f64,
}
#[derive(Default)]
struct Gst {
    ptrs: Vec<(i32, f64, f64)>,
    base: Tf,
    cur: Tf,
    m0: (f64, f64),
    d0: f64,
    moved: bool,
    last_tap: (f64, f64, f64),
    t0: f64,
}
thread_local! {
    static Z: Cell<i32> = const { Cell::new(12) };
    static CENTER: Cell<(f64, f64)> = const { Cell::new((37.5665, 126.9780)) };
    static ORIGIN: Cell<(f64, f64)> = const { Cell::new((0.0, 0.0)) };
    static TILES: RefCell<HashMap<String, web_sys::HtmlElement>> = RefCell::new(HashMap::new());
    static G: RefCell<Gst> = RefCell::new(Gst { base: Tf { s: 1.0, ..Tf::default() }, cur: Tf { s: 1.0, ..Tf::default() }, ..Gst::default() });
    static BOUND: Cell<bool> = const { Cell::new(false) };
    static PENDING: Cell<bool> = const { Cell::new(false) };
}

/// CARTO Voyager 일반 지도 (키 없음, OpenStreetMap 자료라 한국은 한글 지명·길 이름). 화면이 선명하면 2배 그림으로 받아요
fn carto(z: i32, x: i64, y: i64) -> String {
    let sub = ["a", "b", "c", "d"][((x + y).rem_euclid(4)) as usize];
    let at2 = if web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0) >= 1.5 { "@2x" } else { "" };
    format!("https://{sub}.basemaps.cartocdn.com/rastertiles/voyager/{z}/{x}/{y}{at2}.png")
}
fn world(lat: f64, lon: f64, z: i32) -> (f64, f64) {
    let n = TILE * 2f64.powi(z);
    let s = lat.to_radians().clamp(-1.4844, 1.4844);
    ((lon + 180.0) / 360.0 * n, (1.0 - (s.tan() + 1.0 / s.cos()).ln() / std::f64::consts::PI) / 2.0 * n)
}
fn unworld(x: f64, y: f64, z: i32) -> (f64, f64) {
    let n = TILE * 2f64.powi(z);
    ((std::f64::consts::PI * (1.0 - 2.0 * y / n)).sinh().atan().to_degrees(), x / n * 360.0 - 180.0)
}
fn view() -> Option<web_sys::HtmlElement> {
    by_id("map-view")
}
fn size(v: &web_sys::HtmlElement) -> (f64, f64) {
    (v.client_width() as f64, v.client_height() as f64)
}

/// 지금 중심·확대에 맞춰 타일을 깔아요 (보이는 것과 둘레만 남기고 나머지는 지워요)
fn render() {
    PENDING.set(false);
    let (Some(v), Some(layer)) = (view(), by_id("map-tiles")) else { return };
    let (w, h) = size(&v);
    if w < 1.0 {
        return;
    }
    let z = Z.get();
    let (lat, lon) = CENTER.get();
    let (cx, cy) = world(lat, lon, z);
    let (ox, oy) = (cx - w / 2.0, cy - h / 2.0);
    ORIGIN.set((ox, oy));
    if let Some(wd) = by_id("map-world") {
        let _ = wd.style().remove_property("transform");
    }
    G.with(|g| {
        let mut g = g.borrow_mut();
        g.cur = Tf { s: 1.0, ..Tf::default() };
        g.base = g.cur;
    });
    let n = 2f64.powi(z) as i64;
    let (x0, x1) = (((ox - PAD) / TILE).floor() as i64, ((ox + w + PAD) / TILE).floor() as i64);
    let (y0, y1) = (((oy - PAD) / TILE).floor() as i64, ((oy + h + PAD) / TILE).floor() as i64);
    TILES.with(|t| {
        let mut t = t.borrow_mut();
        let mut keep = Vec::new();
        for ty in y0.max(0)..=y1.min(n - 1) {
            for tx in x0..=x1 {
                let wx = tx.rem_euclid(n);
                let key = format!("{z}/{tx}/{ty}");
                keep.push(key.clone());
                let el = t.entry(key).or_insert_with(|| {
                    let img: web_sys::HtmlElement = dom::doc().create_element("img").unwrap().unchecked_into();
                    let _ = img.set_attribute("src", &carto(z, wx, ty));
                    // 못 받은 칸은 OpenStreetMap 기본 타일로 한 번 더 받아 봐요
                    let el2 = img.clone();
                    dom::on(&img, "error", move |_| {
                        if el2.get_attribute("src").is_some_and(|u| u.contains("cartocdn")) {
                            let _ = el2.set_attribute("src", &format!("https://tile.openstreetmap.org/{z}/{wx}/{ty}.png"));
                        }
                    });
                    let _ = img.set_attribute("alt", "");
                    let _ = img.set_attribute("draggable", "false");
                    let _ = img.set_attribute("decoding", "async");
                    img.set_class_name("map-tile");
                    let _ = layer.append_child(&img);
                    img
                });
                let _ = el.style().set_property("transform", &format!("translate3d({}px,{}px,0)", tx as f64 * TILE - ox, ty as f64 * TILE - oy));
            }
        }
        t.retain(|k, el| {
            let ok = keep.contains(k);
            if !ok {
                el.remove();
            }
            ok
        });
    });
    dom::store_set("map-last", &format!("{lat:.5},{lon:.5},{z}"));
}
fn schedule() {
    if PENDING.replace(true) {
        return;
    }
    let cb = Closure::once_into_js(render);
    let _ = web_sys::window().map(|w| w.request_animation_frame(cb.unchecked_ref()));
}
fn apply(t: Tf) {
    if let Some(wd) = by_id("map-world") {
        let _ = wd.style().set_property("transform", &format!("translate3d({}px,{}px,0) scale({})", t.tx, t.ty, t.s));
    }
}
fn mid_dist(g: &Gst) -> ((f64, f64), f64) {
    match g.ptrs.as_slice() {
        [a] => ((a.1, a.2), 0.0),
        [a, b, ..] => (((a.1 + b.1) / 2.0, (a.2 + b.2) / 2.0), (a.1 - b.1).hypot(a.2 - b.2).max(1.0)),
        _ => ((0.0, 0.0), 0.0),
    }
}
fn rebase(g: &mut Gst) {
    g.base = g.cur;
    let (m, d) = mid_dist(g);
    g.m0 = m;
    g.d0 = d;
}
/// 지금 변환을 실제 중심·확대로 바꾸고 한 번만 새로 깔아요
fn commit() {
    let Some(v) = view() else { return };
    let (w, h) = size(&v);
    let t = G.with(|g| g.borrow().cur);
    let z0 = Z.get();
    let (ox, oy) = ORIGIN.get();
    let (u, vv) = ((w / 2.0 - t.tx) / t.s, (h / 2.0 - t.ty) / t.s);
    let z2 = ((z0 as f64 + t.s.log2()).round() as i32).clamp(MIN_Z, MAX_Z);
    let f = 2f64.powi(z2 - z0);
    let (la, lo) = unworld((ox + u) * f, (oy + vv) * f, z2);
    Z.set(z2);
    CENTER.set((la.clamp(-85.0, 85.0), lo));
    render();
}
fn zoom_by(d: i32, fx: f64, fy: f64) {
    let z = Z.get();
    if !(MIN_Z..=MAX_Z).contains(&(z + d)) {
        return;
    }
    let s = 2f64.powi(d);
    G.with(|g| g.borrow_mut().cur = Tf { tx: fx - s * fx, ty: fy - s * fy, s });
    commit();
}
fn local(e: &web_sys::PointerEvent) -> (f64, f64) {
    view().map(|v| { let r = v.get_bounding_client_rect(); (e.client_x() as f64 - r.left(), e.client_y() as f64 - r.top()) }).unwrap_or((0.0, 0.0))
}

fn bind() {
    let Some(v) = view() else { return };
    dom::on(&v, "pointerdown", |e| {
        let Some(p) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
        if let Some(v) = view() {
            let _ = v.set_pointer_capture(p.pointer_id());
        }
        let (x, y) = local(p);
        G.with(|g| {
            let mut g = g.borrow_mut();
            if g.ptrs.is_empty() {
                g.moved = false;
                g.t0 = js_sys::Date::now();
            } else {
                g.moved = true;
            }
            g.ptrs.retain(|q| q.0 != p.pointer_id());
            g.ptrs.push((p.pointer_id(), x, y));
            rebase(&mut g);
        });
    });
    dom::on(&v, "pointermove", |e| {
        let Some(p) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
        let (x, y) = local(p);
        let far = G.with(|g| {
            let mut g = g.borrow_mut();
            let Some(q) = g.ptrs.iter_mut().find(|q| q.0 == p.pointer_id()) else { return false };
            q.1 = x;
            q.2 = y;
            let (m, d) = mid_dist(&g);
            let b = g.base;
            let z = Z.get() as f64;
            let s = (if g.ptrs.len() >= 2 && g.d0 > 0.0 { b.s * d / g.d0 } else { b.s }).clamp(2f64.powf(MIN_Z as f64 - 0.5 - z), 2f64.powf(MAX_Z as f64 + 0.5 - z));
            let k = s / b.s;
            let t = Tf { tx: m.0 - k * (g.m0.0 - b.tx), ty: m.1 - k * (g.m0.1 - b.ty), s };
            if (m.0 - g.m0.0).abs() + (m.1 - g.m0.1).abs() > 6.0 || (s - b.s).abs() > 0.01 {
                g.moved = true;
            }
            g.cur = t;
            apply(t);
            t.tx.abs() > PAD - 30.0 || t.ty.abs() > PAD - 30.0
        });
        if far {
            commit();
            G.with(|g| rebase(&mut g.borrow_mut()));
        }
    });
    for ev in ["pointerup", "pointercancel"] {
        dom::on(&v, ev, |e| {
            let Some(p) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
            let (x, y) = local(p);
            let (done, dbl) = G.with(|g| {
                let mut g = g.borrow_mut();
                g.ptrs.retain(|q| q.0 != p.pointer_id());
                if !g.ptrs.is_empty() {
                    rebase(&mut g);
                    return (false, false);
                }
                let now = js_sys::Date::now();
                let tap = !g.moved && now - g.t0 < 400.0;
                let dbl = tap && now - g.last_tap.0 < 320.0 && (x - g.last_tap.1).abs() + (y - g.last_tap.2).abs() < 30.0;
                g.last_tap = if tap { (now, x, y) } else { (0.0, 0.0, 0.0) };
                (true, dbl)
            });
            if done {
                if dbl {
                    zoom_by(1, x, y);
                } else {
                    commit();
                }
            }
        });
    }
    dom::on(&v, "wheel", |e| {
        e.prevent_default();
        let d = e.dyn_ref::<web_sys::WheelEvent>().map(|w| w.delta_y()).unwrap_or(0.0);
        let (w, h) = view().map(|v| size(&v)).unwrap_or((0.0, 0.0));
        zoom_by(if d < 0.0 { 1 } else { -1 }, w / 2.0, h / 2.0);
    });
    for (id, d) in [("map-zin", 1), ("map-zout", -1)] {
        if let Some(b) = by_id(id) {
            dom::on(&b, "click", move |_| {
                let (w, h) = view().map(|v| size(&v)).unwrap_or((0.0, 0.0));
                zoom_by(d, w / 2.0, h / 2.0);
            });
        }
    }
}

/// 지도 탭을 열 때 (권한은 묻지 않아요)
pub fn open() {
    if !BOUND.replace(true) {
        let last = dom::store_get("map-last", "");
        let p: Vec<f64> = last.split(',').filter_map(|x| x.parse().ok()).collect();
        if p.len() == 3 {
            CENTER.set((p[0], p[1]));
            Z.set((p[2] as i32).clamp(MIN_Z, MAX_Z));
        }
        bind();
    }
    schedule();
}
