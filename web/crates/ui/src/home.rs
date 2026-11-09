//! 홈 탭: 실제 GPS 위치를 지도에 보여 줘요. 지도 라이브러리 없이 OpenStreetMap 타일(256px 정사각형 그림)을 직접 깔아요.
//! 위치는 이 탭이 보이는 동안만 읽어요(권한은 처음 한 번 물어봐요). 끌면 움직이고, +/- 로 확대·축소, 위치 버튼으로 내 자리로 돌아와요.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use js_sys::{Function, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::dom::{self, by_id};

const TILE: f64 = 256.0;
const MIN_Z: i32 = 14; // 더 줄이면 육각형이 너무 많아져 렉이 걸려요
const MAX_Z: i32 = 18;
// 육각형 칸: 확대 16 기준 지도 위에서 꼭짓점까지 45px (약 65m)인 뾰족 위쪽 육각형. 확대에 따라 같이 커지고 작아져요
const HZ: i32 = 16;
const HS: f64 = 45.0;

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
    static ORIGIN: Cell<(f64, f64)> = const { Cell::new((0.0, 0.0)) }; // 마지막으로 타일을 깐 화면 왼쪽 위의 지도 좌표
    static SEL: Cell<Option<(i64, i64)>> = const { Cell::new(None) }; // 눌러서 고른 칸
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
    ORIGIN.set((ox, oy));
    if let Some(wd) = by_id("map-world") {
        let _ = wd.style().remove_property("transform");
    }
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
    draw_hex();
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
/// 육각형 칸 번호 (위치 → q, r)
fn cell_of(wx: f64, wy: f64, z: i32) -> (i64, i64) {
    let k = 2f64.powi(z - HZ);
    let (x, y) = (wx / k, wy / k);
    let q = (3f64.sqrt() / 3.0 * x - y / 3.0) / HS;
    let r = (2.0 / 3.0 * y) / HS;
    let (cx, cz, cy) = (q, r, -q - r);
    let (mut rx, mut ry, mut rz) = (cx.round(), cy.round(), cz.round());
    let (dx, dy, dz) = ((rx - cx).abs(), (ry - cy).abs(), (rz - cz).abs());
    if dx > dy && dx > dz {
        rx = -ry - rz;
    } else if dy > dz {
        ry = -rx - rz;
    } else {
        rz = -rx - ry;
    }
    let _ = ry;
    (rx as i64, rz as i64)
}

/// 육각형 칸 선을 캔버스에 한 번에 그려요 (선 하나로 모아서 그리고, 내 칸·고른 칸만 색을 칠해요)
fn draw_hex() {
    let (Some(v), Some(c)) = (view(), by_id("map-hex")) else { return };
    let cv: web_sys::HtmlCanvasElement = c.unchecked_into();
    let (w, h) = size(&v);
    let dpr = web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0).min(2.0);
    let (pw, ph) = ((w * dpr) as u32, (h * dpr) as u32);
    if cv.width() != pw || cv.height() != ph {
        cv.set_width(pw);
        cv.set_height(ph);
    }
    let Some(ctx) = cv.get_context("2d").ok().flatten().and_then(|c| c.dyn_into::<web_sys::CanvasRenderingContext2d>().ok()) else { return };
    let _ = ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);
    ctx.clear_rect(0.0, 0.0, w, h);
    let z = Z.get();
    let (lat, lon) = CENTER.get();
    let (cx, cy) = world(lat, lon, z);
    let (ox, oy) = (cx - w / 2.0, cy - h / 2.0);
    let k = 2f64.powi(z - HZ);
    let s = HS * k; // 지금 확대에서의 한 변 길이
    let (w3, step_y) = (3f64.sqrt() * s, 1.5 * s);
    let corners = |x: f64, y: f64| -> [(f64, f64); 6] {
        let mut a = [(0.0, 0.0); 6];
        for (i, p) in a.iter_mut().enumerate() {
            let t = (60.0 * i as f64 - 30.0).to_radians();
            *p = (x - ox + s * t.cos(), y - oy + s * t.sin());
        }
        a
    };
    let (r0, r1) = (((oy - s) / step_y).floor() as i64, ((oy + h + s) / step_y).ceil() as i64);
    let mine = ME.get().map(|(la, lo, _)| { let (x, y) = world(la, lo, z); cell_of(x, y, z) });
    let sel = SEL.get();
    ctx.begin_path();
    for r in r0..=r1 {
        let q0 = ((ox - w3) / w3 - r as f64 / 2.0).floor() as i64;
        let q1 = ((ox + w + w3) / w3 - r as f64 / 2.0).ceil() as i64;
        for q in q0..=q1 {
            let (x, y) = (w3 * (q as f64 + r as f64 / 2.0), step_y * r as f64);
            let p = corners(x, y);
            let fill = Some((q, r)) == mine || Some((q, r)) == sel;
            if fill {
                ctx.save();
                ctx.begin_path();
                ctx.move_to(p[0].0, p[0].1);
                for pt in &p[1..] {
                    ctx.line_to(pt.0, pt.1);
                }
                ctx.close_path();
                ctx.set_fill_style_str(if Some((q, r)) == mine { "rgba(10,124,255,0.28)" } else { "rgba(255,159,10,0.30)" });
                ctx.fill();
                ctx.restore();
                ctx.begin_path();
            }
            ctx.move_to(p[0].0, p[0].1);
            for pt in &p[1..] {
                ctx.line_to(pt.0, pt.1);
            }
            ctx.close_path();
        }
    }
    ctx.set_stroke_style_str("rgba(20,40,80,0.45)");
    ctx.set_line_width(1.2);
    ctx.stroke();
}

fn schedule() {
    if PENDING.replace(true) {
        return;
    }
    let cb = Closure::once_into_js(render);
    let _ = dom::doc().default_view().map(|w| w.request_animation_frame(cb.unchecked_ref()));
}

fn set_cell_label(c: Option<(i64, i64)>) {
    let Some(e) = by_id("map-cell") else { return };
    match c {
        Some((q, r)) => {
            e.set_hidden(false);
            e.set_text_content(Some(&format!("칸 {q}, {r}")));
        }
        None => e.set_hidden(true),
    }
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

/// 끄는 동안: 타일은 다시 깔지 않고 덩어리째 옮기고, 육각형 선만 다시 그려요 (많이 벗어났을 때만 타일을 새로 깔아요)
fn pan_view() {
    if PENDING.replace(true) {
        return;
    }
    let cb = Closure::once_into_js(|| {
        PENDING.set(false);
        let Some(v) = view() else { return };
        let (w, h) = size(&v);
        let (lat, lon) = CENTER.get();
        let (cx, cy) = world(lat, lon, Z.get());
        let (ox, oy) = (cx - w / 2.0, cy - h / 2.0);
        let (px, py) = ORIGIN.get();
        if (px - ox).abs() > 200.0 || (py - oy).abs() > 200.0 {
            return render();
        }
        if let Some(wd) = by_id("map-world") {
            let _ = wd.style().set_property("transform", &format!("translate3d({}px,{}px,0)", px - ox, py - oy));
        }
        draw_hex();
    });
    let _ = dom::doc().default_view().map(|w| w.request_animation_frame(cb.unchecked_ref()));
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
        pan_view();
    });
    dom::on(&v, "pointerup", |e| {
        let (Some(p), Some(s)) = (e.dyn_ref::<web_sys::PointerEvent>(), PAN.get()) else { return };
        PAN.set(None);
        let moved = (p.client_x() as f64 - s.0).abs() + (p.client_y() as f64 - s.1).abs();
        if moved <= 6.0 {
            // 톡 눌렀으면 그 자리의 칸을 골라요
            if let Some(v) = view() {
                let r = v.get_bounding_client_rect();
                let (w, h) = size(&v);
                let (lat, lon) = CENTER.get();
                let z = Z.get();
                let (cx, cy) = world(lat, lon, z);
                let (wx, wy) = (cx - w / 2.0 + p.client_x() as f64 - r.left(), cy - h / 2.0 + p.client_y() as f64 - r.top());
                let c = cell_of(wx, wy, z);
                SEL.set(if SEL.get() == Some(c) { None } else { Some(c) });
                set_cell_label(SEL.get());
            }
        }
        render();
    });
    dom::on(&v, "pointercancel", |_| {
        PAN.set(None);
        render();
    });
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
