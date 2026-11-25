//! 놀기 탭: 약속을 올리고(언제·어디서·무엇을), 사람들이 참여해요. 모두에게 올리거나, 고른 사람만 보게 할 수 있어요.
//! 생김새는 SNS와 같은 유리 카드 피드이고, 탭 바 위 둥근 + 버튼으로 새 약속을 만들어요.
//! 서버를 아끼려고 탭을 열 때 한 번 읽고(공개 60개 + 나를 부른 60개), 참여·취소·올리기 뒤에만 다시 읽어요.

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::dom::{self, by_id, esc};

#[derive(Clone, Default)]
struct Plan {
    id: String,
    by: String,
    title: String,
    place: String,
    when: f64,
    text: String,
    private: bool,
    going: Vec<String>,
    at: f64,
}

thread_local! {
    static PLANS: RefCell<Vec<Plan>> = const { RefCell::new(Vec::new()) };
    static PEOPLE: RefCell<Vec<(String, String, String)>> = const { RefCell::new(Vec::new()) };
    static PICK: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static BOUND: Cell<bool> = const { Cell::new(false) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
    static LOADING: Cell<bool> = const { Cell::new(false) };
    static AT: Cell<f64> = const { Cell::new(0.0) };
}

const CAL: &str = r#"<rect x="3.5" y="5" width="17" height="15" rx="3"/><path d="M3.5 10h17M8 3v4M16 3v4"/>"#;
const PIN: &str = r#"<path d="M12 21s-6.5-5.6-6.5-11a6.5 6.5 0 0 1 13 0C18.5 15.4 12 21 12 21z"/><circle cx="12" cy="10" r="2.2"/>"#;
const LOCK: &str = r#"<rect x="5" y="10.5" width="14" height="10" rx="2.5"/><path d="M8 10.5V8a4 4 0 0 1 8 0v2.5"/>"#;
const CHECK: &str = r#"<path d="M5 12.5l4.5 4.5L19 7.5"/>"#;
const TRASH: &str = r#"<path d="M4.5 7h15M9.5 7V5h5v2M6.5 7l1 13h9l1-13"/>"#;

fn svg(d: &str) -> String {
    format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{d}</svg>"#)
}
fn my_uid() -> String {
    cloud::cloud().and_then(|c| cloud::get(&c, "uid").as_string()).unwrap_or_default()
}
fn person(uid: &str) -> (String, String) {
    let p = cloud::call_now("peek", &[JsValue::from_str(uid)]).unwrap_or(JsValue::NULL);
    let g = |k: &str| cloud::get(&p, k).as_string().unwrap_or_default();
    let n = g("name");
    (if n.is_empty() { g("id") } else { n }, g("photo"))
}
/// 11월 25일 (화) 오후 7:00
fn when_text(ms: f64) -> String {
    let d = js_sys::Date::new(&JsValue::from(ms));
    let wd = ["일", "월", "화", "수", "목", "금", "토"][d.get_day() as usize % 7];
    let (h, m) = (d.get_hours(), d.get_minutes());
    let (ap, h12) = if h < 12 { ("오전", if h == 0 { 12 } else { h }) } else { ("오후", if h == 12 { 12 } else { h - 12 }) };
    format!("{}월 {}일 ({wd}) {ap} {h12}:{m:02}", d.get_month() + 1, d.get_date())
}
/// 오늘·내일·n일 뒤
fn soon(ms: f64) -> String {
    let now = js_sys::Date::new_0();
    let a = js_sys::Date::new(&JsValue::from(ms));
    let day = |d: &js_sys::Date| (d.get_full_year() * 400 + d.get_month() * 32 + d.get_date()) as i64;
    let mid = |d: &js_sys::Date| js_sys::Date::new_with_year_month_day(d.get_full_year(), d.get_month() as i32, d.get_date() as i32).get_time();
    let diff = ((mid(&a) - mid(&now)) / 86_400_000.0).round() as i64;
    let _ = day;
    if ms < js_sys::Date::now() {
        "지금 하는 중".into()
    } else {
        match diff {
            0 => "오늘".into(),
            1 => "내일".into(),
            2 => "모레".into(),
            n => format!("{n}일 뒤"),
        }
    }
}

fn card(p: &Plan) -> String {
    let me = my_uid();
    let (hn, hp) = person(&p.by);
    let joined = p.going.contains(&me);
    let mine = p.by == me;
    let faces: String = p
        .going
        .iter()
        .take(5)
        .map(|u| {
            let (n, ph) = person(u);
            format!(r#"<button type="button" class="pl-face" data-uid="{}">{}</button>"#, esc(u), crate::presence::wrap(dom::avatar_html(&n, &ph, "avatar contact pl-fav"), u))
        })
        .collect();
    let more = if p.going.len() > 5 { format!(r#"<span class="pl-more">+{}</span>"#, p.going.len() - 5) } else { String::new() };
    let btn = if mine {
        format!(r#"<button type="button" class="pl-del" data-act="del" aria-label="약속 지우기">{}</button>"#, svg(TRASH))
    } else if joined {
        format!(r#"<button type="button" class="pl-go on" data-act="leave">{}참여 중</button>"#, svg(CHECK))
    } else {
        r#"<button type="button" class="pl-go" data-act="join">참여하기</button>"#.into()
    };
    format!(
        r#"<article class="sns-card pl-card" data-id="{id}"><header class="pl-head"><button type="button" class="sns-who" data-uid="{by}">{av}<span><b>{hn}{bl}</b><small>{ago}</small></span></button><span class="pl-soon">{soon}</span></header><h2 class="pl-title">{title}</h2><div class="pl-chips"><span class="pl-chip">{cal}{when}</span>{place}{lock}</div>{text}<footer class="pl-foot"><div class="pl-faces">{faces}{more}<span class="pl-count">{n}명 참여</span></div>{btn}</footer></article>"#,
        id = esc(&p.id),
        by = esc(&p.by),
        av = crate::presence::wrap(dom::avatar_html(&hn, &hp, "avatar contact sns-cav"), &p.by),
        hn = esc(&hn),
        bl = crate::profile::blue(&p.by),
        ago = crate::sns::ago(p.at),
        soon = soon(p.when),
        title = esc(&p.title),
        cal = svg(CAL),
        when = when_text(p.when),
        place = if p.place.is_empty() { String::new() } else { format!(r#"<span class="pl-chip">{}{}</span>"#, svg(PIN), esc(&p.place)) },
        lock = if p.private { format!(r#"<span class="pl-chip pl-lock">{}부른 사람만</span>"#, svg(LOCK)) } else { String::new() },
        text = if p.text.is_empty() { String::new() } else { format!(r#"<p class="pl-text">{}</p>"#, esc(&p.text)) },
        n = p.going.len(),
    )
}

pub fn repaint() {
    let Some(f) = by_id("play-feed") else { return };
    if !cloud::signed_in() {
        f.set_inner_html(r#"<p class="sns-empty">로그인하면 약속을 볼 수 있어요</p>"#);
        return;
    }
    let html: String = PLANS.with(|p| p.borrow().iter().map(card).collect());
    f.set_inner_html(&if html.is_empty() {
        if LOADING.get() { r#"<p class="sns-empty">불러오는 중이에요</p>"#.into() } else { r#"<div class="pl-empty"><b>아직 약속이 없어요</b><p>아래 + 버튼으로 첫 약속을 올려 보세요</p></div>"#.into() }
    } else {
        html
    });
}

async fn load() {
    AT.set(js_sys::Date::now());
    LOADING.set(PLANS.with(|p| p.borrow().is_empty()));
    repaint();
    let r = cloud::call("plansList", &[]).await;
    LOADING.set(false);
    let Ok(list) = r else {
        crate::shell::toast("약속을 불러오지 못했어요");
        return repaint();
    };
    let cut = js_sys::Date::now() - 3.0 * 3_600_000.0; // 시작하고 3시간 지난 약속은 숨겨요
    let mut v: Vec<Plan> = js_sys::Array::from(&list)
        .iter()
        .map(|it| {
            let s = |k: &str| cloud::get(&it, k).as_string().unwrap_or_default();
            let n = |k: &str| cloud::get(&it, k).as_f64().unwrap_or(0.0);
            let arr = |k: &str| js_sys::Array::from(&cloud::get(&it, k)).iter().filter_map(|x| x.as_string()).collect::<Vec<_>>();
            Plan { id: s("id"), by: s("by"), title: s("title"), place: s("place"), when: n("when"), text: s("text"), private: s("vis") == "pick", going: arr("going"), at: n("at") }
        })
        .filter(|p| p.when >= cut)
        .collect();
    v.sort_by(|a, b| a.when.partial_cmp(&b.when).unwrap_or(std::cmp::Ordering::Equal));
    // 사람 이름·사진을 한 번에 모아 읽어요
    let uids: js_sys::Array = v.iter().flat_map(|p| std::iter::once(p.by.clone()).chain(p.going.iter().take(5).cloned())).map(|u| JsValue::from_str(&u)).collect();
    PLANS.with(|p| *p.borrow_mut() = v);
    repaint();
    let _ = cloud::call("fetchProfiles", &[uids.into()]).await;
    repaint();
}

fn on_feed(e: web_sys::Event) {
    if let Some(b) = dom::closest(&e, "[data-uid]") {
        let uid = dom::data(&b, "uid");
        return crate::profile::show(&uid, &person(&uid).0, None);
    }
    let Some(b) = dom::closest(&e, "[data-act]") else { return };
    let Some(card) = dom::closest(&e, ".pl-card") else { return };
    let (id, act) = (dom::data(&card, "id"), dom::data(&b, "act"));
    if BUSY.replace(true) {
        return;
    }
    dom::vibrate(10);
    spawn_local(async move {
        let r = match act.as_str() {
            "del" => {
                if !crate::shell::ask("이 약속을 지울까요?", "참여한 사람들에게서도 사라져요.", "지우기", "취소").await {
                    BUSY.set(false);
                    return;
                }
                cloud::call("planDelete", &[JsValue::from_str(&id)]).await.map(|_| "약속을 지웠어요")
            }
            "join" => cloud::call("planJoin", &[JsValue::from_str(&id), true.into()]).await.map(|_| "참여했어요"),
            _ => cloud::call("planJoin", &[JsValue::from_str(&id), false.into()]).await.map(|_| "참여를 취소했어요"),
        };
        BUSY.set(false);
        match r {
            Ok(m) => crate::shell::toast(m),
            Err(e) => crate::shell::toast(&format!("하지 못했어요. {}", cloud::why(&e.code))),
        }
        load().await;
    });
}

/* ---------- 새 약속 ---------- */
fn sheet(on: bool) {
    let Some(s) = by_id("play-compose") else { return };
    if on {
        s.set_hidden(false);
        let _ = s.offset_width();
        let _ = s.class_list().add_1("on");
    } else {
        let _ = s.class_list().remove_1("on");
        let s2 = s.clone();
        spawn_local(async move {
            dom::sleep(320).await;
            if !s2.class_list().contains("on") {
                s2.set_hidden(true);
            }
        });
    }
}
fn val(id: &str) -> String {
    by_id(id).and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok()).map(|i| i.value()).or_else(|| by_id(id).and_then(|e| e.dyn_into::<web_sys::HtmlTextAreaElement>().ok()).map(|t| t.value())).unwrap_or_default().trim().to_string()
}
fn paint_pick() {
    let all = by_id("pl-who-all").is_some_and(|b| b.get_attribute("aria-pressed").as_deref() == Some("true"));
    if let Some(l) = by_id("pl-people") {
        l.set_hidden(all);
        let picked = PICK.with(|p| p.borrow().clone());
        let me = my_uid();
        l.set_inner_html(&PEOPLE.with(|p| {
            p.borrow()
                .iter()
                .filter(|(u, _, _)| *u != me)
                .map(|(u, n, ph)| {
                    let on = picked.contains(u);
                    format!(r#"<button type="button" class="pl-pp{}" data-pick="{}" aria-pressed="{on}">{}<span>{}</span><i>{}</i></button>"#, if on { " on" } else { "" }, esc(u), crate::presence::wrap(dom::avatar_html(n, ph, "avatar contact pl-pav"), u), esc(n), svg(CHECK))
                })
                .collect::<String>()
        }));
    }
    if let Some(b) = by_id("pl-post") {
        let ok = !val("pl-title").is_empty() && !val("pl-when").is_empty() && (all || PICK.with(|p| !p.borrow().is_empty()));
        let _ = if ok { b.remove_attribute("disabled") } else { b.set_attribute("disabled", "") };
    }
}
fn set_who(all: bool) {
    for (id, on) in [("pl-who-all", all), ("pl-who-pick", !all)] {
        if let Some(b) = by_id(id) {
            let _ = b.set_attribute("aria-pressed", &on.to_string());
        }
    }
    paint_pick();
}
pub fn open_compose() {
    if !cloud::signed_in() {
        return crate::shell::toast("로그인하고 약속을 올릴 수 있어요");
    }
    PICK.with(|p| p.borrow_mut().clear());
    set_who(true);
    // 기본 시각: 내일 저녁 7시
    if let Some(i) = by_id("pl-when").and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok()) {
        if i.value().is_empty() {
            let t = js_sys::Date::new_0();
            let d = js_sys::Date::new_with_year_month_day_hr_min_sec(t.get_full_year(), t.get_month() as i32, t.get_date() as i32 + 1, 19, 0, 0);
            i.set_value(&format!("{:04}-{:02}-{:02}T19:00", d.get_full_year(), d.get_month() + 1, d.get_date()));
        }
    }
    sheet(true);
    paint_pick();
    if PEOPLE.with(|p| p.borrow().is_empty()) {
        spawn_local(async {
            let list = cloud::call("listUsers", &[true.into()]).await.ok().map(|v| js_sys::Array::from(&cloud::get(&v, "list")).to_vec()).unwrap_or_default();
            let v: Vec<(String, String, String)> = list
                .iter()
                .map(|x| {
                    let s = |k: &str| cloud::get(x, k).as_string().unwrap_or_default();
                    let n = s("name");
                    (s("uid"), if n.is_empty() { s("id") } else { n }, s("photo"))
                })
                .collect();
            PEOPLE.with(|p| *p.borrow_mut() = v);
            paint_pick();
        });
    }
}
fn post() {
    let (title, place, text, when_s) = (val("pl-title"), val("pl-place"), val("pl-text"), val("pl-when"));
    let all = by_id("pl-who-all").is_some_and(|b| b.get_attribute("aria-pressed").as_deref() == Some("true"));
    if title.is_empty() || when_s.is_empty() {
        return crate::shell::toast("무엇을, 언제 할지 적어 주세요");
    }
    if [&title, &place, &text].iter().any(|s| s.contains(['<', '>'])) {
        return crate::shell::toast("< > 는 쓸 수 없어요");
    }
    let when = js_sys::Date::new(&JsValue::from_str(&when_s)).get_time();
    if !when.is_finite() || when < js_sys::Date::now() - 600_000.0 {
        return crate::shell::toast("지나간 시각은 고를 수 없어요");
    }
    let inv: js_sys::Array = if all { js_sys::Array::new() } else { PICK.with(|p| p.borrow().iter().map(|u| JsValue::from_str(u)).collect()) };
    if BUSY.replace(true) {
        return;
    }
    let v = dom::obj(&[("title", JsValue::from_str(&title.chars().take(40).collect::<String>())), ("place", JsValue::from_str(&place.chars().take(30).collect::<String>())), ("text", JsValue::from_str(&text.chars().take(300).collect::<String>())), ("when", when.into()), ("invited", inv.into())]);
    spawn_local(async move {
        let r = cloud::call("planCreate", &[v]).await;
        BUSY.set(false);
        match r {
            Ok(_) => {
                for id in ["pl-title", "pl-place", "pl-text", "pl-when"] {
                    if let Some(i) = by_id(id).and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok()) {
                        i.set_value("");
                    } else if let Some(t) = by_id(id).and_then(|e| e.dyn_into::<web_sys::HtmlTextAreaElement>().ok()) {
                        t.set_value("");
                    }
                }
                sheet(false);
                crate::shell::toast("약속을 올렸어요");
                dom::vibrate(16);
                load().await;
            }
            Err(e) => crate::shell::toast(&format!("올리지 못했어요. {}", cloud::why(&e.code))),
        }
    });
}

fn bind() {
    if let Some(f) = by_id("play-feed") {
        dom::on(&f, "click", on_feed);
    }
    if let Some(s) = by_id("play-compose") {
        dom::on(&s, "click", |e| {
            if dom::closest(&e, "[data-close]").is_some() || dom::closest(&e, ".sns-panel").is_none() {
                return sheet(false);
            }
            if let Some(b) = dom::closest(&e, "[data-pick]") {
                let u = dom::data(&b, "pick");
                PICK.with(|p| {
                    let mut p = p.borrow_mut();
                    if let Some(i) = p.iter().position(|x| *x == u) {
                        p.remove(i);
                    } else if p.len() < 29 {
                        p.push(u);
                    }
                });
                dom::vibrate(6);
                return paint_pick();
            }
            if dom::closest(&e, "#pl-who-all").is_some() {
                return set_who(true);
            }
            if dom::closest(&e, "#pl-who-pick").is_some() {
                return set_who(false);
            }
            if dom::closest(&e, "#pl-post").is_some() {
                post();
            }
        });
        dom::on(&s, "input", |_| paint_pick());
    }
}

/// 놀기 탭을 열 때
pub fn open() {
    crate::sns::bind_once();
    if !BOUND.replace(true) {
        bind();
    }
    repaint();
    // 미리 읽어 둔 걸 먼저 보여주고, 15초가 지났으면 조용히 새로 읽어요
    if cloud::signed_in() && js_sys::Date::now() - AT.get() > 15_000.0 {
        spawn_local(load());
    }
}
