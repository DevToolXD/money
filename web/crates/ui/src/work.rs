//! 일 탭: 사장이거나 직원이면 회사 일을 해요. 아니면 회사를 종류별로 보고, 이름으로 찾고, 골라서 지원해요.
//! 식품은 성능 올리기(일하기), 인공지능은 가짜 검색, 원자력은 핵연료 배합, 컴퓨터는 부품 검사예요.
//! 예전 `app/work.js`와 같은 id·class를 써요. 식품 공장 카드는 `factory.rs`예요 (사업 탭에 붙어요).

use std::cell::{Cell, RefCell};
use std::time::Duration;

use cloud::{fmt_num, Company};
use leptos::html;
use leptos::mount::mount_to;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::ctx::{set_vitals, Ctx};

const FRESH_MS: f64 = 30000.0; // 이 시간 안에 같은 회사 정보로 읽었으면 다시 읽지 않아요 (서버를 아껴요)
const MIX_MAX: u32 = 8; // 한 번에 넣을 수 있는 방울 수
const STIR_MS: f64 = 1600.0; // 꾹 누르고 있어야 하는 시간
const REAGENTS: [(&str, [u8; 3]); 3] = [("분말", [196, 58, 58]), ("안정제", [244, 196, 60]), ("촉매", [52, 110, 214])];
/// 연구실 숫자 이름 (서버 칸 이름과 같아요)
const LAB_KEYS: [&str; 6] = ["nperf", "cap", "spd", "pow", "aiData", "fperf"];
const HITS: [&str; 4] = ["데이터 묶음 1", "데이터 묶음 2", "데이터 묶음 3", "데이터 묶음 4"];
const SEARCH_D: &str = r#"<circle cx="11" cy="11" r="6.5"/><path d="M16 16l4 4"/>"#;
const CHECK_D: &str = r#"<path d="M5 12.5l4.5 4.5L19 7.5"/>"#;

fn kind_short(k: &str) -> &'static str {
    match k {
        "pc" => "컴퓨터",
        "ai" => "인공지능",
        "nuclear" => "원자력 발전",
        _ => "식품",
    }
}
fn kind_full(k: &str) -> &'static str {
    match k {
        "pc" => "컴퓨터 회사",
        "ai" => "인공지능 회사",
        "nuclear" => "원자력 발전 회사",
        _ => "식품 회사",
    }
}
/// 임시 시가총액: 창업 자본 10 토큰 + 직원 1명당 2 토큰
fn cap_of(c: &Company) -> f64 {
    10.0 + 2.0 * c.staff
}
fn date_of(ms: f64) -> String {
    cloud::date_ko(if ms > 0.0 { ms } else { js_sys::Date::now() })
}
fn fmt_n(v: f64) -> String {
    fmt_num(v, 3)
}
fn rgb_css(c: [u8; 3]) -> String {
    format!("rgb({}, {}, {})", c[0], c[1], c[2])
}
fn svg_icon(d: &str, size: u32) -> String {
    format!(r#"<svg viewBox="0 0 24 24" width="{size}" height="{size}" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{d}</svg>"#)
}
fn rand() -> f64 {
    js_sys::Math::random()
}
fn reduce_motion() -> bool {
    web_sys::window()
        .and_then(|w| w.match_media("(prefers-reduced-motion: reduce)").ok().flatten())
        .map(|m| m.matches())
        .unwrap_or(false)
}

/* ---------- 상태 ---------- */

#[derive(Clone)]
enum View {
    Idle,
    Loading,
    Failed(&'static str),
    Market,
    Mine { owner: bool, co: Company, job_at: f64, extra: Option<Company> },
}

/// 화면에 보이는 연구실 숫자 (서버 숫자 + 아직 안 보낸 것)
#[derive(Clone, Copy, Default, PartialEq)]
struct Lab([f64; 6]);

#[derive(Clone, Copy)]
struct Sig {
    view: RwSignal<View, LocalStorage>,
    kind_sel: RwSignal<&'static str>,
    sort_sel: RwSignal<&'static str>,
    term: RwSignal<String>,
    companies: RwSignal<Option<Vec<Company>>, LocalStorage>,
    list_failed: RwSignal<bool>,
    list_loading: RwSignal<bool>,
    lab: RwSignal<Option<Lab>>,
}

thread_local! {
    static CTX: RefCell<Ctx> = RefCell::new(Ctx::default());
    static SIG: Cell<Option<Sig>> = const { Cell::new(None) };
    static MY_JOB: Cell<bool> = const { Cell::new(false) };
    static LOADED_AT: Cell<f64> = const { Cell::new(0.0) };
    static PAINTED_CO: RefCell<JsValue> = RefCell::new(JsValue::UNDEFINED);
    static LOAD_SEQ: Cell<u32> = const { Cell::new(0) };
    static MARKET_SEQ: Cell<u32> = const { Cell::new(0) };
    static SEARCH_SEQ: Cell<u32> = const { Cell::new(0) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
    static LAB_CID: RefCell<Option<String>> = const { RefCell::new(None) };
    static LAB_PEND: Cell<[f64; 6]> = const { Cell::new([0.0; 6]) };
    static LAB_TIMER: Cell<bool> = const { Cell::new(false) };
    /// 화면을 바꿀 때마다 올라가요. 돌던 놀이(검사판·배합)는 자기 번호가 아니면 멈춰요
    static GAME: Cell<u32> = const { Cell::new(0) };
}

pub(crate) fn ctx() -> Ctx {
    CTX.with(|c| c.borrow().clone())
}
fn sig() -> Option<Sig> {
    SIG.get()
}
/// 화면 안에서 id로 찾기
fn by_id(id: &str) -> Option<web_sys::HtmlElement> {
    web_sys::window()?.document()?.get_element_by_id(id)?.dyn_into().ok()
}
/// 글자를 바꾸고 "통" 튀는 움직임을 다시 시작해요
pub(crate) fn count_pop(text: &str) {
    if let Some(c) = by_id("wk-count") {
        c.set_text_content(Some(text));
        let _ = c.class_list().remove_1("pop");
        let _ = c.offset_width();
        let _ = c.class_list().add_1("pop");
    }
}

pub fn init(c: Ctx) {
    CTX.with(|x| *x.borrow_mut() = c);
    let Some(root) = by_id("work-root") else { return };
    root.set_inner_html("");
    mount_to(root, || {
        let s = Sig {
            view: RwSignal::new_local(View::Idle),
            kind_sel: RwSignal::new("all"),
            sort_sel: RwSignal::new("pop"),
            term: RwSignal::new(String::new()),
            companies: RwSignal::new_local(None),
            list_failed: RwSignal::new(false),
            list_loading: RwSignal::new(false),
            lab: RwSignal::new(None),
        };
        SIG.set(Some(s));
        view! { <Work s/> }
    })
    .forget();
}

/* ---------- 읽기 ---------- */

/// 일 탭을 다시 그려요. force가 아니면 같은 회사 정보로 30초 안에 읽은 것은 다시 읽지 않아요
pub fn refresh(force: bool) {
    let Some(s) = sig() else { return };
    if !cloud::signed_in() {
        s.view.set(View::Idle);
        LOADED_AT.set(0.0);
        return;
    }
    let co = ctx().company();
    let same = PAINTED_CO.with(|p| js_sys::Object::is(&p.borrow(), &co));
    let failed = matches!(s.view.get_untracked(), View::Failed(_));
    if !force && !failed && same && js_sys::Date::now() - LOADED_AT.get() < FRESH_MS {
        return;
    }
    if co.is_undefined() {
        s.view.set(View::Loading);
        return;
    }
    let seq = LOAD_SEQ.get() + 1;
    LOAD_SEQ.set(seq);
    if !matches!(s.view.get_untracked(), View::Mine { .. } | View::Market) {
        s.view.set(View::Loading);
    }
    spawn_local(async move {
        let got = async {
            let job = cloud::load_my_job().await?;
            let employer = match &job {
                Some(j) => cloud::get_company(&cloud::get(j, "cid").as_string().unwrap_or_default()).await?,
                None => None,
            };
            Ok::<_, cloud::CloudError>((job, employer))
        }
        .await;
        if LOAD_SEQ.get() != seq {
            return;
        }
        match got {
            Ok((job, employer)) => {
                MY_JOB.set(job.is_some());
                if job.is_some() && employer.is_none() {
                    return s.view.set(View::Failed("다니는 회사를 찾지 못했어요"));
                }
                LOADED_AT.set(js_sys::Date::now());
                PAINTED_CO.with(|p| *p.borrow_mut() = co.clone());
                let mine = if co.is_null() { None } else { Some(Company::from_js(&co)) };
                if mine.is_some() || employer.is_some() {
                    let job_at = job.as_ref().map(|j| cloud::ts_ms(&cloud::get(j, "at"))).unwrap_or(0.0);
                    paint_mine(s, mine, job_at, employer);
                } else {
                    paint_market(s);
                }
            }
            Err(_) => s.view.set(View::Failed("일 정보를 불러오지 못했어요")),
        }
    });
}

fn paint_mine(s: Sig, co: Option<Company>, job_at: f64, employer: Option<Company>) {
    GAME.set(GAME.get() + 1);
    flush_lab(); // 이전 회사의 숫자를 먼저 보내요
    let owner = co.is_some();
    let (primary, extra) = match co {
        Some(c) => (c, employer),
        None => (employer.expect("다니는 회사"), None),
    };
    LAB_CID.with(|l| *l.borrow_mut() = Some(primary.id.clone()));
    s.lab.set(None);
    let cid = primary.id.clone();
    s.view.set(View::Mine { owner, co: primary, job_at, extra });
    load_lab(s, cid);
}

fn paint_market(s: Sig) {
    GAME.set(GAME.get() + 1);
    s.view.set(View::Market);
    load_market(s);
}

fn load_market(s: Sig) {
    let seq = MARKET_SEQ.get() + 1;
    MARKET_SEQ.set(seq);
    s.list_loading.set(true);
    let term = s.term.get_untracked();
    spawn_local(async move {
        let got = cloud::list_companies(&term).await;
        if MARKET_SEQ.get() != seq {
            return;
        }
        match got {
            Ok(list) => {
                s.list_failed.set(false);
                s.companies.set(Some(list));
            }
            Err(_) => s.list_failed.set(true),
        }
        s.list_loading.set(false);
    });
}

fn load_lab(s: Sig, cid: String) {
    spawn_local(async move {
        match cloud::lab_get(&cid).await {
            Ok(l) => {
                if LAB_CID.with(|x| x.borrow().as_deref() != Some(cid.as_str())) {
                    return;
                }
                let pend = LAB_PEND.get();
                let mut v = [0.0; 6];
                for (i, k) in LAB_KEYS.iter().enumerate() {
                    v[i] = cloud::get(&l, k).as_f64().unwrap_or(0.0) + pend[i];
                }
                s.lab.set(Some(Lab(v)));
            }
            Err(e) => cloud::warn(&format!("연구실 숫자를 불러오지 못했어요: {e}")),
        }
    });
}

/* ---------- 연구실 숫자: 누른 것은 모아서 2.5초에 한 번 서버에 보내요 ---------- */

fn queue_lab(k: usize, v: f64) {
    let mut p = LAB_PEND.get();
    p[k] += v;
    LAB_PEND.set(p);
    if let Some(s) = sig() {
        s.lab.update(|l| {
            if let Some(l) = l {
                l.0[k] += v;
            }
        });
    }
    if !LAB_TIMER.get() {
        LAB_TIMER.set(true);
        set_timeout(flush_lab, Duration::from_millis(2500));
    }
}

fn flush_lab() {
    LAB_TIMER.set(false);
    let d = LAB_PEND.get();
    let Some(cid) = LAB_CID.with(|l| l.borrow().clone()) else { return };
    if d.iter().all(|x| *x == 0.0) {
        return;
    }
    LAB_PEND.set([0.0; 6]);
    // 트랜잭션이 다시 부를 수 있어서, 같은 값을 여러 번 돌려줄 수 있는 함수로 넘겨요
    let f = Closure::<dyn FnMut(JsValue) -> JsValue>::new(move |l: JsValue| {
        let next = js_sys::Object::assign(&js_sys::Object::new(), l.unchecked_ref());
        for (i, k) in LAB_KEYS.iter().enumerate() {
            let cur = cloud::get(&l, k).as_f64().unwrap_or(0.0);
            let _ = js_sys::Reflect::set(&next, &JsValue::from_str(k), &JsValue::from_f64(cur + d[i]));
        }
        next.into()
    })
    .into_js_value();
    spawn_local(async move {
        if let Err(e) = cloud::lab_change(&cid, &f).await {
            cloud::warn(&format!("연구실 숫자를 올리지 못했어요: {e}"));
        }
    });
}

/* ---------- 지원: 회사 창 (아래에서 올라와요) ---------- */

thread_local! {
    static SHEET: RefCell<Option<(web_sys::Element, web_sys::Element, Box<dyn std::any::Any>)>> = const { RefCell::new(None) };
}

fn close_sheet() {
    let Some((dim, bx, handle)) = SHEET.with(|s| s.borrow_mut().take()) else { return };
    let _ = dim.class_list().remove_1("on");
    let _ = bx.class_list().remove_1("on");
    set_timeout(
        move || {
            drop(handle);
            dim.remove();
            bx.remove();
        },
        Duration::from_millis(420),
    );
}

fn open_detail(c: Company) {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return };
    let Some(app) = doc.query_selector(".app").ok().flatten() else { return };
    close_sheet();
    let (Ok(dim), Ok(bx)) = (doc.create_element("div"), doc.create_element("div")) else { return };
    dim.set_class_name("wk-dim");
    bx.set_class_name("wk-sheet");
    let _ = app.append_with_node_2(&dim, &bx);
    let on_dim = Closure::<dyn FnMut()>::new(close_sheet).into_js_value();
    let _ = dim.add_event_listener_with_callback("click", on_dim.unchecked_ref());
    let handle = mount_to(bx.clone().unchecked_into(), move || view! { <Sheet c/> });
    // 시작 자리를 먼저 잡아야 올라오는 움직임이 나와요
    let _ = bx.unchecked_ref::<web_sys::HtmlElement>().offset_height();
    let _ = dim.class_list().add_1("on");
    let _ = bx.class_list().add_1("on");
    SHEET.with(|s| *s.borrow_mut() = Some((dim, bx, Box::new(handle))));
}

#[component]
fn Sheet(c: Company) -> impl IntoView {
    let me = cloud::cloud().map(|o| cloud::get(&o, "uid")).and_then(|v| v.as_string()).unwrap_or_default();
    let mine = c.owner == me;
    let has_job = MY_JOB.get();
    let why = if mine {
        "내 회사에는 지원할 수 없어요".to_string()
    } else if has_job {
        "이미 다니는 회사가 있어요. 먼저 그만둬야 해요".to_string()
    } else if c.open < 1.0 {
        "지금은 모집하지 않아요".to_string()
    } else {
        format!("지원하면 바로 입사해요. {} 일을 하게 돼요.", kind_short(&c.kind))
    };
    let blocked = mine || has_job || c.open < 1.0;
    let busy = RwSignal::new(false);
    let mark = ctx().mark(&c.name, &c.logo, "avatar contact wk-av big");
    let co = c.clone();
    let apply = move |_| {
        if BUSY.get() {
            return;
        }
        let co = co.clone();
        spawn_local(async move {
            let yes = ctx().ask(&format!("{}에 지원할까요?", co.name), &format!("{}에 들어가면 여기서 일하게 돼요.", kind_full(&co.kind)), "지원하기", "취소").await;
            if !yes {
                return;
            }
            BUSY.set(true);
            busy.set(true);
            match cloud::apply_job(&co.id, &co.kind).await {
                Ok(()) => {
                    close_sheet();
                    ctx().toast(&format!("{}에 입사했어요", co.name));
                    refresh(true);
                }
                Err(e) => {
                    busy.set(false);
                    ctx().toast(if e.code == "permission-denied" { "지원하지 못했어요. 자리가 없거나 이미 다니는 곳이 있어요" } else { "지원하지 못했어요. 잠시 뒤 다시 해 주세요" });
                }
            }
            BUSY.set(false);
        });
    };
    view! {
        <div class="grabber"></div>
        <div class="wk-sheet-head">
            <span style="display:contents" inner_html=mark></span>
            <div><h2>{c.name.clone()}</h2><p>{kind_full(&c.kind)}</p></div>
        </div>
        <Stats c=c.clone()/>
        <p class="wk-why">{why}</p>
        <button type="button" class="btn primary wk-apply" class:busy=move || busy.get() disabled=blocked on:click=apply>"지원하기"</button>
        <button type="button" class="gg wk-cancel" data-close on:click=move |_| close_sheet()>"닫기"</button>
    }
}

/* ---------- 화면 ---------- */

#[component]
fn Stats(c: Company) -> impl IntoView {
    view! {
        <div class="wk-stats">
            <div class="gg wk-stat"><b>{format!("{}명", c.staff)}</b><span>"직원"</span></div>
            <div class="gg wk-stat"><b>{format!("{}명", c.open)}</b><span>"모집 중"</span></div>
            <div class="gg wk-stat"><b>{fmt_n(cap_of(&c))}</b><span>"시총 토큰"</span></div>
        </div>
    }
}

#[component]
fn Work(s: Sig) -> impl IntoView {
    move || match s.view.get() {
        View::Idle => ().into_any(),
        View::Loading => view! { <p class="biz-loading">"불러오는 중이에요"</p> }.into_any(),
        View::Failed(msg) => view! {
            <div class="card wk-work">
                <p class="wk-head">{msg}</p>
                <button type="button" class="btn" id="wk-retry" on:click=move |_| refresh(true)>"다시 해 보기"</button>
            </div>
        }
        .into_any(),
        View::Market => view! { <Market s/> }.into_any(),
        View::Mine { owner, co, job_at, extra } => view! { <Mine s owner co job_at extra/> }.into_any(),
    }
}

#[component]
fn Market(s: Sig) -> impl IntoView {
    const KINDS: [(&str, &str); 5] = [("all", "전체"), ("food", "식품"), ("pc", "컴퓨터"), ("ai", "인공지능"), ("nuclear", "원자력 발전")];
    const SORTS: [(&str, &str); 5] = [("pop", "인기순"), ("cap", "시총순"), ("few", "남은 자리 적은 순"), ("many", "남은 자리 많은 순"), ("new", "최근 창업순")];
    let rows = move || {
        let kind = s.kind_sel.get();
        let sort = s.sort_sel.get();
        let mut list: Vec<Company> = s.companies.get().unwrap_or_default().into_iter().filter(|c| kind == "all" || c.kind == kind).collect();
        let by_name = |a: &Company, b: &Company| a.name.cmp(&b.name);
        list.sort_by(|a, b| match sort {
            "cap" => cap_of(b).total_cmp(&cap_of(a)).then(by_name(a, b)),
            "few" => a.open.total_cmp(&b.open).then(by_name(a, b)),
            "many" => b.open.total_cmp(&a.open).then(by_name(a, b)),
            "new" => b.at.total_cmp(&a.at),
            _ => b.staff.total_cmp(&a.staff).then(by_name(a, b)),
        });
        list
    };
    let on_search = move |ev: leptos::ev::Event| {
        s.term.set(event_target_value(&ev));
        let my = SEARCH_SEQ.get() + 1;
        SEARCH_SEQ.set(my);
        // 글자를 치는 동안에는 기다렸다가 한 번만 찾아요
        set_timeout(
            move || {
                if SEARCH_SEQ.get() == my {
                    load_market(s);
                }
            },
            Duration::from_millis(320),
        );
    };
    view! {
        <p class="lead">"회사를 골라 지원해요"</p>
        <div class="wk-sorts wk-kinds" role="group" aria-label="회사 종류">
            {KINDS.iter().map(|(k, l)| {
                let k: &'static str = k;
                view! { <button type="button" class="gg wk-pill" aria-pressed=move || (s.kind_sel.get() == k).to_string() data-k=k on:click=move |_| s.kind_sel.set(k)>{*l}</button> }
            }).collect_view()}
        </div>
        <label class="gg wk-search">
            <span style="display:contents" inner_html=svg_icon(SEARCH_D, 18)></span>
            <input id="wk-q" type="search" placeholder="회사 이름으로 찾아요" autocomplete="off" maxlength="20" prop:value=move || s.term.get_untracked() aria-label="회사 이름" on:input=on_search/>
        </label>
        <div class="wk-sorts" role="group" aria-label="정렬">
            {SORTS.iter().map(|(k, l)| {
                let k: &'static str = k;
                view! { <button type="button" class="gg wk-pill" aria-pressed=move || (s.sort_sel.get() == k).to_string() data-s=k on:click=move |_| s.sort_sel.set(k)>{*l}</button> }
            }).collect_view()}
        </div>
        <p class="wk-meta" id="wk-meta">
            {move || {
                if s.companies.with(|c| c.is_none()) {
                    return String::new();
                }
                let n = rows().len();
                if n > 0 { format!("회사 {n}곳") } else { String::new() }
            }}
        </p>
        <div class="card wk-list" class:loading=move || s.list_loading.get() id="wk-list" aria-live="polite">
            {move || {
                if s.list_failed.get() {
                    return view! { <p class="wk-empty">"회사 목록을 불러오지 못했어요. 잠시 뒤 다시 해 주세요"</p> }.into_any();
                }
                if s.companies.with(|c| c.is_none()) {
                    return view! { <p class="wk-empty">"불러오는 중이에요"</p> }.into_any();
                }
                let list = rows();
                if list.is_empty() {
                    let msg = if s.term.get_untracked().trim().is_empty() { "아직 회사가 없어요" } else { "이름이 맞는 회사가 없어요" };
                    return view! { <p class="wk-empty">{msg}</p> }.into_any();
                }
                list.into_iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let mark = ctx().mark(&c.name, &c.logo, "avatar contact wk-av");
                        let open = c.open;
                        let line = format!("{} · 직원 {} · 시총 {} 토큰", kind_full(&c.kind), format_args!("{}명", c.staff), fmt_n(cap_of(&c)));
                        let (id, name) = (c.id.clone(), c.name.clone());
                        view! {
                            <button type="button" class="wk-row" data-id=id style=format!("--i:{}", i.min(10)) on:click=move |_| open_detail(c.clone())>
                                <span style="display:contents" inner_html=mark></span>
                                <span class="wk-body"><b>{name}</b><span>{line}</span></span>
                                <span class="wk-tag" class:off=open < 1.0>{if open >= 1.0 { format!("자리 {open}") } else { "마감".into() }}</span>
                            </button>
                        }
                    })
                    .collect_view()
                    .into_any()
            }}
        </div>
    }
}

#[component]
fn Mine(s: Sig, owner: bool, co: Company, job_at: f64, extra: Option<Company>) -> impl IntoView {
    let mark = ctx().mark(&co.name, &co.logo, "avatar contact wk-av big");
    let since = if owner { format!("{}에 창업했어요", date_of(co.at)) } else { format!("{}에 입사했어요", date_of(job_at)) };
    let quit_co = if owner { extra } else { Some(co.clone()) };
    let card = match co.kind.as_str() {
        "ai" => view! { <AiCard s/> }.into_any(),
        "nuclear" => view! { <MixCard s/> }.into_any(),
        "pc" => view! { <BoardCard s/> }.into_any(),
        _ => view! { <FoodCard s cid=co.id.clone()/> }.into_any(),
    };
    view! {
        <div class="card wk-hero">
            <span style="display:contents" inner_html=mark></span>
            <div>
                <p class="wk-role">{format!("{} · {}", if owner { "사장" } else { "직원" }, kind_short(&co.kind))}</p>
                <h2>{co.name.clone()}</h2>
                <p class="wk-since">{since}</p>
            </div>
        </div>
        <Stats c=co.clone()/>
        {card}
        {quit_co.map(|q| {
            let (cid, name) = (q.id.clone(), q.name.clone());
            view! { <button type="button" class="gg wk-quit" data-quit=cid.clone() data-name=name.clone() on:click=move |_| quit(cid.clone(), name.clone())>{format!("{}에서 그만두기", q.name)}</button> }
        })}
    }
}

fn quit(cid: String, name: String) {
    if BUSY.get() {
        return;
    }
    spawn_local(async move {
        let c = ctx();
        if !c.ask(&format!("{name}을 그만둘까요?"), "그만두면 이 회사에서 일할 수 없어요. 다른 회사에 다시 지원할 수 있어요.", "그만두기", "취소").await {
            return;
        }
        if !c.ask("정말 하시겠습니까?", "그만두면 자리를 다시 얻어야 해요.", "네, 그만둘게요", "아니요").await {
            return;
        }
        BUSY.set(true);
        match cloud::quit_job(&cid).await {
            Ok(()) => {
                c.toast(&format!("{name}을 그만뒀어요"));
                refresh(true);
            }
            Err(_) => c.toast("그만두지 못했어요. 잠시 뒤 다시 해 주세요"),
        }
        BUSY.set(false);
    });
}

fn lab_text(s: Sig, k: usize) -> impl Fn() -> String {
    move || s.lab.get().map(|l| fmt_n(l.0[k])).unwrap_or_else(|| "…".into())
}

/* ---------- 식품: 일하기 (에너지 1 → 식품 성능 1) ---------- */

#[component]
fn FoodCard(s: Sig, cid: String) -> impl IntoView {
    let work = move |_| {
        let cid = cid.clone();
        spawn_local(async move {
            match cloud::staff_work(&cid, 1).await {
                Ok(v) => {
                    set_vitals(&v);
                    ctx().toast("일을 했어요 · 식품 성능 +1 · 에너지 -1");
                }
                Err(e) => ctx().toast(match e.code.as_str() {
                    "fainted" => "쓰러져 있어서 지금은 일할 수 없어요",
                    "tired" => "에너지가 모자라요. 먹고 쉬어 주세요",
                    _ if e.message.is_empty() => "잠시 뒤 다시 해 주세요",
                    _ => &e.message,
                }),
            }
            load_lab(s, cid);
        });
    };
    view! {
        <div class="card wk-work wk-food">
            <p class="wk-head">"식품 성능 올리기"</p>
            <p class="wk-note">"배합을 다듬어 식품 성능을 올려요. 성능이 높을수록 사장님이 한 번에 더 많이 만들 수 있어요. 음식은 사장님이 만들어요."</p>
            <div class="wk-nums"><span>"식품 성능 "<b id="lab-fp">{lab_text(s, 5)}</b></span></div>
            <button type="button" class="btn primary" id="wk-tune" on:click=work>"배합 다듬기"</button>
        </div>
    }
}

/* ---------- 인공지능: 가짜 검색 (받기를 누르면 막대가 차고 랜덤한 양이 쌓여요) ---------- */

#[derive(Clone, Copy, PartialEq)]
enum Hit {
    Idle,
    Busy(f64),
    Done(u32),
}

#[component]
fn AiCard(s: Sig) -> impl IntoView {
    let hits: [RwSignal<Hit>; 4] = std::array::from_fn(|_| RwSignal::new(Hit::Idle));
    let got = RwSignal::new(0u32);
    let game = GAME.get();
    let fetch = move |i: usize| {
        if hits[i].get_untracked() != Hit::Idle {
            return;
        }
        hits[i].set(Hit::Busy(0.0));
        let dur = if reduce_motion() { 0.0 } else { 1600.0 };
        let t0 = js_sys::Date::now();
        fn tick(h: RwSignal<Hit>, got: RwSignal<u32>, t0: f64, dur: f64, game: u32) {
            request_animation_frame(move || {
                if GAME.get() != game {
                    return;
                }
                let p = if dur > 0.0 { ((js_sys::Date::now() - t0) / dur).min(1.0) } else { 1.0 };
                if p < 1.0 {
                    h.set(Hit::Busy(p));
                    return tick(h, got, t0, dur, game);
                }
                let mb = 20 + (rand() * 381.0).floor() as u32;
                h.set(Hit::Done(mb));
                got.update(|g| *g += 1);
                queue_lab(4, mb as f64);
                count_pop(&format!("이번에 받은 데이터 {}개", got.get_untracked()));
            });
        }
        tick(hits[i], got, t0, dur, game);
    };
    view! {
        <div class="card wk-work wk-ai">
            <p class="wk-head">"가짜 검색 연습"</p>
            <p class="wk-note">"아무 데이터나 받아 보세요. 받는 양은 랜덤이고, 많이 쌓일수록 인공지능이 좋아져요. 실제로 저장되는 파일은 없어요."</p>
            <div class="wk-nums"><span>"쌓인 데이터 "<b id="lab-data">{lab_text(s, 4)}</b>" MB"</span></div>
            <label class="gg wk-fake">
                <span style="display:contents" inner_html=svg_icon(SEARCH_D, 18)></span>
                <input type="text" disabled placeholder="검색어를 입력해 보세요" aria-label="가짜 검색"/>
            </label>
            <ul class="wk-hits">
                {HITS.iter().enumerate().map(|(i, t)| {
                    let h = hits[i];
                    view! {
                        <li class="wk-hit" style=format!("--i:{i}")>
                            <span class="wk-hit-t"><b>{*t}</b><span>"받으면 양이 보여요"</span></span>
                            <button type="button" class="gg wk-get" data-i=i.to_string()
                                class:busy=move || matches!(h.get(), Hit::Busy(_))
                                class:done=move || matches!(h.get(), Hit::Done(_))
                                disabled=move || matches!(h.get(), Hit::Done(_))
                                on:click=move |_| fetch(i)
                                inner_html=move || match h.get() {
                                    Hit::Idle => "받기".to_string(),
                                    Hit::Busy(p) => format!(r#"<span class="wk-meter"><i style="transform:scaleX({p})"></i></span><em>{}%</em>"#, (p * 100.0).round()),
                                    Hit::Done(mb) => format!("{}{mb} MB", svg_icon(CHECK_D, 16)),
                                }
                            ></button>
                        </li>
                    }
                }).collect_view()}
            </ul>
            <p class="wk-count" id="wk-count">"이번에 받은 데이터 0개"</p>
        </div>
    }
}

/* ---------- 원자력: 핵연료 배합 (스포이드로 한 방울씩, 꾹 눌러 섞기) ---------- */

/// 재료를 방울 수만큼 섞은 색 (방울이 없으면 None)
fn mix_rgb(c: [u32; 3]) -> Option<[u8; 3]> {
    let n = c[0] + c[1] + c[2];
    if n == 0 {
        return None;
    }
    let ch = |k: usize| ((0..3).map(|i| c[i] as f64 * REAGENTS[i].1[k] as f64).sum::<f64>() / n as f64).round() as u8;
    Some([ch(0), ch(1), ch(2)])
}
/// 두 색이 얼마나 가까운지 (1은 같은 색)
fn close_of(a: [u8; 3], b: [u8; 3]) -> f64 {
    let d = |i: usize| a[i] as f64 - b[i] as f64;
    (1.0 - (d(0).powi(2) + d(1).powi(2) + d(2).powi(2)).sqrt() / 100.0).clamp(0.0, 1.0)
}
/// 목표 색은 8방울 안에서 섞어 만들 수 있는 색이에요 (맞출 수 있는 문제)
fn new_goal() -> [u8; 3] {
    loop {
        let c = [0, 0, 0].map(|_: u32| (rand() * 5.0).floor() as u32);
        let n = c[0] + c[1] + c[2];
        if (3..=MIX_MAX).contains(&n) {
            return mix_rgb(c).unwrap_or([0, 0, 0]);
        }
    }
}

const FLASK_PATH: &str = "M50 14H70M50 14V52L16 124Q11 136 24 139H96Q109 136 104 124L70 52V14";

#[component]
fn MixCard(s: Sig) -> impl IntoView {
    let counts = RwSignal::new([0u32; 3]);
    let sel = RwSignal::new(0usize);
    let goal = RwSignal::new(new_goal());
    let stir = RwSignal::new(0.0f64); // 0이면 안 섞는 중, 아니면 시작 시각
    let stir_p = RwSignal::new(0.0f64);
    let log = RwSignal::new("재료를 고르고 플라스크를 눌러 한 방울씩 떨어뜨려 보세요".to_string());
    let wrap = NodeRef::<html::Div>::new();
    let dropper = NodeRef::<html::Div>::new();
    let game = GAME.get();
    let total = move || counts.with(|c| c[0] + c[1] + c[2]);
    let pct = move || mix_rgb(counts.get()).map(|m| (close_of(m, goal.get()) * 100.0).round() as u32).unwrap_or(0);

    let float_text = move |text: String| {
        let Some(w) = wrap.get_untracked() else { return };
        let Some(doc) = web_sys::window().and_then(|x| x.document()) else { return };
        let Ok(f) = doc.create_element("span") else { return };
        f.set_class_name("wk-float");
        f.set_text_content(Some(&text));
        let _ = f.set_attribute("style", "left:50%;top:96px");
        let _ = w.append_child(&f);
        set_timeout(move || f.remove(), Duration::from_millis(900));
    };
    let drop_one = move |_| {
        if stir.get_untracked() > 0.0 {
            return;
        }
        if total() >= MIX_MAX {
            return log.set("더는 못 넣어요. 꾹 눌러 섞어 보세요".into());
        }
        let i = sel.get_untracked();
        counts.update(|c| c[i] += 1);
        let t = total() as f64;
        if let (Some(w), Some(doc)) = (wrap.get_untracked(), web_sys::window().and_then(|x| x.document())) {
            // 떨어질 거리: 방울이 닿을 액체 면까지 (플라스크 그림의 좌표를 화면 픽셀로 바꿔요)
            let surface = 11.25 + (60.0 + (1.0 - t / MIX_MAX as f64) * 80.0) * 1.25;
            if let Ok(d) = doc.create_element("span") {
                d.set_class_name("lab-drop");
                let _ = d.set_attribute("style", &format!("--c:{};--fall:{}px", rgb_css(REAGENTS[i].1), (surface - 62.0).max(0.0)));
                let _ = w.append_child(&d);
                set_timeout(move || d.remove(), Duration::from_millis(600));
            }
            if let Some(dip) = dropper.get_untracked() {
                let _ = dip.class_list().remove_1("dip");
                let _ = dip.offset_width(); // 같은 움직임을 다시 시작해요
                let _ = dip.class_list().add_1("dip");
            }
        }
        log.set(format!("{} 한 방울을 떨어뜨렸어요", REAGENTS[i].0));
    };
    let done = move || {
        stir.set(0.0);
        stir_p.set(0.0);
        let close = close_of(mix_rgb(counts.get_untracked()).unwrap_or([0, 0, 0]), goal.get_untracked());
        let gain = 1 + (rand() * (4.0 + (26.0 * close).round())).floor() as u32;
        queue_lab(0, gain as f64);
        float_text(format!("성능 +{gain}"));
        log.set(format!("핵연료 성능이 {gain} 올랐어요 · 목표 색과 {}% 일치", (close * 100.0).round()));
        counts.set([0, 0, 0]);
        goal.set(new_goal());
    };
    // 꾹 누르고 있는 동안 섞여요. 1.6초를 채우면 배합이 끝나요. 중간에 손을 떼면 처음부터예요
    let begin = move || {
        if stir.get_untracked() > 0.0 {
            return;
        }
        if total() == 0 {
            return log.set("재료를 먼저 떨어뜨려 주세요".into());
        }
        let t0 = js_sys::Date::now();
        stir.set(t0);
        log.set("저어 주세요. 손을 떼면 멈춰요".into());
        fn tick(stir: RwSignal<f64>, stir_p: RwSignal<f64>, t0: f64, done: impl Fn() + Copy + 'static, game: u32) {
            request_animation_frame(move || {
                if GAME.get() != game || stir.get_untracked() != t0 {
                    return;
                }
                let p = ((js_sys::Date::now() - t0) / STIR_MS).min(1.0);
                stir_p.set(p);
                if p >= 1.0 {
                    return done();
                }
                tick(stir, stir_p, t0, done, game);
            });
        }
        tick(stir, stir_p, t0, done, game);
    };
    let cancel = move || {
        if stir.get_untracked() == 0.0 {
            return;
        }
        stir.set(0.0);
        stir_p.set(0.0);
        log.set("조금 더 저어 주세요. 끝까지 누르고 있어야 섞여요".into());
    };
    let reset = move |_| {
        if stir.get_untracked() > 0.0 {
            return;
        }
        counts.set([0, 0, 0]);
        log.set("비웠어요. 재료를 골라 다시 떨어뜨려 보세요".into());
    };
    let liquid_style = move || {
        let t = total();
        let fill = mix_rgb(counts.get()).map(rgb_css).unwrap_or_else(|| "transparent".into());
        format!("transform:translateY({}px);fill:{fill}", (1.0 - (t as f64 / MIX_MAX as f64).min(1.0)) * 80.0)
    };
    view! {
        <div class="card wk-work wk-nuc">
            <p class="wk-head">"핵연료 배합"</p>
            <p class="wk-note">"재료를 고르고 플라스크를 누르면 스포이드로 한 방울씩 떨어져요. 목표 색에 가깝게 섞은 뒤, 아래 버튼을 손을 떼지 않고 꾹 누르면 섞여요. 가까울수록 핵연료 성능이 많이 올라가요."</p>
            <div class="wk-nums"><span>"핵연료 성능 "<b id="lab-np">{lab_text(s, 0)}</b></span></div>
            <div class="lab-bench">
                <div class="lab-top">
                    <span class="lab-sw" id="wk-target" aria-hidden="true" style=move || format!("--c:{}", rgb_css(goal.get()))></span>
                    <div class="lab-meta">
                        <b>"목표 색"</b>
                        <span id="wk-match">{move || format!("일치 {}%", pct())}</span>
                        <span class="lab-meter" aria-hidden="true"><i id="wk-meter" style=move || format!("width:{}%", pct())></i></span>
                    </div>
                </div>
                <div class="lab-flaskwrap" node_ref=wrap>
                    <div class="lab-dropper" id="wk-dropper" aria-hidden="true" node_ref=dropper>
                        <svg viewBox="0 0 34 70" aria-hidden="true">
                            <rect x="10" y="1" width="14" height="22" rx="7" fill="none" stroke="#000" stroke-width="1.8"/>
                            <path d="M13 23V52L17 66L21 52V23" fill="none" stroke="#000" stroke-width="1.8" stroke-linejoin="round" stroke-linecap="round"/>
                            <rect class="lab-dropper-liq" x="14.5" y="28" width="5" height="22" rx="2" style=move || format!("fill:{}", rgb_css(REAGENTS[sel.get()].1))/>
                        </svg>
                    </div>
                    <button type="button" class="lab-flask" class:is-stir=move || { stir.get() > 0.0 } id="wk-flask" aria-label="선택한 재료를 한 방울 떨어뜨리기" on:click=drop_one>
                        <svg viewBox="0 0 120 150" aria-hidden="true">
                            <defs><clipPath id="wk-flask-clip"><path d="M50 14H70V52L104 124Q109 136 96 139H24Q11 136 16 124L50 52Z"/></clipPath></defs>
                            <g clip-path="url(#wk-flask-clip)"><rect class="lab-liquid" x="0" y="60" width="120" height="90" style=liquid_style/></g>
                            <path d=FLASK_PATH fill="none" stroke="#000" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"/>
                            <path d="M94.5 119.25H99.5M85.4 99.5H90.4M76.1 79.75H81.1M66.8 60H71.8" fill="none" stroke="#000" stroke-width="1.5" stroke-linecap="round"/>
                        </svg>
                    </button>
                </div>
                <p class="lab-drops" id="wk-drops">{move || format!("{} / {} 방울", total(), MIX_MAX)}</p>
                <div class="lab-reagents" role="radiogroup" aria-label="재료 고르기">
                    {REAGENTS.iter().enumerate().map(|(i, (name, rgb))| view! {
                        <button type="button" class="lab-reagent" role="radio" aria-checked=move || (sel.get() == i).to_string() data-r=i.to_string()
                            on:click=move |_| { if stir.get_untracked() == 0.0 { sel.set(i) } }>
                            <svg viewBox="0 0 40 48" aria-hidden="true">
                                <rect x="14" y="1.5" width="12" height="6" rx="2" fill="none" stroke="#000" stroke-width="1.8"/>
                                <path d="M16 8v6Q8 17 8 26v14q0 6 6 6h12q6 0 6-6V26q0-9-8-12V8" fill="none" stroke="#000" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
                                <rect x="11" y="29" width="18" height="14" rx="3" fill=rgb_css(*rgb)/>
                            </svg>
                            <b>{*name}</b>
                            <span id=format!("wk-rc{i}")>{move || format!("{}방울", counts.get()[i])}</span>
                        </button>
                    }).collect_view()}
                </div>
                <div class="lab-actions">
                    <button type="button" class="btn lab-reset" id="wk-reset" on:click=reset>"비우기"</button>
                    <button type="button" class="btn primary lab-stir" id="wk-stir"
                        on:pointerdown=move |e: web_sys::PointerEvent| {
                            if e.button() != 0 { return; }
                            if let Some(t) = e.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) {
                                let _ = t.set_pointer_capture(e.pointer_id());
                            }
                            begin();
                        }
                        on:pointerup=move |_| cancel()
                        on:pointercancel=move |_| cancel()
                        on:keydown=move |e: web_sys::KeyboardEvent| {
                            if (e.key() == "Enter" || e.key() == " ") && !e.repeat() { e.prevent_default(); begin(); }
                        }
                        on:keyup=move |e: web_sys::KeyboardEvent| { if e.key() == "Enter" || e.key() == " " { cancel(); } }
                    >"꾹 눌러 섞기"</button>
                </div>
                <span class="lab-stirbar" aria-hidden="true"><i id="wk-stirbar" style=move || format!("width:{}%", (stir_p.get() * 100.0).round())></i></span>
            </div>
            <p class="wk-count" id="wk-mixlog">{move || log.get()}</p>
        </div>
    }
}

/* ---------- 컴퓨터: 부품 검사 (열·스파크를 누르면 용량·속도·전기 가운데 하나가 올라요) ---------- */

#[component]
fn BoardCard(s: Sig) -> impl IntoView {
    let board = NodeRef::<html::Div>::new();
    let stack = RwSignal::new(0u32);
    let game = GAME.get();
    // 0.65초마다 점이 하나 떠요 (최대 6개, 2.2초 뒤 사라져요)
    fn spawn(board: NodeRef<html::Div>, game: u32) {
        set_timeout(
            move || {
                if GAME.get() != game {
                    return;
                }
                if let (Some(b), Some(doc)) = (board.get_untracked(), web_sys::window().and_then(|w| w.document())) {
                    if b.query_selector_all(".wk-spot").map(|l| l.length()).unwrap_or(0) < 6 {
                        if let Ok(spot) = doc.create_element("button") {
                            let _ = spot.set_attribute("type", "button");
                            spot.set_class_name(if rand() < 0.5 { "wk-spot heat" } else { "wk-spot spark" });
                            let _ = spot.set_attribute("style", &format!("left:{}%;top:{}%", 8.0 + rand() * 84.0, 8.0 + rand() * 84.0));
                            let _ = spot.set_attribute("data-v", &(1 + (rand() * 8.0).floor() as u32).to_string());
                            let _ = b.append_child(&spot);
                            set_timeout(move || spot.remove(), Duration::from_millis(2200));
                        }
                    }
                }
                spawn(board, game);
            },
            Duration::from_millis(650),
        );
    }
    spawn(board, game);
    let hit = move |e: leptos::ev::MouseEvent| {
        let Some(spot) = e.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()).and_then(|t| t.closest(".wk-spot").ok().flatten()) else { return };
        let v: f64 = spot.get_attribute("data-v").and_then(|x| x.parse().ok()).unwrap_or(1.0);
        let pick = (rand() * 3.0).floor() as usize;
        let (k, label) = [(1, "용량"), (2, "속도"), (3, "전기")][pick.min(2)];
        let style = spot.get_attribute("style").unwrap_or_default();
        spot.remove();
        stack.update(|n| *n += 1);
        queue_lab(k, v);
        if let (Some(b), Some(doc)) = (board.get_untracked(), web_sys::window().and_then(|w| w.document())) {
            if let Ok(f) = doc.create_element("span") {
                f.set_class_name("wk-float");
                f.set_text_content(Some(&format!("{label} +{v}")));
                let _ = f.set_attribute("style", &style);
                let _ = b.append_child(&f);
                set_timeout(move || f.remove(), Duration::from_millis(700));
            }
        }
    };
    view! {
        <div class="card wk-work wk-pc">
            <p class="wk-head">"컴퓨터 부품 검사"</p>
            <p class="wk-note">"열이 뜨거나 스파크가 튀는 곳을 누르면 용량·속도·전기 가운데 하나가 랜덤으로 올라가요. 쌓일수록 개발 중인 컴퓨터가 좋아져요."</p>
            <div class="gg wk-board" id="wk-board" aria-label="부품 검사판" node_ref=board on:click=hit>
                <svg class="wk-chip" viewBox="0 0 120 120" aria-hidden="true">
                    <rect x="34" y="34" width="52" height="52" rx="6"/>
                    <path d="M48 34V22M60 34V22M72 34V22M48 86v12M60 86v12M72 86v12M34 48H22M34 60H22M34 72H22M86 48h12M86 60h12M86 72h12"/>
                </svg>
            </div>
            <div class="wk-nums">
                <span>"용량 "<b id="lab-cap">{lab_text(s, 1)}</b></span>
                <span>"속도 "<b id="lab-spd">{lab_text(s, 2)}</b></span>
                <span>"전기 "<b id="lab-pow">{lab_text(s, 3)}</b></span>
                <span>"검사 "<b id="wk-stack">{move || stack.get()}</b>"번"</span>
            </div>
        </div>
    }
}
