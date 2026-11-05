//! 랭킹: 토큰을 가장 많이 가진 순서 (위 세 명은 단상, 나머지는 줄, 맨 아래는 내 순위)

use std::cell::Cell;

use wasm_bindgen::prelude::*;

use crate::dom::{self, avatar_html, by_id, esc};

const CROWN: &str = r#"<svg class="rk-crown" viewBox="0 0 28 22" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 7l5.500 5L14 3l5.500 9L25 7l-2 11H5L3 7z"/><path d="M6 21h16"/></svg>"#;

thread_local! {
    static BUSY: Cell<bool> = const { Cell::new(false) };
    static AT: Cell<f64> = const { Cell::new(0.0) };
}
fn s(v: &JsValue, k: &str) -> String {
    cloud::get(v, k).as_string().unwrap_or_default()
}
/// 1억이 넘으면 "1.2억", 아니면 천 단위 쉼표
fn fmt_tok(n: f64) -> String {
    if n >= 1e8 {
        let t = format!("{:.1}", n / 1e8);
        format!("{}억", t.strip_suffix(".0").unwrap_or(&t))
    } else {
        cloud::fmt_num(n, 3)
    }
}
fn photo_of(uid: &str) -> String {
    let p = cloud::call_now("peek", &[JsValue::from_str(uid)]).unwrap_or(JsValue::UNDEFINED);
    s(&p, "photo")
}
fn pod(u: Option<&JsValue>, n: usize) -> String {
    match u {
        Some(u) => format!(
            r#"<div class="rk-col g{n}" data-uid="{}" data-name="{}" data-rank="{n}">{CROWN}<span class="rk-n">{n}</span>{}<b class="rk-name">{}</b><span class="rk-val">{}</span></div>"#,
            esc(&s(u, "uid")),
            esc(&s(u, "name")),
            avatar_html(&s(u, "name"), &photo_of(&s(u, "uid")), "avatar contact"),
            esc(&s(u, "name")),
            fmt_tok(cloud::get(u, "bal").as_f64().unwrap_or(0.0))
        ),
        None => format!(r#"<div class="rk-col g{n} empty"><span class="rk-n">{n}</span><b class="rk-name">—</b><span class="rk-val">&nbsp;</span></div>"#),
    }
}
fn paint(list: &[JsValue], my_rank: &str) {
    let me = cloud::cloud().map(|c| s(&c, "uid")).unwrap_or_default();
    if let Some(p) = by_id("rk-podium") {
        p.set_inner_html(&(pod(list.get(1), 2) + &pod(list.first(), 1) + &pod(list.get(2), 3)));
    }
    if let Some(l) = by_id("rk-list") {
        l.set_inner_html(
            &list
                .iter()
                .skip(3)
                .enumerate()
                .map(|(i, u)| {
                    format!(
                        r#"<li class="rk-row{}" data-uid="{}" data-name="{}" data-rank="{}"><span class="rk-rn">{}</span>{}<span class="rk-rname">{}</span><span class="rk-rval">{}</span></li>"#,
                        if s(u, "uid") == me { " me" } else { "" },
                        esc(&s(u, "uid")),
                        esc(&s(u, "name")),
                        i + 4,
                        i + 4,
                        avatar_html(&s(u, "name"), &photo_of(&s(u, "uid")), "avatar contact"),
                        esc(&s(u, "name")),
                        fmt_tok(cloud::get(u, "bal").as_f64().unwrap_or(0.0))
                    )
                })
                .collect::<String>(),
        );
    }
    if let Some(e) = by_id("rk-empty") {
        e.set_hidden(list.len() > 1);
        e.set_text_content(Some("아직 순위에 오른 사람이 적어요"));
    }
    if let Some(m) = by_id("rk-me") {
        let bal = crate::money::balance();
        m.set_hidden(bal.is_none());
        m.set_inner_html(&format!(
            r#"<span class="rk-mn">{my_rank}</span>{}<b>나</b><span class="rk-mval">{} 토큰</span>"#,
            avatar_html(&dom::store_get("profile-name", "나"), &dom::store_get("profile-photo", ""), "avatar contact"),
            fmt_tok(bal.unwrap_or(0.0))
        ));
    }
}
/// 순위를 읽어요. force가 아니면 20초 안에 다시 읽지 않아요 (탭을 오갈 때마다 서버를 읽지 않아요)
pub fn load(force: bool) {
    if !cloud::signed_in() || !crate::chat::live() || BUSY.get() {
        return;
    }
    if !force && js_sys::Date::now() - AT.get() < 20000.0 {
        return;
    }
    BUSY.set(true);
    leptos::task::spawn_local(async move {
        let r: Result<(), cloud::CloudError> = async {
            let bal = crate::money::balance();
            if let Some(b) = bal {
                let _ = cloud::call("syncRank", &[b.into()]).await; // 내 잔액을 먼저 순위표에 올려요
            }
            let list: Vec<JsValue> = js_sys::Array::from(&cloud::call("loadRanks", &[force.into()]).await?).iter().collect();
            let top: js_sys::Array = list.iter().take(12).map(|u| cloud::get(u, "uid")).collect();
            let _ = cloud::call("fetchProfiles", &[top.into()]).await; // 위쪽 사람들 사진
            let me = cloud::cloud().map(|c| s(&c, "uid")).unwrap_or_default();
            let at = list.iter().position(|u| s(u, "uid") == me);
            let mut my = at.map(|i| (i + 1).to_string()).unwrap_or_else(|| "50+".into());
            if at.is_none() {
                if let Ok(above) = cloud::call("countAbove", &[bal.unwrap_or(0.0).into()]).await {
                    let a = above.as_f64().unwrap_or(99.0);
                    if a < 50.0 {
                        my = (a as i64 + 1).to_string();
                    }
                }
            }
            paint(&list, &my);
            AT.set(js_sys::Date::now());
            Ok(())
        }
        .await;
        if r.is_err() {
            if let Some(e) = by_id("rk-empty") {
                e.set_hidden(false);
                e.set_text_content(Some("순위를 불러오지 못했어요. 잠시 뒤 다시 해 주세요"));
            }
        }
        BUSY.set(false);
    });
}
pub fn init() {
    for id in ["rk-podium", "rk-list"] {
        if let Some(el) = by_id(id) {
            dom::on(&el, "click", |e| {
                let Some(b) = dom::closest(&e, "[data-uid]") else { return };
                crate::profile::show(&dom::data(&b, "uid"), &dom::data(&b, "name"), dom::data(&b, "rank").parse().ok());
            });
        }
    }
}
