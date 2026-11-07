//! 돈 탭 (예전 app.js의 돈·은행 부분): 잔액, 이자 상품과 복리, 지갑 쓰기 줄 세우기, 이자 정산, 거래내역, 쓴 돈 그래프, 카드 순서 바꾸기.
//! 진짜 잔액은 서버 지갑이 지켜요 (firestore.rules). 다른 화면(app.js)은 window.__bank 로 잔액을 바로 읽어요.

mod art;
mod chart;
mod reorder;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use js_sys::{Function, Promise, Reflect};
use leptos::prelude::set_timeout;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{future_to_promise, JsFuture};

use crate::ctx::Ctx;
use crate::dom::{self, by_id, esc};

const YEAR_S: f64 = 365.0 * 24.0 * 3600.0;
const RATES: [u32; 10] = [5, 10, 15, 20, 25, 30, 35, 40, 45, 50];
const WEEK_S: f64 = 7.0 * 86400.0;

/// 값: 연이율이 두 배가 되면 값은 약 3.5배 (firestore.rules의 가격표와 같아야 해요)
pub fn price(r: u32) -> f64 {
    (20.0 * (r as f64 / 5.0).powf(1.8)).round()
}
/// 연 rate% 복리로 secs초 동안 늘어나는 비율
fn growth(rate: f64, secs: f64) -> f64 {
    (1.0 + rate / 100.0).powf(secs / YEAR_S) - 1.0
}
fn ko(v: f64) -> String {
    cloud::fmt_num(v, 3)
}
/// "1,250 토큰"
pub fn w(n: f64) -> String {
    format!("{} 토큰", ko(n))
}
fn now() -> f64 {
    js_sys::Date::now()
}

#[derive(Clone, Copy, Default, PartialEq)]
struct Bank {
    ready: bool,
    rate: f64,
    bal: f64,
    t: f64,
}
impl Bank {
    /// 아직 서버에 정산하지 않은 이자 (화면에서는 바로 더해 보여주고, 서버 정산은 10분에 한 번만 해요)
    fn pending(&self) -> f64 {
        if self.rate > 0.0 && self.ready {
            (self.bal * growth(self.rate, ((now() - self.t) / 1000.0).max(0.0).min(WEEK_S))).floor()
        } else {
            0.0
        }
    }
    fn shown(&self) -> f64 {
        self.bal + self.pending()
    }
    fn next_in(&self) -> f64 {
        let need = (self.pending() + 1.0) / self.bal.max(1.0);
        let secs = (YEAR_S * (1.0 + need).ln()) / (1.0 + self.rate / 100.0).ln();
        (secs - (now() - self.t) / 1000.0).ceil().max(1.0)
    }
}
fn every_text(bal: f64, rate: f64) -> String {
    let per = bal * growth(rate, 1.0);
    if per >= 1.0 {
        format!("1초마다 {} 토큰", ko(per.floor()))
    } else {
        format!("약 {}초마다 1 토큰", ko((1.0 / per.max(1e-9)).ceil()))
    }
}

/* ---------- 거래 기록 (이 기기에서 일어난 이자·상품·선물. 보여주기용이에요) ---------- */

#[derive(Clone, PartialEq)]
pub(crate) struct Tx {
    t: f64,
    name: String,
    cat: String,
    icon: String,
    amt: f64,
}
impl Tx {
    fn from_js(v: &JsValue) -> Option<Tx> {
        let g = |k| cloud::get(v, k);
        Some(Tx { t: g("t").as_f64()?, name: g("name").as_string()?, cat: g("cat").as_string().unwrap_or_default(), icon: g("icon").as_string().unwrap_or_default(), amt: g("amt").as_f64()? })
    }
    fn to_js(&self) -> JsValue {
        dom::obj(&[("t", self.t.into()), ("name", JsValue::from_str(&self.name)), ("cat", JsValue::from_str(&self.cat)), ("icon", JsValue::from_str(&self.icon)), ("amt", self.amt.into())])
    }
}

fn ic(k: &str) -> Option<&'static str> {
    Some(match k {
        "store" => r#"<path d="M4 9l1.500-5h13L20 9M4 9v11h16V9M4 9c0 1.800 1.500 2.800 2.700 2.800S9.300 10.800 9.300 9c0 1.800 1.200 2.800 2.700 2.800s2.700-1 2.700-2.800c0 1.800 1.300 2.800 2.700 2.800S20 10.800 20 9M10 20v-5h4v5"/>"#,
        "cup" => r#"<path d="M5 9h11v5a5 5 0 0 1-5 5h-1a5 5 0 0 1-5-5zM16 10h1.500a2.500 2.500 0 0 1 0 5H16M8 3v3M12 3v3"/>"#,
        "bus" => r#"<rect x="5" y="4" width="14" height="14" rx="3"/><path d="M5 12h14M8 18v2M16 18v2M8.500 15h.01M15.500 15h.01"/>"#,
        "card" => r#"<rect x="3" y="6" width="18" height="12" rx="3"/><path d="M3 10h18M7 15h3"/>"#,
        "income" => r#"<circle cx="12" cy="12" r="8.500"/><path d="M12 16V8M8.500 11.500L12 8l3.500 3.500"/>"#,
        "gift" => r#"<rect x="4" y="9" width="16" height="11" rx="2"/><path d="M3 9h18v3H3zM12 9v11M12 9C10 9 8 8 8 6.500S10.500 4.500 12 9zM12 9c2 0 4-1 4-2.500S13.500 4.500 12 9z"/>"#,
        "book" => r#"<path d="M5 4.500h10a3 3 0 0 1 3 3V20H8a3 3 0 0 1-3-3zM8 17h10"/>"#,
        "ticket" => r#"<path d="M4 8a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v2a2 2 0 0 0 0 4v2a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-2a2 2 0 0 0 0-4zM14 6v12"/>"#,
        "bag" => r#"<path d="M5 8h14l-1 12H6zM9 8V6.500a3 3 0 0 1 6 0V8"/>"#,
        "percent" => r#"<circle cx="12" cy="12" r="8.500"/><path d="M15.500 8.500l-7 7M9 9.500h.01M15 14.500h.01"/>"#,
        _ => return None,
    })
}
fn ico(k: &str) -> String {
    format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">{}</svg>"#, ic(k).unwrap_or(""))
}
fn cat_color(c: &str) -> &'static str {
    match c {
        "식비" => "#ffe6d2",
        "카페" => "#f0e4d6",
        "교통" => "#dbe8ff",
        "문화" => "#ece1ff",
        "쇼핑" => "#ffdbe6",
        "입금" | "송금" => "#ffe0e0",
        "은행" => "#d6f1e6",
        _ => "#eceff3",
    }
}
const CHEV2: &str = r#"<svg class="tchev" viewBox="0 0 8 14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M1 1l6 6-6 6"/></svg>"#;
const CHEV_GR: &str = r#"<svg class="tchev gr" viewBox="0 0 8 14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M1 1l6 6-6 6"/></svg>"#;
const COIN: &str = r#"<svg class="coin" viewBox="0 0 36 32" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><ellipse cx="12" cy="6.500" rx="9" ry="3.200"/><path d="M3 6.500v5c0 1.800 4 3.200 9 3.200M3 11.500v5c0 1.800 4 3.200 9 3.200M3 16.500v5c0 1.800 4 3.200 9 3.200"/><circle cx="23.500" cy="19.500" r="10.500" fill="var(--bg-card)"/><path d="M23 13.500v8.500c0 1.200.7 1.900 1.900 1.900h.9M19.800 17h6.400"/></svg>"#;

/// 그날 0시 (이 기기 시간)
fn start_of_day(t: f64) -> f64 {
    let d = js_sys::Date::new(&JsValue::from_f64(t));
    js_sys::Date::new_with_year_month_day(d.get_full_year(), d.get_month() as i32, d.get_date() as i32).get_time()
}
fn day_name(t: f64) -> String {
    let d0 = start_of_day(t);
    let diff = ((start_of_day(now()) - d0) / 864e5).round();
    if diff == 0.0 {
        "오늘".into()
    } else if diff == 1.0 {
        "어제".into()
    } else {
        let d = js_sys::Date::new(&JsValue::from_f64(d0));
        format!("{}월 {}일", d.get_month() + 1, d.get_date())
    }
}
fn hm(t: f64) -> String {
    let d = js_sys::Date::new(&JsValue::from_f64(t));
    format!("{:02}:{:02}", d.get_hours(), d.get_minutes())
}
fn amt_html(n: f64) -> String {
    format!(r#"<b class="t-amt"><i class="{}">{}{}</i> 토큰</b>"#, if n > 0.0 { "pos" } else { "neg" }, if n > 0.0 { "+" } else { "-" }, ko(n.abs()))
}
fn tx_row(e: &Tx) -> String {
    format!(
        r#"<li class="t-row tx"><span class="t-ico" style="--c:{}">{}</span><span class="t-main"><b class="ts-name">{}</b><span class="t-label">{}</span><span class="t-label">{} {}</span></span>{}{CHEV_GR}</li>"#,
        cat_color(&e.cat),
        ico(&e.icon),
        esc(&e.name),
        esc(&e.cat),
        day_name(e.t),
        hm(e.t),
        amt_html(e.amt)
    )
}
fn groups(list: &[Tx]) -> String {
    if list.is_empty() {
        r#"<p class="t-note">아직 거래내역이 없어요</p>"#.into()
    } else {
        format!(r#"<ul class="t-rows">{}</ul>"#, list.iter().map(tx_row).collect::<String>())
    }
}

/* ---------- 상태 ---------- */

thread_local! {
    static CTX: RefCell<Ctx> = RefCell::new(Ctx::default());
    static BANK: Cell<Bank> = Cell::new(Bank::default());
    static TXS: RefCell<Vec<Tx>> = const { RefCell::new(Vec::new()) };
    static TX_VER: Cell<u32> = const { Cell::new(0) };
    static SHOWN: RefCell<HashMap<&'static str, String>> = RefCell::new(HashMap::new());
    static RENDERED_TX: Cell<i64> = const { Cell::new(-1) };
    static RENDERED_HIST: Cell<i64> = const { Cell::new(-1) };
    static PRODUCT_SIG: RefCell<String> = const { RefCell::new(String::new()) };
    static CHAIN: RefCell<Option<Promise>> = const { RefCell::new(None) };
    static LAST_WRITE: Cell<f64> = const { Cell::new(0.0) };
    static CLAIMING: Cell<bool> = const { Cell::new(false) };
    static BUYING: Cell<bool> = const { Cell::new(false) };
    static FIRST_WALLET: Cell<bool> = const { Cell::new(false) };
    static RANK_SEQ: Cell<u32> = const { Cell::new(0) };
}
fn ctx() -> Ctx {
    CTX.with(|c| c.borrow().clone())
}
fn bank() -> Bank {
    BANK.get()
}
/// 지갑 숫자를 바꾸고 window.__bank 에도 비춰요 (app.js의 다른 화면이 바로 읽어요)
fn set_bank(b: Bank) {
    BANK.set(b);
    let o = dom::obj(&[("ready", b.ready.into()), ("rate", b.rate.into()), ("bal", b.bal.into()), ("t", b.t.into())]);
    let _ = Reflect::set(&js_sys::global(), &JsValue::from_str("__bank"), &o);
}
fn txs_top(n: usize) -> Vec<Tx> {
    TXS.with(|t| t.borrow().iter().take(n).cloned().collect())
}
pub(crate) fn all_tx() -> Vec<Tx> {
    TXS.with(|t| t.borrow().clone())
}
fn load_txs() {
    let raw = dom::store_get("bank-tx", "[]");
    let list: Vec<Tx> = js_sys::JSON::parse(&raw)
        .ok()
        .filter(|a| a.is_array())
        .map(|a| js_sys::Array::from(&a).iter().filter_map(|v| Tx::from_js(&v)).filter(|e| ic(&e.icon).is_some()).take(200).collect())
        .unwrap_or_default();
    TXS.with(|t| *t.borrow_mut() = list);
}
/// 거래 한 줄을 맨 앞에 남겨요 (이 기기에 200개까지)
pub fn log(e: &JsValue) {
    let Some(e) = Tx::from_js(e) else { return };
    TXS.with(|t| {
        let mut t = t.borrow_mut();
        t.insert(0, e);
        t.truncate(200);
        let arr = js_sys::Array::new();
        for x in t.iter() {
            arr.push(&x.to_js());
        }
        if let Ok(s) = js_sys::JSON::stringify(&arr) {
            dom::store_set("bank-tx", &String::from(s));
        }
    });
    TX_VER.set(TX_VER.get() + 1);
    render();
}
fn log_tx(name: &str, cat: &str, icon: &str, amt: f64) {
    log(&Tx { t: now(), name: name.into(), cat: cat.into(), icon: icon.into(), amt }.to_js());
}

/* ---------- 지갑 쓰기 줄 세우기: 2.2초 간격 (서버 규칙이 2초보다 빠른 연속 쓰기를 막아요) ---------- */

/// f는 약속을 돌려주는 JS 함수예요. 앞의 일이 끝나고 2.2초가 지난 뒤에 불러요
pub fn wallet_op(f: Function) -> Promise {
    let prev = CHAIN.with(|c| c.borrow_mut().take());
    let run = future_to_promise(async move {
        if let Some(p) = prev {
            let _ = JsFuture::from(p).await;
        }
        let wait = 2200.0 - (now() - LAST_WRITE.get());
        if wait > 0.0 {
            dom::sleep(wait as i32).await;
        }
        let out = match f.call0(&JsValue::NULL) {
            Ok(v) => match v.dyn_into::<Promise>() {
                Ok(p) => JsFuture::from(p).await,
                Err(v) => Ok(v),
            },
            Err(e) => Err(e),
        };
        LAST_WRITE.set(now());
        out
    });
    let r2 = run.clone();
    // 실패해도 줄은 계속 이어져요
    let settled = future_to_promise(async move {
        let _ = JsFuture::from(r2).await;
        Ok(JsValue::UNDEFINED)
    });
    CHAIN.with(|c| *c.borrow_mut() = Some(settled));
    run
}
async fn wallet_run(name: &'static str, args: Vec<JsValue>) -> Result<JsValue, JsValue> {
    let f = Closure::once_into_js(move || cloud::call_now(name, &args).unwrap_or_else(|e| Promise::reject(&dom::obj(&[("code", JsValue::from_str(&e.code)), ("message", JsValue::from_str(&e.message))])).into()));
    JsFuture::from(wallet_op(f.unchecked_into())).await
}

/// 쌓인 이자를 서버에 정산해요. 같은 이자가 나오는 가장 짧은 시간만 쓰고 나머지 시간은 다음으로 넘겨서, 소수점 이자가 사라지지 않아요
pub fn claim() -> Promise {
    future_to_promise(async {
        claim_now().await;
        Ok(JsValue::UNDEFINED)
    })
}
async fn claim_now() {
    let b = bank();
    if !cloud::signed_in() || !b.ready || b.rate <= 0.0 || CLAIMING.get() {
        return;
    }
    let avail = WEEK_S.min(((now() - b.t) / 1000.0).floor() - 15.0); // 15초는 폰과 서버 시계 차이를 위한 여유예요
    if avail < 1.0 {
        return;
    }
    let all = (b.bal * growth(b.rate, avail)).floor();
    if all < 1.0 {
        return;
    }
    let (mut lo, mut hi) = (1.0f64, avail);
    while lo < hi {
        let mid = ((lo + hi) / 2.0).floor();
        if (b.bal * growth(b.rate, mid)).floor() >= all {
            hi = mid;
        } else {
            lo = mid + 1.0;
        }
    }
    let gain = (b.bal * growth(b.rate, lo)).floor();
    let t_new = b.t + lo * 1000.0;
    CLAIMING.set(true);
    match wallet_run("claimInterest", vec![gain.into(), t_new.into()]).await {
        Ok(_) => log_tx("이자", "은행", "percent", gain),
        Err(e) => cloud::warn(&format!("이자를 정산하지 못했어요: {:?}", cloud::get(&e, "code").as_string().unwrap_or_default())),
    }
    CLAIMING.set(false);
}

/* ---------- 순위표: 지갑이 바뀌면 잠시 뒤 내 값도 맞춰요 (서버가 20초에 한 번만 받아요) ---------- */

fn push_rank(delay: u64) {
    let seq = RANK_SEQ.get() + 1;
    RANK_SEQ.set(seq);
    set_timeout(
        move || {
            if RANK_SEQ.get() != seq {
                return;
            }
            let b = bank();
            if !cloud::signed_in() || !b.ready {
                return;
            }
            leptos::task::spawn_local(async move {
                let r = cloud::call("syncRank", &[b.bal.into()]).await.ok().and_then(|v| v.as_string()).unwrap_or_default();
                if r == "soon" {
                    push_rank(22000);
                }
            });
        },
        std::time::Duration::from_millis(delay),
    );
}

/// 서버 지갑을 듣기 시작해요 (로그인한 뒤)
pub fn start_wallet() {
    let cb = Closure::<dyn FnMut(JsValue)>::new(move |wv: JsValue| {
        if wv.is_null() || wv.is_undefined() {
            return;
        }
        let g = |k| cloud::get(&wv, k).as_f64().unwrap_or(0.0);
        let old = bank();
        let first = if old.ready { Some(old.bal) } else { None };
        set_bank(Bank { ready: true, rate: g("rate"), bal: g("bal"), t: g("t") });
        if first != Some(g("bal")) {
            push_rank(if first.is_none() { 6000 } else { 4000 });
        }
        render();
        if dom::has_class("bank-sheet", "is-open") {
            render_products();
        }
        if !FIRST_WALLET.get() {
            FIRST_WALLET.set(true);
            set_timeout(|| leptos::task::spawn_local(claim_now()), std::time::Duration::from_millis(3000));
        }
    });
    let _ = cloud::call_now("watchWallet", &[cb.into_js_value()]);
}

/* ---------- 화면 ---------- */

fn sv2(d: &str) -> String {
    format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">{d}</svg>"#)
}
fn mv(id: &str, title: &str, body: &str, extra: &str) -> String {
    format!(r#"<div class="movable mv" data-id="{id}" role="button" tabindex="0"{extra}><h2 class="ac-h">{title}{CHEV2}</h2><div class="ac-box">{body}</div></div>"#)
}
fn card(id: &str) -> String {
    match id {
        "bank" => mv("bank", "이자 상품", r#"<b class="t-big" id="bank-rate"></b><p class="t-note" id="bank-note" data-volatile></p>"#, r#" id="open-bank""#),
        _ => mv("hist", "최근 거래내역", r#"<div id="tx-top"></div>"#, r#" id="open-history""#),
    }
}

/// 바뀐 것만 화면에 써요 (같은 글을 매초 다시 쓰면 화면 사진 갱신이 계속 일어나요)
fn put(id: &'static str, s: String, html: bool) {
    let same = SHOWN.with(|m| m.borrow().get(id) == Some(&s));
    if same {
        return;
    }
    if let Some(el) = by_id(id) {
        if html {
            el.set_inner_html(&s);
        } else {
            el.set_text_content(Some(&s));
        }
    }
    SHOWN.with(|m| m.borrow_mut().insert(id, s));
}

/// 잔액·이자 상품 카드·최근 거래내역을 다시 써요 (바뀐 것만)
pub fn render() {
    let b = bank();
    put("bal-num", format!("{COIN}<span>{}</span><small>토큰</small>", ko(if b.ready { b.shown() } else { 0.0 })), true); // 불러오기 전이나 못 불러오면 0
    if b.ready && b.rate > 0.0 {
        put("bank-rate", format!("연 {}% 복리", b.rate), false);
        put("bank-note", format!("{} · 다음 이자까지 {}초", every_text(b.shown(), b.rate), b.next_in()), false);
    } else {
        put("bank-rate", "가입한 상품이 없어요".into(), false);
        put("bank-note", format!("연 5% 상품 {} 토큰부터 시작해요", ko(price(5))), false);
    }
    let v = TX_VER.get() as i64;
    if RENDERED_TX.get() != v {
        RENDERED_TX.set(v);
        put("tx-top", groups(&txs_top(5)), true);
    }
}
fn render_history() {
    let v = TX_VER.get() as i64;
    if RENDERED_HIST.get() == v {
        return;
    }
    RENDERED_HIST.set(v);
    if let Some(el) = by_id("history-list") {
        el.set_inner_html(&groups(&txs_top(80)));
    }
}
fn render_products() {
    let b = bank();
    let sig = format!("{}|{}|{}", b.rate, b.bal, b.ready);
    if PRODUCT_SIG.with(|s| *s.borrow() == sig) {
        return;
    }
    PRODUCT_SIG.with(|s| *s.borrow_mut() = sig);
    dom::set_text("bank-bal", &format!("내 토큰 {}", w(if b.ready { b.bal } else { 0.0 })));
    let rows: String = RATES
        .iter()
        .map(|&r| {
            let p = price(r);
            let rf = r as f64;
            let own = b.rate == rf;
            let lower = rf < b.rate;
            let afford = b.ready && b.bal >= p;
            let off = own || lower || !afford;
            let label = if own {
                "가입 중"
            } else if lower {
                "보유 중"
            } else if afford {
                "구입"
            } else {
                "토큰 부족"
            };
            format!(
                r#"<li class="t-row prod"><span class="t-ico sm" style="--c:#d6f1e6">{}</span><span class="t-main"><b class="ts-name">연 {r}% 복리</b><span class="t-label">{}</span></span><span class="prod-side"><b class="prod-price">{} 토큰</b><button class="t-btn small {}" data-buy="{r}" type="button" {}>{label}</button></span></li>"#,
                ico("percent"),
                every_text(b.bal, rf),
                ko(p),
                if off { "grey" } else { "" },
                if off { "disabled" } else { "" }
            )
        })
        .collect();
    if let Some(el) = by_id("bank-list") {
        el.set_inner_html(&rows);
    }
}
fn sheet_open(on: bool, id: &str) {
    let el = by_id(id).map(JsValue::from).unwrap_or(JsValue::UNDEFINED);
    let _ = ctx().call("sheetOpen", &[on.into(), el]);
}
pub(crate) fn open_history() {
    render_history();
    sheet_open(true, "history-sheet");
}
pub(crate) fn open_products() {
    render_products();
    sheet_open(true, "bank-sheet");
}

async fn buy(r: u32) {
    if BUYING.get() || !RATES.contains(&r) {
        return;
    }
    let p = price(r);
    let c = ctx();
    let ok = c.ask(&format!("연 {r}% 이자 상품"), &format!("{} 토큰을 내고 구입할까요? 구입하면 내 토큰이 복리로 계속 늘어나요.", ko(p)), "구입", "취소").await;
    if !ok || bank().bal < p {
        return;
    }
    BUYING.set(true);
    claim_now().await; // 지금까지 쌓인 이자를 먼저 받아 두어요 (상품을 사면 이자 시계가 새로 시작해요)
    match wallet_run("buyProduct", vec![(r as f64).into(), p.into()]).await {
        Ok(_) => {
            log_tx("이자 상품 구입", "은행", "percent", -p);
            c.toast(&format!("연 {r}% 이자 상품에 가입했어요"));
        }
        Err(_) => c.toast("구입하지 못했어요. 잠시 뒤 다시 해 주세요"),
    }
    BUYING.set(false);
}

/// 1초마다 화면에 보이는 숫자만 새로 계산해요. 보이지 않으면(다른 탭, 화면 꺼짐) 아무것도 하지 않아요
fn tick() {
    if dom::doc_hidden() || !bank().ready {
        return;
    }
    let bank_open = dom::has_class("bank-sheet", "is-open");
    if dom::screen_active("money") || bank_open {
        render();
    }
    if bank_open {
        render_products();
    }
    if dom::has_class("history-sheet", "is-open") {
        render_history();
    }
}

pub fn init(c: Ctx) {
    CTX.with(|x| *x.borrow_mut() = c);
    load_txs();
    set_bank(bank());
    let Some(root) = by_id("money-root") else { return };
    let ids = ["bank", "hist"];
    let saved: Vec<String> = dom::store_get("money-order", "").split(',').filter(|x| ids.contains(x)).map(String::from).collect();
    let mut order = saved.clone();
    order.extend(ids.iter().filter(|x| !saved.iter().any(|s| s == *x)).map(|x| x.to_string()));
    root.set_inner_html(&format!(
        r#"
    <div class="ac-top"><button class="ac-ic" id="ac-search" type="button" aria-label="거래내역">{}</button><button class="ac-ic" id="ac-more" type="button" aria-label="이자 상품">{}</button></div>
    <div class="ac-card"><span class="ac-word">token</span><canvas class="ac-art" id="ac-art" aria-hidden="true"></canvas></div>
    <div class="ac-bal"><div class="ac-bal-row"><div><span class="ac-bl">잔액</span><b id="bal-num" data-volatile></b></div><button class="ac-pill" id="ac-send" type="button">보내기 또는 요청</button></div></div>
    <div class="ac-fine"><span class="ac-fine-l">토큰<br>발행소</span><i></i><b>TOKEN</b></div>
    <div id="movable">{}</div>"#,
        sv2(r#"<circle cx="11" cy="11" r="7"/><path d="M16.500 16.500L21 21"/>"#),
        sv2(r#"<circle cx="12" cy="12" r="9"/><path d="M8 12h.01M12 12h.01M16 12h.01"/>"#),
        order.iter().map(|id| card(id)).collect::<String>()
    ));
    art::mount();
    render();
    if let Some(b) = by_id("ac-search") {
        dom::on(&b, "click", |_| open_history());
    }
    if let Some(b) = by_id("ac-more") {
        dom::on(&b, "click", |_| open_products());
    }
    for b in dom::qsa(&dom::doc().document_element().expect("html"), "[data-close]") {
        dom::on(&b, "click", |_| sheet_open(false, ""));
    }
    if let Some(l) = by_id("bank-list") {
        dom::on(&l, "click", |e| {
            let Some(btn) = dom::closest(&e, "[data-buy]") else { return };
            if btn.dyn_ref::<web_sys::HtmlButtonElement>().is_some_and(|b| b.disabled()) {
                return;
            }
            let r: u32 = dom::data(&btn, "buy").parse().unwrap_or(0);
            leptos::task::spawn_local(buy(r));
        });
    }
    if let Some(m) = by_id("movable") {
        dom::on(&m, "click", |e| {
            if reorder::swallow() {
                return;
            }
            let Some(c) = dom::closest(&e, ".movable") else { return };
            match dom::data(&c, "id").as_str() {
                "hist" => open_history(),
                "bank" => open_products(),
                _ => chart::open(),
            }
        });
        reorder::init(m);
    }
    chart::init();
    // 10분에 한 번 이자를 정산하고, 화면을 떠날 때도 정산해요
    if let Some(w) = web_sys::window() {
        let c = Closure::<dyn FnMut()>::new(|| {
            if !dom::doc_hidden() {
                leptos::task::spawn_local(claim_now());
            }
        });
        let _ = w.set_interval_with_callback_and_timeout_and_arguments_0(c.as_ref().unchecked_ref(), 10 * 60 * 1000);
        c.forget();
        let t = Closure::<dyn FnMut()>::new(tick);
        let _ = w.set_interval_with_callback_and_timeout_and_arguments_0(t.as_ref().unchecked_ref(), 1000);
        t.forget();
    }
    dom::on(&dom::doc(), "visibilitychange", |_| {
        if dom::doc_hidden() {
            leptos::task::spawn_local(claim_now());
        } else {
            tick();
        }
    });
}

/// 토큰을 쓰기 전에: 지갑이 아직 안 왔으면 기다리고, 모자라 보이면 쌓인 이자를 먼저 받아서 다시 봐요.
/// 화면에 보이는 토큰(이자 포함)이 있는데 "모자라요"가 뜨지 않게 해요. 안 되면 사람이 읽을 이유를 돌려줘요
pub async fn ensure(amt: f64) -> Result<f64, String> {
    for _ in 0..20 {
        if bank().ready {
            break;
        }
        dom::sleep(150).await;
    }
    let b = bank();
    if !b.ready {
        return Err("지갑을 불러오는 중이에요. 잠시 뒤 다시 해 주세요".into());
    }
    if b.bal >= amt {
        return Ok(b.bal);
    }
    claim_now().await;
    for _ in 0..12 {
        if bank().bal >= amt {
            return Ok(bank().bal);
        }
        dom::sleep(150).await;
    }
    Err(format!("토큰이 모자라요. 내 토큰은 {}이에요", w(bank().bal)))
}
/// 지금 지갑 (다른 러스트 화면이 읽어요)
pub fn balance() -> Option<f64> {
    let b = bank();
    b.ready.then_some(b.bal)
}
