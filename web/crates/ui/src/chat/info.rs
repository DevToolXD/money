//! 대화 정보 (아이폰 메시지와 같은 모양·움직임): 대화 위쪽 사진이나 이름을 누르면, 그 자리에서 진한 유리 카드가
//! 화면 폭 약 2/3 크기로 나타나 잠깐 머문 뒤 화면 전체로 튕기듯 펼쳐져요 (약 0.35초). 닫으면 약 0.2초 만에 이름 자리로 빨려 들어가요.
//! 안에는 큰 사진·이름, 둥근 버튼 셋(프로필·멸공 보내기·알림), 탭(정보·배경·사진)이 있어요.

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::{open_chat, render, touch_pref, with_chats, Chat, WALLS};
use crate::dom::{self, by_id, esc, safe_img};

thread_local! {
    static TAB: RefCell<String> = RefCell::new("info".into());
    static OPEN: Cell<bool> = const { Cell::new(false) };
    static BOUND: Cell<bool> = const { Cell::new(false) };
}

fn svg(d: &str) -> String {
    format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{d}</svg>"#)
}
const BACK: &str = r#"<path d="M15 5l-7 7 7 7"/>"#;
const PERSON: &str = r#"<circle cx="12" cy="8.5" r="3.8"/><path d="M4.8 20c.8-3.6 3.6-5.6 7.2-5.6s6.4 2 7.2 5.6"/>"#;
const GIFT: &str = r#"<rect x="3.5" y="8.5" width="17" height="4" rx="1.2"/><path d="M5 12.5v7.5h14v-7.5M12 8.5V20M12 8.5c-1.5-3.5-5.5-3.5-5.5-1 0 1.5 2.5 1 5.5 1zM12 8.5c1.5-3.5 5.5-3.5 5.5-1 0 1.5-2.5 1-5.5 1z"/>"#;
const BELL: &str = r#"<path d="M6 16.5V11a6 6 0 0 1 12 0v5.5l1.6 1.8H4.4z"/><path d="M10 20.6a2 2 0 0 0 4 0"/>"#;
const BELL_OFF: &str = r#"<path d="M6 16.5V11a6 6 0 0 1 9-5.2M18 11v5.5l1.6 1.8H6M4 4l16 16"/><path d="M10 20.6a2 2 0 0 0 4 0"/>"#;

fn root() -> Option<web_sys::HtmlElement> {
    if let Some(e) = by_id("cinfo") {
        return Some(e);
    }
    let th = by_id("thread")?;
    let el: web_sys::HtmlElement = dom::doc().create_element("div").ok()?.dyn_into().ok()?;
    el.set_id("cinfo");
    el.set_class_name("ci");
    el.set_hidden(true);
    let _ = el.set_attribute("role", "dialog");
    let _ = el.set_attribute("aria-label", "대화 정보");
    let _ = th.append_child(&el);
    Some(el)
}

fn sw(on: bool, act: &str) -> String {
    format!(r#"<button type="button" class="ci-sw{}" role="switch" aria-checked="{on}" data-act="{act}"><i></i></button>"#, if on { " on" } else { "" })
}

fn body(c: &Chat, tab: &str) -> String {
    match tab {
        "wall" => {
            let cur = super::thread::wall_of(&c.id);
            let names = [("white", "흰색"), ("sunset", "노을"), ("aurora", "오로라"), ("night", "밤하늘"), ("mint", "민트")];
            format!(
                r#"<div class="ci-walls">{}</div>"#,
                names.iter().map(|(k, n)| format!(r#"<button type="button" class="wall-opt" data-wall-pick="{k}" aria-pressed="{}"><i data-wall="{k}"></i>{n}</button>"#, cur == *k)).collect::<String>()
            )
        }
        "photos" => {
            let pics: Vec<String> = c.msgs.iter().filter_map(|m| {
                let t = cloud::get(&m.raw, "type").as_string().unwrap_or_default();
                if t == "photo" || t == "draw" { Some(safe_img(&cloud::get(&m.raw, "img").as_string().unwrap_or_default(), 140000).to_string()) } else { None }
            }).filter(|s| !s.is_empty()).collect();
            if pics.is_empty() {
                r#"<p class="ci-empty">주고받은 사진이 없어요</p>"#.into()
            } else {
                format!(r#"<div class="ci-photos">{}</div>"#, pics.iter().rev().map(|p| format!(r#"<span style="background-image:url({p})"></span>"#)).collect::<String>())
            }
        }
        _ => {
            let who = if c.group {
                let rows: String = c.members.iter().map(|u| {
                    let p = cloud::call_now("peek", &[JsValue::from_str(u)]).unwrap_or(JsValue::NULL);
                    let g = |k: &str| cloud::get(&p, k).as_string().unwrap_or_default();
                    let n = Some(g("name")).filter(|n| !n.is_empty()).unwrap_or_else(|| "알 수 없음".into());
                    format!(r#"<button type="button" class="ci-mem" data-uid="{}">{}<span>{}{}</span></button>"#, esc(u), crate::presence::wrap(dom::avatar_html(&n, &g("photo"), "avatar contact"), u), esc(&n), crate::profile::blue(u))
                }).collect();
                format!(r#"<div class="ci-card"><p class="ci-label">대화 상대 {}명</p><div class="ci-mems">{rows}</div></div>"#, c.members.len())
            } else {
                let p = cloud::call_now("peek", &[JsValue::from_str(&c.other)]).unwrap_or(JsValue::NULL);
                let id = cloud::get(&p, "id").as_string().unwrap_or_default();
                format!(r#"<div class="ci-card ci-row"><div><p class="ci-label">아이디</p><b>@{}</b></div>{}</div>"#, esc(&id), if crate::presence::online(&c.other) { r#"<span class="ci-on">접속 중</span>"# } else { "" })
            };
            format!(
                r#"{who}<div class="ci-card ci-list"><div class="ci-li"><span>알림 가리기</span>{}</div><div class="ci-li"><span>대화 맨 위에 고정</span>{}</div></div><button type="button" class="ci-card ci-link" data-act="{}">{}</button>"#,
                sw(c.muted, "mute"),
                sw(c.pin, "pin"),
                if c.group { "edit" } else { "profile" },
                if c.group { "단톡방 이름·사진 바꾸기" } else { "프로필 보기" }
            )
        }
    }
}

fn paint() {
    let (Some(el), Some(c)) = (root(), open_chat()) else { return };
    let tab = TAB.with(|t| t.borrow().clone());
    let tabs = [("info", "정보"), ("wall", "배경"), ("photos", "사진")];
    let av = dom::avatar_html(&c.name, &c.photo, "avatar contact ci-big");
    el.set_inner_html(&format!(
        r#"<div class="ci-panel"><header class="ci-top"><button type="button" class="ci-round" data-act="close" aria-label="닫기">{back}</button>{edit}</header><div class="ci-head">{av}<h2 class="ci-name">{name}{bl}</h2></div><div class="ci-acts"><button type="button" class="ci-act" data-act="profile" aria-label="{pl}">{person}</button><button type="button" class="ci-act" data-act="gift" aria-label="멸공 보내기"{gd}>{gift}</button><button type="button" class="ci-act" data-act="mute" aria-label="알림">{bell}</button></div><div class="ci-tabs" role="tablist">{tabs}</div><div class="ci-body">{body}</div></div>"#,
        back = svg(BACK),
        edit = if c.group { r#"<button type="button" class="ci-pill" data-act="edit">편집</button>"# } else { "" },
        name = esc(&c.name),
        bl = if c.group { "" } else { crate::profile::blue(&c.other) },
        pl = if c.group { "대화 상대" } else { "프로필" },
        person = svg(PERSON),
        gd = if c.group { " disabled" } else { "" },
        gift = svg(GIFT),
        bell = svg(if c.muted { BELL_OFF } else { BELL }),
        tabs = tabs.iter().map(|(k, n)| format!(r#"<button type="button" role="tab" class="ci-tab{}" data-tab="{k}" aria-selected="{}">{n}</button>"#, if *k == tab { " on" } else { "" }, *k == tab)).collect::<String>(),
        body = body(&c, &tab),
    ));
    // 열린 채로 다시 그릴 때(정보·배경·사진 탭 바꾸기)는 여는 움직임을 다시 하지 않아요
    if OPEN.get() {
        if let Some(p) = el.query_selector(".ci-panel").ok().flatten() {
            let _ = p.class_list().add_1("still");
        }
    }
}

fn toggle(f: fn(&mut Chat)) {
    let Some(id) = open_chat().map(|c| c.id) else { return };
    let changed = with_chats(|cs| {
        let c = cs.iter_mut().find(|c| c.id == id)?;
        f(c);
        Some(c.clone())
    });
    if let Some(c) = changed {
        super::update_open(|o| {
            o.pin = c.pin;
            o.muted = c.muted;
        });
        touch_pref(&c);
    }
    dom::vibrate(8);
    render();
    paint();
}

fn on_click(e: web_sys::Event) {
    if let Some(t) = dom::closest(&e, "[data-tab]") {
        TAB.with(|x| *x.borrow_mut() = dom::data(&t, "tab"));
        paint();
        if let Some(b) = root().and_then(|r| r.query_selector(".ci-body").ok().flatten()) {
            dom::reveal(&b);
        }
        return;
    }
    if let Some(w) = dom::closest(&e, "[data-wall-pick]") {
        let pick = dom::data(&w, "wall-pick");
        if let (Some(c), true) = (open_chat(), WALLS.contains(&pick.as_str())) {
            super::thread::set_wall(&c.id, &pick);
        }
        return paint();
    }
    if let Some(m) = dom::closest(&e, "[data-uid]") {
        let uid = dom::data(&m, "uid");
        return crate::profile::show(&uid, "", None);
    }
    let Some(b) = dom::closest(&e, "[data-act]") else { return };
    let Some(c) = open_chat() else { return };
    match dom::data(&b, "act").as_str() {
        "close" => close(),
        "mute" => toggle(|c| c.muted = !c.muted),
        "pin" => toggle(|c| c.pin = !c.pin),
        "profile" if !c.group => crate::profile::show(&c.other, &c.name, None),
        "profile" => {
            TAB.with(|x| *x.borrow_mut() = "info".into());
            paint();
        }
        "gift" if !c.group => {
            close();
            super::gift::open(&c.id, &c.other, &c.name);
        }
        "edit" => {
            close();
            super::thread::open_group_sheet(&c);
        }
        _ => {}
    }
}

/// 대화 위쪽 사진이나 이름을 눌렀을 때
pub fn open() {
    let Some(el) = root() else { return };
    if !BOUND.replace(true) {
        dom::on(&el, "click", on_click);
    }
    TAB.with(|t| *t.borrow_mut() = "info".into());
    paint();
    OPEN.set(true);
    el.set_hidden(false);
    let _ = el.class_list().remove_2("closing", "on");
    let _ = el.offset_width();
    let _ = el.class_list().add_1("on");
    dom::vibrate(8);
}
pub fn close() {
    let Some(el) = by_id("cinfo") else { return };
    if !OPEN.replace(false) {
        return;
    }
    let _ = el.class_list().add_1("closing");
    let el2 = el.clone();
    wasm_bindgen_futures::spawn_local(async move {
        dom::sleep(230).await;
        if !OPEN.get() {
            let _ = el2.class_list().remove_2("on", "closing");
            el2.set_hidden(true);
        }
    });
}
