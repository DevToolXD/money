//! 도박장 (슬롯 3칸): 메시지의 멸공 선물 카드를 10초 동안 꾹 누르면 열려요.
//!   칸마다 별 그림이 나올 확률은 32%예요. 별이 2칸이면 건 돈의 1.5배, 3칸이면 10배, 그 밖(별 0~1칸)은 0배를 돌려받아요.
//!   건 돈은 먼저 빠지고 배율만큼 돌아와요. 지갑에는 (돌려받는 돈 - 건 돈)만큼 한 번에 바뀌어요.
//!   결과는 이 기기에서 정해요. 서버(firestore.rules의 'bet')는 건 돈과 배율의 모양만 확인해요.
//! 칸은 그림 다섯 개를 이은 띠를 CSS 애니메이션(WAAPI)으로 돌려요. 2초마다 그리는 곳이 아니라서 매 프레임 DOM을 건드리지 않아요.

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{Element, HtmlElement};

use crate::dom::{self, by_id};

/// 칸마다 별이 나올 확률
pub const HIT: f64 = 0.32;
/// 건 돈 (칩). 모두 100의 배수라서 1.5배도 딱 떨어져요
const BETS: [f64; 4] = [100.0, 1_000.0, 10_000.0, 100_000.0];
/// 그림 번호 0은 별 (맞으면 세는 그림), 1~4는 다른 그림
const STAR: usize = 0;
const NSYM: usize = 5;
/// 띠 한 줄에 든 그림 칸 수. 띠 높이는 칸 한 개의 30배예요
const STRIP: usize = 30;
/// 그림 한 칸의 높이 (띠 높이 기준 %)
const STEP: f64 = 100.0 / STRIP as f64;
/// 그림 5칸(한 바퀴)이 지나가는 시간
const SPIN_MS: f64 = 420.0;
/// 손으로 10초를 채우는 시간
const HOLD_MS: i32 = 10_000;

/// 선 아이콘 안쪽 그림 (viewBox 24, 선 굵기는 CSS에서 1.8)
const SYMS: [&str; NSYM] = [
    // 별 (맞으면 세는 그림)
    r#"<path d="M12 3.6l2.5 5.2 5.7.8-4.1 4 1 5.6-5.1-2.7-5.1 2.7 1-5.6-4.1-4 5.7-.8z"/>"#,
    // 동전
    r#"<circle cx="12" cy="12" r="8.5"/><circle cx="12" cy="12" r="4.2"/>"#,
    // 가방
    r#"<path d="M5 8.5h14l-1 11.5H6zM9 8.5V7a3 3 0 0 1 6 0v1.5"/>"#,
    // 종
    r#"<path d="M6.5 16.5V11a5.5 5.5 0 0 1 11 0v5.5l1.5 2h-14z"/><path d="M10 20.5a2 2 0 0 0 4 0"/>"#,
    // 다이아몬드
    r#"<path d="M7.5 4.5h9L20 9l-8 11-8-11z"/><path d="M4 9h16"/>"#,
];

thread_local! {
    static BET: Cell<usize> = const { Cell::new(0) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
    static BOUND: Cell<bool> = const { Cell::new(false) };
    /// 칸마다 쉬고 있는 띠 위치 (그림 칸 번호). 다음 돌리기는 여기서 이어가요
    static REST: RefCell<[usize; 3]> = const { RefCell::new([1, 2, 3]) };
    /// 이번 돌리기를 시작한 띠 위치 (띠 높이 %)
    static BASE: RefCell<[f64; 3]> = const { RefCell::new([0.0; 3]) };
    /// 칸마다 지금 움직이는 애니메이션 (돌리는 중이거나 멈추는 중)
    static ANIM: RefCell<[Option<JsValue>; 3]> = const { RefCell::new([None, None, None]) };
    static SEQ: Cell<u32> = const { Cell::new(0) };
    /// 꾹 누르기 번호 (0이면 누르는 중이 아니에요)
    static HOLD: Cell<u32> = const { Cell::new(0) };
    static START: Cell<(f64, f64)> = const { Cell::new((0.0, 0.0)) };
    static BAR: RefCell<Option<Element>> = const { RefCell::new(None) };
    /// 10초를 채운 뒤 떼었을 때 따라 나오는 클릭(받기 등)을 막는 시각
    static SWALLOW_UNTIL: Cell<f64> = const { Cell::new(0.0) };
}

/* ---------- 순수한 계산 (시험할 수 있게 난수를 밖에서 받아요) ---------- */

/// 칸 세 개의 그림 번호를 정해요. rnd는 0 이상 1 미만의 난수
pub fn roll(rnd: &mut impl FnMut() -> f64) -> [usize; 3] {
    let mut out = [0usize; 3];
    for o in out.iter_mut() {
        *o = if rnd() < HIT { STAR } else { 1 + ((rnd() * 4.0) as usize).min(3) };
    }
    out
}
/// 별이 나온 칸 수
pub fn hits(sym: &[usize; 3]) -> usize {
    sym.iter().filter(|s| **s == STAR).count()
}
/// 돌려받는 배율: 별 3칸은 10배, 2칸은 1.5배, 그 밖은 0배
pub fn mult(h: usize) -> f64 {
    match h {
        3 => 10.0,
        2 => 1.5,
        _ => 0.0,
    }
}
/// 지갑이 바뀌는 양: 건 돈을 내고 배율만큼 돌려받아요 (잃으면 -건 돈, 1.5배면 +절반, 10배면 +9배)
pub fn net(bet: f64, h: usize) -> f64 {
    bet * mult(h) - bet
}

/* ---------- 그림과 화면 ---------- */

fn svg(inner: &str, cls: &str) -> String {
    format!(r#"<svg class="{cls}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{inner}</svg>"#)
}
fn fmt(v: f64) -> String {
    cloud::fmt_num(v, 0)
}
fn root() -> Option<HtmlElement> {
    by_id("casino-root")
}

fn page() -> String {
    let strip: String = (0..STRIP).map(|i| format!(r#"<span class="cz-cell">{}</span>"#, svg(SYMS[i % NSYM], "cz-ic"))).collect();
    let reels: String = (0..3).map(|r| format!(r#"<div class="cz-reel" data-r="{r}"><div class="cz-strip">{strip}</div></div>"#)).collect();
    let chips: String = BETS.iter().enumerate().map(|(i, b)| format!(r#"<button type="button" class="cz-chip" data-bet="{i}" aria-pressed="false">{}</button>"#, fmt(*b))).collect();
    let stars = |n: usize| -> String { (0..n).map(|_| svg(SYMS[STAR], "cz-ic sm")).collect() };
    format!(
        r#"<h1 class="cz-title" id="title-casino">도박장</h1>
<p class="cz-bal">내 멸공 <b id="cz-bal"></b></p>
<div class="cz-card">
<div class="cz-machine" role="img" aria-label="슬롯 세 칸">{reels}</div>
<p class="cz-result" id="cz-result" aria-live="polite">건 멸공을 고르고 돌려 보세요</p>
</div>
<h2 class="section-title">멸공 걸기</h2>
<div class="cz-chips" role="group" aria-label="건 멸공">{chips}</div>
<button class="cz-spin" id="cz-spin" type="button">100 멸공 걸고 돌리기</button>
<h2 class="section-title">배율</h2>
<div class="card cz-rules"><ul>
<li><span class="cz-sym">{s3}</span><span>3칸 모두 맞으면</span><b>10배</b></li>
<li><span class="cz-sym">{s2}</span><span>2칸 맞으면</span><b>1.5배</b></li>
<li><span class="cz-sym"></span><span>그 밖의 경우</span><b>0배</b></li>
</ul><p class="cz-note">칸마다 별 그림이 나올 확률은 32%예요. 건 돈은 먼저 빠지고, 배율만큼 돌아와요.</p></div>"#,
        s3 = stars(3),
        s2 = stars(2),
    )
}

fn paint_bal() {
    dom::set_text("cz-bal", &crate::money::w(crate::money::balance().unwrap_or(0.0)));
}
fn paint_chips() {
    let Some(r) = root() else { return };
    let i = BET.get();
    for (k, b) in dom::qsa(&r, "[data-bet]").into_iter().enumerate() {
        dom::toggle(&b, "on", k == i);
        let _ = b.set_attribute("aria-pressed", if k == i { "true" } else { "false" });
    }
    dom::set_text("cz-spin", &format!("{} 멸공 걸고 돌리기", fmt(BETS[i])));
}
fn set_result(text: &str, tone: &str) {
    dom::set_text("cz-result", text);
    if let Some(el) = by_id("cz-result") {
        el.set_class_name(&format!("cz-result {tone}").trim_end().to_string());
    }
}
fn set_busy(on: bool) {
    BUSY.set(on);
    if let Some(b) = by_id("cz-spin") {
        if on {
            let _ = b.set_attribute("disabled", "");
        } else {
            let _ = b.remove_attribute("disabled");
        }
    }
}

/* ---------- 칸 돌리기 (띠 위치는 % 로 정해요. 띠는 그림 30칸이라서 한 바퀴를 넘어 가도 빈 데가 안 보여요) ---------- */

fn strip_el(r: usize) -> Option<HtmlElement> {
    dom::doc().query_selector(&format!(r#".cz-reel[data-r="{r}"] .cz-strip"#)).ok().flatten()?.dyn_into().ok()
}
fn reel_el(r: usize) -> Option<Element> {
    dom::doc().query_selector(&format!(r#".cz-reel[data-r="{r}"]"#)).ok().flatten()
}
fn pct(v: f64) -> String {
    format!("translateY({v:.4}%)")
}
fn frames(a: f64, b: f64) -> String {
    format!(r#"[{{"transform":"{}"}},{{"transform":"{}"}}]"#, pct(a), pct(b))
}
fn cancel_anim(r: usize) {
    if let Some(a) = ANIM.with(|x| x.borrow_mut()[r].take()) {
        dom::cancel(&a);
    }
}
fn cancel_all() {
    for r in 0..3 {
        cancel_anim(r);
    }
}
/// 애니메이션이 지금 몇 밀리초째인지 (currentTime)
fn anim_time(a: &JsValue) -> f64 {
    js_sys::Reflect::get(a, &JsValue::from_str("currentTime")).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

/// 돌리기 시작: 쉬고 있던 자리를 같은 그림끼리 옮겨 두고(그림은 그대로) 세 칸을 계속 돌려요
fn start_spin() {
    REST.with(|rs| {
        let mut rs = rs.borrow_mut();
        for k in rs.iter_mut() {
            while *k >= 10 {
                *k -= 5;
            }
        }
    });
    for r in 0..3 {
        cancel_anim(r);
        if let Some(el) = reel_el(r) {
            let _ = el.class_list().remove_1("hit");
        }
        let Some(st) = strip_el(r) else { continue };
        let k = REST.with(|rs| rs.borrow()[r]);
        let base = -(STEP * k as f64);
        BASE.with(|b| b.borrow_mut()[r] = base);
        let _ = st.style().set_property("transform", &pct(base));
        if dom::reduce_motion() {
            continue;
        }
        let opts = format!(r#"{{"duration":{SPIN_MS},"iterations":100000,"easing":"linear"}}"#);
        let a = dom::anim_always(&st, &frames(base, base - STEP * NSYM as f64), &opts);
        ANIM.with(|x| x.borrow_mut()[r] = a);
    }
}

/// 칸 하나를 그림 s에 멈춰요. 지금 돌고 있던 자리에서 감속하며 8칸 이상 가서 멈춰요
fn stop_reel(r: usize, s: usize) {
    let Some(st) = strip_el(r) else { return };
    let spinning = ANIM.with(|x| x.borrow()[r].clone());
    let base = BASE.with(|b| b.borrow()[r]);
    let from = match &spinning {
        Some(a) => base - STEP * NSYM as f64 * ((anim_time(a) % SPIN_MS) / SPIN_MS),
        None => base,
    };
    cancel_anim(r);
    let mut k = REST.with(|rs| rs.borrow()[r]) + 8;
    while k % NSYM != s {
        k += 1;
    }
    REST.with(|rs| rs.borrow_mut()[r] = k);
    let to = -(STEP * k as f64);
    if spinning.is_none() || dom::reduce_motion() {
        let _ = st.style().set_property("transform", &pct(to));
        return;
    }
    let opts = r#"{"duration":900,"easing":"cubic-bezier(.3,.45,.25,1)"}"#;
    let a = dom::anim_always(&st, &frames(from, to), opts);
    ANIM.with(|x| x.borrow_mut()[r] = a);
}

/// 돌리기가 실패하면 쉬던 자리로 되돌려요 (결과는 없어요)
fn snap_back() {
    cancel_all();
    apply_rest();
}
/// 칸을 쉬는 자리로 옮겨요 (열 때와 실패했을 때)
fn apply_rest() {
    for r in 0..3 {
        if let Some(st) = strip_el(r) {
            let k = REST.with(|rs| rs.borrow()[r]);
            let _ = st.style().set_property("transform", &pct(-(STEP * k as f64)));
        }
    }
}

fn mark_hits(sym: &[usize; 3]) {
    for (r, s) in sym.iter().enumerate() {
        if let Some(el) = reel_el(r) {
            dom::toggle(&el, "hit", *s == STAR);
        }
    }
}

/// 맞으면 칸 위에 잠깐 글자가 떠요: 3칸이면 "잭팟", 2칸이면 받은 멸공
fn celebrate(h: usize, delta: f64) {
    if h < 2 {
        return;
    }
    let Some(card) = dom::doc().query_selector(".cz-card").ok().flatten() else { return };
    let Ok(el) = dom::doc().create_element("div") else { return };
    if h == 3 {
        el.set_class_name("cz-jackpot");
        el.set_text_content(Some("잭팟"));
    } else {
        el.set_class_name("cz-pop");
        el.set_text_content(Some(&format!("+{}", fmt(delta))));
    }
    let _ = el.set_attribute("aria-hidden", "true");
    let _ = card.append_child(&el);
    spawn_local(async move {
        dom::sleep(2500).await;
        el.remove();
    });
}

fn result(bet: f64, h: usize, delta: f64) -> (String, &'static str) {
    match h {
        3 => (format!("3칸 모두 맞았어요. 10배예요. 지갑에 +{} 멸공", fmt(delta)), "win"),
        2 => (format!("2칸 맞았어요. 1.5배예요. 지갑에 +{} 멸공", fmt(delta)), "win"),
        1 => (format!("1칸만 맞았어요. 0배예요. 지갑에서 -{} 멸공", fmt(bet)), ""),
        _ => (format!("하나도 안 맞았어요. 0배예요. 지갑에서 -{} 멸공", fmt(bet)), ""),
    }
}

async fn spin() {
    if BUSY.get() {
        return;
    }
    set_busy(true);
    let bet = BETS[BET.get().min(BETS.len() - 1)];
    if let Err(m) = crate::money::ensure(bet).await {
        set_busy(false);
        crate::shell::ask("돌릴 수 없어요", &m, "확인", "").await;
        return;
    }
    let sym = roll(&mut || js_sys::Math::random());
    let h = hits(&sym);
    let delta = net(bet, h);
    set_result("돌리는 중이에요", "");
    start_spin();
    // 지갑 쓰기는 기존 줄(2.2초 간격)에 서요. 돌아가는 동안 기다려요
    let f = Closure::once_into_js(move || cloud::call_now("casinoBet", &[bet.into(), delta.into()]).unwrap_or(JsValue::UNDEFINED));
    let r = cloud::settle(crate::money::wallet_op(f.unchecked_into()).into()).await;
    match r {
        Ok(_) => {
            for (r, s) in sym.iter().enumerate() {
                if r > 0 {
                    dom::sleep(320).await;
                }
                stop_reel(r, *s);
            }
            dom::sleep(900).await;
            mark_hits(&sym);
            celebrate(h, delta);
            let (text, tone) = result(bet, h, delta);
            set_result(&text, tone);
            dom::vibrate(if h == 3 { 40 } else { 12 });
            crate::money::log(&dom::obj(&[
                ("t", js_sys::Date::now().into()),
                ("name", JsValue::from_str(&format!("도박장 슬롯 {h}칸 맞춤"))),
                ("cat", "도박".into()),
                ("icon", "ticket".into()),
                ("amt", delta.into()),
            ]));
            dom::sleep(300).await;
            paint_bal();
        }
        Err(e) => {
            snap_back();
            set_result("돌리지 못했어요. 잠시 뒤 다시 해 주세요", "");
            crate::shell::toast(&format!("돌리지 못했어요. {}", cloud::why(&e.code)));
        }
    }
    set_busy(false);
}

/* ---------- 열기·닫기 ---------- */

/// 도박장을 열어요. 돌리는 중이면 지금 칸을 그대로 두고 보여만 줘요
pub fn open() {
    bind();
    let Some(c) = by_id("casino") else { return };
    if !BUSY.get() {
        cancel_all();
        REST.with(|rs| *rs.borrow_mut() = [1, 2, 3]);
        if let Some(r) = root() {
            r.set_inner_html(&page());
        }
        apply_rest();
        paint_bal();
        paint_chips();
    }
    let _ = c.class_list().add_1("is-open");
}
fn close() {
    if let Some(c) = by_id("casino") {
        let _ = c.class_list().remove_1("is-open");
    }
}
fn bind() {
    if BOUND.replace(true) {
        return;
    }
    let Some(c) = by_id("casino") else { return };
    dom::on(&c, "click", |e| {
        if dom::closest(&e, "#casino-close").is_some() {
            return close();
        }
        if BUSY.get() {
            return;
        }
        if let Some(b) = dom::closest(&e, "[data-bet]") {
            BET.set(dom::data(&b, "bet").parse::<usize>().unwrap_or(0).min(BETS.len() - 1));
            dom::vibrate(5);
            return paint_chips();
        }
        if dom::closest(&e, "#cz-spin").is_some() {
            spawn_local(spin());
        }
    });
}

/* ---------- 메시지의 멸공 선물 카드를 10초 꾹 누르기 ---------- */

/// 메시지 목록(#bubbles)에 꾹 누르기를 붙여요. 카드 밖으로 끌거나 떼면 처음부터예요
pub fn bind_hold() {
    let Some(b) = by_id("bubbles") else { return };
    dom::on(&b, "pointerdown", |e| {
        let Some(card) = dom::closest(&e, ".cash[data-gift]") else { return };
        if let Some(pe) = e.dyn_ref::<web_sys::PointerEvent>() {
            if pe.button() != 0 {
                return;
            }
            START.set((pe.client_x() as f64, pe.client_y() as f64));
        }
        hold_start(&card);
    });
    dom::on(&b, "contextmenu", |e| {
        if dom::closest(&e, ".cash[data-gift]").is_some() {
            e.prevent_default();
        }
    });
    let doc: web_sys::EventTarget = dom::doc().unchecked_into();
    dom::on(&doc, "pointerup", |_| hold_end());
    dom::on(&doc, "pointercancel", |_| hold_end());
    dom::on(&doc, "pointermove", |e| {
        if HOLD.get() == 0 {
            return;
        }
        let Some(pe) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
        let (x, y) = START.get();
        if (pe.client_x() as f64 - x).hypot(pe.client_y() as f64 - y) > 16.0 {
            hold_end();
        }
    });
    // 10초를 채운 뒤 떼면, 그 뒤에 나오는 클릭(받기 같은 버튼)은 한 번 막아요
    let cap = Closure::<dyn FnMut(web_sys::Event)>::new(|e: web_sys::Event| {
        if js_sys::Date::now() < SWALLOW_UNTIL.get() {
            e.stop_propagation();
            e.prevent_default();
        }
    });
    let _ = doc.add_event_listener_with_callback_and_bool("click", cap.as_ref().unchecked_ref(), true);
    cap.forget();
}

fn hold_start(card: &HtmlElement) {
    hold_end();
    let id = SEQ.with(|s| {
        let v = s.get().wrapping_add(1).max(1);
        s.set(v);
        v
    });
    HOLD.set(id);
    // 진행 막대: 카드 아래쪽에 10초 동안 차요 (CSS 애니메이션이라 다시 그리는 것과 상관없이 움직여요)
    let rect = card.get_bounding_client_rect();
    if let Ok(bar) = dom::doc().create_element("div") {
        bar.set_class_name("cz-hold");
        if let Some(h) = bar.dyn_ref::<HtmlElement>() {
            let st = h.style();
            let _ = st.set_property("left", &format!("{}px", rect.left()));
            let _ = st.set_property("width", &format!("{}px", rect.width()));
            let _ = st.set_property("top", &format!("{}px", rect.bottom() - 6.0));
        }
        if let Some(body) = dom::doc().body() {
            let _ = body.append_child(&bar);
        }
        BAR.with(|b| *b.borrow_mut() = Some(bar));
    }
    dom::vibrate(6);
    spawn_local(async move {
        dom::sleep(HOLD_MS).await;
        if HOLD.get() != id {
            return;
        }
        hold_end();
        SWALLOW_UNTIL.set(js_sys::Date::now() + 900.0);
        dom::vibrate(24);
        open();
    });
}

/// 꾹 누르기를 멈춰요 (막대도 지워요)
fn hold_end() {
    HOLD.set(0);
    if let Some(b) = BAR.with(|b| b.borrow_mut().take()) {
        b.remove();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payout_table() {
        assert_eq!(mult(3), 10.0);
        assert_eq!(mult(2), 1.5);
        assert_eq!(mult(1), 0.0);
        assert_eq!(mult(0), 0.0);
        assert_eq!(net(100.0, 3), 900.0);
        assert_eq!(net(100.0, 2), 50.0);
        assert_eq!(net(100.0, 1), -100.0);
        assert_eq!(net(100.0, 0), -100.0);
        for b in BETS {
            assert_eq!((net(b, 2)) * 2.0, b, "1.5배는 딱 떨어져야 해요");
        }
    }

    #[test]
    fn hit_rate_is_32_percent_per_reel() {
        let mut seed: u64 = 12345;
        let mut rnd = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let n = 200_000;
        let (mut reel_hits, mut three, mut two) = (0usize, 0usize, 0usize);
        for _ in 0..n {
            let s = roll(&mut rnd);
            let h = hits(&s);
            reel_hits += h;
            if h == 3 {
                three += 1;
            }
            if h == 2 {
                two += 1;
            }
        }
        let p = reel_hits as f64 / (3.0 * n as f64);
        assert!((p - 0.32).abs() < 0.004, "칸마다 맞을 확률 {p}");
        // 3칸 0.32^3 = 3.28%, 2칸 3*0.32^2*0.68 = 20.9%
        assert!((three as f64 / n as f64 - 0.0328).abs() < 0.003, "3칸 {three}");
        assert!((two as f64 / n as f64 - 0.2089).abs() < 0.005, "2칸 {two}");
    }

    #[test]
    fn fixed_rolls() {
        // 0.0은 별, 0.9는 다른 그림(마지막)
        assert_eq!(roll(&mut || 0.0), [0, 0, 0]);
        assert_eq!(roll(&mut || 0.9), [4, 4, 4]);
    }
}
