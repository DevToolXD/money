//! 게임 탭: 독소전쟁 대항전. 레드팀(소련)과 블루팀(독일) 중 하나를 골라, 팀원과 함께 누를수록 전선이 밀려요.
//! 지도는 1942년 독일 최대 판도(10개 지역)와 소련(실제 주 경계로 묶은 16개 지역)에서 시작해요.
//! 두 팀 클릭 비율만큼의 영토 넓이로 목표 전선이 정해지고, 지도는 그 목표를 향해 한 지역씩 점령해 가요.
//! 서버를 아끼려고: 클릭은 모아 두었다가 4.5초에 한 번만 보내고(규칙은 4초, 한 번에 200까지),
//! 전체 점수는 문서 하나만, 게임 탭을 보고 있을 때만 들어요. 랭킹은 30초에 한 번 20명만 읽어요.

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::dom::{self, by_id, esc};

const MAP: &str = include_str!("game_map.json");
/// 한 번에 보내는 최대 클릭 (서버 규칙과 같아요)
const MAX_FLUSH: f64 = 200.0;
/// 보내는 간격 (규칙의 4초보다 넉넉하게)
const FLUSH_MS: f64 = 4500.0;
/// 손으로 누를 수 있는 빠르기보다 빠른 클릭은 버려요 (1초 20번)
const MAX_PER_SEC: u32 = 20;

#[derive(Clone)]
struct Region {
    name: String,
    side: String,
    d: String,
    cx: f64,
    cy: f64,
    area: f64,
}
#[derive(Clone)]
struct Row {
    uid: String,
    team: String,
    name: String,
    clicks: f64,
}

thread_local! {
    static REGIONS: RefCell<Vec<Region>> = const { RefCell::new(Vec::new()) };
    static BUILT: Cell<bool> = const { Cell::new(false) };
    /// 서버 점수 (소련, 독일)
    static SR: Cell<f64> = const { Cell::new(0.0) };
    static SB: Cell<f64> = const { Cell::new(0.0) };
    /// 지도에 그려진 전선: 앞에서부터 이만큼 독일 땅이에요
    static SHOWN: Cell<usize> = const { Cell::new(10) };
    /// 내 팀 ("r"/"b", 아직 안 골랐으면 빈 글자), 서버에 올라간 내 클릭
    static TEAM: RefCell<String> = const { RefCell::new(String::new()) };
    static ME_LOADED: Cell<bool> = const { Cell::new(false) };
    static MY_SENT: Cell<f64> = const { Cell::new(0.0) };
    /// 아직 안 보낸 클릭, 보내는 중인 클릭, 마지막으로 보낸 시각
    static PENDING: Cell<f64> = const { Cell::new(0.0) };
    static FLYING: Cell<bool> = const { Cell::new(false) };
    static LAST_FLUSH: Cell<f64> = const { Cell::new(0.0) };
    /// 1초 안 클릭 수 (너무 빠른 클릭 거르기)
    static SEC: Cell<(f64, u32)> = const { Cell::new((0.0, 0)) };
    static UNSUB: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
    static TIMER: Cell<i32> = const { Cell::new(0) };
    static STEP_AT: Cell<f64> = const { Cell::new(0.0) };
    static RANK: RefCell<Vec<Row>> = const { RefCell::new(Vec::new()) };
    static RANK_AT: Cell<f64> = const { Cell::new(0.0) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
    /// 처음 받은 점수는 움직임 없이 바로 그 전선으로 그려요
    static GOT: Cell<bool> = const { Cell::new(false) };
    /// 지도 전체 보기 (끄면 카메라가 전선을 따라가요)
    static FULL: Cell<bool> = const { Cell::new(false) };
    static CAM: RefCell<String> = const { RefCell::new(String::new()) };
}

/// 지도 안의 SVG 요소 (HTML 요소가 아니라서 by_id로는 못 찾아요)
fn svg_el(id: &str) -> Option<web_sys::Element> {
    dom::doc().get_element_by_id(id)
}
fn now() -> f64 {
    js_sys::Date::now()
}
fn fmt(v: f64) -> String {
    cloud::fmt_num(v.max(0.0).floor(), 0)
}
fn team() -> String {
    TEAM.with(|t| t.borrow().clone())
}
fn side_name(t: &str) -> &'static str {
    if t == "r" {
        "소련"
    } else {
        "독일"
    }
}
fn my_uid() -> String {
    cloud::cloud().and_then(|c| cloud::get(&c, "uid").as_string()).unwrap_or_default()
}
fn my_name() -> String {
    let u = cloud::cloud().map(|c| cloud::get(&c, "user")).unwrap_or(JsValue::NULL);
    let n = cloud::get(&u, "name").as_string().unwrap_or_default();
    let n = if n.trim().is_empty() { cloud::get(&u, "id").as_string().unwrap_or_default() } else { n };
    n.chars().take(20).collect()
}

/// 지금 보이는 점수 (내 팀은 아직 안 보낸 클릭까지 더해요)
fn scores() -> (f64, f64) {
    let p = PENDING.get();
    let t = team();
    (SR.get() + if t == "r" { p } else { 0.0 }, SB.get() + if t == "b" { p } else { 0.0 })
}
/// 두 팀 클릭 비율로 정한 목표 전선: 독일 땅의 넓이가 독일 클릭 비율과 가장 가까워지는 자리
/// (지역 개수가 아니라 차지한 영토 넓이로 따져요. 양쪽 모두 최소 한 지역은 남아요. 아무도 안 눌렀으면 1942년 판도)
fn target() -> usize {
    let (r, b) = scores();
    if r + b <= 0.0 {
        return start();
    }
    REGIONS.with(|regs| {
        let regs = regs.borrow();
        let n = regs.len();
        if n < 2 {
            return start();
        }
        let total: f64 = regs.iter().map(|g| g.area).sum();
        let want = total * b / (r + b);
        let mut best = (1usize, f64::MAX);
        let mut acc = 0.0;
        for (k, g) in regs.iter().enumerate().take(n - 1) {
            acc += g.area;
            let d = (acc - want).abs();
            if d < best.1 {
                best = (k + 1, d);
            }
        }
        best.0
    })
}

/// 지역 수와, 아무도 안 눌렀을 때의 1942년 전선 (독일 지역 수)
fn count() -> usize {
    REGIONS.with(|r| r.borrow().len())
}
fn start() -> usize {
    REGIONS.with(|r| r.borrow().iter().filter(|g| g.side == "b").count())
}
fn load_regions() {
    if REGIONS.with(|r| !r.borrow().is_empty()) {
        return;
    }
    let Ok(j) = js_sys::JSON::parse(MAP) else { return };
    let list = js_sys::Array::from(&cloud::get(&j, "regions"));
    let v: Vec<Region> = list
        .iter()
        .map(|r| {
            let s = |k: &str| cloud::get(&r, k).as_string().unwrap_or_default();
            let f = |k: &str| cloud::get(&r, k).as_f64().unwrap_or(0.0);
            Region { name: s("name"), side: s("side"), d: s("d"), cx: f("cx"), cy: f("cy"), area: f("area") }
        })
        .collect();
    REGIONS.with(|r| *r.borrow_mut() = v);
}

const AIM: &str = r#"<circle cx="12" cy="12" r="7.5"/><circle cx="12" cy="12" r="2.2"/><path d="M12 2v4.5M12 17.5V22M2 12h4.5M17.5 12H22"/>"#;
fn svg(d: &str) -> String {
    format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{d}</svg>"#)
}

fn build() {
    if BUILT.get() {
        return;
    }
    let Some(root) = by_id("game-root") else { return };
    load_regions();
    if !GOT.get() {
        SHOWN.set(start());
    }
    let ctx = js_sys::JSON::parse(MAP).ok().map(|j| cloud::get(&j, "context").as_string().unwrap_or_default()).unwrap_or_default();
    let k = SHOWN.get();
    let paths: String = REGIONS.with(|r| {
        r.borrow()
            .iter()
            .enumerate()
            .map(|(i, g)| format!(r#"<path id="gm-p{i}" class="gm-reg {}" d="{}"><title>{}</title></path>"#, if i < k { "b" } else { "r" }, g.d, esc(&g.name)))
            .collect()
    });
    root.set_inner_html(&format!(
        r#"<header class="sns-head"><h1 id="title-game">독소전쟁</h1></header>
<div class="gm-card">
  <div class="gm-top"><span class="gm-side r"><i></i>소련 <b id="gm-rp">50%</b></span><span class="gm-side b">독일 <b id="gm-bp">50%</b><i></i></span></div>
  <div class="gm-bar" aria-hidden="true"><span class="gm-bar-r" id="gm-bar"></span></div>
  <p class="gm-news" id="gm-news"></p>
  <div class="gm-view" id="gm-view"><svg class="gm-map" id="gm-svg" viewBox="0 24 1000 780" role="img" aria-label="전쟁 지도"><path class="gm-ctx" d="{ctx}"/><g id="gm-regs">{paths}</g><g id="gm-fx"></g><g class="gm-lbl"><text id="gm-rt" class="r" x="0" y="0">0</text><text id="gm-bt" class="b" x="0" y="0">0</text></g></svg><button type="button" class="gm-full" id="gm-full">전체 지도</button></div>
</div>
<div class="gm-play" id="gm-play"></div><div class="gm-fxl" id="gm-fxl" aria-hidden="true"></div>
<section class="gm-rank" aria-labelledby="gm-rank-t"><h2 id="gm-rank-t">랭킹</h2><div class="gm-cols" id="gm-rank"></div><p class="gm-mine" id="gm-mine"></p></section>"#
    ));
    BUILT.set(true);
    if let Some(b) = by_id("gm-full") {
        dom::on(&b, "click", |_| {
            FULL.set(!FULL.get());
            dom::set_text("gm-full", if FULL.get() { "전선 보기" } else { "전체 지도" });
            dom::vibrate(6);
            camera();
        });
    }
    camera();
    if let Some(p) = by_id("gm-play") {
        dom::on(&p, "click", on_play_click);
        // 누르는 순간 바로 세요 (손을 떼기를 기다리지 않아요). 여러 손가락도 따로 세요
        dom::on(&p, "pointerdown", |e| {
            if dom::closest(&e, "#gm-hit").is_some() {
                e.prevent_default();
                let (x, y) = e.dyn_ref::<web_sys::PointerEvent>().map(|p| (p.client_x() as f64, p.client_y() as f64)).unwrap_or((0.0, 0.0));
                hit(x, y);
            }
        });
    }
    paint_play();
    paint_score();
}

fn paint_play() {
    let Some(p) = by_id("gm-play") else { return };
    if !cloud::signed_in() {
        return dom::set_html_if(&p, r#"<p class="sns-empty">로그인하면 전쟁에 참여할 수 있어요</p>"#);
    }
    if !ME_LOADED.get() {
        return dom::set_html_if(&p, r#"<p class="sns-empty">불러오는 중이에요</p>"#);
    }
    let t = team();
    if t.is_empty() {
        return dom::set_html_if(
            &p,
            r#"<p class="gm-ask">어느 편에 설까요? 한 번 고르면 바꿀 수 없어요</p><div class="gm-pick"><button type="button" class="gm-team r" data-team="r"><b>레드팀</b><span>소련</span></button><button type="button" class="gm-team b" data-team="b"><b>블루팀</b><span>독일</span></button></div>"#,
        );
    }
    dom::set_html_if(
        &p,
        &format!(
            r#"<button type="button" class="gm-hit {t}" id="gm-hit">{}<b>{}군 공격</b></button><p class="gm-me"><span>내 클릭 <b id="gm-my">0</b></span><span>우리 팀 <b id="gm-ours">0</b></span></p>"#,
            svg(AIM),
            side_name(&t)
        ),
    );
    paint_score();
}

fn on_play_click(e: web_sys::Event) {
    let Some(b) = dom::closest(&e, "[data-team]") else { return };
    let t = dom::data(&b, "team");
    if BUSY.replace(true) {
        return;
    }
    dom::vibrate(10);
    spawn_local(async move {
        let side = if t == "r" { "레드팀(소련)" } else { "블루팀(독일)" };
        if crate::shell::ask(&format!("{side}으로 참전할까요?"), "한 번 고르면 팀을 바꿀 수 없어요.", "참전하기", "취소").await {
            match cloud::call("gameJoin", &[JsValue::from_str(&t), JsValue::from_str(&my_name())]).await {
                Ok(_) => {
                    TEAM.with(|x| *x.borrow_mut() = t.clone());
                    MY_SENT.set(0.0);
                    LAST_FLUSH.set(now());
                    paint_play();
                    crate::shell::toast(&format!("{side}에 들어왔어요. 눌러서 전선을 밀어요"));
                    RANK_AT.set(0.0);
                }
                Err(e) => crate::shell::toast(&format!("참전하지 못했어요. {}", cloud::why(&e.code))),
            }
        }
        BUSY.set(false);
    });
}

fn hit(x: f64, y: f64) {
    if team().is_empty() {
        return;
    }
    let t = now();
    let (s0, c) = SEC.get();
    let (s0, c) = if t - s0 > 1000.0 { (t, 0) } else { (s0, c) };
    if c >= MAX_PER_SEC {
        return;
    }
    SEC.set((s0, c + 1));
    PENDING.set(PENDING.get() + 1.0);
    dom::vibrate(12);
    impact(x, y);
    paint_score();
    run();
}

/// 타격감: 버튼이 꾹 눌렸다 튀어나오고, 누른 자리에서 팀 색 +1과 고리가 튀고, 지도가 살짝 흔들려요.
/// 모두 transform·opacity 움직임(Web Animations)이라 레이아웃을 다시 계산하지 않아요
fn impact(x: f64, y: f64) {
    let t = team();
    if let Some(b) = by_id("gm-hit") {
        dom::anim(&b, r#"[{"transform":"scale(1)"},{"transform":"scale(0.9)","offset":0.3},{"transform":"scale(1.03)","offset":0.7},{"transform":"scale(1)"}]"#, r#"{"duration":260,"easing":"cubic-bezier(.2,.8,.2,1)","fill":"none"}"#);
    }
    if let Some(v) = by_id("gm-view") {
        let dx = if js_sys::Math::random() < 0.5 { -3 } else { 3 };
        dom::anim(&v, &format!(r#"[{{"transform":"translate(0,0)"}},{{"transform":"translate({dx}px,-2px)","offset":0.25}},{{"transform":"translate({}px,2px)","offset":0.6}},{{"transform":"translate(0,0)"}}]"#, -dx), r#"{"duration":180,"easing":"ease-out","fill":"none"}"#);
    }
    if dom::reduce_motion() || (x == 0.0 && y == 0.0) {
        return;
    }
    let Some(layer) = by_id("gm-fxl") else { return };
    // 한꺼번에 너무 많이 쌓이지 않게 (연타해도 가벼워요)
    if layer.child_element_count() > 14 {
        if let Some(f) = layer.first_element_child() {
            f.remove();
        }
    }
    let drift = (js_sys::Math::random() - 0.5) * 60.0;
    let html = format!(
        r#"<i class="gm-ring {t}" style="left:{x:.0}px;top:{y:.0}px"></i><b class="gm-plus {t}" style="left:{x:.0}px;top:{y:.0}px;--dx:{drift:.0}px">+1</b>"#
    );
    let _ = layer.insert_adjacent_html("beforeend", &html);
    let l2 = layer.clone();
    leptos::prelude::set_timeout(
        move || {
            for _ in 0..2 {
                if let Some(f) = l2.first_element_child() {
                    f.remove();
                }
            }
        },
        std::time::Duration::from_millis(700),
    );
}

/// 숫자·막대·라벨을 바뀐 것만 고쳐요
fn paint_score() {
    let (r, b) = scores();
    let tot = r + b;
    let bp = if tot > 0.0 { (b / tot * 100.0).round() } else { 50.0 };
    dom::set_text("gm-rp", &format!("{}%", 100.0 - bp));
    dom::set_text("gm-bp", &format!("{bp}%"));
    if let Some(bar) = by_id("gm-bar") {
        let w = format!("{}%", 100.0 - bp);
        if bar.style().get_property_value("width").ok().as_deref() != Some(w.as_str()) {
            let _ = bar.style().set_property("width", &w);
        }
    }
    let t = team();
    if !t.is_empty() {
        dom::set_text("gm-my", &fmt(MY_SENT.get() + PENDING.get()));
        dom::set_text("gm-ours", &fmt(if t == "r" { r } else { b }));
    }
    paint_labels(r, b);
}

/// 각 진영 땅 한가운데(넓이로 무게를 둔 중심)에 그 팀 클릭 수를 써요
fn paint_labels(r: f64, b: f64) {
    let k = SHOWN.get();
    REGIONS.with(|regs| {
        let regs = regs.borrow();
        let n = regs.len();
        if n < 2 {
            return;
        }
        let center = |a: &[Region]| {
            let w: f64 = a.iter().map(|g| g.area).sum::<f64>().max(1.0);
            (a.iter().map(|g| g.cx * g.area).sum::<f64>() / w, a.iter().map(|g| g.cy * g.area).sum::<f64>() / w)
        };
        // 전선을 따라가는 동안에는 화면에 보이는 전선 가까운 땅 가운데, 전체 보기면 진영 전체 가운데
        let (b0, r1) = if FULL.get() { (0, n) } else { (k.saturating_sub(3), (k + 3).min(n)) };
        for (id, part, v) in [("gm-bt", &regs[b0..k], b), ("gm-rt", &regs[k..r1], r)] {
            let Some(el) = svg_el(id) else { continue };
            let (x, y) = center(part);
            let (x, y) = (format!("{x:.0}"), format!("{y:.0}"));
            if el.get_attribute("x").as_deref() != Some(&x) {
                let _ = el.set_attribute("x", &x);
            }
            if el.get_attribute("y").as_deref() != Some(&y) {
                let _ = el.set_attribute("y", &y);
            }
            let s = fmt(v);
            if el.text_content().as_deref() != Some(&s) {
                el.set_text_content(Some(&s));
            }
        }
    });
}

/// 전선을 목표 쪽으로 한 지역만 옮겨요 (한 번에 하나씩, 점령하는 움직임과 함께)
fn step() {
    let k = SHOWN.get();
    let t = target();
    if t == k || now() - STEP_AT.get() < 900.0 {
        return;
    }
    STEP_AT.set(now());
    let (i, to_b) = if t > k { (k, true) } else { (k - 1, false) };
    SHOWN.set(if to_b { k + 1 } else { k - 1 });
    let name = REGIONS.with(|r| r.borrow().get(i).map(|g| g.name.clone()).unwrap_or_default());
    if let Some(p) = svg_el(&format!("gm-p{i}")) {
        let cl = p.class_list();
        let _ = cl.remove_2("r", "b");
        let _ = cl.add_1(if to_b { "b" } else { "r" });
        dom::anim(&p, r#"[{"transform":"scale(1)","filter":"brightness(1)"},{"transform":"scale(1.06)","filter":"brightness(1.45)","offset":0.35},{"transform":"scale(1)","filter":"brightness(1)"}]"#, r#"{"duration":700,"easing":"cubic-bezier(.2,.8,.2,1)","fill":"none"}"#);
    }
    blast(i, to_b);
    camera();
    let news = format!("{}이 {}{} 점령했어요", if to_b { "독일" } else { "소련" }, name, obj_josa(&name));
    dom::set_text("gm-news", &news);
    if let Some(n) = by_id("gm-news") {
        let _ = n.class_list().remove_1("pop");
        let _ = n.offset_width();
        let _ = n.class_list().add_1("pop");
    }
    paint_labels(scores().0, scores().1);
}

/// 점령한 땅 가운데에서 팀 색 고리가 두 번 퍼져요 (깃발을 꽂는 느낌)
fn blast(i: usize, to_b: bool) {
    if dom::reduce_motion() {
        return;
    }
    let Some(fx) = svg_el("gm-fx") else { return };
    let Some((cx, cy)) = REGIONS.with(|r| r.borrow().get(i).map(|g| (g.cx, g.cy))) else { return };
    let html: String = (0..2)
        .map(|n| format!(r#"<circle class="gm-ring {}" cx="{cx:.0}" cy="{cy:.0}" r="34" style="animation-delay:{}ms"/>"#, if to_b { "b" } else { "r" }, n * 180))
        .collect();
    fx.set_inner_html(&html);
    leptos::prelude::set_timeout(move || fx.set_inner_html(""), std::time::Duration::from_millis(1300));
}

/// 카메라: 전선 양쪽 두 지역씩이 화면에 꽉 차게 지도를 키우고 옮겨요.
/// SVG 안을 다시 그리지 않고 SVG 통째로 CSS transform만 바꿔서(합성 단계에서 처리) 가벼워요
fn camera() {
    let Some(el) = svg_el("gm-svg") else { return };
    let k = SHOWN.get();
    let (s, tx, ty) = if FULL.get() {
        (1.0, 0.0, 0.0)
    } else {
        let Some((x0, y0, x1, y1)) = REGIONS.with(|r| {
            let r = r.borrow();
            if r.len() < 2 {
                return None;
            }
            let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for g in &r[k.saturating_sub(2)..(k + 2).min(r.len())] {
                let h = g.area.sqrt() * 0.8;
                x0 = x0.min(g.cx - h);
                x1 = x1.max(g.cx + h);
                y0 = y0.min(g.cy - h);
                y1 = y1.max(g.cy + h);
            }
            Some((x0, y0, x1, y1))
        }) else {
            return;
        };
        let (w, h) = ((x1 - x0).max(300.0) + 60.0, (y1 - y0).max(234.0) + 60.0);
        let s = (1000.0 / w).min(780.0 / h).clamp(1.0, 3.2);
        // 보이는 판(1000x780, 위 24 잘림)에서의 비율 자리
        let (fx, fy) = (((x0 + x1) / 2.0) / 1000.0, ((y0 + y1) / 2.0 - 24.0) / 780.0);
        ((s * 100.0).round() / 100.0, (0.5 - fx * s).clamp(1.0 - s, 0.0), (0.5 - fy * s).clamp(1.0 - s, 0.0))
    };
    let t = format!("translate({:.2}%,{:.2}%) scale({s})", tx * 100.0, ty * 100.0);
    if CAM.with(|c| *c.borrow() == t) {
        return;
    }
    CAM.with(|c| *c.borrow_mut() = t.clone());
    let _ = el.set_attribute("style", &format!("transform:{t};--s:{s}"));
    paint_labels(scores().0, scores().1);
}

/// 받침이 있으면 "을", 없으면 "를"
fn obj_josa(w: &str) -> &'static str {
    match w.chars().last() {
        Some(c) if ('가'..='힣').contains(&c) && (c as u32 - 0xAC00) % 28 != 0 => "을",
        _ => "를",
    }
}

/* ---------- 서버 ---------- */

fn watch() {
    if UNSUB.with(|u| u.borrow().is_some()) || !cloud::signed_in() {
        return;
    }
    let cb = Closure::<dyn FnMut(JsValue)>::new(|v: JsValue| {
        SR.set(cloud::get(&v, "r").as_f64().unwrap_or(0.0));
        SB.set(cloud::get(&v, "b").as_f64().unwrap_or(0.0));
        if !GOT.replace(true) {
            let k = target();
            SHOWN.set(k);
            for i in 0..count() {
                if let Some(p) = svg_el(&format!("gm-p{i}")) {
                    let _ = p.set_attribute("class", &format!("gm-reg {}", if i < k { "b" } else { "r" }));
                }
            }
            camera();
        }
        paint_score();
        run();
    })
    .into_js_value();
    if let Ok(u) = cloud::call_now("gameWatch", &[cb]) {
        UNSUB.with(|x| *x.borrow_mut() = u.dyn_into().ok());
    }
}
fn unwatch() {
    if let Some(f) = UNSUB.with(|u| u.borrow_mut().take()) {
        let _ = f.call0(&JsValue::NULL);
    }
}

fn flush() {
    let t = team();
    let p = PENDING.get();
    if t.is_empty() || p <= 0.0 || FLYING.get() || now() - LAST_FLUSH.get() < FLUSH_MS {
        return;
    }
    let k = p.min(MAX_FLUSH);
    FLYING.set(true);
    PENDING.set(p - k);
    LAST_FLUSH.set(now());
    spawn_local(async move {
        let r = cloud::call("gameFlush", &[JsValue::from_str(&t), k.into()]).await;
        FLYING.set(false);
        match r {
            Ok(_) => MY_SENT.set(MY_SENT.get() + k),
            Err(_) => {
                // 못 보낸 클릭은 다음에 다시 보내요
                PENDING.set(PENDING.get() + k);
                LAST_FLUSH.set(now());
            }
        }
        paint_score();
    });
}

async fn load_rank() {
    RANK_AT.set(now());
    let Ok(list) = cloud::call("gameTop", &[]).await else { return };
    let v: Vec<Row> = js_sys::Array::from(&list)
        .iter()
        .map(|it| {
            let s = |k: &str| cloud::get(&it, k).as_string().unwrap_or_default();
            Row { uid: s("uid"), team: s("team"), name: s("name"), clicks: cloud::get(&it, "clicks").as_f64().unwrap_or(0.0) }
        })
        .collect();
    RANK.with(|r| *r.borrow_mut() = v);
    paint_rank();
}
fn paint_rank() {
    let Some(el) = by_id("gm-rank") else { return };
    let me = my_uid();
    let rows = RANK.with(|r| r.borrow().clone());
    let col = |t: &str| -> String {
        let list: String = rows
            .iter()
            .filter(|x| x.team == t)
            .take(5)
            .enumerate()
            .map(|(i, x)| format!(r#"<li class="{}"><em>{}</em><span>{}</span><b>{}</b></li>"#, if x.uid == me { "me" } else { "" }, i + 1, esc(&x.name), fmt(x.clicks)))
            .collect();
        let body = if list.is_empty() { r#"<li class="none"><span>아직 없어요</span></li>"#.to_string() } else { list };
        format!(r#"<div class="gm-col {t}"><h3><i></i>{}</h3><ol>{body}</ol></div>"#, side_name(t))
    };
    dom::set_html_if(&el, &format!("{}{}", col("r"), col("b")));
    let t = team();
    let mine = if t.is_empty() {
        String::new()
    } else {
        let all: Vec<&Row> = rows.iter().filter(|x| x.team == t).collect();
        match all.iter().position(|x| x.uid == me) {
            Some(i) => format!("{}팀에서 {}등이에요", side_name(&t), i + 1),
            None => format!("{}팀 순위 밖이에요. 더 눌러서 올라가 보세요", side_name(&t)),
        }
    };
    dom::set_text("gm-mine", &mine);
}

async fn load_me() {
    if ME_LOADED.get() {
        return;
    }
    if let Ok(v) = cloud::call("gameMe", &[]).await {
        if !v.is_null() && !v.is_undefined() {
            TEAM.with(|x| *x.borrow_mut() = cloud::get(&v, "team").as_string().unwrap_or_default());
            MY_SENT.set(cloud::get(&v, "clicks").as_f64().unwrap_or(0.0));
            // 서버는 마지막으로 보낸 때부터 4초를 세요. 열자마자 보내지 않게 이때부터 기다려요
            LAST_FLUSH.set(now());
        }
        ME_LOADED.set(true);
    }
    paint_play();
}

/* ---------- 도는 시계 (게임 탭을 보는 동안, 또는 보낼 클릭이 남은 동안만) ---------- */

fn tick() {
    let active = dom::screen_active("game") && !dom::doc_hidden();
    if active {
        watch();
        step();
        if now() - RANK_AT.get() > 30_000.0 {
            spawn_local(load_rank());
        }
    } else {
        unwatch();
    }
    flush();
    if !active && PENDING.get() <= 0.0 && !FLYING.get() {
        stop();
    }
}
fn run() {
    if TIMER.get() != 0 {
        return;
    }
    let cb = Closure::<dyn FnMut()>::new(tick).into_js_value();
    if let Some(w) = web_sys::window() {
        TIMER.set(w.set_interval_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 300).unwrap_or(0));
    }
}
fn stop() {
    if let Some(w) = web_sys::window() {
        w.clear_interval_with_handle(TIMER.get());
    }
    TIMER.set(0);
}

/// 게임 탭을 열 때
pub fn open() {
    build();
    if !cloud::signed_in() {
        return paint_play();
    }
    watch();
    run();
    spawn_local(async {
        load_me().await;
        if now() - RANK_AT.get() > 30_000.0 {
            load_rank().await;
        }
    });
}
