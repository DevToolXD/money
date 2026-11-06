//! 식품 공장 카드 (사장만, 사업 탭 안). 예전 app/work.js의 factoryInto를 옮겼어요.
//! 공장 한 대는 한 개를 60초에 만들어요. 만든 만큼 재료값이 나가고 재고에 바로 들어가요.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::time::Duration;

use cloud::{fmt_num, get, ts_ms, CloudError};
use leptos::mount::mount_to;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::{JsCast, JsValue};

use crate::work::ctx;

const FAC_MS: f64 = 60000.0;

/// 만드는 상품 (회사 첫 상품)
#[derive(Clone, PartialEq)]
struct Prod {
    id: String,
    name: String,
    cost_cents: f64,
    price_cents: f64,
    stock: f64,
    paused: bool,
}

/// 돌고 있는 공장: 상품, 남은 개수, 다음 개수를 재기 시작한 시각(ms), 처음 정한 개수
#[derive(Clone, PartialEq)]
struct Run {
    pid: String,
    left: f64,
    at: f64,
    total: f64,
}

/// 연구실에서 공장에 필요한 숫자
#[derive(Clone, PartialEq)]
struct FacLab {
    plv: f64,
    cash: f64,
    run: Option<Run>,
}

#[derive(Clone, Copy)]
struct FacSig {
    cid: StoredValue<String>,
    prod: RwSignal<Option<Option<Prod>>>,
    /// 회사가 파는 상품 전부 (공장에서 무엇을 만들지 골라요)
    all: RwSignal<Vec<Prod>>,
    note: RwSignal<String>,
    lab: RwSignal<Option<FacLab>>,
    n: RwSignal<String>,
    tick: RwSignal<u32>,
}

struct Mounted {
    el: web_sys::HtmlElement,
    cid: String,
    _handle: Box<dyn Any>,
    timers: Vec<IntervalHandle>,
}

thread_local! {
    static MOUNTED: RefCell<Option<Mounted>> = const { RefCell::new(None) };
    static SETTLING: Cell<bool> = const { Cell::new(false) };
    static LAST_SETTLE: Cell<f64> = const { Cell::new(0.0) };
}

/// 한 개가 나오는 데 걸리는 시간(ms): 60초 ÷ 공장 대수 (대수가 늘어도 전체 속도는 같아요)
fn unit_ms(plv: f64) -> f64 {
    FAC_MS / plv.max(1.0)
}
/// 지금까지 나온(아직 정산하지 않은) 개수. skew는 서버가 시계 차이를 감안해 덜 세는 시간이에요
fn due(r: &Run, plv: f64, now: f64, skew: f64) -> f64 {
    let e = (now - skew - r.at).max(0.0).floor();
    r.left.min((e * plv.max(1.0) / FAC_MS).floor())
}

fn clock(secs: f64) -> String {
    let s = secs.max(0.0) as i64;
    if s >= 3600 {
        format!("{}시간 {}분", s / 3600, (s % 3600) / 60)
    } else if s >= 60 {
        format!("{}분 {}초", s / 60, s % 60)
    } else {
        format!("{s}초")
    }
}
/// "오후 3:12"
fn at_clock(ms: f64) -> String {
    let d = js_sys::Date::new(&JsValue::from_f64(ms));
    let (h, m) = (d.get_hours(), d.get_minutes());
    format!("{} {}:{:02}", if h < 12 { "오전" } else { "오후" }, if h % 12 == 0 { 12 } else { h % 12 }, m)
}
fn digits(v: &str) -> String {
    v.chars().filter(|c| c.is_ascii_digit()).take(6).collect()
}
fn fac_msg(e: &CloudError) -> String {
    match e.code.as_str() {
        "busy" => "공장이 이미 돌고 있어요".into(),
        "poor" => "회사 금고에 토큰이 모자라요".into(),
        "nolab" => "공장을 아직 만들지 않았어요".into(),
        "bad" => "개수를 확인해 주세요".into(),
        "max" => "공장을 더 살 수 없어요".into(),
        "unsellable" => "지금은 만들 수 없는 상품이에요".into(),
        _ => format!("공장을 돌리지 못했어요. {}", cloud::why(&e.code)),
    }
}

fn stop_timers(m: &mut Mounted) {
    for t in m.timers.drain(..) {
        t.clear();
    }
}

/// 사업 탭: 식품 공장 카드를 그 자리에 그리고 연결해요 (회사가 바뀌면 다시 그려요)
pub fn factory_into(el: web_sys::HtmlElement, cid: String) {
    let same = MOUNTED.with(|m| {
        m.borrow().as_ref().is_some_and(|m| m.cid == cid && m.el == el && el.first_element_child().is_some())
    });
    if same {
        return;
    }
    // 예전 카드와 시계를 치워요
    if let Some(mut old) = MOUNTED.with(|m| m.borrow_mut().take()) {
        stop_timers(&mut old);
        drop(old);
    }
    let _ = el.set_attribute("data-cid", &cid);
    el.set_inner_html("");
    let s = FacSig {
        cid: StoredValue::new(cid.clone()),
        prod: RwSignal::new(None),
        all: RwSignal::new(Vec::new()),
        note: RwSignal::new("만들 상품을 불러오는 중이에요".into()),
        lab: RwSignal::new(None),
        n: RwSignal::new("50".into()),
        tick: RwSignal::new(0),
    };
    let handle = mount_to(el.clone(), move || view! { <FactoryCard s/> });
    let mut timers = Vec::new();
    // 1초마다 남은 시간을 다시 그리고, 한 개가 나올 때마다 정산해요. 카드가 화면에서 빠지면 멈춰요
    let el2 = el.clone();
    if let Ok(t) = set_interval_with_handle(
        move || {
            if !el2.is_connected() {
                MOUNTED.with(|m| {
                    if let Some(m) = m.borrow_mut().as_mut() {
                        if m.el == el2 {
                            stop_timers(m);
                        }
                    }
                });
                return;
            }
            s.tick.update(|t| *t = t.wrapping_add(1));
            maybe_settle(s);
        },
        Duration::from_millis(1000),
    ) {
        timers.push(t);
    }
    MOUNTED.with(|m| *m.borrow_mut() = Some(Mounted { el, cid, _handle: Box::new(handle), timers }));
    spawn_local(async move {
        load_products(s).await;
        load_lab(s).await;
        settle(s); // 페이지를 열면 그동안 만든 것부터 받아요
    });
}

fn alive(s: FacSig) -> bool {
    let cid = s.cid.try_get_value();
    MOUNTED.with(|m| m.borrow().as_ref().is_some_and(|m| Some(&m.cid) == cid.as_ref()))
}

async fn load_products(s: FacSig) {
    let Some(cid) = s.cid.try_get_value() else { return };
    match cloud::list_products(&cid).await {
        Ok(list) => {
            if !alive(s) {
                return;
            }
            let all: Vec<Prod> = list
                .iter()
                .map(|p| Prod {
                    id: get(p, "id").as_string().unwrap_or_default(),
                    name: get(p, "name").as_string().unwrap_or_default(),
                    cost_cents: get(p, "costCents").as_f64().unwrap_or(0.0),
                    price_cents: get(p, "priceCents").as_f64().unwrap_or(0.0),
                    stock: get(p, "stock").as_f64().unwrap_or(0.0),
                    paused: get(p, "paused").is_truthy(),
                })
                .collect();
            // 돌고 있는 상품이 있으면 그것을, 아니면 고른 상품을 그대로 두고, 없으면 멈추지 않은 첫 상품
            let running = s.lab.with_untracked(|l| l.as_ref().and_then(|l| l.run.as_ref()).map(|r| r.pid.clone()));
            let keep = running.or_else(|| s.prod.with_untracked(|p| p.as_ref().and_then(|p| p.as_ref()).map(|p| p.id.clone())));
            let first = keep
                .and_then(|id| all.iter().find(|p| p.id == id).cloned())
                .or_else(|| all.iter().find(|p| !p.paused).cloned())
                .or_else(|| all.first().cloned());
            s.note.set(match &first {
                Some(_) => "만들 상품을 고르세요 · 만든 음식은 한 개씩 바로 재고에 들어가요".into(),
                None => "아직 파는 상품이 없어요. 사장님이 상품을 정하면 여기서 만들어요".into(),
            });
            s.all.set(all);
            s.prod.set(Some(first));
        }
        Err(e) => {
            cloud::warn(&format!("만들 상품을 불러오지 못했어요: {e}"));
            if alive(s) {
                s.note.set(format!("만들 상품을 불러오지 못했어요. {}", cloud::why(&e.code)));
            }
        }
    }
}

async fn load_lab(s: FacSig) {
    let Some(cid) = s.cid.try_get_value() else { return };
    match cloud::lab_get(&cid).await {
        Ok(l) => {
            if !alive(s) {
                return;
            }
            let p = get(&l, "prod");
            let left = get(&p, "left").as_f64().filter(|v| v.fract() == 0.0);
            let at = get(&p, "at");
            let run = match left {
                Some(left) if !at.is_null() && !at.is_undefined() => Some(Run {
                    pid: get(&p, "pid").as_string().unwrap_or_default(),
                    left,
                    at: ts_ms(&at),
                    // 예전에 시작한 공장은 처음 개수를 몰라요: 남은 개수를 처음 개수로 봐요
                    total: get(&p, "total").as_f64().filter(|t| *t >= left).unwrap_or(left),
                }),
                _ => None,
            };
            // 돌고 있는 상품을 고른 상품으로 맞춰요
            if let Some(r) = &run {
                let pick = s.all.with_untracked(|a| a.iter().find(|p| p.id == r.pid).cloned());
                if let Some(pr) = pick {
                    s.prod.set(Some(Some(pr)));
                }
            }
            s.lab.set(Some(FacLab { plv: get(&l, "plv").as_f64().unwrap_or(1.0), cash: get(&l, "cash").as_f64().unwrap_or(0.0), run }));
        }
        Err(e) => cloud::warn(&format!("연구실 숫자를 불러오지 못했어요: {e}")),
    }
}

/// 한 개가 나올 때마다 (서버 기준으로 나왔으면) 정산해요. 4초 안에 또 하지는 않아요 (서버를 아껴요)
fn maybe_settle(s: FacSig) {
    if !alive(s) || SETTLING.get() || js_sys::Date::now() - LAST_SETTLE.get() < 4000.0 {
        return;
    }
    let ready = s.lab.with_untracked(|l| l.as_ref().and_then(|l| l.run.as_ref().map(|r| due(r, l.plv, js_sys::Date::now(), 3000.0) >= 1.0)).unwrap_or(false));
    if ready {
        settle(s);
    }
}

/// 정산: 지금까지 만든 만큼 재고가 늘고 재료값이 나가요 (돌고 있을 때만 불러요)
fn settle(s: FacSig) {
    if !alive(s) || !s.lab.get_untracked().is_some_and(|l| l.run.is_some()) || SETTLING.replace(true) {
        return;
    }
    let Some(cid) = s.cid.try_get_value() else {
        SETTLING.set(false);
        return;
    };
    let pid = s.lab.with_untracked(|l| l.as_ref().and_then(|l| l.run.as_ref()).map(|r| r.pid.clone())).unwrap_or_default();
    spawn_local(async move {
        let r = cloud::lab_settle_factory(&cid).await;
        LAST_SETTLE.set(js_sys::Date::now());
        SETTLING.set(false);
        match r {
            Ok(units) => {
                if !alive(s) || units < 1.0 {
                    return;
                }
                // 재고는 서버 숫자에 바로 더해서 보여요 (다시 읽지 않아요)
                let add = |p: &mut Prod| {
                    if p.id == pid {
                        p.stock += units;
                    }
                };
                s.all.update(|a| a.iter_mut().for_each(add));
                s.prod.update(|p| {
                    if let Some(Some(p)) = p.as_mut() {
                        add(p);
                    }
                });
                crate::biz::add_stock(&pid, units);
                load_lab(s).await;
                crate::biz::lab_refresh(); // 금고 숫자도 맞춰요
            }
            Err(e) => {
                cloud::warn(&format!("공장 정산을 하지 못했어요: {e}"));
                if alive(s) {
                    s.note.set(format!("만든 것을 정산하지 못했어요. {}. 곧 다시 해 볼게요", cloud::why(&e.code)));
                }
            }
        }
    });
}

/// 재고 숫자 옆에 "+1"이 떠올라요
fn plus_pop(n: f64) {
    let Some(el) = crate::dom::by_id("wk-stock") else { return };
    let Some(parent) = el.parent_element() else { return };
    if let Ok(sp) = crate::dom::doc().create_element("em") {
        sp.set_class_name("fac-plus");
        sp.set_text_content(Some(&format!("+{}", n as i64)));
        let _ = parent.append_child(&sp);
        set_timeout(move || sp.remove(), Duration::from_millis(1100));
    }
    let _ = el.class_list().remove_1("pop");
    let _ = el.offset_width();
    let _ = el.class_list().add_1("pop");
    crate::dom::vibrate(6);
}

#[component]
fn FactoryCard(s: FacSig) -> impl IntoView {
    let n_val = move || s.n.with(|v| v.parse::<f64>().unwrap_or(0.0));
    let plv = move || s.lab.with(|l| l.as_ref().map(|l| l.plv.max(1.0)).unwrap_or(1.0));
    let unit = move || s.prod.with(|p| p.as_ref().and_then(|p| p.as_ref()).map(|p| p.cost_cents).unwrap_or(0.0)) / 100.0;
    let price = move || s.prod.with(|p| p.as_ref().and_then(|p| p.as_ref()).map(|p| p.price_cents).unwrap_or(0.0)) / 100.0;
    let cash = move || s.lab.with(|l| l.as_ref().map(|l| l.cash).unwrap_or(0.0)) / 100.0;
    let run = move || s.lab.with(|l| l.as_ref().and_then(|l| l.run.clone()));
    // 지금까지 나온 개수 (정산 전 것 포함: 서버에 적히는 것보다 먼저 화면에 보여요)
    let due_now = move || {
        s.tick.track();
        let p = plv();
        run().map(|r| due(&r, p, js_sys::Date::now(), 0.0)).unwrap_or(0.0)
    };
    let made = move || run().map(|r| (r.total - r.left + due_now()).max(0.0)).unwrap_or(0.0);
    let remaining = move || run().map(|r| (r.left - due_now()).max(0.0)).unwrap_or(0.0);
    // 새 개수가 나올 때마다 재고 옆에 +1
    Effect::new(move |prev: Option<f64>| {
        let m = made();
        if let Some(p) = prev {
            if m > p && run().is_some() {
                plus_pop(m - p);
            }
        }
        m
    });
    let stock = move || match s.prod.get() {
        Some(Some(p)) => {
            // 돌고 있는 상품이면 정산 전에 나온 개수도 더해서 보여요
            let extra = if run().is_some_and(|r| r.pid == p.id) { due_now() } else { 0.0 };
            fmt_num(p.stock + extra, 0)
        }
        _ => "…".into(),
    };
    let running = move || run().is_some();
    // 시작하기 전: 개수를 정하면 시간·재료값·판매액·이익이 바로 나와요
    let afford = move || {
        let u = unit();
        if u <= 0.0 { f64::INFINITY } else { (cash() / u + 1e-9).floor() }
    };
    let est = move || {
        let n = n_val();
        if n < 1.0 {
            return view! { <p class="lab-sub">"몇 개 만들지 정해 주세요"</p> }.into_any();
        }
        let (u, p, pl) = (unit(), price(), plv());
        let cost = n * u;
        let rev = n * p;
        let secs = (n * unit_ms(pl) / 1000.0).ceil();
        let short = afford() < n;
        view! {
            <dl class="fac-sum">
                <div><dt>"걸리는 시간"</dt><dd>{format!("약 {} (한 개마다 {}초)", clock(secs), fmt_num(unit_ms(pl) / 1000.0, 1))}</dd></div>
                <div><dt>"재료값 합계"</dt><dd>{format!("{} 토큰", fmt_num(cost, 2))}</dd></div>
                <div><dt>"다 팔면"</dt><dd>{format!("{} 토큰", fmt_num(rev, 2))}</dd></div>
                <div class="fac-profit"><dt>"예상 이익"</dt><dd class:neg=move || { rev < cost }>{format!("{} 토큰", fmt_num(rev - cost, 2))}</dd></div>
                <div class:short=short><dt>"회사 금고"</dt><dd>{if short { format!("{} 토큰 · 재료값이 모자라요 ({}개까지만 만들 수 있어요)", fmt_num(cash(), 2), fmt_num(afford().min(1e9), 0)) } else { format!("{} 토큰 → 만들 때마다 재료값이 나가요", fmt_num(cash(), 2)) }}</dd></div>
            </dl>
        }
        .into_any()
    };
    // 돌아가는 중: 지금까지 몇 개, 다음 개수까지, 다 만들어지기까지
    let live = move || {
        s.tick.track();
        let Some(r) = run() else { return ().into_any() };
        let (pl, now) = (plv(), js_sys::Date::now());
        let um = unit_ms(pl);
        let m = made();
        let rem = remaining();
        let e = (now - r.at).max(0.0);
        let next = ((um - e % um) / 1000.0).ceil().max(1.0);
        let end_ms = r.at + r.left * um;
        let finish = ((end_ms - now) / 1000.0).ceil().max(0.0);
        let u = unit();
        let short = unit() > 0.0 && cash() < u && rem > 0.0;
        let pct = if r.total > 0.0 { (m / r.total * 100.0).clamp(0.0, 100.0) } else { 0.0 };
        view! {
            <div class="fac-live" role="status">
                <div class="fac-bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow=pct.round()><i style=format!("width:{pct}%")></i></div>
                <p class="fac-big">{format!("{} / {}개 만들었어요", fmt_num(m, 0), fmt_num(r.total, 0))}</p>
                <p class="fac-line">{format!("남은 {}개 · 다음 1개까지 {}초", fmt_num(rem, 0), next as i64)}</p>
                <p class="fac-line">{format!("다 만들어지기까지 {} · {}에 끝나요", clock(finish), at_clock(end_ms))}</p>
                <p class="fac-line fac-dim">{format!("지금까지 재료값 {} 토큰 · 다 팔면 {} 토큰", fmt_num(m * u, 2), fmt_num(m * price(), 2))}</p>
                {short.then(|| view! { <p class="fac-warn">"재료값이 모자라 멈췄어요. 금고에 토큰을 넣어 주세요"</p> })}
            </div>
        }
        .into_any()
    };
    let start_off = move || {
        running()
            || !s.prod.with(|p| matches!(p, Some(Some(p)) if !p.paused))
            || n_val() < 1.0
            || s.lab.with(|l| l.is_none())
            || afford() < 1.0
    };
    let buy_off = move || { running() || s.lab.with(|l| l.as_ref().is_none_or(|l| l.cash < 50000.0)) };
    let start_label = move || {
        if running() {
            format!("공장 돌아가는 중 · {} / {}개", fmt_num(made(), 0), run().map(|r| fmt_num(r.total, 0)).unwrap_or_default())
        } else if afford() < 1.0 && s.lab.with(|l| l.is_some()) {
            "금고에 재료값이 없어요".to_string()
        } else {
            format!("{}개 만들기 시작", fmt_num(n_val(), 0))
        }
    };

    let on_input = move |ev: web_sys::Event| {
        let Some(inp) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) else { return };
        let d = digits(&inp.value());
        if inp.value() != d {
            inp.set_value(&d);
        }
        s.n.set(d);
    };
    let start = move |_| {
        let n = n_val() as u32;
        let pid = s.prod.with_untracked(|p| p.as_ref().and_then(|p| p.as_ref()).filter(|p| !p.paused).map(|p| p.id.clone()));
        let Some(pid) = pid else {
            ctx().toast("먼저 팔 상품을 정해 주세요");
            return;
        };
        let Some(cid) = s.cid.try_get_value() else { return };
        spawn_local(async move {
            match cloud::lab_start_factory(&cid, &pid, n).await {
                Ok(()) => ctx().toast(&format!("공장을 돌리기 시작했어요 · {n}개, 한 개씩 바로 재고에 들어가요")),
                Err(e) => ctx().toast(&fac_msg(&e)),
            }
            load_lab(s).await;
        });
    };
    let buy = move |_| {
        let Some(cid) = s.cid.try_get_value() else { return };
        spawn_local(async move {
            match cloud::lab_buy_factory(&cid).await {
                Ok(plv) => ctx().toast(&format!("공장이 {}대가 됐어요", plv as i64)),
                Err(e) => ctx().toast(&fac_msg(&e)),
            }
            load_lab(s).await;
            crate::biz::lab_refresh();
        });
    };

    view! {
        <div class="card wk-work wk-food fac" class:is-running=running>
            <p class="wk-head">"식품 공장"</p>
            <p class="wk-note" id="wk-make">{move || s.note.get()}</p>
            <div class="wk-sorts fac-pick" role="group" aria-label="만들 상품">
                <For each=move || s.all.get() key=|p| (p.id.clone(), p.name.clone(), p.stock as i64) let:p>
                    {
                        let id = p.id.clone();
                        let id2 = p.id.clone();
                        let pp = p.clone();
                        view! {
                            <button type="button" class="gg wk-pill" data-pid=id.clone()
                                aria-pressed=move || s.prod.with(|x| x.as_ref().and_then(|x| x.as_ref()).is_some_and(|x| x.id == id2)).to_string()
                                disabled=move || { running() || pp.paused }
                                on:click=move |_| s.prod.set(Some(Some(p.clone())))>
                                {pp.name.clone()}
                            </button>
                        }
                    }
                </For>
            </div>
            <div class="wk-nums">
                <span class="fac-stock">"재고 "<b id="wk-stock">{stock}</b>"개"</span>
                <span>"공장 "<b id="fac-count">{move || if s.lab.with(|l| l.is_some()) { (plv() as i64).to_string() } else { "…".into() }}</b>"대"</span>
                <span>"개당 재료값 "<b id="wk-unit">{move || fmt_num(unit(), 2)}</b>" 토큰"</span>
            </div>
            <label class="lab-field" class:hide=running>
                <span>"만들 개수"</span>
                <input id="fac-n" class="lab-input" type="text" inputmode="numeric" autocomplete="off" maxlength="6" aria-label="만들 개수" prop:value=move || s.n.get() on:input=on_input/>
            </label>
            <div id="fac-est" class="fac-est" class:hide=running>{est}</div>
            <button class="btn primary fac-go" type="button" id="fac-start" class:running=running disabled=start_off on:click=start>
                <span class="fac-spin" aria-hidden="true"></span>{start_label}
            </button>
            <div id="fac-left" class="fac-leftbox">{live}</div>
            <button class="btn" type="button" id="fac-buy" disabled=buy_off on:click=buy>"공장 추가 · 500 토큰"</button>
        </div>
    }
}
