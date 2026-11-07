//! 친구 초대: 내 초대 링크(…/?ref=내 번호)를 보내고, 그 링크로 친구가 가입하면 친구 한 명마다 50 토큰을 받을 수 있어요.
//! 자동으로 들어오지 않고, 이 화면에서 "받기"를 눌러야 들어와요 (서버 규칙: invites, 지갑 op 'invite').

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::dom::{self, by_id, esc};

const SITE: &str = "https://www.xn--sn3bu3q0e19c.com/";

/// 앱을 열 때: 주소에 초대 표시(?ref=)가 있으면 기억해 둬요 (가입할 때 써요)
pub fn init() {
    if let Some(w) = web_sys::window() {
        if let Ok(q) = w.location().search() {
            if let Ok(p) = web_sys::UrlSearchParams::new_with_str(&q) {
                if let Some(r) = p.get("ref").filter(|r| !r.is_empty() && r.len() <= 128 && r.chars().all(|c| c.is_ascii_alphanumeric())) {
                    dom::store_set("invite-ref", &r);
                }
            }
        }
    }
    if let Some(b) = by_id("invite-share") {
        dom::on(&b, "click", |_| share());
    }
    if let Some(l) = by_id("invite-list") {
        dom::on(&l, "click", |e| {
            if let Some(b) = dom::closest(&e, "[data-claim]") {
                claim(dom::data(&b, "claim"));
            }
        });
    }
}

fn my_uid() -> String {
    cloud::cloud().map(|c| cloud::get(&c, "uid").as_string().unwrap_or_default()).unwrap_or_default()
}

fn share() {
    if !cloud::signed_in() {
        return crate::shell::toast("로그인하고 친구를 초대할 수 있어요");
    }
    let link = format!("{SITE}?ref={}", my_uid());
    let Some(w) = web_sys::window() else { return };
    // 보내는 창 없이 링크만 복사해요
    let _ = w.navigator().clipboard().write_text(&link);
    crate::shell::toast("초대 링크를 복사했어요");
}

/// 친구 초대 화면을 열 때: 내가 초대한 친구 목록을 읽어요
pub fn open() {
    let Some(l) = by_id("invite-list") else { return };
    if !cloud::signed_in() {
        l.set_inner_html(r#"<li class="inv-empty">로그인하면 초대한 친구가 보여요</li>"#);
        return;
    }
    spawn_local(async move {
        let Ok(v) = cloud::call("invitesList", &[]).await else {
            l.set_inner_html(r#"<li class="inv-empty">목록을 불러오지 못했어요. 잠시 뒤 다시 열어 주세요</li>"#);
            return;
        };
        let list: Vec<JsValue> = js_sys::Array::from(&v).to_vec();
        let who: js_sys::Array = list.iter().map(|x| cloud::get(x, "uid")).collect();
        let _ = cloud::call("fetchProfiles", &[who.into()]).await;
        if list.is_empty() {
            l.set_inner_html(r#"<li class="inv-empty">아직 초대한 친구가 없어요</li>"#);
            return;
        }
        let rows: String = list
            .iter()
            .map(|x| {
                let uid = cloud::get(x, "uid").as_string().unwrap_or_default();
                let p = cloud::call_now("peek", &[JsValue::from_str(&uid)]).unwrap_or(JsValue::NULL);
                let name = cloud::get(&p, "name").as_string().filter(|s| !s.is_empty()).unwrap_or_else(|| "새 친구".into());
                let photo = cloud::get(&p, "photo").as_string().unwrap_or_default();
                let done = cloud::get(x, "claimed") == JsValue::TRUE;
                let at = cloud::date_ko(cloud::get(x, "at").as_f64().unwrap_or(0.0));
                format!(
                    r#"<li class="row inv-row">{}<span class="row-main"><b>{}</b><span>{} 가입</span></span>{}</li>"#,
                    dom::avatar_html(&name, &photo, "avatar contact inv-av"),
                    esc(&name),
                    esc(&at),
                    if done { r#"<span class="inv-done">받음</span>"#.to_string() } else { format!(r#"<button type="button" class="btn primary inv-claim" data-claim="{}">50 토큰 받기</button>"#, esc(&uid)) }
                )
            })
            .collect();
        l.set_inner_html(&rows);
    });
}

fn claim(r: String) {
    spawn_local(async move {
        match cloud::call("inviteClaim", &[JsValue::from_str(&r)]).await {
            Ok(_) => {
                crate::shell::toast("친구 초대 보상 50 토큰을 받았어요");
                open();
            }
            Err(e) => crate::shell::toast(&format!("받지 못했어요. {}", cloud::why(&e.code))),
        }
    });
}
