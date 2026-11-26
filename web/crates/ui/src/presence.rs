//! 접속 중 표시: 앱이 보이는 동안 1분에 한 번 내 시각을 쓰고, 최근 3분 안에 들어온 사람 목록을 1분에 한 번 읽어요.
//! 그 사람들의 동그라미 오른쪽 아래에 초록 점을 붙여요 (인스타처럼).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::dom;

thread_local! {
    static SEEN: RefCell<HashMap<String, f64>> = RefCell::new(HashMap::new());
    static TIMER: Cell<i32> = const { Cell::new(0) };
}

/// 3분 안에 들어왔으면 접속 중
pub fn online(uid: &str) -> bool {
    let now = js_sys::Date::now();
    SEEN.with(|s| s.borrow().get(uid).is_some_and(|t| now - t < 180_000.0))
}
/// 지금 접속 중인 사람 번호들 (나는 빼요)
pub fn online_uids(me: &str) -> Vec<String> {
    let now = js_sys::Date::now();
    SEEN.with(|s| s.borrow().iter().filter(|(u, t)| u.as_str() != me && now - **t < 180_000.0).map(|(u, _)| u.clone()).collect())
}
/// 동그라미에 접속 점을 붙여요 (접속 중이 아니면 그대로)
pub fn wrap(avatar: String, uid: &str) -> String {
    // 뽑기로 얻어 낀 장신구(테두리·든 물건)도 여기서 얹어요
    let deco = crate::gacha::deco_html(&crate::gacha::deco_of(uid));
    if online(uid) {
        format!(r#"<span class="av-on">{avatar}{deco}<i class="on-dot" aria-label="접속 중"></i></span>"#)
    } else if !deco.is_empty() {
        format!(r#"<span class="av-on">{avatar}{deco}</span>"#)
    } else {
        avatar
    }
}

fn tick() {
    if !cloud::signed_in() || dom::doc_hidden() {
        return;
    }
    wasm_bindgen_futures::spawn_local(async {
        let _ = cloud::call("presenceBeat", &[]).await;
        if let Ok(list) = cloud::call("presenceList", &[]).await {
            let mut m = HashMap::new();
            for it in js_sys::Array::from(&list).iter() {
                if let (Some(u), Some(t)) = (cloud::get(&it, "uid").as_string(), cloud::get(&it, "at").as_f64()) {
                    m.insert(u, t);
                }
            }
            let me = cloud::cloud().and_then(|c| cloud::get(&c, "uid").as_string()).unwrap_or_default();
            m.insert(me, js_sys::Date::now());
            let changed = SEEN.with(|s| {
                let mut s = s.borrow_mut();
                let before: Vec<String> = s.keys().cloned().collect();
                let ch = before.len() != m.len() || before.iter().any(|k| !m.contains_key(k));
                *s = m;
                ch
            });
            if changed {
                crate::sns::repaint_people();
                crate::play::repaint();
                crate::chat::render();
            }
        }
    });
}

/// 로그인한 뒤 한 번 불러요
pub fn start() {
    if TIMER.get() != 0 {
        return;
    }
    tick();
    let cb = Closure::<dyn FnMut()>::new(tick).into_js_value();
    if let Some(w) = web_sys::window() {
        TIMER.set(w.set_interval_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 60_000).unwrap_or(0));
    }
    dom::on(&dom::doc(), "visibilitychange", |_| {
        if !dom::doc_hidden() {
            tick();
        }
    });
}
