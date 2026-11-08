//! 홈 탭: 실제 GPS 위치를 지도에 보여 줘요. 지도 라이브러리 없이 OpenStreetMap 타일(256px 정사각형 그림)을 직접 깔아요.
//! 위치는 이 탭이 보이는 동안만 읽어요(권한은 처음 한 번 물어봐요). 끌면 움직이고, +/- 로 확대·축소, 위치 버튼으로 내 자리로 돌아와요.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use js_sys::{Function, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::dom::{self, by_id};

const TILE: f64 = 256.0;
const MIN_Z: i32 = 3;
const MAX_Z: i32 = 19;

thread_local! {
    static Z: Cell<i32> = const { Cell::new(17) };
    // 지도 가운데의 실제 좌표 (위도, 경도). 처음엔 서울 시청
    static CENTER: Cell<(f64, f64)> = const { Cell::new((37.5665, 126.9780)) };
    static ME: Cell<Option<(f64, f64, f64)>> = const { Cell::new(None) }; // 위도, 경도, 오차(m)
    static FOLLOW: Cell<bool> = const { Cell::new(true) };
    static WATCH: Cell<Option<i32>> = const { Cell::new(None) };
    static TILES: RefCell<HashMap<String, web_sys::HtmlElement>> = RefCell::new(HashMap::new());
    static PAN: Cell<Option<(f64, f64, f64, f64, i32)>> = const { Cell::new(None) }; // 시작 x, y, 시작 중심 world x, y, 포인터 id
    static PENDING: Cell<bool> = const { Cell::new(false) };
    static BOUND: Cell<bool> = const { Cell::new(false) };
}

fn world(lat: f64, lon: f64, z: i32) -> (f64, f64) {
    let n = TILE * 2f64.powi(z);
    let x = (lon + 180.0) / 360.0 * n;
    let s = lat.to_radians().clamp(-1.4844, 1.4844);
    let y = (1.0 - (s.tan() + 1.0 / s.cos()).ln() / std::f64::consts::PI) / 2.0 * n;
    (x, y)
}
fn unworld(x: f64, y: f64, z: i32) -> (f64, f64) {
    let n = TILE * 2f64.powi(z);
    let lon = x / n * 360.0 - 180.0;
    let lat = (std::f64::consts::PI * (1.0 - 2.0 * y / n)).sinh().atan().to_degrees();
    (lat, lon)
}
fn view() -> Option<web_sys::HtmlElement> {
    by_id("map-view")
}
fn size(v: &web_sys::HtmlElement) -> (f64, f64) {
    (v.client_width() as f64, v.client_height() as f64)
}

/// 지도를 지금 중심·확대에 맞춰 다시 깔아요 (보이는 타일만 남기고 나머지는 지워요)
fn render() {
    PENDING.set(false);
    let Some(v) = view() else { return };
    let (w, h) = size(&v);
    if w < 1.0 || h < 1.0 {
        return;
    }
    let z = Z.get();
    let (lat, lon) = CENTER.get();
    let (cx, cy) = world(lat, lon, z);
    let (ox, oy) = (cx - w / 2.0, cy - h / 2.0);
    let n = 2f64.powi(z) as i64;
    let (x0, x1) = ((ox / TILE).floor() as i64, ((ox + w) / TILE).floor() as i64);
    let (y0, y1) = ((oy / TILE).floor() as i64, ((oy + h) / TILE).floor() as i64);
    let Some(layer) = by_id("map-tiles") else { return };
    TILES.with(|t| {
        let mut t = t.borrow_mut();
        let mut keep: Vec<String> = Vec::new();
        for ty in y0..=y1 {
            if ty < 0 || ty >= n {
                continue;
            }
            for tx in x0..=x1 {
                let wx = tx.rem_euclid(n);
                let key = format!("{z}/{wx}/{ty}/{tx}");
                keep.push(key.clone());
                let el = t.entry(key).or_insert_with(|| {
                    let img: web_sys::HtmlElement = dom::doc().create_element("img").unwrap().unchecked_into();
                    let _ = img.set_attribute("src", &format!("https://tile.openstreetmap.org/{z}/{wx}/{ty}.png"));
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
    // 내 위치 점과 오차 원
    if let Some(dot) = by_id("map-me") {
        match ME.get() {
            Some((la, lo, acc)) => {
                let (mx, my) = world(la, lo, z);
                let mpp = 156543.03392 * la.to_radians().cos() / 2f64.powi(z) * (TILE / 256.0); // 1px 가 몇 m
                let r = (acc / mpp).clamp(10.0, 600.0);
                let _ = dot.style().set_property("transform", &format!("translate3d({}px,{}px,0)", mx - ox, my - oy));
                let _ = dot.style().set_property("--acc", &format!("{}px", r * 2.0));
                dot.set_hidden(false);
            }
            None => dot.set_hidden(true),
        }
    }
}
fn schedule() {
    if PENDING.replace(true) {
        return;
    }
    let cb = Closure::once_into_js(render);
    let _ = dom::doc().default_view().map(|w| w.request_animation_frame(cb.unchecked_ref()));
}

fn set_status(t: &str) {
    dom::set_text("map-status", t);
    if let Some(e) = by_id("map-status") {
        e.set_hidden(t.is_empty());
    }
}

fn geo() -> Option<(JsValue, web_sys::Navigator)> {
    let n = web_sys::window()?.navigator();
    let g = Reflect::get(&n, &"geolocation".into()).ok().filter(|g| g.is_object())?;
    Some((g, n))
}

fn on_pos(p: JsValue) {
    let c = Reflect::get(&p, &"coords".into()).unwrap_or(JsValue::UNDEFINED);
    let g = |k: &str| Reflect::get(&c, &k.into()).ok().and_then(|v| v.as_f64());
    let (Some(la), Some(lo)) = (g("latitude"), g("longitude")) else { return };
    ME.set(Some((la, lo, g("accuracy").unwrap_or(30.0))));
    set_status("");
    if FOLLOW.get() {
        CENTER.set((la, lo));
    }
    schedule();
}
fn on_err(e: JsValue) {
    let code = Reflect::get(&e, &"code".into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0) as i32;
    set_status(match code {
        1 => "위치 권한이 꺼져 있어요. 브라우저(설정)에서 이 사이트의 위치를 허용해 주세요",
        2 => "지금은 위치를 알 수 없어요. 하늘이 트인 곳에서 다시 해 보세요",
        3 => "위치를 찾는 데 오래 걸려요",
        _ => "위치를 읽을 수 없어요",
    });
}

/// 이 탭이 보이는 동안 위치를 계속 받아요 (앱이 꺼지거나 다른 탭에 가면 브라우저가 멈춰요)
fn start_watch() {
    if WATCH.get().is_some() {
        return;
    }
    let Some((g, _)) = geo() else {
        return set_status("이 브라우저는 위치를 지원하지 않아요");
    };
    set_status("내 위치를 찾는 중이에요…");
    let ok = Closure::<dyn FnMut(JsValue)>::new(on_pos).into_js_value();
    let er = Closure::<dyn FnMut(JsValue)>::new(on_err).into_js_value();
    let opt = dom::obj(&[("enableHighAccuracy", true.into()), ("maximumAge", 2000.into()), ("timeout", 20000.into())]);
    if let Ok(f) = Reflect::get(&g, &"watchPosition".into()).and_then(|f| f.dyn_into::<Function>()) {
        if let Ok(id) = f.call3(&g, &ok, &er, &opt) {
            WATCH.set(id.as_f64().map(|v| v as i32));
        }
    }
}
fn stop_watch() {
    let Some(id) = WATCH.take() else { return };
    if let Some((g, _)) = geo() {
        if let Ok(f) = Reflect::get(&g, &"clearWatch".into()).and_then(|f| f.dyn_into::<Function>()) {
            let _ = f.call1(&g, &JsValue::from(id));
        }
    }
}

fn zoom(d: i32) {
    let z = (Z.get() + d).clamp(MIN_Z, MAX_Z);
    if z != Z.get() {
        Z.set(z);
        // 확대 단계가 바뀌면 타일 주소가 모두 달라지니 새로 깔아요
        schedule();
    }
}
fn recenter() {
    FOLLOW.set(true);
    match ME.get() {
        Some((la, lo, _)) => {
            CENTER.set((la, lo));
            schedule();
        }
        None => {
            stop_watch();
            start_watch();
        }
    }
}

fn bind() {
    if BOUND.replace(true) {
        return;
    }
    let Some(v) = view() else { return };
    dom::on(&v, "pointerdown", |e| {
        let Some(p) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
        let (la, lo) = CENTER.get();
        let (wx, wy) = world(la, lo, Z.get());
        PAN.set(Some((p.client_x() as f64, p.client_y() as f64, wx, wy, p.pointer_id())));
        if let Some(v) = view() {
            let _ = v.set_pointer_capture(p.pointer_id());
        }
    });
    dom::on(&v, "pointermove", |e| {
        let (Some(p), Some(s)) = (e.dyn_ref::<web_sys::PointerEvent>(), PAN.get()) else { return };
        if p.pointer_id() != s.4 {
            return;
        }
        let (dx, dy) = (p.client_x() as f64 - s.0, p.client_y() as f64 - s.1);
        if dx.abs() + dy.abs() > 3.0 {
            FOLLOW.set(false);
        }
        let (la, lo) = unworld(s.2 - dx, s.3 - dy, Z.get());
        CENTER.set((la.clamp(-85.0, 85.0), lo));
        schedule();
    });
    for ev in ["pointerup", "pointercancel"] {
        dom::on(&v, ev, |_| PAN.set(None));
    }
    dom::on(&v, "wheel", |e| {
        e.prevent_default();
        let d = e.dyn_ref::<web_sys::WheelEvent>().map(|w| w.delta_y()).unwrap_or(0.0);
        zoom(if d < 0.0 { 1 } else { -1 });
    });
    for (id, f) in [("map-zin", (|| zoom(1)) as fn()), ("map-zout", || zoom(-1)), ("map-me-btn", recenter)] {
        if let Some(b) = by_id(id) {
            dom::on(&b, "click", move |_| f());
        }
    }
}

/// 홈 탭이 열렸을 때
pub fn open() {
    bind();
    start_watch();
    // 화면이 보이고 크기가 정해진 다음에 깔아요
    wasm_bindgen_futures::spawn_local(async {
        dom::sleep(60).await;
        render();
    });
}
/// 다른 탭으로 갈 때: 배터리를 아끼려고 위치 읽기를 멈춰요
pub fn leave() {
    stop_watch();
}
