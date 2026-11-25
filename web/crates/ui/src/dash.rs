//! 홈 탭: 내 멸공, 내 땅(곳 수·모인 통행료), 내 약속, 코인, 친구 초대, 지금 접속한 사람, 다가오는 약속.
//! 앱이 켜질 때 미리 한 번 읽어 두고, 탭을 열 때는 이미 그려진 것을 보여줘요 (읽은 지 1분이 지났을 때만 조용히 다시 읽어요).

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::dom::{self, by_id, esc};

#[derive(Clone, Default)]
struct Mine {
    lands: usize,
    pot: f64,
}
#[derive(Clone)]
struct P {
    id: String,
    by: String,
    title: String,
    place: String,
    when: f64,
    n: usize,
}
thread_local! {
    static MINE: RefCell<Option<Mine>> = const { RefCell::new(None) };
    static PLANS: RefCell<Vec<P>> = const { RefCell::new(Vec::new()) };
    static AT: Cell<f64> = const { Cell::new(0.0) };
    static BOUND: Cell<bool> = const { Cell::new(false) };
    static TIMER: Cell<i32> = const { Cell::new(0) };
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
fn fmt(n: f64) -> String {
    cloud::fmt_num(n, 0)
}
fn when_text(ms: f64) -> String {
    let d = js_sys::Date::new(&JsValue::from(ms));
    let wd = ["일", "월", "화", "수", "목", "금", "토"][d.get_day() as usize % 7];
    let (h, m) = (d.get_hours(), d.get_minutes());
    let (ap, h12) = if h < 12 { ("오전", if h == 0 { 12 } else { h }) } else { ("오후", if h == 12 { 12 } else { h - 12 }) };
    format!("{}월 {}일 ({wd}) {ap} {h12}:{m:02}", d.get_month() + 1, d.get_date())
}

/// 숫자와 목록을 다시 그려요 (서버를 읽지 않아요)
pub fn repaint() {
    if by_id("hm-bal").is_none() {
        return;
    }
    let signed = cloud::signed_in();
    match crate::money::balance() {
        Some(b) if signed => {
            dom::set_text("hm-bal", &fmt(b.floor()));
            dom::set_text("hm-sub", "");
        }
        _ => {
            dom::set_text("hm-bal", if signed { "…" } else { "0" });
            dom::set_text("hm-sub", if signed { "" } else { "로그인하면 보여요" });
        }
    }
    let m = MINE.with(|m| m.borrow().clone());
    dom::set_text("hm-lands", &format!("{}곳", m.as_ref().map(|m| m.lands).unwrap_or(0)));
    dom::set_text("hm-pot", &match &m {
        Some(m) if m.pot > 0.0 => format!("모인 통행료 {} 멸공", fmt(m.pot)),
        Some(m) if m.lands > 0 => "통행료를 기다려요".into(),
        _ => "지도에서 땅을 사 보세요".into(),
    });
    let me = my_uid();
    let (mine, next): (Vec<P>, Option<P>) = PLANS.with(|p| {
        let p = p.borrow();
        (p.iter().filter(|x| x.by == me).cloned().collect(), p.first().cloned())
    });
    dom::set_text("hm-plans", &format!("{}개", mine.len()));
    dom::set_text("hm-plan-next", &next.map(|n| format!("다음: {}", n.title)).unwrap_or_else(|| "참여한 약속이 없어요".into()));
    dom::set_text("hm-coin", "");
    // 지금 접속한 사람
    if let Some(l) = by_id("hm-online") {
        let ids = crate::presence::online_uids(&me);
        l.set_inner_html(&if ids.is_empty() {
            r#"<p class="sns-empty">지금 접속한 사람이 없어요</p>"#.to_string()
        } else {
            ids.iter()
                .map(|u| {
                    let (n, ph) = person(u);
                    format!(r#"<button type="button" class="sns-p" data-uid="{}" role="listitem">{}<span class="sns-n">{}</span></button>"#, esc(u), crate::presence::wrap(dom::avatar_html(&n, &ph, "avatar contact sns-av"), u), esc(&n))
                })
                .collect()
        });
    }
    // 다가오는 약속 (내가 참여하는 것, 가까운 순 3개)
    if let Some(l) = by_id("hm-plan-list") {
        let list = PLANS.with(|p| p.borrow().iter().take(3).cloned().collect::<Vec<_>>());
        l.set_inner_html(&if list.is_empty() {
            r#"<div class="pl-empty"><b>다가오는 약속이 없어요</b><p>놀기 탭에서 약속을 올리거나 참여해 보세요</p></div>"#.to_string()
        } else {
            list.iter()
                .map(|p| {
                    format!(
                        r#"<button type="button" class="hm-plan" data-go="play"><b>{}</b><span>{}{}</span><small>{}명 참여</small></button>"#,
                        esc(&p.title),
                        when_text(p.when),
                        if p.place.is_empty() { String::new() } else { format!(" · {}", esc(&p.place)) },
                        p.n
                    )
                })
                .collect()
        });
    }
}

/// 서버에서 내 땅과 내 약속을 읽어요 (force가 아니면 1분 안에는 다시 읽지 않아요)
pub fn load(force: bool) {
    if !cloud::signed_in() || (!force && js_sys::Date::now() - AT.get() < 60_000.0) {
        return;
    }
    AT.set(js_sys::Date::now());
    spawn_local(async {
        if let Ok(list) = cloud::call("landsMine", &[]).await {
            let arr = js_sys::Array::from(&list);
            let mut pot = 0.0;
            for it in arr.iter() {
                pot += cloud::get(&it, "pot").as_f64().unwrap_or(0.0);
            }
            MINE.with(|m| *m.borrow_mut() = Some(Mine { lands: arr.length() as usize, pot }));
        }
        if let Ok(list) = cloud::call("plansList", &[]).await {
            let me = my_uid();
            let cut = js_sys::Date::now() - 3.0 * 3_600_000.0;
            let mut v: Vec<P> = js_sys::Array::from(&list)
                .iter()
                .filter_map(|it| {
                    let g = cloud::get(&it, "going");
                    let going: Vec<String> = js_sys::Array::from(&g).iter().filter_map(|x| x.as_string()).collect();
                    let when = cloud::get(&it, "when").as_f64().unwrap_or(0.0);
                    (going.contains(&me) && when >= cut).then(|| P { id: cloud::get(&it, "id").as_string().unwrap_or_default(), by: cloud::get(&it, "by").as_string().unwrap_or_default(), title: cloud::get(&it, "title").as_string().unwrap_or_default(), place: cloud::get(&it, "place").as_string().unwrap_or_default(), when, n: going.len() })
                })
                .collect();
            v.sort_by(|a, b| a.when.partial_cmp(&b.when).unwrap_or(std::cmp::Ordering::Equal));
            let _ = v.iter().map(|p| p.id.len()).sum::<usize>();
            PLANS.with(|p| *p.borrow_mut() = v);
        }
        repaint();
    });
}

/// 탭 이동 버튼 (지도·놀기는 탭 바 둘째 쪽에 있어요)
fn go(to: &str) {
    match to {
        "coin" => {
            crate::shell::go_screen("coin");
            crate::coin::open();
        }
        "invite" => {
            crate::shell::go_screen("invite");
            crate::invite::open();
        }
        tab => {
            let tab = tab.to_string();
            spawn_local(async move {
                let q = |t: &str| dom::doc().query_selector(&format!(".tabbar .tab[data-tab=\"{t}\"]")).ok().flatten().and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok());
                if q(&tab).is_none() {
                    // 탭 바 둘째 쪽에 있으면 한 쪽 넘긴 뒤 눌러요
                    let tb = js_sys::Reflect::get(&js_sys::global(), &"tabbar".into()).unwrap_or(JsValue::NULL);
                    if let Ok(f) = js_sys::Reflect::get(&tb, &"flipPage".into()).and_then(|f| f.dyn_into::<js_sys::Function>()) {
                        let _ = f.call0(&tb);
                    }
                    dom::sleep(450).await;
                }
                if let Some(b) = q(&tab) {
                    b.click();
                }
            });
        }
    }
}

fn bind() {
    let Some(sc) = by_id("screen-home") else { return };
    dom::on(&sc, "click", |e| {
        if let Some(b) = dom::closest(&e, "[data-uid]") {
            let uid = dom::data(&b, "uid");
            return crate::profile::show(&uid, &person(&uid).0, None);
        }
        if let Some(b) = dom::closest(&e, "[data-go]") {
            dom::vibrate(8);
            go(&dom::data(&b, "go"));
        }
    });
}

/// 앱이 켜진 뒤 미리 읽어 두어요 (홈을 열기 전에 이미 그려져 있게)
pub fn warm() {
    if !BOUND.replace(true) {
        bind();
    }
    load(true);
    if TIMER.get() == 0 {
        let cb = Closure::<dyn FnMut()>::new(repaint).into_js_value();
        if let Some(w) = web_sys::window() {
            TIMER.set(w.set_interval_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 2000).unwrap_or(0));
        }
    }
    repaint();
}
pub fn open() {
    if !BOUND.replace(true) {
        bind();
    }
    repaint();
    load(false);
}
