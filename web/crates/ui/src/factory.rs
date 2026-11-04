//! 식품 공장 카드 (사장만, 사업 탭 안). 예전 app/work.js의 factoryInto를 옮겼어요.
//! 공장 한 대는 한 개를 60초에 만들어요. 만든 만큼 재료값이 나가고 재고에 바로 들어가요.

use std::any::Any;
use std::cell::RefCell;
use std::time::Duration;

use cloud::{fmt_num, get, ts_ms, CloudError};
use leptos::mount::mount_to;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

use crate::work::{count_pop, ctx};

const FAC_MS: f64 = 60000.0;

/// 만드는 상품 (회사 첫 상품)
#[derive(Clone, PartialEq)]
struct Prod {
    id: String,
    name: String,
    cost_cents: f64,
    stock: f64,
    paused: bool,
}

/// 연구실에서 공장에 필요한 숫자
#[derive(Clone, Copy, PartialEq)]
struct FacLab {
    plv: f64,
    cash: f64,
    /// 돌고 있으면 (남은 개수, 시작 시각 ms)
    run: Option<(f64, f64)>,
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
}

fn clock(secs: f64) -> String {
    let s = secs as i64;
    if s >= 60 {
        format!("{}분 {}초", s / 60, s % 60)
    } else {
        format!("{s}초")
    }
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
        _ if !e.message.is_empty() => e.message.clone(),
        _ => "잠시 뒤 다시 해 주세요".into(),
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
    // 1초마다 남은 시간을 다시 그려요. 카드가 화면에서 빠지면 멈춰요
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
        },
        Duration::from_millis(1000),
    ) {
        timers.push(t);
    }
    if let Ok(t) = set_interval_with_handle(move || settle(s), Duration::from_millis(5000)) {
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
                    stock: get(p, "stock").as_f64().unwrap_or(0.0),
                    paused: get(p, "paused").is_truthy(),
                })
                .collect();
            // 고른 상품을 그대로 두고, 없으면 멈추지 않은 첫 상품
            let keep = s.prod.with_untracked(|p| p.as_ref().and_then(|p| p.as_ref()).map(|p| p.id.clone()));
            let first = keep
                .and_then(|id| all.iter().find(|p| p.id == id).cloned())
                .or_else(|| all.iter().find(|p| !p.paused).cloned())
                .or_else(|| all.first().cloned());
            s.note.set(match &first {
                Some(_) => "만들 상품을 고르세요 · 만든 음식은 상점에 진열돼요".into(),
                None => "아직 파는 상품이 없어요. 사장님이 상품을 정하면 여기서 만들어요".into(),
            });
            s.all.set(all);
            s.prod.set(Some(first));
        }
        Err(e) => {
            cloud::warn(&format!("만들 상품을 불러오지 못했어요: {e}"));
            if alive(s) {
                s.note.set("만들 상품을 불러오지 못했어요".into());
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
                Some(left) if !at.is_null() && !at.is_undefined() => Some((left, ts_ms(&at))),
                _ => None,
            };
            s.lab.set(Some(FacLab {
                plv: get(&l, "plv").as_f64().unwrap_or(1.0),
                cash: get(&l, "cash").as_f64().unwrap_or(0.0),
                run,
            }));
        }
        Err(e) => cloud::warn(&format!("연구실 숫자를 불러오지 못했어요: {e}")),
    }
}

/// 정산: 지금까지 만든 만큼 재고가 늘고 재료값이 나가요 (돌고 있을 때만 불러요)
fn settle(s: FacSig) {
    if !alive(s) || !s.lab.get_untracked().is_some_and(|l| l.run.is_some()) {
        return;
    }
    let Some(cid) = s.cid.try_get_value() else { return };
    spawn_local(async move {
        match cloud::lab_settle_factory(&cid).await {
            Ok(units) => {
                if !alive(s) || units < 1.0 {
                    return;
                }
                count_pop(&format!("{}개 만들었어요", units as i64));
                load_lab(s).await;
                load_products(s).await;
            }
            Err(e) => cloud::warn(&format!("공장 정산을 하지 못했어요: {e}")),
        }
    });
}

#[component]
fn FactoryCard(s: FacSig) -> impl IntoView {
    let n_val = move || s.n.with(|v| v.parse::<f64>().unwrap_or(0.0));
    let plv = move || s.lab.with(|l| l.map(|l| l.plv.max(1.0)).unwrap_or(1.0));
    let unit = move || s.prod.with(|p| p.as_ref().and_then(|p| p.as_ref()).map(|p| p.cost_cents).unwrap_or(0.0)) / 100.0;
    let stock = move || match s.prod.get() {
        Some(Some(p)) => fmt_num(p.stock, 0),
        _ => "…".into(),
    };
    let est = move || {
        let n = n_val();
        if n >= 1.0 {
            let gap = FAC_MS / plv();
            format!(
                "{}개 · 공장 {}대면 약 {} · 재료값 {} 토큰",
                n as i64,
                plv() as i64,
                clock((n * gap / 1000.0).ceil()),
                fmt_num(n * unit(), 2)
            )
        } else {
            "몇 개 만들지 정해 주세요".into()
        }
    };
    let left = move || {
        s.tick.track();
        let Some(l) = s.lab.get() else { return String::new() };
        let Some((left, at)) = l.run else { return String::new() };
        let gap = FAC_MS / plv();
        let ms = (js_sys::Date::now() - at).max(0.0);
        let next = ((gap - ms % gap) / 1000.0).ceil();
        let cost = s.prod.with(|p| p.as_ref().and_then(|p| p.as_ref()).map(|p| p.cost_cents));
        if cost.is_some_and(|c| l.cash < c) {
            "재료값이 모자라 멈췄어요. 금고에 토큰을 넣어 주세요".into()
        } else {
            format!("만드는 중 · 남은 {}개 · 다음 한 개까지 {}초", left as i64, next as i64)
        }
    };
    let running = move || s.lab.with(|l| l.is_some_and(|l| l.run.is_some()));
    let start_off = move || { running() || !s.prod.with(|p| matches!(p, Some(Some(_)))) || n_val() < 1.0 || s.lab.with(|l| l.is_none()) };
    let buy_off = move || { running() || s.lab.with(|l| l.is_none_or(|l| l.cash < 50000.0)) };

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
                Ok(()) => ctx().toast(&format!("{n}개 만들기를 시작했어요. 한 개씩 재고에 들어가요")),
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
        });
    };

    view! {
        <div class="card wk-work wk-food">
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
                <span>"재고 "<b id="wk-stock">{stock}</b>"개"</span>
                <span>"공장 "<b id="fac-count">{move || if s.lab.with(|l| l.is_some()) { (plv() as i64).to_string() } else { "…".into() }}</b>"대"</span>
                <span>"개당 재료값 "<b id="wk-unit">{move || fmt_num(unit(), 2)}</b>" 토큰"</span>
            </div>
            <label class="lab-field">
                <span>"만들 개수"</span>
                <input id="fac-n" class="lab-input" type="text" inputmode="numeric" autocomplete="off" maxlength="6" aria-label="만들 개수" prop:value=move || s.n.get() on:input=on_input/>
            </label>
            <p class="lab-sub" id="fac-est">{est}</p>
            <button class="btn primary" type="button" id="fac-start" disabled=start_off on:click=start>"돌리기"</button>
            <p class="lab-sub" id="fac-left">{left}</p>
            <button class="btn" type="button" id="fac-buy" disabled=buy_off on:click=buy>"공장 추가 · 500 토큰"</button>
        </div>
    }
}
