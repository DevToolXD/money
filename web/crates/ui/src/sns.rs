//! SNS 탭 (지금은 위쪽만): 큰 "SNS" 제목, 오른쪽에 알림 종과 내 동그라미, 아래에 사람들 동그라미 줄.
//! 사람을 누르면 초록 테두리로 골라져요. 사람 정보는 30분 캐시를 쓰고, 목록은 탭을 열 때 한 번만 읽어요.

use std::cell::{Cell, RefCell};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::dom::{self, by_id, esc};

thread_local! {
    static PEOPLE: RefCell<Vec<(String, String, String)>> = const { RefCell::new(Vec::new()) }; // 번호, 이름, 사진
    static PICK: RefCell<String> = const { RefCell::new(String::new()) };
    static LOADED: Cell<bool> = const { Cell::new(false) };
    static BOUND: Cell<bool> = const { Cell::new(false) };
}

fn me() -> (String, String) {
    let Some(c) = cloud::cloud() else { return (String::new(), String::new()) };
    let uid = cloud::get(&c, "uid").as_string().unwrap_or_default();
    let p = cloud::call_now("peek", &[JsValue::from_str(&uid)]).unwrap_or(JsValue::NULL);
    let name = cloud::get(&p, "name").as_string().filter(|s| !s.is_empty()).or_else(|| cloud::get(&cloud::get(&c, "user"), "name").as_string()).unwrap_or_default();
    (name, cloud::get(&p, "photo").as_string().unwrap_or_default())
}

fn paint() {
    if let Some(m) = by_id("sns-me") {
        let (n, ph) = me();
        m.set_inner_html(&if cloud::signed_in() { dom::avatar_html(&n, &ph, "avatar contact sns-me-av") } else { String::new() });
    }
    let Some(el) = by_id("sns-people") else { return };
    let pick = PICK.with(|p| p.borrow().clone());
    let html: String = PEOPLE.with(|p| {
        p.borrow()
            .iter()
            .map(|(uid, name, photo)| {
                format!(
                    r#"<button type="button" class="sns-p{}" data-uid="{}" role="listitem"><span class="sns-ring">{}</span><span class="sns-n">{}</span></button>"#,
                    if *uid == pick { " on" } else { "" },
                    esc(uid),
                    dom::avatar_html(name, photo, "avatar contact sns-av"),
                    esc(name)
                )
            })
            .collect()
    });
    el.set_inner_html(&if html.is_empty() && !cloud::signed_in() { r#"<p class="sns-empty">로그인하면 사람들이 보여요</p>"#.into() } else { html });
}

/// SNS 탭을 열 때
pub fn open() {
    if !BOUND.replace(true) {
        if let Some(el) = by_id("sns-people") {
            dom::on(&el, "click", |e| {
                let Some(b) = dom::closest(&e, "[data-uid]") else { return };
                let uid = dom::data(&b, "uid");
                PICK.with(|p| *p.borrow_mut() = uid.clone());
                paint();
                let name = PEOPLE.with(|p| p.borrow().iter().find(|x| x.0 == uid).map(|x| x.1.clone()).unwrap_or_default());
                crate::profile::show(&uid, &name, None);
            });
        }
        if let Some(b) = by_id("sns-bell") {
            dom::on(&b, "click", |_| crate::shell::toast("새 알림이 없어요"));
        }
    }
    paint();
    if !cloud::signed_in() || LOADED.replace(true) {
        return;
    }
    spawn_local(async {
        let list = cloud::call("listUsers", &[true.into()]).await.ok().map(|v| js_sys::Array::from(&cloud::get(&v, "list")).to_vec()).unwrap_or_default();
        let ppl: Vec<(String, String, String)> = list
            .iter()
            .map(|v| {
                let s = |k: &str| cloud::get(v, k).as_string().unwrap_or_default();
                let n = s("name");
                (s("uid"), if n.is_empty() { s("id") } else { n }, s("photo"))
            })
            .collect();
        PEOPLE.with(|p| *p.borrow_mut() = ppl);
        paint();
    });
}
