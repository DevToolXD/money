//! 홈 탭: 실제 GPS 위치를 지도에 보여 줘요. 지도 라이브러리 없이 OpenStreetMap 타일(256px 정사각형 그림)을 직접 깔아요.
//! 위치는 이 탭이 보이는 동안만 읽어요(권한은 처음 한 번 물어봐요). 끌면 움직이고, +/- 로 확대·축소, 위치 버튼으로 내 자리로 돌아와요.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use js_sys::{Function, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::dom::{self, by_id};

const TILE: f64 = 256.0;
const MIN_Z: i32 = 15; // 더 줄이면 육각형이 너무 많아져 렉이 걸려요
const MAX_Z: i32 = 18;
// 육각형 칸: 확대 16 기준 지도 위에서 꼭짓점까지 24px (약 35m)인 뾰족 위쪽 육각형. 확대에 따라 같이 커지고 작아져요
const HZ: i32 = 16;
const PADC: f64 = 520.0; // 육각형 캔버스는 화면보다 사방으로 이만큼 크게 한 번만 그려 두고, 끌 때는 옮기기만 해요
const HS: f64 = 24.0;

thread_local! {
    static Z: Cell<i32> = const { Cell::new(17) };
    // 지도 가운데의 실제 좌표 (위도, 경도). 처음엔 서울 시청
    static CENTER: Cell<(f64, f64)> = const { Cell::new((37.5665, 126.9780)) };
    static ME: Cell<Option<(f64, f64, f64)>> = const { Cell::new(None) }; // 위도, 경도, 오차(m)
    static FOLLOW: Cell<bool> = const { Cell::new(true) };
    static WATCH: Cell<Option<i32>> = const { Cell::new(None) };
    static TILES: RefCell<HashMap<String, web_sys::HtmlElement>> = RefCell::new(HashMap::new());
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
    G.with(|g| {
        let mut g = g.borrow_mut();
        g.cur = Tf { s: 1.0, ..Tf::default() };
        g.base = g.cur;
    });
    let n = 2f64.powi(z) as i64;
    let (x0, x1) = (((ox - 512.0) / TILE).floor() as i64, ((ox + w + 512.0) / TILE).floor() as i64);
    let (y0, y1) = (((oy - 512.0) / TILE).floor() as i64, ((oy + h + 512.0) / TILE).floor() as i64);
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
    load_lands(false);
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

#[derive(Clone)]
struct Land {
    owner: String,
    name: String,
    lv: usize,
    pot: f64,
}
const LV_NAME: [&str; 5] = ["빈 땅", "집 1채", "집 2채", "집 3채", "호텔"];
const TOLL: [f64; 5] = [1.0, 2.0, 4.0, 8.0, 16.0];
const BUILD: [f64; 4] = [20.0, 40.0, 80.0, 160.0];
const REFUND: [f64; 5] = [5.0, 15.0, 35.0, 75.0, 155.0];
thread_local! {
    static LANDS: RefCell<HashMap<(i64, i64), Land>> = RefCell::new(HashMap::new());
    static CHUNKS: RefCell<HashMap<String, f64>> = RefCell::new(HashMap::new()); // 구역 → 읽은 시각
    static LAST_CELL: Cell<Option<(i64, i64)>> = const { Cell::new(None) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
    static HUD: Cell<i32> = const { Cell::new(0) };
}
fn my_uid() -> String {
    cloud::cloud().map(|c| cloud::get(&c, "uid").as_string().unwrap_or_default()).unwrap_or_default()
}
fn land_at(c: (i64, i64)) -> Option<Land> {
    LANDS.with(|l| l.borrow().get(&c).cloned())
}
fn chunk_of(c: (i64, i64)) -> String {
    format!("{}_{}", c.0.div_euclid(16), c.1.div_euclid(16))
}
fn hue_of(uid: &str) -> u32 {
    uid.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32)) % 360
}
fn poly(ctx: &web_sys::CanvasRenderingContext2d, pts: &[(f64, f64)], fill: &str) {
    ctx.begin_path();
    ctx.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..] {
        ctx.line_to(p.0, p.1);
    }
    ctx.close_path();
    ctx.set_fill_style_str(fill);
    ctx.fill();
}
/// 세모 지붕 집 (모노폴리 말). kind 0 = 초록 집, 1 = 빨간 호텔. 지도가 기울어져 눌려 보이는 만큼 키를 키워 그려요
fn gable(ctx: &web_sys::CanvasRenderingContext2d, x: f64, y: f64, w: f64, hgt: f64, kind: usize) {
    let (wall, end, roof, edge) = [("#1c9a55", "#15803f", "#3ccf80", "#8ff0b8"), ("#e0393f", "#bb2a30", "#ff6b70", "#ffb3b6")][kind];
    let (d, hgt, rh) = (w * 0.5, hgt * 1.35, w * 0.95);
    let near = (x + w * 0.5, y - hgt + d * 0.5 - rh);
    let far = (x - w * 0.5, y - hgt - d * 0.5 - rh);
    // 바닥 그림자
    poly(ctx, &[(x - w, y + d * 0.4), (x, y + d * 1.4), (x + w * 1.3, y + d * 0.7), (x + w * 0.3, y - d * 0.3)], "rgba(10,30,20,0.22)");
    poly(ctx, &[(x - w, y - hgt), (x, y - hgt + d), (x, y + d), (x - w, y)], wall);
    poly(ctx, &[(x, y - hgt + d), near, (x + w, y - hgt), (x + w, y), (x, y + d)], end);
    poly(ctx, &[(x - w, y - hgt), (x, y - hgt + d), near, far], roof);
    ctx.begin_path();
    ctx.move_to(far.0, far.1);
    ctx.line_to(near.0, near.1);
    ctx.set_stroke_style_str(edge);
    ctx.set_line_width((w * 0.12).max(1.0));
    ctx.stroke();
}
/// 작은 입체 상자 (건물): 왼쪽 벽·오른쪽 벽·지붕
#[allow(dead_code)]
fn boxy(ctx: &web_sys::CanvasRenderingContext2d, x: f64, y: f64, w: f64, hgt: f64, wall: (&str, &str), roof: &str) {
    let d = w * 0.5;
    poly(ctx, &[(x - w, y - hgt), (x, y - hgt + d), (x, y + d), (x - w, y)], wall.0);
    poly(ctx, &[(x, y - hgt + d), (x + w, y - hgt), (x + w, y), (x, y + d)], wall.1);
    poly(ctx, &[(x, y - hgt - d), (x + w, y - hgt), (x, y - hgt + d), (x - w, y - hgt)], roof);
}

/// 육각형 칸 선과 땅을 캔버스에 그려요. 주인 있는 땅은 두께가 있는 입체 타일로 솟아 있고, 그 위에 집·호텔이 서 있어요.
fn draw_hex() {
    let (Some(v), Some(c)) = (view(), by_id("map-hex")) else { return };
    let cv: web_sys::HtmlCanvasElement = c.unchecked_into();
    let (w, h) = size(&v);
    let (w, h) = (w + PADC * 2.0, h + PADC * 2.0);
    let dpr = web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0).min(1.5);
    let (pw, ph) = ((w * dpr) as u32, (h * dpr) as u32);
    if cv.width() != pw || cv.height() != ph {
        cv.set_width(pw);
        cv.set_height(ph);
    }
    let Some(ctx) = cv.get_context("2d").ok().flatten().and_then(|c| c.dyn_into::<web_sys::CanvasRenderingContext2d>().ok()) else { return };
    let _ = ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);
    ctx.clear_rect(0.0, 0.0, w, h);
    let z = Z.get();
    let (ox, oy) = ORIGIN.get();
    let (ox, oy) = (ox - PADC, oy - PADC);
    let k = 2f64.powi(z - HZ);
    let s = HS * k; // 지금 확대에서의 한 변 길이
    let (w3, step_y) = (3f64.sqrt() * s, 1.5 * s);
    let corners = |x: f64, y: f64, sc: f64| -> [(f64, f64); 6] {
        let mut a = [(0.0, 0.0); 6];
        for (i, p) in a.iter_mut().enumerate() {
            let t = (60.0 * i as f64 - 30.0).to_radians();
            *p = (x - ox + s * sc * t.cos(), y - oy + s * sc * t.sin());
        }
        a
    };
    let (r0, r1) = (((oy - s) / step_y).floor() as i64, ((oy + h + s) / step_y).ceil() as i64);
    let mine = ME.get().map(|(la, lo, _)| { let (x, y) = world(la, lo, z); cell_of(x, y, z) });
    let sel = SEL.get();
    let me = my_uid();
    // 1) 칸 선: 한 번에 모아서
    ctx.begin_path();
    let mut owned: Vec<((i64, i64), f64, f64, Land)> = Vec::new();
    for r in r0..=r1 {
        let q0 = ((ox - w3) / w3 - r as f64 / 2.0).floor() as i64;
        let q1 = ((ox + w + w3) / w3 - r as f64 / 2.0).ceil() as i64;
        for q in q0..=q1 {
            let (x, y) = (w3 * (q as f64 + r as f64 / 2.0), step_y * r as f64);
            let p = corners(x, y, 1.0);
            if let Some(l) = land_at((q, r)) {
                owned.push(((q, r), x, y, l));
            }
            ctx.move_to(p[0].0, p[0].1);
            for pt in &p[1..] {
                ctx.line_to(pt.0, pt.1);
            }
            ctx.close_path();
        }
    }
    // 유리 타일 느낌: 굵은 흰 테두리 위에 가는 어두운 선
    ctx.set_stroke_style_str("rgba(255,255,255,0.9)");
    ctx.set_line_width(3.4);
    ctx.stroke();
    ctx.set_stroke_style_str("rgba(70,90,130,0.30)");
    ctx.set_line_width(0.9);
    ctx.stroke();
    // 2) 내가 서 있는 칸 (주인 없을 때): 옅은 파랑
    let cell_xy = |c: (i64, i64)| (w3 * (c.0 as f64 + c.1 as f64 / 2.0), step_y * c.1 as f64);
    if let Some(c) = mine.filter(|c| land_at(*c).is_none()) {
        let (x, y) = cell_xy(c);
        poly(&ctx, &corners(x, y, 1.0), "rgba(10,124,255,0.22)");
    }
    // 3) 주인 있는 땅: 위쪽 줄부터 (아래 줄이 위 줄을 가려요)
    for (c, x, y, l) in &owned {
        let hue = if l.owner == me { 212 } else { hue_of(&l.owner) };
        let th = s * (0.16 + 0.05 * l.lv as f64); // 타일 두께
        let p = corners(*x, *y, 0.94);
        let t: Vec<(f64, f64)> = p.iter().map(|a| (a.0, a.1 - th)).collect();
        // 그림자
        let sh: Vec<(f64, f64)> = p.iter().map(|a| (a.0 + th * 0.55, a.1 + th * 0.7)).collect();
        poly(&ctx, &sh, "rgba(20,30,60,0.16)");
        // 옆면 (오른쪽 → 앞 → 왼쪽, 빛은 왼쪽 위에서)
        for (i, lum) in [(0usize, 36), (1, 42), (2, 50), (3, 46)] {
            let j = (i + 1) % 6;
            poly(&ctx, &[p[i], p[j], t[j], t[i]], &format!("hsla({hue},62%,{lum}%,0.86)"));
        }
        // 윗면과 윤기
        poly(&ctx, &t, &format!("hsla({hue},78%,60%,0.62)"));
        let g = corners(*x, *y, 0.7);
        let g: Vec<(f64, f64)> = g.iter().map(|a| (a.0, a.1 - th)).collect();
        poly(&ctx, &g, &format!("hsla({hue},90%,80%,0.35)"));
        ctx.begin_path();
        ctx.move_to(t[3].0, t[3].1);
        for i in [4, 5, 0] {
            ctx.line_to(t[i].0, t[i].1);
        }
        ctx.set_stroke_style_str("rgba(255,255,255,0.85)");
        ctx.set_line_width(1.4);
        ctx.stroke();
        // 건물
        let (bx, by) = (x - ox, y - oy - th);
        // 모노폴리 말처럼: 초록 집, 빨간 호텔 (세모 지붕이 있는 플라스틱 조각)
        let u = s * 0.2;
        let house = |dx: f64, dy: f64| gable(&ctx, bx + dx, by + dy, u, u, 0);
        match l.lv {
            1 => house(0.0, 0.0),
            2 => {
                house(-u * 1.3, -u * 0.2);
                house(u * 1.3, u * 0.2);
            }
            3 => {
                house(0.0, -u * 1.1);
                house(-u * 1.4, u * 0.3);
                house(u * 1.4, u * 0.3);
            }
            4 => gable(&ctx, bx, by + u * 0.2, u * 1.9, u * 1.5, 1),
            _ => {}
        }
        // 이름 (충분히 클 때만)
        if s >= 36.0 && !l.name.is_empty() {
            ctx.set_font(&format!("700 {}px -apple-system, 'Apple SD Gothic Neo', sans-serif", (s * 0.22).round()));
            ctx.set_text_align("center");
            ctx.set_fill_style_str("rgba(255,255,255,0.95)");
            let _ = ctx.fill_text(&l.name, bx, by + s * 0.62);
        }
        let _ = c;
    }
    // 4) 고른 칸 테두리
    if let Some(c) = sel {
        let (x, y) = cell_xy(c);
        let th = land_at(c).map(|l| s * (0.16 + 0.05 * l.lv as f64)).unwrap_or(0.0);
        let p = corners(x, y, 1.0);
        ctx.begin_path();
        ctx.move_to(p[0].0, p[0].1 - th);
        for pt in &p[1..] {
            ctx.line_to(pt.0, pt.1 - th);
        }
        ctx.close_path();
        ctx.set_stroke_style_str("#ff9f0a");
        ctx.set_line_width(3.0);
        ctx.stroke();
    }
}

/// 지금 보이는 구역의 땅을 읽어요. 읽은 지 60초 안 된 구역은 다시 읽지 않아요 (서버를 아껴요)
fn load_lands(force: bool) {
    if !cloud::signed_in() {
        return;
    }
    let Some(v) = view() else { return };
    let (w, h) = size(&v);
    let z = Z.get();
    let (ox, oy) = ORIGIN.get();
    let now = js_sys::Date::now();
    let mut need: Vec<String> = Vec::new();
    let step = 60.0;
    let mut y = -step;
    while y <= h + step {
        let mut x = -step;
        while x <= w + step {
            let ch = chunk_of(cell_of(ox + x, oy + y, z));
            if !need.contains(&ch) && CHUNKS.with(|c| force || c.borrow().get(&ch).is_none_or(|t| now - t > 60000.0)) {
                need.push(ch);
            }
            x += step;
        }
        y += step;
    }
    if need.is_empty() {
        return;
    }
    need.truncate(30);
    CHUNKS.with(|c| need.iter().for_each(|n| { c.borrow_mut().insert(n.clone(), now); }));
    wasm_bindgen_futures::spawn_local(async move { fetch_chunks(need).await });
}
async fn fetch_chunks(need: Vec<String>) {
    let arr: js_sys::Array = need.iter().map(|n| JsValue::from_str(n)).collect();
    match cloud::call("landsIn", &[arr.into()]).await {
        Ok(list) => {
            LANDS.with(|l| {
                let mut l = l.borrow_mut();
                l.retain(|c, _| !need.contains(&chunk_of(*c)));
                for it in js_sys::Array::from(&list).iter() {
                    let g = |k: &str| cloud::get(&it, k);
                    let (Some(q), Some(r)) = (g("q").as_f64(), g("r").as_f64()) else { continue };
                    l.insert((q as i64, r as i64), Land { owner: g("owner").as_string().unwrap_or_default(), name: g("name").as_string().unwrap_or_default(), lv: g("lv").as_f64().unwrap_or(0.0).clamp(0.0, 4.0) as usize, pot: g("pot").as_f64().unwrap_or(0.0) });
                }
            });
            draw_hex();
            paint_sheet();
        }
        Err(_) => CHUNKS.with(|c| need.iter().for_each(|n| { c.borrow_mut().remove(n); })),
    }
}

fn dist(a: (i64, i64), b: (i64, i64)) -> i64 {
    let (dq, dr) = (a.0 - b.0, a.1 - b.1);
    (dq.abs() + dr.abs() + (dq + dr).abs()) / 2
}
fn my_cell() -> Option<(i64, i64)> {
    ME.get().map(|(la, lo, _)| { let z = Z.get(); let (x, y) = world(la, lo, z); cell_of(x, y, z) })
}
fn tok(n: f64) -> String {
    cloud::fmt_num(n, 0)
}

/// 고른 칸의 카드: 빈 땅이면 사기, 내 땅이면 이름·건물·받기·팔기, 남의 땅이면 주인과 통행료
fn paint_sheet() {
    let Some(el) = by_id("land-sheet") else { return };
    let Some(c) = SEL.get() else {
        let _ = el.class_list().remove_1("on");
        return;
    };
    let me = my_uid();
    let html = match land_at(c) {
        None => {
            let near = my_cell().is_some_and(|m| dist(m, c) <= 1);
            format!(
                r#"<p class="ld-eyebrow">칸 {}, {}</p><h2 class="ld-title">주인 없는 땅</h2><p class="ld-sub">사면 내 땅이 되고, 다른 사람이 지나갈 때마다 통행료를 받아요.</p>{}"#,
                c.0,
                c.1,
                if near { r#"<button type="button" class="btn primary ld-btn" data-act="buy">땅 사기 · 10 토큰</button>"#.to_string() } else { r#"<p class="ld-note">이 칸이나 바로 옆 칸에 가야 살 수 있어요</p>"#.to_string() }
            )
        }
        Some(l) if l.owner == me => format!(
            r#"<p class="ld-eyebrow">내 땅 · {}</p><div class="ld-name"><input id="ld-name" maxlength="12" autocomplete="off" placeholder="땅 이름 (12자까지)" value="{}" aria-label="땅 이름"><button type="button" class="btn ld-save" data-act="name">저장</button></div><p class="ld-sub">지나가면 통행료 {} 토큰 · 모인 통행료 <b>{} 토큰</b></p><div class="ld-row">{}{}</div><button type="button" class="ld-sell" data-act="sell">팔기 · {} 토큰 돌려받기</button>"#,
            LV_NAME[l.lv],
            dom::esc(&l.name),
            tok(TOLL[l.lv]),
            tok(l.pot),
            if l.lv < 4 { format!(r#"<button type="button" class="btn primary ld-btn" data-act="build">{} 짓기 · {} 토큰</button>"#, LV_NAME[l.lv + 1], tok(BUILD[l.lv])) } else { String::new() },
            if l.pot > 0.0 { format!(r#"<button type="button" class="btn ld-btn" data-act="rent">통행료 받기 · {} 토큰</button>"#, tok(l.pot)) } else { String::new() },
            tok(REFUND[l.lv] + l.pot)
        ),
        Some(l) => {
            let p = cloud::call_now("peek", &[JsValue::from_str(&l.owner)]).unwrap_or(JsValue::NULL);
            let who = cloud::get(&p, "name").as_string().filter(|n| !n.is_empty()).unwrap_or_else(|| "다른 사람".into());
            format!(
                r#"<p class="ld-eyebrow">{}님의 땅 · {}</p><h2 class="ld-title">{}</h2><p class="ld-sub">이 칸에 들어가면 통행료 <b>{} 토큰</b>을 내요.</p>"#,
                dom::esc(&who),
                LV_NAME[l.lv],
                if l.name.is_empty() { "이름 없는 땅".to_string() } else { dom::esc(&l.name) },
                tok(TOLL[l.lv])
            )
        }
    };
    el.set_inner_html(&format!(r#"<span class="ld-grab"></span>{html}"#));
    let _ = el.class_list().add_1("on");
}

/// 지갑이 바뀌는 땅 행동 하나 (지갑 쓰기 줄을 서서 차례대로 해요)
async fn wallet_call(name: &'static str, args: Vec<JsValue>) -> Result<JsValue, JsValue> {
    let f = Closure::once_into_js(move || cloud::call_now(name, &args).unwrap_or(JsValue::UNDEFINED));
    wasm_bindgen_futures::JsFuture::from(crate::money::wallet_op(f.unchecked_into())).await
}
fn refresh_cell(c: (i64, i64)) {
    let ch = chunk_of(c);
    CHUNKS.with(|m| { m.borrow_mut().insert(ch.clone(), js_sys::Date::now()); });
    wasm_bindgen_futures::spawn_local(async move { fetch_chunks(vec![ch]).await });
}
fn act(kind: String) {
    let Some(c) = SEL.get() else { return };
    if !cloud::signed_in() {
        return crate::shell::toast("로그인하고 땅을 살 수 있어요");
    }
    if BUSY.replace(true) {
        return;
    }
    let id = format!("{}_{}", c.0, c.1);
    let land = land_at(c);
    wasm_bindgen_futures::spawn_local(async move {
        let toast = |t: &str| crate::shell::toast(t);
        let r: Result<String, String> = async {
            match kind.as_str() {
                "buy" => {
                    crate::money::ensure(10.0).await?;
                    wallet_call("landBuy", vec![(c.0 as f64).into(), (c.1 as f64).into(), "".into()]).await.map_err(|_| "땅을 사지 못했어요. 방금 다른 사람이 샀을 수 있어요".to_string())?;
                    dom::vibrate(18);
                    Ok("땅을 샀어요. 이름을 지어 보세요".into())
                }
                "name" => {
                    let name = by_id("ld-name").and_then(|i| i.dyn_into::<web_sys::HtmlInputElement>().ok()).map(|i| i.value()).unwrap_or_default();
                    let name = name.trim().to_string();
                    if name.chars().count() > 12 || name.contains(['<', '>']) {
                        return Err("이름은 12자까지, < > 없이 적어 주세요".into());
                    }
                    cloud::call("landName", &[JsValue::from_str(&id), JsValue::from_str(&name)]).await.map_err(|e| format!("이름을 바꾸지 못했어요. {}", cloud::why(&e.code)))?;
                    Ok("땅 이름을 바꿨어요".into())
                }
                "build" => {
                    let lv = land.as_ref().map(|l| l.lv).unwrap_or(0).min(3);
                    crate::money::ensure(BUILD[lv]).await?;
                    wallet_call("landAct", vec![JsValue::from_str(&id), "build".into()]).await.map_err(|_| "짓지 못했어요. 잠시 뒤 다시 해 주세요".to_string())?;
                    dom::vibrate(18);
                    Ok(format!("{}을(를) 지었어요. 통행료가 {} 토큰이 됐어요", LV_NAME[lv + 1], tok(TOLL[lv + 1])))
                }
                "rent" => {
                    let got = wallet_call("landAct", vec![JsValue::from_str(&id), "rent".into()]).await.map_err(|_| "받지 못했어요. 잠시 뒤 다시 해 주세요".to_string())?;
                    Ok(format!("통행료 {} 토큰을 받았어요", tok(got.as_f64().unwrap_or(0.0))))
                }
                _ => {
                    if !crate::shell::ask("이 땅을 팔까요?", "건물도 함께 사라지고, 들인 토큰의 절반과 모인 통행료가 돌아와요.", "팔기", "취소").await {
                        return Err(String::new());
                    }
                    let got = wallet_call("landAct", vec![JsValue::from_str(&id), "sell".into()]).await.map_err(|_| "팔지 못했어요. 잠시 뒤 다시 해 주세요".to_string())?;
                    Ok(format!("땅을 팔고 {} 토큰을 받았어요", tok(got.as_f64().unwrap_or(0.0))))
                }
            }
        }
        .await;
        match r {
            Ok(m) => toast(&m),
            Err(m) if !m.is_empty() => toast(&m),
            _ => {}
        }
        BUSY.set(false);
        refresh_cell(c);
        paint_hud();
    });
}

/// 새 칸에 들어갔을 때: 남의 땅이면 통행료를 내요 (같은 땅은 1시간에 한 번만)
fn entered(c: (i64, i64)) {
    if !cloud::signed_in() {
        return;
    }
    wasm_bindgen_futures::spawn_local(async move {
        fetch_chunks(vec![chunk_of(c)]).await;
        let Some(l) = land_at(c) else { return };
        if l.owner == my_uid() {
            return;
        }
        let id = format!("{}_{}", c.0, c.1);
        let key = format!("toll-{id}");
        let last: f64 = dom::store_get(&key, "0").parse().unwrap_or(0.0);
        if js_sys::Date::now() - last < 3_600_000.0 {
            return;
        }
        let toll = TOLL[l.lv];
        let name = if l.name.is_empty() { "남의 땅".to_string() } else { format!("\"{}\"", l.name) };
        if crate::money::ensure(toll).await.is_err() {
            return crate::shell::toast(&format!("{name}에 들어왔어요. 토큰이 모자라 통행료 {} 토큰을 못 냈어요", tok(toll)));
        }
        if wallet_call("landAct", vec![JsValue::from_str(&id), "toll".into()]).await.is_ok() {
            dom::store_set(&key, &js_sys::Date::now().to_string());
            dom::vibrate(30);
            crate::shell::toast(&format!("{name}에 들어왔어요. 통행료 {} 토큰을 냈어요", tok(toll)));
            paint_hud();
        }
    });
}
fn paint_hud() {
    let Some(e) = by_id("map-hud") else { return };
    match crate::money::balance() {
        Some(b) if cloud::signed_in() => {
            e.set_hidden(false);
            dom::set_text("map-hud-n", &tok(b.floor()));
        }
        _ => e.set_hidden(true),
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
    let cell = { let (x, y) = world(la, lo, HZ); cell_of(x, y, HZ) };
    if LAST_CELL.replace(Some(cell)) != Some(cell) {
        entered(cell);
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

/// 손가락 하나는 끌기, 둘은 확대·축소(핀치), 두 번 톡은 확대. 모든 움직임은 지도 덩어리(타일+육각형+내 위치)에 한 번에 거는
/// transform 하나로만 해서, 육각형이 지도에 붙어서 움직이고 JS가 할 일은 숫자 몇 개뿐이에요. 손을 떼면 한 번만 다시 깔아요.
#[derive(Default, Clone, Copy)]
struct Tf {
    tx: f64,
    ty: f64,
    s: f64,
}
#[derive(Default)]
struct Gst {
    ptrs: Vec<(i32, f64, f64)>, // id, x, y (지도 안 좌표)
    base: Tf,                   // 지금 손가락 모양이 시작될 때의 변환
    cur: Tf,
    m0: (f64, f64),             // 시작 때 손가락(들)의 가운데
    d0: f64,                    // 시작 때 두 손가락 사이 거리
    moved: bool,
    t0: f64,                    // 처음 닿은 시각
    last_tap: (f64, f64, f64),  // 지난 탭 시각, x, y
}
thread_local! {
    static G: RefCell<Gst> = RefCell::new(Gst { base: Tf { s: 1.0, ..Tf::default() }, cur: Tf { s: 1.0, ..Tf::default() }, ..Gst::default() });
}
fn apply(t: Tf) {
    if let Some(wd) = by_id("map-world") {
        let _ = wd.style().set_property("transform", &format!("translate3d({}px,{}px,0) scale({})", t.tx, t.ty, t.s));
    }
}
fn mid_dist(g: &Gst) -> ((f64, f64), f64) {
    match g.ptrs.as_slice() {
        [a] => ((a.1, a.2), 0.0),
        [a, b, ..] => (((a.1 + b.1) / 2.0, (a.2 + b.2) / 2.0), ((a.1 - b.1).hypot(a.2 - b.2)).max(1.0)),
        _ => ((0.0, 0.0), 0.0),
    }
}
/// 손가락 모양이 바뀔 때(닿음·뗌): 지금 변환을 기준으로 다시 시작해요
fn rebase(g: &mut Gst) {
    g.base = g.cur;
    let (m, d) = mid_dist(g);
    g.m0 = m;
    g.d0 = d;
}
/// 지금 변환을 실제 중심·확대 단계로 바꾸고 지도를 한 번 새로 깔아요
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
    G.with(|g| {
        let mut g = g.borrow_mut();
        g.cur = Tf { s: 1.0, ..Tf::default() };
        g.base = g.cur;
    });
    render();
}
fn tap_at(x: f64, y: f64) {
    let Some(v) = view() else { return };
    let (w, h) = size(&v);
    let z = Z.get();
    let (lat, lon) = CENTER.get();
    let (cx, cy) = world(lat, lon, z);
    let c = cell_of(cx - w / 2.0 + x, cy - h / 2.0 + y, z);
    SEL.set(if SEL.get() == Some(c) { None } else { Some(c) });
    dom::vibrate(8);
    draw_hex();
    paint_sheet();
    if let Some(l) = land_at(c) {
        // 주인 이름을 알아 와서 카드를 다시 그려요
        wasm_bindgen_futures::spawn_local(async move {
            let _ = cloud::call("fetchProfiles", &[js_sys::Array::of1(&JsValue::from_str(&l.owner)).into()]).await;
            paint_sheet();
        });
    }
}
/// 확대 단계를 하나 바꿔요 (버튼·휠·두 번 톡). 화면 가운데(또는 fx, fy)를 기준으로 해요
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
    // 지도는 뒤로 기울어져 있어요. 손가락 자리는 기울어진 판 위의 좌표로 받아요 (브라우저가 기울기를 거꾸로 풀어 줘요).
    // 판(#map-hit)은 화면보다 사방으로 커서, 화면 왼쪽 위에 해당하는 만큼을 빼요
    let (w, h) = view().map(|v| size(&v)).unwrap_or((0.0, 0.0));
    (e.offset_x() as f64 - w * 0.4, e.offset_y() as f64 - h * 0.6)
}

fn bind() {
    if BOUND.replace(true) {
        return;
    }
    let Some(v) = by_id("map-hit") else { return };
    dom::on(&v, "pointerdown", |e| {
        let Some(p) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
        if let Some(v) = by_id("map-hit") {
            let _ = v.set_pointer_capture(p.pointer_id());
        }
        let (x, y) = local(p);
        G.with(|g| {
            let mut g = g.borrow_mut();
            if g.ptrs.is_empty() {
                g.moved = false;
                g.t0 = js_sys::Date::now();
            } else {
                g.moved = true; // 손가락이 둘이면 탭이 아니에요
            }
            g.ptrs.retain(|q| q.0 != p.pointer_id());
            g.ptrs.push((p.pointer_id(), x, y));
            rebase(&mut g);
        });
    });
    dom::on(&v, "pointermove", |e| {
        let Some(p) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
        let (x, y) = local(p);
        let big = G.with(|g| {
            let mut g = g.borrow_mut();
            let Some(q) = g.ptrs.iter_mut().find(|q| q.0 == p.pointer_id()) else { return false };
            q.1 = x;
            q.2 = y;
            let (m, d) = mid_dist(&g);
            let b = g.base;
            let mut s = if g.ptrs.len() >= 2 && g.d0 > 0.0 { b.s * d / g.d0 } else { b.s };
            // 확대 단계 바깥으로 너무 나가지 않게 (조금은 여유를 둬요)
            let z = Z.get() as f64;
            s = s.clamp(2f64.powf(MIN_Z as f64 - 0.4 - z), 2f64.powf(MAX_Z as f64 + 0.4 - z));
            let k = s / b.s;
            let t = Tf { tx: m.0 - k * (g.m0.0 - b.tx), ty: m.1 - k * (g.m0.1 - b.ty), s };
            if (m.0 - g.m0.0).abs() + (m.1 - g.m0.1).abs() > 6.0 || (s - b.s).abs() > 0.01 {
                g.moved = true;
            }
            g.cur = t;
            apply(t);
            if g.moved {
                FOLLOW.set(false);
            }
            // 너무 멀리 끌면(그려 둔 범위 끝) 새로 깔고 이어서 끌어요
            t.tx.abs() > 230.0 || t.ty.abs() > 230.0
        });
        if big {
            commit();
            G.with(|g| rebase(&mut g.borrow_mut()));
        }
    });
    for ev in ["pointerup", "pointercancel"] {
        dom::on(&v, ev, |e| {
            let Some(p) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
            let (x, y) = local(p);
            let (done, tap, dbl) = G.with(|g| {
                let mut g = g.borrow_mut();
                g.ptrs.retain(|q| q.0 != p.pointer_id());
                if !g.ptrs.is_empty() {
                    rebase(&mut g);
                    return (false, false, false);
                }
                let now = js_sys::Date::now();
                let tap = !g.moved && now - g.t0 < 400.0 && e.type_() == "pointerup";
                let dbl = tap && now - g.last_tap.0 < 320.0 && (x - g.last_tap.1).abs() + (y - g.last_tap.2).abs() < 30.0;
                g.last_tap = if tap { (now, x, y) } else { (0.0, 0.0, 0.0) };
                (true, tap, dbl)
            });
            if !done {
                return;
            }
            if dbl {
                zoom_by(1, x, y);
            } else if tap {
                tap_at(x, y);
            } else {
                commit();
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
    if let Some(b) = by_id("map-me-btn") {
        dom::on(&b, "click", |_| recenter());
    }
    if let Some(sh) = by_id("land-sheet") {
        dom::on(&sh, "click", |e| {
            if let Some(b) = dom::closest(&e, "[data-act]") {
                act(dom::data(&b, "act"));
            }
        });
    }
}

/// 홈 탭이 열렸을 때
pub fn open() {
    bind();
    start_watch();
    paint_hud();
    if HUD.get() == 0 {
        let cb = Closure::<dyn FnMut()>::new(paint_hud).into_js_value();
        HUD.set(web_sys::window().and_then(|w| w.set_interval_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 2000).ok()).unwrap_or(0));
    }
    // 화면이 보이고 크기가 정해진 다음에 깔아요
    wasm_bindgen_futures::spawn_local(async {
        dom::sleep(60).await;
        render();
    });
}
/// 다른 탭으로 갈 때: 배터리를 아끼려고 위치 읽기를 멈춰요
pub fn leave() {
    stop_watch();
    if let Some(w) = web_sys::window() {
        w.clear_interval_with_handle(HUD.replace(0));
    }
}
