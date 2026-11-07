//! 연구실·발전소 (원자력 발전·컴퓨터·인공지능), 식품 회사 금고. 상품은 한 개예요. 숫자는 직원 일 탭에서 올라가요.
//! 모든 회사 공통: 회사 금고 넣기·빼기, 단가. 인공지능 회사는 전기·컴퓨터를 사고, 다른 회사는 인공지능 사용 시간을 사요.

use std::cell::Cell;
use std::time::Duration;

use cloud::{fmt_num, get, ts_ms};
use leptos::mount::mount_to;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::prelude::*;

use super::{co_str, company, ctx};
use crate::dom::{self, by_id, read_num};

const LAB_ABS_MAX: f64 = 100000.0; // 한 번에 만들 수 있는 전기 양의 절대 상한

/// 연구실 숫자 하나 (없으면 0)
fn n(l: &JsValue, k: &str) -> f64 {
    get(l, k).as_f64().unwrap_or(0.0)
}
fn has(l: &JsValue, k: &str) -> bool {
    get(l, k).is_truthy()
}
fn fmt_lab(v: f64) -> String {
    fmt_num(v, 3)
}
/// 센트를 토큰으로 보여요 (100센트 = 1 토큰)
fn fmt_tok(cents: f64) -> String {
    fmt_num(cents / 100.0, 2)
}
fn fmt_sec(s: f64) -> String {
    let s = s as i64;
    if s >= 60 {
        format!("{}분 {}초", s / 60, s % 60)
    } else {
        format!("{s}초")
    }
}
/// 한 번에 최대: 핵연료 성능 × 10
fn max_gen(l: &JsValue) -> f64 {
    LAB_ABS_MAX.min(10.0 * n(l, "nperf"))
}
fn left_of(job: &JsValue) -> f64 {
    ((ts_ms(&get(job, "started")) + n(job, "secs") * 1000.0 - js_sys::Date::now()) / 1000.0).ceil().max(0.0)
}
fn gen_text(l: &JsValue, left: f64) -> String {
    let amt = fmt_lab(n(&get(l, "gen"), "amt"));
    if left > 0.0 {
        format!("{amt} 전기를 만들고 있어요 · 남은 시간 {}", fmt_sec(left))
    } else {
        format!("{amt} 전기를 다 만들었어요")
    }
}
fn trn_text(l: &JsValue, left: f64) -> String {
    let d = fmt_lab(n(&get(l, "trn"), "d"));
    if left > 0.0 {
        format!("{d} MB를 학습하고 있어요 · 남은 시간 {}", fmt_sec(left))
    } else {
        format!("{d} MB 학습을 마쳤어요")
    }
}
/// 예상: 걸리는 시간과 비용은 같아요 (1초 = 1 토큰). 성능이 높을수록 둘 다 줄어요
fn plan_text(l: &JsValue, amt: f64) -> String {
    if !(amt >= 1.0 && amt <= max_gen(l)) {
        return format!("1부터 {} 사이로 적어 주세요", fmt_lab(max_gen(l)));
    }
    let secs = (amt * 100.0 / n(l, "nperf")).ceil();
    format!("비용 {} 토큰 · 약 {} 걸려요{}", fmt_lab(secs), fmt_sec(secs), if n(l, "cash") < secs * 100.0 { " · 금고가 부족해요" } else { "" })
}
/// 학습: 데이터 양으로 배치 수·전기·시간·점수를 미리 보여줘요 (규칙과 같은 식)
fn train_plan(l: &JsValue, d: f64) -> String {
    let m = get(l, "pcM");
    if !(n(l, "pcN") >= 1.0 && m.is_truthy()) {
        return "먼저 컴퓨터를 사야 학습할 수 있어요".into();
    }
    if !(d >= 1.0 && d <= n(l, "aiData")) {
        return format!("1부터 {} MB 사이로 적어 주세요", fmt_lab(n(l, "aiData")));
    }
    let batches = (d / n(&m, "cap")).ceil();
    let used = batches * n(&m, "pow");
    let secs = (batches / n(&m, "spd")).ceil();
    let score = (d * (n(&m, "cap") + n(&m, "spd")) / 100.0).floor();
    format!("전기 {} 사용 · 약 {} 걸려요 · 개발 점수 {}{}", fmt_lab(used), fmt_sec(secs), fmt_lab(score), if n(l, "elec") < used { " · 전기가 부족해요" } else { "" })
}

/// 시장 줄 하나 (사는 곳)
#[derive(Clone)]
struct Row {
    id: String,
    name: String,
    desc: String,
    kind: &'static str,
    def: u32,
}
#[derive(Clone)]
enum Market {
    Loading,
    Failed,
    Ai { e: Vec<Row>, p: Vec<Row>, mine: String },
    Use { rows: Vec<Row>, hours: String },
}

#[derive(Clone)]
struct LabNow {
    kind: String,
    l: JsValue,
}

#[derive(Clone, Copy)]
struct S {
    now: RwSignal<Option<Result<LabNow, ()>>, LocalStorage>,
    market: RwSignal<Market, LocalStorage>,
    gen_amt: RwSignal<f64>,
    train_d: RwSignal<f64>,
    tick: RwSignal<u32>,
}
thread_local! {
    static SIG: Cell<Option<S>> = const { Cell::new(None) };
    static SEQ: Cell<u32> = const { Cell::new(0) };
    static TICKING: Cell<bool> = const { Cell::new(false) };
}

pub fn init() {
    let Some(root) = by_id("biz-lab") else { return };
    let s = S {
        now: RwSignal::new_local(None),
        market: RwSignal::new_local(Market::Loading),
        gen_amt: RwSignal::new(100.0),
        train_d: RwSignal::new(0.0),
        tick: RwSignal::new(0),
    };
    SIG.set(Some(s));
    root.set_inner_html("");
    mount_to(root, move || view! { <LabCard s/> }).forget();
}

/// 서버에서 연구실 숫자를 다시 읽어 그려요
pub fn paint() {
    let (Some(s), Some(co)) = (SIG.get(), company()) else { return };
    let cid = co_str(&co, "id");
    let kind = co_str(&co, "kind");
    let seq = SEQ.get() + 1;
    SEQ.set(seq);
    spawn_local(async move {
        match cloud::lab_get(&cid).await {
            Ok(l) => {
                // 다른 회사로 바뀌었으면 그리지 않아요
                if SEQ.get() != seq || company().map(|c| co_str(&c, "id")) != Some(cid.clone()) {
                    return;
                }
                s.train_d.set(n(&l, "aiData"));
                s.market.set(Market::Loading);
                s.now.set(Some(Ok(LabNow { kind: kind.clone(), l: l.clone() })));
                load_market(s, cid, kind == "ai", l.clone());
                let busy = |k| has(&l, k) && left_of(&get(&l, k)) > 0.0;
                if busy("gen") || busy("trn") {
                    start_tick(s);
                }
            }
            Err(_) => {
                if SEQ.get() == seq {
                    s.now.set(Some(Err(())));
                }
            }
        }
    });
}

/// 발전·학습 중에는 1초마다 남은 시간만 고쳐요. 끝나면 받기 버튼이 켜져요
fn start_tick(s: S) {
    if TICKING.get() {
        return;
    }
    TICKING.set(true);
    let h: std::rc::Rc<Cell<Option<IntervalHandle>>> = Default::default();
    let h2 = h.clone();
    let t = set_interval_with_handle(
        move || {
            let busy = s.now.with_untracked(|x| match x {
                Some(Ok(x)) => ["gen", "trn"].iter().any(|k| has(&x.l, k) && left_of(&get(&x.l, k)) > 0.0),
                _ => false,
            });
            s.tick.update(|t| *t = t.wrapping_add(1));
            if !busy || by_id("biz-lab").is_none() {
                if let Some(h) = h2.take() {
                    h.clear();
                }
                TICKING.set(false);
            }
        },
        Duration::from_millis(1000),
    );
    h.set(t.ok());
}

/// 시장 (인공지능 회사): 전기 파는 곳과 컴퓨터 파는 곳의 단가·재고. 다른 회사: 인공지능 사용
fn load_market(s: S, cid: String, ai: bool, mine: JsValue) {
    spawn_local(async move {
        let got: Result<Market, cloud::CloudError> = async {
            let list = cloud::list_companies("").await?;
            let pick = |k: &str| list.iter().filter(|c| c.kind == k && c.id != cid).take(8).cloned().collect::<Vec<_>>();
            let labs = |cs: Vec<cloud::Company>| async move {
                let mut out = Vec::new();
                for c in cs {
                    let l = cloud::lab_get(&c.id).await?;
                    out.push((c, l));
                }
                Ok::<_, cloud::CloudError>(out)
            };
            let row = |c: &cloud::Company, desc: String, kind: &'static str, def: u32| Row { id: c.id.clone(), name: c.name.clone(), desc, kind, def };
            if ai {
                let nl = labs(pick("nuclear")).await?;
                let pl = labs(pick("pc")).await?;
                let e = nl
                    .iter()
                    .filter(|(_, l)| n(l, "price") > 0.0 && n(l, "elec") > 0.0)
                    .map(|(c, l)| row(c, format!("1 전기 {} 토큰 · 재고 {}", fmt_tok(n(l, "price")), fmt_lab(n(l, "elec"))), "e", 10))
                    .collect();
                let p = pl
                    .iter()
                    .filter(|(_, l)| n(l, "price") > 0.0 && has(l, "rel"))
                    .map(|(c, l)| {
                        let r = get(l, "rel");
                        row(c, format!("1대 {} 토큰 · 용량 {} · 속도 {} · 전기 {}", fmt_tok(n(l, "price")), fmt_lab(n(&r, "cap")), fmt_lab(n(&r, "spd")), fmt_lab(n(&r, "pow"))), "p", 1)
                    })
                    .collect();
                let m = get(&mine, "pcM");
                let extra = if m.is_truthy() { format!(" (용량 {} · 속도 {} · 전기 {})", fmt_lab(n(&m, "cap")), fmt_lab(n(&m, "spd")), fmt_lab(n(&m, "pow"))) } else { String::new() };
                Ok(Market::Ai { e, p, mine: format!("전기 {} · 컴퓨터 {}대{extra}", fmt_lab(n(&mine, "elec")), fmt_lab(n(&mine, "pcN"))) })
            } else {
                let al = labs(pick("ai")).await?;
                let rows = al
                    .iter()
                    .filter(|(_, l)| n(l, "price") > 0.0 && get(l, "aiScore").as_f64().is_some_and(|v| v >= 0.0))
                    .map(|(c, l)| row(c, format!("1시간 {} 토큰 · 점수 {}", fmt_tok(n(l, "price")), fmt_lab(n(l, "aiScore"))), "h", 1))
                    .collect();
                Ok(Market::Use { rows, hours: format!("내 사용 시간 {}시간", fmt_lab(n(&mine, "aiH"))) })
            }
        }
        .await;
        if company().map(|c| co_str(&c, "id")) != Some(cid) {
            return;
        }
        s.market.set(got.unwrap_or(Market::Failed));
    });
}

#[component]
fn LabCard(s: S) -> impl IntoView {
    move || match s.now.get() {
        None => ().into_any(),
        Some(Err(())) => view! { <p class="lab-sub">"연구실 숫자를 불러오지 못했어요"</p> }.into_any(),
        Some(Ok(x)) => body(s, x).into_any(),
    }
}

/// 회사 금고 줄 (모든 회사 공통): 넣기·빼기
fn cash_view(l: &JsValue) -> impl IntoView {
    view! {
        <div class="lab-cash">
            <p class="lab-sub">"회사 금고 "<b>{fmt_tok(n(l, "cash"))}</b>" 토큰"</p>
            <label class="lab-field"><span>"넣거나 뺄 토큰"</span><input id="cash-amt" class="lab-input" type="text" inputmode="numeric" autocomplete="off" maxlength="9" value="100" aria-label="금고에 넣거나 뺄 토큰"/></label>
            <div class="lab-actions">
                <button type="button" class="btn" data-cash="out" on:click=move |ev| cash(ev, -1.0)>"빼기"</button>
                <button type="button" class="btn primary" data-cash="in" on:click=move |ev| cash(ev, 1.0)>"넣기"</button>
            </div>
        </div>
    }
}
/// 단가 줄 (사장이 정해요): 파는 한 단위의 가격을 토큰으로 적어요
fn price_view(l: &JsValue, label: &'static str) -> impl IntoView {
    view! {
        <div class="lab-price">
            <label class="lab-field"><span>{label}</span><span class="lab-unit"><input id="price-amt" class="lab-input" type="text" inputmode="decimal" autocomplete="off" maxlength="9" value=fmt_tok(n(l, "price")) aria-label="단가 (토큰)"/><b>"토큰"</b></span></label>
            <button type="button" class="btn" data-lab="price" on:click=move |ev| act(ev, "price")>"단가 정하기"</button>
        </div>
    }
}

fn body(s: S, x: LabNow) -> AnyView {
    let l = x.l.clone();
    match x.kind.as_str() {
        "food" | "" => view! {
            <p class="lab-t">"식품 회사 · 금고"</p>
            {cash_view(&l)}
            <p class="lab-sub">"만든 음식은 상점에 진열돼요. 상점에서 팔리면 금고에 들어와요"</p>
            <AiUse s/>
        }
        .into_any(),
        "nuclear" => {
            let head = view! {
                <p class="lab-t">"원자력 발전 · 상품 1개: 전기"</p>
                <p class="lab-big">{fmt_lab(n(&l, "elec"))}<small>"전기 창고"</small></p>
                <p class="lab-sub">{format!("핵연료 성능 {} · 성능이 높을수록 적은 돈으로, 빨리, 많이 만들어요", fmt_lab(n(&l, "nperf")))}</p>
            };
            if has(&l, "gen") {
                let (l1, l2) = (l.clone(), l.clone());
                view! {
                    {head}
                    <p class="lab-sub" id="lab-left">{move || { s.tick.track(); gen_text(&l1, left_of(&get(&l1, "gen"))) }}</p>
                    <button class="btn primary" type="button" data-lab="claim" disabled=move || { s.tick.track(); left_of(&get(&l2, "gen")) > 0.0 } on:click=move |ev| act(ev, "claim")>"창고에 받기"</button>
                    {cash_view(&l)}
                    {price_view(&l, "전기 1단위 가격")}
                    <AiUse s/>
                }
                .into_any()
            } else {
                let l1 = l.clone();
                let on_amt = move |ev: web_sys::Event| {
                    let Some(inp) = ev.target().and_then(|t| wasm_bindgen::JsCast::dyn_into::<web_sys::HtmlInputElement>(t).ok()) else { return };
                    let d: String = inp.value().chars().filter(|c| c.is_ascii_digit()).take(6).collect();
                    if inp.value() != d {
                        inp.set_value(&d);
                    }
                    s.gen_amt.set(if d.is_empty() { 0.0 } else { dom::js_number(&d) });
                };
                view! {
                    {head}
                    <label class="lab-field"><span>"만들 전기"</span><input id="lab-amt" class="lab-input" type="text" inputmode="numeric" autocomplete="off" maxlength="6" value=fmt_plain(s.gen_amt.get_untracked()) aria-label="만들 전기 양" on:input=on_amt/></label>
                    <p class="lab-sub" id="lab-est">{move || plan_text(&l1, s.gen_amt.get())}</p>
                    <button class="btn primary" type="button" data-lab="gen" on:click=move |ev| act(ev, "gen")>"발전 시작하기"</button>
                    {cash_view(&l)}
                    {price_view(&l, "전기 1단위 가격")}
                    <AiUse s/>
                }
                .into_any()
            }
        }
        "pc" => {
            let rel = get(&l, "rel");
            let rl = rel.is_truthy();
            let same = rl && n(&rel, "cap") == n(&l, "cap") && n(&rel, "spd") == n(&l, "spd") && n(&rel, "pow") == n(&l, "pow");
            let none = n(&l, "cap") == 0.0 && n(&l, "spd") == 0.0 && n(&l, "pow") == 0.0;
            view! {
                <p class="lab-t">"컴퓨터 · 상품 1개"</p>
                <div class="lab-stats"><span>"처리 용량"<b>{fmt_lab(n(&l, "cap"))}</b></span><span>"처리 속도"<b>{fmt_lab(n(&l, "spd"))}</b></span><span>"전기값"<b>{fmt_lab(n(&l, "pow"))}</b></span></div>
                <p class="lab-sub">{if rl { format!("출시한 숫자 · 용량 {} · 속도 {} · 전기 {}", fmt_lab(n(&rel, "cap")), fmt_lab(n(&rel, "spd")), fmt_lab(n(&rel, "pow"))) } else { "아직 출시하지 않았어요".into() }}</p>
                <button class="btn primary" type="button" data-lab="rel" disabled={same || none} on:click=move |ev| act(ev, "rel")>{if rl { "다시 출시하기" } else { "출시하기" }}</button>
                {cash_view(&l)}
                {price_view(&l, "컴퓨터 1대 가격")}
                <AiUse s/>
            }
            .into_any()
        }
        _ => {
            let dev = n(&l, "aiDev");
            let score = get(&l, "aiScore").as_f64();
            let released = score.is_some_and(|v| v >= 0.0);
            let trn = if has(&l, "trn") {
                let (l1, l2) = (l.clone(), l.clone());
                view! {
                    <p class="lab-sub" id="lab-trn-left">{move || { s.tick.track(); trn_text(&l1, left_of(&get(&l1, "trn"))) }}</p>
                    <button class="btn primary" type="button" data-lab="train-claim" disabled=move || { s.tick.track(); left_of(&get(&l2, "trn")) > 0.0 } on:click=move |ev| act(ev, "train-claim")>"학습 결과 받기"</button>
                }
                .into_any()
            } else {
                let l1 = l.clone();
                let on_d = move |ev: web_sys::Event| {
                    let v = ev.target().and_then(|t| wasm_bindgen::JsCast::dyn_into::<web_sys::HtmlInputElement>(t).ok()).map(|i| i.value()).unwrap_or_default();
                    let d = dom::js_number(&v.replace(',', ""));
                    s.train_d.set(if d.is_nan() { 0.0 } else { d });
                };
                view! {
                    <label class="lab-field"><span>"학습할 데이터 (MB)"</span><input id="train-amt" class="lab-input" type="text" inputmode="numeric" autocomplete="off" maxlength="9" value=fmt_plain(n(&l, "aiData")) aria-label="학습할 데이터 양" on:input=on_d/></label>
                    <p class="lab-sub" id="train-est">{move || train_plan(&l1, s.train_d.get())}</p>
                    <button class="btn primary" type="button" data-lab="train" disabled={n(&l, "pcN") < 1.0} on:click=move |ev| act(ev, "train")>"학습 시작하기"</button>
                }
                .into_any()
            };
            view! {
                <p class="lab-t">"인공지능 · 상품 1개"</p>
                <p class="lab-big">{fmt_lab(n(&l, "aiData"))}<small>"MB 쌓인 데이터"</small></p>
                <p class="lab-sub">{format!("개발 점수 {}{}", fmt_lab(dev), if released { format!(" · 출시 점수 {}", fmt_lab(score.unwrap_or(0.0))) } else { " · 아직 출시 전이에요".into() })}</p>
                {trn}
                <button class="btn primary" type="button" data-lab="rel" disabled={dev < 1.0 || Some(dev) == score} on:click=move |ev| act(ev, "rel")>{if released { "다시 출시하기" } else { "출시하기" }}</button>
                {cash_view(&l)}
                {price_view(&l, "사용 1시간 가격")}
                <AiMarket s/>
            }
            .into_any()
        }
    }
}
fn fmt_plain(v: f64) -> String {
    goods::shapes::num(v)
}

fn buy_rows(rows: Vec<Row>) -> impl IntoView {
    rows.into_iter()
        .map(|r| {
            let (id, kind) = (r.id.clone(), r.kind);
            view! {
                <div class="lab-row">
                    <div class="lab-rowt"><b>{r.name.clone()}</b><span>{r.desc}</span></div>
                    <div class="lab-rowbuy">
                        <input id=format!("buy-{}", r.id) class="lab-input" type="text" inputmode="numeric" autocomplete="off" maxlength="6" value=r.def.to_string() aria-label=format!("{}{}", r.name, if kind == "h" { " 사용 시간" } else { "에서 살 수량" })/>
                        <button type="button" class="btn primary" data-buy=kind data-seller=r.id.clone() on:click=move |ev| buy(ev, id.clone(), kind)>"사기"</button>
                    </div>
                </div>
            }
        })
        .collect_view()
}

#[component]
fn AiMarket(s: S) -> impl IntoView {
    view! {
        <div class="lab-market" id="lab-market">
            {move || match s.market.get() {
                Market::Loading => view! { <p class="lab-sub">"시장을 불러오는 중이에요"</p> }.into_any(),
                Market::Failed => view! { <p class="lab-sub">"시장을 불러오지 못했어요"</p> }.into_any(),
                Market::Ai { e, p, mine } => view! {
                    <p class="lab-t">"전기 사기"</p>
                    {if e.is_empty() { view! { <p class="lab-sub">"지금 파는 원자력 발전 회사가 없어요"</p> }.into_any() } else { buy_rows(e).into_any() }}
                    <p class="lab-t">"컴퓨터 사기"</p>
                    {if p.is_empty() { view! { <p class="lab-sub">"지금 파는 컴퓨터 회사가 없어요"</p> }.into_any() } else { buy_rows(p).into_any() }}
                    <p class="lab-t">"내 창고"</p>
                    <p class="lab-sub">{mine}</p>
                }
                .into_any(),
                Market::Use { .. } => ().into_any(),
            }}
        </div>
    }
}

#[component]
fn AiUse(s: S) -> impl IntoView {
    view! {
        <div class="lab-market" id="lab-aiuse">
            {move || match s.market.get() {
                Market::Failed => view! { <p class="lab-sub">"인공지능 사용을 불러오지 못했어요"</p> }.into_any(),
                Market::Use { rows, hours } => view! {
                    <p class="lab-t">"인공지능 사용"</p>
                    {if rows.is_empty() { view! { <p class="lab-sub">"지금 쓸 수 있는 인공지능이 없어요"</p> }.into_any() } else { buy_rows(rows).into_any() }}
                    <p class="lab-sub">{hours}</p>
                }
                .into_any(),
                _ => ().into_any(),
            }}
        </div>
    }
}

/* ---------- 누르기 ---------- */

fn btn_of(ev: &web_sys::MouseEvent) -> Option<web_sys::HtmlButtonElement> {
    wasm_bindgen::JsCast::dyn_into(ev.current_target()?).ok()
}

fn cash(ev: web_sys::MouseEvent, dir: f64) {
    let (Some(co), Some(btn)) = (company(), btn_of(&ev)) else { return };
    if btn.disabled() {
        return;
    }
    let c = ctx();
    let v = read_num("cash-amt");
    if !(v.fract() == 0.0 && (1.0..=1_000_000_000.0).contains(&v)) {
        return c.toast("1 이상의 토큰 수를 적어 주세요");
    }
    btn.set_disabled(true);
    let label = btn.text_content().unwrap_or_default();
    btn.set_text_content(Some("처리하는 중…"));
    let cid = co_str(&co, "id");
    spawn_local(async move {
        // 쌓인 이자를 먼저 받아 두어요
        if let Ok(p) = c.call("settle", &[]) {
            let _ = cloud::settle(p).await;
        }
        let cid2 = cid.clone();
        let r = c
            .wallet_op(move || {
                cloud::call_now("labCash", &[JsValue::from_str(&cid2), JsValue::from(v), JsValue::from(dir)]).unwrap_or_else(|e| js_sys::Promise::reject(&JsValue::from_str(&e.message)).into())
            })
            .await;
        match r {
            Ok(_) => c.toast(&if dir > 0.0 { format!("{} 토큰을 금고에 넣었어요", fmt_lab(v)) } else { format!("{} 토큰을 지갑으로 뺐어요", fmt_lab(v)) }),
            Err(e) => c.toast(&if e.code == "insufficient" { if dir > 0.0 { format!("지갑 토큰이 모자라요. 내 토큰은 {}이에요", crate::money::w(crate::money::balance().unwrap_or(0.0))) } else { "회사 금고에 토큰이 모자라요".to_string() } } else { format!("처리하지 못했어요. {}", cloud::why(&e.code)) }),
        }
        btn.set_text_content(Some(&label));
        btn.set_disabled(false);
        paint();
    });
}

fn buy(ev: web_sys::MouseEvent, seller: String, kind: &'static str) {
    let (Some(co), Some(btn)) = (company(), btn_of(&ev)) else { return };
    if btn.disabled() {
        return;
    }
    let c = ctx();
    let v = read_num(&format!("buy-{seller}"));
    if !(v.fract() == 0.0 && v >= 1.0) {
        return c.toast("1 이상의 수량을 적어 주세요");
    }
    btn.set_disabled(true);
    spawn_local(async move {
        match cloud::call("labBuy", &[JsValue::from_str(&co_str(&co, "id")), JsValue::from_str(&seller), JsValue::from_str(kind), JsValue::from(v)]).await {
            Ok(_) => c.toast(&if kind == "e" { format!("전기 {}을 샀어요", fmt_lab(v)) } else { format!("컴퓨터 {}대를 샀어요", fmt_lab(v)) }),
            Err(e) => c.toast(match e.code.as_str() {
                "insufficient" => "금고에 토큰이 부족해요",
                "stock" => "재고가 부족해요",
                "unreleased" => "아직 출시하지 않은 상품이에요",
                "bad" => "수량을 확인해 주세요",
                _ => "처리하지 못했어요. 잠시 뒤 다시 해 주세요",
            }),
        }
        paint();
    });
}

fn act(ev: web_sys::MouseEvent, what: &'static str) {
    let (Some(co), Some(btn), Some(s)) = (company(), btn_of(&ev), SIG.get()) else { return };
    if btn.disabled() {
        return;
    }
    let c = ctx();
    let l = s.now.with_untracked(|x| match x {
        Some(Ok(x)) => Some(x.l.clone()),
        _ => None,
    });
    let amt = s.gen_amt.get_untracked();
    if what == "gen" && !(amt.fract() == 0.0 && amt >= 1.0 && l.as_ref().is_some_and(|l| amt <= max_gen(l))) {
        return c.toast(&format!("1부터 {} 사이로 적어 주세요", fmt_lab(l.as_ref().map(max_gen).unwrap_or(1000.0))));
    }
    let price = read_num("price-amt");
    if what == "price" && !(price >= 0.0 && price <= 1_000_000.0) {
        return c.toast("0 이상의 토큰 수를 적어 주세요");
    }
    btn.set_disabled(true);
    let cid = co_str(&co, "id");
    let kind = co_str(&co, "kind");
    spawn_local(async move {
        let a = |v: &str| JsValue::from_str(v);
        let r: Result<(), cloud::CloudError> = async {
            match what {
                "gen" => {
                    let r = cloud::call("labGenStart", &[a(&cid), JsValue::from(amt)]).await?.as_string().unwrap_or_default();
                    c.toast(&match r.as_str() {
                        "ok" => format!("전기 {}을 만들기 시작했어요", fmt_lab(amt)),
                        "busy" => "이미 발전 중이에요".into(),
                        "poor" => "금고에 토큰이 부족해요".into(),
                        "max" => "한 번에 만들 수 있는 양을 넘었어요".into(),
                        _ => "처리하지 못했어요".into(),
                    });
                }
                "train" => {
                    let d = read_num("train-amt");
                    let r = cloud::call("labTrainStart", &[a(&cid), JsValue::from(d)]).await?.as_string().unwrap_or_default();
                    c.toast(&match r.as_str() {
                        "ok" => format!("{} MB 학습을 시작했어요", fmt_lab(d)),
                        "busy" => "이미 학습 중이에요".into(),
                        "nocomp" => "먼저 컴퓨터를 사야 해요".into(),
                        "nodata" => "데이터 양을 확인해 주세요".into(),
                        "poor" => "전기가 부족해요".into(),
                        _ => "처리하지 못했어요".into(),
                    });
                }
                "train-claim" => {
                    let got = cloud::call("labTrainClaim", &[a(&cid)]).await?.as_f64().unwrap_or(-1.0);
                    c.toast(&if got >= 0.0 { format!("학습 결과: 개발 점수 {}", fmt_lab(got)) } else { "아직 학습 중이에요".into() });
                }
                "claim" => {
                    let got = cloud::call("labGenClaim", &[a(&cid)]).await?.as_f64().unwrap_or(0.0);
                    c.toast(&if got != 0.0 { format!("전기 {}을 창고에 받았어요", fmt_lab(got)) } else { "아직 발전 중이에요".into() });
                }
                "price" => {
                    let cents = (price * 100.0).round();
                    cloud::lab_change(&cid, &dom::js_fn1(move |l| dom::with_key(&l, "price", &JsValue::from(cents)))).await?;
                    c.toast("단가를 정했어요");
                }
                _ if kind == "pc" => {
                    cloud::lab_change(
                        &cid,
                        &dom::js_fn1(|l| {
                            let rel = dom::obj(&[("cap", get(&l, "cap")), ("spd", get(&l, "spd")), ("pow", get(&l, "pow"))]);
                            dom::with_key(&l, "rel", &rel)
                        }),
                    )
                    .await?;
                    c.toast("컴퓨터를 출시했어요");
                }
                _ => {
                    cloud::lab_change(&cid, &dom::js_fn1(|l| dom::with_key(&l, "aiScore", &get(&l, "aiDev")))).await?;
                    c.toast("인공지능을 출시했어요");
                }
            }
            Ok(())
        }
        .await;
        if let Err(e) = &r {
            c.toast(&format!("처리하지 못했어요. {}", cloud::why(&e.code)));
        }
        paint();
    });
}
