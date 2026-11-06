//! 피드백 탭: 버그 제보·제안·기타를 개발자(관리자)에게 보내요. 관리 탭에서 관리자가 받은 피드백을 읽고 지워요.

use std::cell::RefCell;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::dom::{self, by_id, esc};

thread_local! {
    static KIND: RefCell<String> = RefCell::new("bug".into());
}

fn area() -> Option<web_sys::HtmlTextAreaElement> {
    by_id("fb-text")?.dyn_into().ok()
}
fn paint() {
    let k = KIND.with(|k| k.borrow().clone());
    for b in dom::qsa(&dom::doc().document_element().expect("html"), "#fb-kinds [data-fb]") {
        let _ = b.set_attribute("aria-pressed", &(dom::data(&b, "fb") == k).to_string());
    }
    let n = area().map(|a| a.value().trim().chars().count()).unwrap_or(0);
    dom::set_text("fb-count", &format!("{n} / 1000"));
    if let Some(b) = by_id("fb-send") {
        let _ = if n >= 2 { b.remove_attribute("disabled") } else { b.set_attribute("disabled", "") };
    }
}

pub fn init() {
    if let Some(g) = by_id("fb-kinds") {
        dom::on(&g, "click", |e| {
            if let Some(b) = dom::closest(&e, "[data-fb]") {
                KIND.with(|k| *k.borrow_mut() = dom::data(&b, "fb"));
                paint();
            }
        });
    }
    if let Some(t) = by_id("fb-text") {
        dom::on(&t, "input", |_| paint());
    }
    if let Some(b) = by_id("fb-send") {
        dom::on(&b, "click", |_| send());
    }
    if let Some(l) = by_id("fb-list") {
        dom::on(&l, "click", |e| {
            if let Some(b) = dom::closest(&e, "[data-fb-del]") {
                let id = dom::data(&b, "fb-del");
                spawn_local(async move {
                    if cloud::call("feedbackDelete", &[JsValue::from_str(&id)]).await.is_ok() {
                        admin_list();
                    }
                });
            }
        });
    }
}

pub fn open() {
    paint();
}

fn send() {
    if !cloud::signed_in() {
        return crate::shell::toast("로그인하고 보낼 수 있어요");
    }
    let Some(a) = area() else { return };
    let text = a.value();
    if text.trim().chars().count() < 2 {
        return;
    }
    let kind = KIND.with(|k| k.borrow().clone());
    if let Some(b) = by_id("fb-send") {
        let _ = b.set_attribute("disabled", "");
    }
    spawn_local(async move {
        match cloud::call("feedbackSend", &[JsValue::from_str(&kind), JsValue::from_str(&text)]).await {
            Ok(_) => {
                if let Some(a) = area() {
                    a.set_value("");
                }
                crate::shell::toast("고마워요. 개발자에게 보냈어요");
            }
            Err(e) => crate::shell::toast(&format!("보내지 못했어요. {}", cloud::why(&e.code))),
        }
        paint();
    });
}

/// 관리 탭: 받은 피드백 (최근 50개)
pub fn admin_list() {
    let Some(l) = by_id("fb-list") else { return };
    spawn_local(async move {
        let Ok(v) = cloud::call("feedbackList", &[]).await else {
            l.set_inner_html(r#"<li class="inv-empty">피드백을 불러오지 못했어요</li>"#);
            return;
        };
        let list = js_sys::Array::from(&v).to_vec();
        let who: js_sys::Array = list.iter().map(|x| cloud::get(x, "by")).collect();
        let _ = cloud::call("fetchProfiles", &[who.into()]).await;
        if list.is_empty() {
            l.set_inner_html(r#"<li class="inv-empty">아직 받은 피드백이 없어요</li>"#);
            return;
        }
        let rows: String = list
            .iter()
            .map(|x| {
                let g = |k: &str| cloud::get(x, k).as_string().unwrap_or_default();
                let p = cloud::call_now("peek", &[JsValue::from_str(&g("by"))]).unwrap_or(JsValue::NULL);
                let name = cloud::get(&p, "name").as_string().unwrap_or_else(|| "알 수 없음".into());
                let kind = match g("kind").as_str() {
                    "bug" => "버그",
                    "idea" => "제안",
                    _ => "기타",
                };
                format!(
                    r#"<li class="fb-row"><div class="fb-h"><em class="fb-tag k-{}">{kind}</em><b>{}</b><time>{}</time><button type="button" class="fb-del" data-fb-del="{}" aria-label="지우기">지우기</button></div><p>{}</p></li>"#,
                    esc(&g("kind")),
                    esc(&name),
                    esc(&cloud::when_ko(cloud::get(x, "at").as_f64().unwrap_or(0.0))),
                    esc(&g("id")),
                    esc(&g("text"))
                )
            })
            .collect();
        l.set_inner_html(&rows);
    });
}
