//! 사람 카드: 메시지·랭킹·SNS에서 사람을 누르면 아래에서 리퀴드 글라스 카드가 올라와요.
//! 큰 사진, 이름, @아이디, (알면) 순위, 그리고 메시지 버튼. 사람 정보는 30분 캐시(peek)를 먼저 쓰고 없을 때만 읽어요.

use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::dom::{self, esc};

const SEND: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M21 3L10.5 13.5"/><path d="M21 3l-6.5 18-4-7.5L3 9.5 21 3z"/></svg>"#;
const X: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" aria-hidden="true"><path d="M7 7l10 10M17 7L7 17"/></svg>"#;

thread_local! {
    static EL: RefCell<Option<web_sys::HtmlElement>> = const { RefCell::new(None) };
    static WHO: RefCell<String> = const { RefCell::new(String::new()) };
}

fn info(uid: &str) -> (String, String, String) {
    let p = cloud::call_now("peek", &[JsValue::from_str(uid)]).unwrap_or(JsValue::NULL);
    let g = |k: &str| cloud::get(&p, k).as_string().unwrap_or_default();
    (g("name"), g("id"), g("photo"))
}

fn paint(el: &web_sys::HtmlElement, uid: &str, name: &str, rank: Option<usize>) {
    let (n, id, photo) = info(uid);
    let name = if n.is_empty() { name.to_string() } else { n };
    let me = cloud::cloud().map(|c| cloud::get(&c, "uid").as_string().unwrap_or_default()).unwrap_or_default() == uid;
    el.set_inner_html(&format!(
        r#"<div class="pf-card lg-card" role="dialog" aria-modal="true" aria-label="{n}"><span class="pf-grab"></span><button type="button" class="pf-x glass-btn" data-act="close" aria-label="닫기">{X}</button><div class="pf-av">{av}</div><h2 class="pf-name">{n}</h2><p class="pf-sub">{id}{rk}</p>{btn}</div>"#,
        n = esc(&name),
        av = dom::avatar_html(&name, &photo, "avatar contact pf-avatar"),
        id = if id.is_empty() { String::new() } else { format!("@{}", esc(&id)) },
        rk = rank.map(|r| format!(r#"<span class="pf-rank">{r}위</span>"#)).unwrap_or_default(),
        btn = if me { String::new() } else { format!(r#"<button type="button" class="pf-msg lg-btn" data-act="msg">{SEND}<span>메시지</span></button>"#) },
    ));
}

fn root() -> Option<web_sys::HtmlElement> {
    if let Some(e) = EL.with(|e| e.borrow().clone()) {
        return Some(e);
    }
    let app = dom::app_el()?;
    let el: web_sys::HtmlElement = dom::doc().create_element("div").ok()?.dyn_into().ok()?;
    el.set_class_name("pf");
    el.set_hidden(true);
    let _ = app.append_child(&el);
    dom::on(&el, "click", |e| {
        let act = dom::closest(&e, "[data-act]").map(|b| dom::data(&b, "act"));
        match act.as_deref() {
            Some("close") => hide(),
            Some("msg") => {
                let uid = WHO.with(|w| w.borrow().clone());
                hide();
                let (n, id, photo) = info(&uid);
                leptos::task::spawn_local(async move {
                    match crate::chat::people::chat_with(&uid, &id, &n, &photo).await {
                        Ok(cid) => crate::chat::thread::open(&cid),
                        Err(_) => crate::shell::toast("대화를 열지 못했어요"),
                    }
                });
            }
            _ => {
                // 카드 밖(어두운 곳)을 누르면 닫혀요
                if dom::closest(&e, ".pf-card").is_none() {
                    hide();
                }
            }
        }
    });
    EL.with(|e| *e.borrow_mut() = Some(el.clone()));
    Some(el)
}

/// 사람 카드를 열어요. name은 캐시에 없을 때 잠깐 보여줄 이름이에요
pub fn show(uid: &str, name: &str, rank: Option<usize>) {
    if uid.is_empty() {
        return;
    }
    let Some(el) = root() else { return };
    WHO.with(|w| *w.borrow_mut() = uid.to_string());
    paint(&el, uid, name, rank);
    el.set_hidden(false);
    let _ = el.offset_width();
    let _ = el.class_list().add_1("on");
    dom::vibrate(6);
    if cloud::call_now("peek", &[JsValue::from_str(uid)]).map(|v| v.is_null()).unwrap_or(true) {
        let (uid, name) = (uid.to_string(), name.to_string());
        leptos::task::spawn_local(async move {
            let _ = cloud::call("fetchProfiles", &[js_sys::Array::of1(&JsValue::from_str(&uid)).into()]).await;
            if WHO.with(|w| *w.borrow() == uid) {
                if let Some(el) = EL.with(|e| e.borrow().clone()) {
                    paint(&el, &uid, &name, rank);
                }
            }
        });
    }
}

pub fn hide() {
    let Some(el) = EL.with(|e| e.borrow().clone()) else { return };
    let _ = el.class_list().remove_1("on");
    leptos::task::spawn_local(async move {
        dom::sleep(320).await;
        if !el.class_list().contains("on") {
            el.set_hidden(true);
        }
    });
}
