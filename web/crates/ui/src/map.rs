//! 지도 탭: 위성사진 지도. 관리자가 브이월드 키(meta/vworld)를 넣으면 한국 안은 브이월드 위성 사진 + 한글 지명(Hybrid),
//! 한국 밖이거나 키가 없거나 브이월드가 안 될 때는 Esri World Imagery 타일을 써요 (출처만 작게 적어요).
//! 위치 권한은 묻지 않아요 (처음엔 서울, 마지막으로 본 자리를 기억해요).
//! 렉 없이: 끌기·핀치·두 번 톡 확대는 지도 덩어리 하나에 transform만 걸고, 손을 떼거나 많이 벗어날 때만 타일을 다시 깔아요.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

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
    /// 브이월드 키. None이면 Esri
    static VKEY: RefCell<Option<String>> = const { RefCell::new(None) };
    static FAILS: Cell<u32> = const { Cell::new(0) };
    static SRC_LOADED: Cell<bool> = const { Cell::new(false) };
    static MANAGE_BOUND: Cell<bool> = const { Cell::new(false) };
}

const ESRI_ATTR: &str = "Esri, Maxar, Earthstar Geographics";
const V_ATTR: &str = "VWorld (국토교통부) · Esri";
const V_MAX_Z: i32 = 18;

fn esri(z: i32, x: i64, y: i64) -> String {
    format!("https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}")
}
fn vkey() -> Option<String> {
    VKEY.with(|k| k.borrow().clone())
}
/// 이 타일이 한국 둘레 안인가 (브이월드는 한국만 있어요)
fn in_korea(z: i32, x: i64, y: i64) -> bool {
    let (la, lo) = unworld((x as f64 + 0.5) * TILE, (y as f64 + 0.5) * TILE, z);
    (32.5..=39.0).contains(&la) && (124.0..=132.5).contains(&lo)
}
fn vworld(key: &str, layer: &str, ext: &str, z: i32, x: i64, y: i64) -> String {
    format!("https://api.vworld.kr/req/wmts/1.0.0/{key}/{layer}/{z}/{y}/{x}.{ext}")
}
/// 깔린 타일을 모두 지우고 새 출처로 다시 깔아요
fn reset_tiles() {
    TILES.with(|t| {
        for (_, el) in t.borrow_mut().drain() {
            el.remove();
        }
    });
    dom::set_text("map-attr", if vkey().is_some() { V_ATTR } else { ESRI_ATTR });
    schedule();
}
/// 브이월드가 안 되면(키가 틀리거나 막힘) 모두 Esri로 돌아가요
fn vworld_failed() {
    if VKEY.with(|k| k.borrow_mut().take()).is_some() {
        reset_tiles();
    }
}
fn fail_once() {
    if FAILS.with(|f| { f.set(f.get() + 1); f.get() }) >= 8 {
        vworld_failed();
    }
}

fn load_source() {
    if SRC_LOADED.replace(true) {
        return;
    }
    spawn_local(async {
        let key = cloud::call("vworldGet", &[]).await.ok().and_then(|v| v.as_string()).unwrap_or_default();
        if key.is_empty() {
            return;
        }
        FAILS.set(0);
        VKEY.with(|k| *k.borrow_mut() = Some(key));
        reset_tiles();
    });
}

/// 키로 서울 타일 하나를 받아 봐서 되는 키인지 확인해요
async fn key_works(key: &str) -> bool {
    let Ok(img) = web_sys::HtmlImageElement::new() else { return false };
    let (tx, rx) = futures_channel_like();
    let (ok, err) = (tx.clone(), tx);
    let a = Closure::once_into_js(move || ok.set(Some(true)));
    let b = Closure::once_into_js(move || err.set(Some(false)));
    img.set_onload(Some(a.unchecked_ref()));
    img.set_onerror(Some(b.unchecked_ref()));
    img.set_src(&vworld(key, "Satellite", "jpeg", 12, 1745, 3175));
    for _ in 0..60 {
        if let Some(v) = rx.get() {
            return v;
        }
        dom::sleep(100).await;
    }
    false
}
fn futures_channel_like() -> (std::rc::Rc<Cell<Option<bool>>>, std::rc::Rc<Cell<Option<bool>>>) {
    let c = std::rc::Rc::new(Cell::new(None));
    (c.clone(), c)
}

/// 관리 탭: 브이월드 키 넣기
pub fn bind_manage() {
    if MANAGE_BOUND.replace(true) {
        return;
    }
    let input = || by_id("vworld-key").and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok());
    spawn_local(async move {
        let k = cloud::call("vworldGet", &[]).await.ok().and_then(|v| v.as_string()).unwrap_or_default();
        if let Some(i) = input() {
            i.set_value(&k);
        }
    });
    if let Some(b) = by_id("vworld-save") {
        dom::on(&b, "click", move |_| {
            let k: String = input().map(|i| i.value()).unwrap_or_default().trim().to_string();
            if !(20..=64).contains(&k.len()) || !k.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return crate::shell::toast("브이월드 키는 영문·숫자·- 20~64자예요");
            }
            spawn_local(async move {
                if !key_works(&k).await {
                    return crate::shell::toast("이 키로 지도를 열지 못했어요. 키와 사이트 주소 등록을 확인해 주세요");
                }
                match cloud::call("vworldSet", &[JsValue::from_str(&k)]).await {
                    Ok(_) => {
                        FAILS.set(0);
                        VKEY.with(|x| *x.borrow_mut() = Some(k));
                        SRC_LOADED.set(true);
                        reset_tiles();
                        crate::shell::toast("브이월드 키를 저장했어요");
                    }
                    Err(e) => crate::shell::toast(&format!("저장하지 못했어요. {}", cloud::why(&e.code))),
                }
            });
        });
    }
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
                let vk = vkey().filter(|_| z <= V_MAX_Z && in_korea(z, wx, ty));
                let pos = format!("translate3d({}px,{}px,0)", tx as f64 * TILE - ox, ty as f64 * TILE - oy);
                let el = t.entry(key.clone()).or_insert_with(|| {
                    let img = new_tile(&layer, "map-tile");
                    match &vk {
                        Some(k) => {
                            let _ = img.set_attribute("src", &vworld(k, "Satellite", "jpeg", z, wx, ty));
                            // 그 칸만 못 받으면 그 칸만 Esri로. 많이 실패하면(키·사이트 주소 문제) 모두 Esri로 돌아가요
                            let el2 = img.clone();
                            dom::on(&img, "error", move |_| {
                                if el2.get_attribute("src").is_some_and(|u| u.contains("vworld")) {
                                    let _ = el2.set_attribute("src", &esri(z, wx, ty));
                                    fail_once();
                                }
                            });
                        }
                        None => {
                            let _ = img.set_attribute("src", &esri(z, wx, ty));
                        }
                    }
                    img
                });
                let _ = el.style().set_property("transform", &pos);
                // 한글 지명·길 이름 (브이월드 Hybrid, 투명 그림)이 사진 위에 겹쳐요
                if let Some(k) = &vk {
                    let hk = format!("h/{key}");
                    keep.push(hk.clone());
                    let h = t.entry(hk).or_insert_with(|| {
                        let img = new_tile(&layer, "map-tile map-tile-h");
                        let _ = img.set_attribute("src", &vworld(k, "Hybrid", "png", z, wx, ty));
                        let i2 = img.clone();
                        dom::on(&img, "error", move |_| {
                            let _ = i2.style().set_property("display", "none");
                        });
                        img
                    });
                    let _ = h.style().set_property("transform", &pos);
                }
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
fn new_tile(layer: &web_sys::Element, class: &str) -> web_sys::HtmlElement {
    let img: web_sys::HtmlElement = dom::doc().create_element("img").unwrap().unchecked_into();
    let _ = img.set_attribute("alt", "");
    let _ = img.set_attribute("draggable", "false");
    let _ = img.set_attribute("decoding", "async");
    img.set_class_name(class);
    let _ = layer.append_child(&img);
    img
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
    load_source();
    schedule();
}
