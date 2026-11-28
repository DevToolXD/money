//! 접속 중 표시: 앱이 보이는 동안 1분에 한 번 내 시각을 쓰고, 최근 3분 안에 들어온 사람 목록은 2분에 한 번 읽어요 (읽기를 반으로 아껴요).
//! 그 사람들의 동그라미 오른쪽 아래에 초록 점을 붙여요 (인스타처럼).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::dom;

thread_local! {
    static SEEN: RefCell<HashMap<String, f64>> = RefCell::new(HashMap::new());
    static TIMER: Cell<i32> = const { Cell::new(0) };
    static TICKS: Cell<u32> = const { Cell::new(0) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
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
    // 상점에서 사서 낀 장신구도 여기서 얹어요
    let deco = crate::shop::deco_html(&crate::shop::deco_of(uid));
    if online(uid) {
        format!(r#"<span class="av-on">{avatar}{deco}<i class="on-dot" aria-label="접속 중"></i></span>"#)
    } else if !deco.is_empty() {
        format!(r#"<span class="av-on">{avatar}{deco}</span>"#)
    } else {
        avatar
    }
}

/// 월급: 앱을 켜 두고 본 시간 5분마다 20 멸공이 들어와요 (받을 게 있으면 지갑 쓰기 줄에 서서 받아요)
fn pay_salary() {
    if BUSY.replace(true) {
        return;
    }
    wasm_bindgen_futures::spawn_local(async move {
        let due = cloud::call("salaryDue", &[]).await.ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
        if due < 1.0 {
            BUSY.set(false);
            return;
        }
        let f = Closure::once_into_js(move || cloud::call_now("salaryClaim", &[due.into()]).unwrap_or(JsValue::UNDEFINED));
        let r = cloud::settle(crate::money::wallet_op(f.unchecked_into()).into()).await;
        BUSY.set(false);
        if let Ok(v) = r {
            let amt = v.as_f64().unwrap_or(0.0);
            if amt > 0.0 {
                crate::notify::show_info("월급", &format!("월급 {} 멸공이 들어왔어요", amt as i64));
                crate::money::log(&dom::obj(&[
                    ("t", js_sys::Date::now().into()),
                    ("name", "월급 (5분마다 20 멸공)".into()),
                    ("cat", "월급".into()),
                    ("icon", "gift".into()),
                    ("amt", amt.into()),
                ]));
            }
        }
    });
}

fn tick() {
    if !cloud::signed_in() || dom::doc_hidden() {
        return;
    }
    wasm_bindgen_futures::spawn_local(async {
        if cloud::call("presenceBeat", &[]).await.is_ok() {
            pay_salary();
        }
        // 목록은 두 번에 한 번만 읽어요 (처음은 바로)
        let n = TICKS.get();
        TICKS.set(n.wrapping_add(1));
        if n % 2 != 0 {
            return;
        }
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
/// 처음 들어왔을 때 한 번, 월급을 아주 간단히 알려요
fn tip() {
    if dom::store_get("salary-tip", "") == "1" {
        return;
    }
    wasm_bindgen_futures::spawn_local(async {
        dom::sleep(2500).await;
        crate::notify::show_info("멸공 월급", "5분마다 20 멸공을 받아요 (앱을 보는 동안만)");
        dom::store_set("salary-tip", "1");
    });
}

pub fn start() {
    if TIMER.get() != 0 {
        return;
    }
    tip();
    tick();
    let cb = Closure::<dyn FnMut()>::new(tick).into_js_value();
    if let Some(w) = web_sys::window() {
        TIMER.set(w.set_interval_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 60_000).unwrap_or(0));
    }
    dom::on(&dom::doc(), "visibilitychange", |_| {
        if !dom::doc_hidden() {
            // 뒤에 가 있던 시간은 월급 시간에 넣지 않아요 (돌아온 때부터 다시 세요)
            let _ = cloud::call_now("playResume", &[]);
            tick();
        }
    });
}
