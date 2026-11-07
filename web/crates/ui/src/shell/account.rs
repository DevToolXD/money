//! 계정 화면: 이름·소개·사진 (이 기기와 서버에 저장), 프로필 편집 시트, 알림 스위치, 나누기 버튼

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::{save_settings, sheet_open, toast};
use crate::dom::{self, by_id, safe_img};

pub fn apply_profile() {
    let name = dom::store_get("profile-name", "내 계정");
    dom::set_text("profile-name", &name);
    dom::set_text("profile-bio", &dom::store_get("profile-bio", "소개를 추가해 보세요"));
    let Some(av) = by_id("profile-avatar") else { return };
    // 이름을 바꾸면 기본 프로필(회색 사람 모양) 대신 이름의 첫 글자가 보여요
    let custom = name != "내 계정";
    let photo = dom::store_get("profile-photo", "");
    let photo = safe_img(&photo, 40000);
    av.set_class_name(if custom { "avatar" } else { "avatar-default" });
    let st = av.style();
    let _ = st.set_property("width", "");
    let _ = st.set_property("height", "");
    av.set_text_content(Some(&if custom { name.chars().next().map(String::from).unwrap_or_default() } else { String::new() }));
    let _ = st.set_property("background-image", &if photo.is_empty() { String::new() } else { format!("url({photo})") });
    if !photo.is_empty() {
        let _ = av.class_list().add_1("has-photo");
        av.set_text_content(Some(""));
    }
}

pub fn init() {
    apply_profile();
    let pick = || {
        if let Some(f) = by_id("file-profile") {
            f.click();
        }
    };
    if let Some(b) = by_id("photo-edit") {
        dom::on(&b, "click", move |_| pick());
    }
    if let Some(b) = by_id("profile-avatar") {
        dom::on(&b, "click", move |_| pick());
    }
    if let Some(f) = by_id("file-profile") {
        dom::on(&f, "change", |_| {
            let Some(file) = crate::chat::take_file("file-profile") else { return };
            leptos::task::spawn_local(async move {
                let r = async {
                    let data = crate::chat::read_image(file, 160.0, 36000, 0.8, true).await.map_err(|_| ())?;
                    dom::store_set("profile-photo", &data);
                    apply_profile();
                    if cloud::signed_in() {
                        cloud::call("setPhoto", &[JsValue::from_str(&data)]).await.map_err(|_| ())?;
                    }
                    Ok::<(), ()>(())
                }
                .await;
                toast(if r.is_ok() { "프로필 사진을 바꿨어요" } else { "사진을 바꾸지 못했어요" });
            });
        });
    }
    for b in dom::qsa(&dom::doc().document_element().expect("html"), ".js-edit-profile") {
        dom::on(&b, "click", |_| {
            if let Some(n) = by_id("edit-name").and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok()) {
                n.set_value(&by_id("profile-name").and_then(|p| p.text_content()).unwrap_or_default());
                n.set_disabled(false);
            }
            if let Some(b) = by_id("edit-bio").and_then(|e| e.dyn_into::<web_sys::HtmlTextAreaElement>().ok()) {
                b.set_value(&dom::store_get("profile-bio", ""));
            }
            sheet_open(true, None);
        });
    }
    if let Some(b) = by_id("sheet-cancel") {
        dom::on(&b, "click", |_| sheet_open(false, None));
    }
    if let Some(b) = by_id("sheet-save") {
        dom::on(&b, "click", |_| {
            let bio = by_id("edit-bio").and_then(|e| e.dyn_into::<web_sys::HtmlTextAreaElement>().ok()).map(|t| t.value()).unwrap_or_default();
            let bio = bio.trim();
            dom::store_set("profile-bio", if bio.is_empty() { "소개를 추가해 보세요" } else { bio });
            let name = by_id("edit-name").and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok()).map(|t| t.value()).unwrap_or_default();
            let name = name.trim().to_string();
            let old = dom::store_get("profile-name", "내 계정");
            if !name.is_empty() && name != old {
                let ko = name.chars().count() >= 2 && name.chars().count() <= 6 && name.chars().all(|c| ('가'..='힣').contains(&c));
                let en = name.len() >= 3 && name.len() <= 40 && name.chars().all(|c| c.is_ascii_alphabetic() || c == ' ') && !name.starts_with(' ') && !name.ends_with(' ');
                if !(ko || en) {
                    return toast("이름은 한글 2~6자나 영어로 적어 주세요");
                }
                leptos::task::spawn_local(async move {
                    match cloud::call("setName", &[JsValue::from_str(&name)]).await {
                        Ok(_) => {
                            dom::store_set("profile-name", &name);
                            apply_profile();
                            save_settings();
                            super::ranking::load(true);
                            toast("이름을 바꿨어요");
                        }
                        Err(e) => toast(&format!("이름을 바꾸지 못했어요. {}", cloud::why(&e.code))),
                    }
                });
            }
            apply_profile();
            save_settings();
            sheet_open(false, None);
        });
    }
    if let Some(n) = by_id("toggle-notify") {
        // 켜 두었고 권한도 있으면 켜진 채로 보여요
        let on0 = dom::store_get("notify-on", "") == "1" && crate::notify::permission() == "granted";
        let _ = n.set_attribute("aria-checked", &on0.to_string());
        let n2 = n.clone();
        dom::on(&n, "click", move |_| {
            let want = n2.get_attribute("aria-checked").as_deref() != Some("true");
            let n3 = n2.clone();
            if !want {
                dom::store_set("notify-on", "");
                let _ = n3.set_attribute("aria-checked", "false");
                return;
            }
            leptos::task::spawn_local(async move {
                if crate::notify::ask_permission().await {
                    dom::store_set("notify-on", "1");
                    let _ = n3.set_attribute("aria-checked", "true");
                    toast("새 메시지 알림을 켰어요");
                } else {
                    let _ = n3.set_attribute("aria-checked", "false");
                    toast(if crate::notify::permission() == "denied" { "알림이 막혀 있어요. 휴대폰 설정에서 이 사이트 알림을 허용해 주세요" } else { "이 브라우저는 알림을 지원하지 않아요. 아이폰은 홈 화면에 추가한 앱에서만 돼요" });
                }
            });
        });
    }
    let segs = dom::qsa(&dom::doc().document_element().expect("html"), ".segment button");
    for b in segs.clone() {
        let segs = segs.clone();
        let b2 = b.clone();
        dom::on(&b, "click", move |_| {
            for o in &segs {
                let _ = o.set_attribute("aria-selected", &(*o == b2).to_string());
            }
        });
    }
}
