//! 뽑기와 인벤: 메뉴(줄 세 개)에서 들어가요. 10 멸공으로 한 번 뽑으면 확률에 따라 장신구가 나와요.
//! 장신구는 디스코드 프로필 꾸미기처럼 동그란 사진 둘레의 테두리와 손에 든 물건이에요.
//!   일반 70%: 나뭇가지를 든 나무 테두리 / 희귀 25%: 안경을 쓴 은빛 테두리 / 전설 5%: 구찌 지갑을 든 금빛 테두리
//! 무엇이 나올지는 서버가 찍은 시각으로 정해져서 앱이 고를 수 없어요 (규칙이 같은 계산으로 끼기를 검사해요).
//! 인벤에서 낀 장신구는 내 프로필에 저장돼, 다른 사람 화면의 내 동그라미에도 보여요.

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::dom::{self, by_id, esc};

/// (번호, 이름, 등급 이름, 확률 %)
pub const ITEMS: [(&str, &str, &str, u32); 3] = [("gucci", "구찌 지갑", "전설", 5), ("glasses", "안경", "희귀", 25), ("branch", "나뭇가지", "일반", 70)];

thread_local! {
    /// 내가 뽑은 것 (뽑기 번호, 장신구)
    static MINE: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
    static LOADED: Cell<bool> = const { Cell::new(false) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
    static BOUND: Cell<bool> = const { Cell::new(false) };
}

/// 서버 규칙과 같은 계산: 뽑은 시각(밀리초)의 끝 두 자리
pub fn tier(at_ms: f64) -> &'static str {
    let m = (at_ms.floor() as i64).rem_euclid(100);
    if m < 5 {
        "gucci"
    } else if m < 30 {
        "glasses"
    } else {
        "branch"
    }
}
/// 받침이 있으면 "을", 없으면 "를"
fn eul(w: &str) -> &'static str {
    match w.chars().last() {
        Some(c) if ('가'..='힣').contains(&c) && (c as u32 - 0xAC00) % 28 != 0 => "을",
        _ => "를",
    }
}
fn info(item: &str) -> (&'static str, &'static str) {
    ITEMS.iter().find(|x| x.0 == item).map(|x| (x.1, x.2)).unwrap_or(("", ""))
}

/// 장신구 그림 (사진보다 20% 큰 판, 0~120 좌표. 사진은 가운데 반지름 50)
fn art(item: &str) -> &'static str {
    match item {
        // 나무 테두리 + 오른쪽 아래 손에 든 나뭇가지와 잎 두 장
        "branch" => r##"<circle cx="60" cy="60" r="54" fill="none" stroke="#9a6b3f" stroke-width="5"/><circle cx="60" cy="60" r="54" fill="none" stroke="#c89464" stroke-width="1.6" stroke-dasharray="7 9"/><path d="M24 22c4-8 12-10 18-7-5 1-9 4-11 9z" fill="#5fae4e"/><path d="M80 116L113 66" stroke="#7a4b24" stroke-width="5.5" stroke-linecap="round"/><path d="M101 84l13-4M94 94l-4-12" stroke="#7a4b24" stroke-width="3.2" stroke-linecap="round"/><path d="M114 80c6-6 6-13 2-17-5 3-7 10-2 17z" fill="#5fae4e"/><path d="M89 81c-8-3-12-9-10-14 6 0 11 6 10 14z" fill="#6cc05a"/><circle cx="90" cy="104" r="9" fill="#f2c7a0" stroke="#d9a47c" stroke-width="1.5"/>"##,
        // 은빛 테두리 + 얼굴에 쓴 안경
        "glasses" => r##"<circle cx="60" cy="60" r="54" fill="none" stroke="url(#dgS)" stroke-width="6"/><path d="M98 18l2 5 5 2-5 2-2 5-2-5-5-2 5-2z" fill="#fff"/><rect x="28" y="46" width="27" height="19" rx="8" fill="rgba(150,205,255,.28)" stroke="#16181c" stroke-width="4"/><rect x="65" y="46" width="27" height="19" rx="8" fill="rgba(150,205,255,.28)" stroke="#16181c" stroke-width="4"/><path d="M55 54q5-5 10 0" fill="none" stroke="#16181c" stroke-width="3.5" stroke-linecap="round"/><path d="M28 52l-12-4M92 52l12-4" stroke="#16181c" stroke-width="3.5" stroke-linecap="round"/><path d="M33 50l7 0" stroke="#fff" stroke-width="2.4" stroke-linecap="round" opacity=".8"/>"##,
        // 금빛 테두리와 반짝임 + 오른쪽 아래 손에 든 지갑 (짙은 가죽, 금 잠금쇠)
        "gucci" => r##"<circle cx="60" cy="60" r="54" fill="none" stroke="url(#dgG)" stroke-width="7"/><circle cx="60" cy="60" r="58.5" fill="none" stroke="#f5d061" stroke-width="1" opacity=".7"/><path d="M18 20l2.5 6 6 2.5-6 2.5-2.5 6-2.5-6-6-2.5 6-2.5z" fill="#ffe27a"/><path d="M104 10l1.8 4.2 4.2 1.8-4.2 1.8-1.8 4.2-1.8-4.2-4.2-1.8 4.2-1.8z" fill="#fff6c8"/><rect x="72" y="80" width="44" height="31" rx="6" fill="#3a2418" stroke="#d4a017" stroke-width="2.5"/><path d="M72 91h44" stroke="#d4a017" stroke-width="1.6"/><rect x="88" y="86" width="12" height="10" rx="3" fill="url(#dgG)" stroke="#8a6206" stroke-width="1.2"/><circle cx="80" cy="108" r="9" fill="#f2c7a0" stroke="#d9a47c" stroke-width="1.5"/>"##,
        _ => "",
    }
}
/// 동그라미 위에 얹는 장신구 (없으면 빈 글자)
pub fn deco_html(item: &str) -> String {
    let a = art(item);
    if a.is_empty() {
        return String::new();
    }
    format!(r#"<i class="deco deco-{item}" aria-hidden="true"><svg viewBox="0 0 120 120">{a}</svg></i>"#)
}
/// 이 사람이 낀 장신구 (프로필 캐시에서)
pub fn deco_of(uid: &str) -> String {
    let p = cloud::call_now("peek", &[JsValue::from_str(uid)]).unwrap_or(JsValue::NULL);
    cloud::get(&p, "deco").as_string().unwrap_or_default()
}

fn my_uid() -> String {
    cloud::cloud().and_then(|c| cloud::get(&c, "uid").as_string()).unwrap_or_default()
}
/// 내 동그라미 (큰 것) + 장신구
fn me_avatar(item: &str, cls: &str) -> String {
    let p = cloud::call_now("peek", &[JsValue::from_str(&my_uid())]).unwrap_or(JsValue::NULL);
    let g = |k: &str| cloud::get(&p, k).as_string().unwrap_or_default();
    let name = if g("name").is_empty() { dom::store_get("profile-name", "나") } else { g("name") };
    let photo = if g("photo").is_empty() { dom::store_get("profile-photo", "") } else { g("photo") };
    format!(r#"<span class="dv {cls}">{}{}</span>"#, dom::avatar_html(&name, &photo, "avatar contact"), deco_html(item))
}

async fn load_mine() {
    let Ok(v) = cloud::call("gachaList", &[]).await else { return };
    let list: Vec<(String, String)> = js_sys::Array::from(&v).iter().map(|x| (cloud::get(&x, "id").as_string().unwrap_or_default(), tier(cloud::get(&x, "at").as_f64().unwrap_or(0.0)).to_string())).collect();
    MINE.with(|m| *m.borrow_mut() = list);
    LOADED.set(true);
}

/* ---------- 뽑기 화면 ---------- */

fn paint_gacha() {
    let Some(root) = by_id("gacha-root") else { return };
    if !cloud::signed_in() {
        return dom::set_html_if(&root, r#"<p class="sns-empty">로그인하면 뽑기를 할 수 있어요</p>"#);
    }
    let bal = crate::money::balance().unwrap_or(0.0);
    let rows: String = ITEMS
        .iter()
        .map(|(k, n, r, p)| format!(r#"<li class="gc-odd {k}">{}<span class="gc-t"><b>{n}</b><small>{r}</small></span><em>{p}%</em></li>"#, me_avatar(k, "sm")))
        .collect();
    dom::set_html_if(
        &root,
        &format!(
            r#"<div class="card gc-hero"><div class="gc-ball" id="gc-ball"><span class="gc-shine"></span></div><p class="gc-sub">동그라미 사진을 꾸미는 장신구가 나와요</p><button type="button" class="gc-go" id="gc-go">10 멸공으로 뽑기</button><p class="gc-bal">내 멸공 <b>{}</b></p></div>
<h2 class="section-title">나올 수 있는 것</h2><ul class="card gc-odds">{rows}</ul>"#,
            crate::money::w(bal)
        ),
    );
}
pub fn open_gacha() {
    bind_once();
    paint_gacha();
}
async fn pull() {
    if BUSY.replace(true) {
        return;
    }
    if let Err(m) = crate::money::ensure(10.0).await {
        BUSY.set(false);
        crate::shell::ask("뽑기를 할 수 없어요", &m, "확인", "").await;
        return;
    }
    let ball = by_id("gc-ball");
    if let Some(b) = &ball {
        let _ = b.class_list().remove_1("open");
        let _ = b.class_list().add_1("shake");
    }
    dom::vibrate(10);
    let f = Closure::once_into_js(|| cloud::call_now("gachaPull", &[]).unwrap_or(JsValue::UNDEFINED));
    let r = cloud::settle(crate::money::wallet_op(f.unchecked_into()).into()).await;
    // 흔드는 움직임을 다 보여주고 열어요
    dom::sleep(700).await;
    if let Some(b) = &ball {
        let _ = b.class_list().remove_1("shake");
    }
    BUSY.set(false);
    match r {
        Ok(v) => {
            let id = cloud::get(&v, "id").as_string().unwrap_or_default();
            let item = tier(cloud::get(&v, "at").as_f64().unwrap_or(0.0));
            MINE.with(|m| m.borrow_mut().push((id.clone(), item.to_string())));
            crate::money::log(&dom::obj(&[("t", js_sys::Date::now().into()), ("name", JsValue::from_str("뽑기")), ("cat", "쇼핑".into()), ("icon", "gift".into()), ("amt", (-10.0).into())]));
            show_result(item, &id);
            paint_gacha();
        }
        Err(e) => crate::shell::toast(&format!("뽑지 못했어요. {}", cloud::why(&e.code))),
    }
}
/// 결과 창: 내 동그라미에 새 장신구를 얹어 보여주고, 바로 끼거나 닫아요
fn show_result(item: &str, pull_id: &str) {
    let Some(el) = by_id("gc-result") else { return };
    let (n, r) = info(item);
    el.set_inner_html(&format!(
        r#"<div class="gc-card {item}"><p class="gc-tier">{r}</p>{}<h3>{n}</h3><div class="gc-acts"><button type="button" class="gc-btn ghost" data-act="close">확인</button><button type="button" class="gc-btn" data-act="wear" data-item="{item}" data-pull="{}">바로 끼기</button></div></div>"#,
        me_avatar(item, "lg"),
        esc(pull_id)
    ));
    el.set_hidden(false);
    let _ = el.offset_width();
    let _ = el.class_list().add_1("on");
    dom::vibrate(if item == "gucci" { 30 } else { 12 });
}
fn hide_result() {
    let Some(el) = by_id("gc-result") else { return };
    let _ = el.class_list().remove_1("on");
    spawn_local(async move {
        dom::sleep(260).await;
        if !el.class_list().contains("on") {
            el.set_hidden(true);
        }
    });
}
async fn wear(item: String, pull_id: String) {
    match cloud::call("decoSet", &[JsValue::from_str(&item), JsValue::from_str(&pull_id)]).await {
        Ok(_) => {
            let n = info(&item).0;
            crate::shell::toast(&if item.is_empty() { "장신구를 뺐어요".to_string() } else { format!("{n}{}꼈어요", eul(n)) });
            paint_inv();
            crate::sns::repaint_people();
        }
        Err(e) => crate::shell::toast(&format!("끼지 못했어요. {}", cloud::why(&e.code))),
    }
}

/* ---------- 인벤 화면 ---------- */

fn paint_inv() {
    let Some(root) = by_id("inv-root") else { return };
    if !cloud::signed_in() {
        return dom::set_html_if(&root, r#"<p class="sns-empty">로그인하면 인벤을 볼 수 있어요</p>"#);
    }
    let now = deco_of(&my_uid());
    let mine = MINE.with(|m| m.borrow().clone());
    let cards: String = ITEMS
        .iter()
        .map(|(k, n, r, _)| {
            let have: Vec<&(String, String)> = mine.iter().filter(|x| x.1 == *k).collect();
            let foot = if have.is_empty() {
                r#"<span class="iv-lock">아직 없어요</span>"#.to_string()
            } else if now == *k {
                r#"<button type="button" class="iv-btn on" data-act="off">끼는 중</button>"#.to_string()
            } else {
                format!(r#"<button type="button" class="iv-btn" data-act="wear" data-item="{k}" data-pull="{}">끼기</button>"#, esc(&have[0].0))
            };
            format!(
                r#"<li class="iv-card {k}{}">{}<b>{n}</b><small>{r}{}</small>{foot}</li>"#,
                if have.is_empty() { " locked" } else { "" },
                me_avatar(k, "md"),
                if have.len() > 1 { format!(" · {}개", have.len()) } else { String::new() }
            )
        })
        .collect();
    let (cur_n, _) = info(&now);
    dom::set_html_if(
        &root,
        &format!(
            r#"<div class="card iv-hero">{}<p>{}</p></div><h2 class="section-title">내 장신구</h2><ul class="iv-grid">{cards}</ul>{}"#,
            me_avatar(&now, "xl"),
            if cur_n.is_empty() { "끼고 있는 장신구가 없어요".to_string() } else { format!("<b>{cur_n}</b>{} 끼고 있어요", eul(cur_n)) },
            if mine.is_empty() { r#"<p class="iv-empty">메뉴의 뽑기에서 10 멸공으로 장신구를 뽑아 보세요</p>"# } else { "" }
        ),
    );
}
pub fn open_inv() {
    bind_once();
    paint_inv();
    if cloud::signed_in() {
        spawn_local(async {
            load_mine().await;
            paint_inv();
        });
    }
}

fn bind_once() {
    if BOUND.replace(true) {
        return;
    }
    if let Some(r) = by_id("gacha-root") {
        dom::on(&r, "click", |e| {
            if dom::closest(&e, "#gc-go").is_some() {
                spawn_local(pull());
            }
        });
    }
    let act = |e: web_sys::Event| {
        let Some(b) = dom::closest(&e, "[data-act]") else {
            if dom::closest(&e, ".gc-card").is_none() && dom::closest(&e, "#gc-result").is_some() {
                hide_result();
            }
            return;
        };
        match dom::data(&b, "act").as_str() {
            "wear" => {
                hide_result();
                spawn_local(wear(dom::data(&b, "item"), dom::data(&b, "pull")));
            }
            "off" => spawn_local(wear(String::new(), String::new())),
            _ => hide_result(),
        }
    };
    for id in ["gc-result", "inv-root"] {
        if let Some(r) = by_id(id) {
            dom::on(&r, "click", act);
        }
    }
}
