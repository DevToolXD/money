//! 뽑기와 인벤: 메뉴(줄 세 개)에서 들어가요. 10 멸공으로 한 번 뽑으면 확률에 따라 장신구가 나와요.
//! 장신구는 테두리 없이 물건만 딱: 실제 사진에서 배경을 지운(누끼) 그림을 동그란 사진 위 제자리에 얹어요
//! (안경은 얼굴, 모자·헤드폰·오리는 머리, 목걸이는 목, 시계는 손목, 바나나·지갑·금괴 같은 건 손, 람보르기니는 발밑).
//! 자리마다 하나씩, 여러 개를 같이 낄 수 있어요 (최대 6개).
//! 무엇이 나올지는 서버가 찍은 시각(밀리초 끝 세 자리)으로 정해져서 앱이 고를 수 없어요. 규칙이 같은 계산으로 끼기를 검사해요.
//! 사진은 위키미디어 공용의 자유 이용 사진이고, 출처는 뽑기 화면 맨 아래에 있어요.

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::dom::{self, by_id, esc};

/// (번호, 이름, 등급, 천분율 확률, 자리). 순서와 확률은 서버 규칙(pullOf)과 같아야 해요
pub const ITEMS: [(&str, &str, &str, u32, &str); 13] = [
    ("lambo", "람보르기니", "전설", 15, "ground"),
    ("goldbar", "금괴", "전설", 25, "hand"),
    ("wallet", "명품 지갑", "영웅", 35, "hand"),
    ("watch", "손목시계", "영웅", 40, "wrist"),
    ("chain", "금목걸이", "영웅", 45, "neck"),
    ("headphones", "헤드폰", "희귀", 55, "head"),
    ("cap", "야구모자", "희귀", 60, "head"),
    ("sunglasses", "선글라스", "희귀", 60, "face"),
    ("glasses", "안경", "희귀", 65, "face"),
    ("coffee", "커피", "일반", 140, "hand"),
    ("duck", "고무 오리", "일반", 150, "head"),
    ("banana", "바나나", "일반", 150, "hand"),
    ("branch", "나뭇가지", "일반", 160, "hand"),
];
/// 사진 출처 (이름, 작가, 라이선스, 원본 주소)
const CREDITS: &[(&str, &str, &str, &str)] = &include!("deco_credits.in");

thread_local! {
    /// 내가 뽑은 것 (뽑기 번호, 장신구)
    static MINE: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
    static BOUND: Cell<bool> = const { Cell::new(false) };
}

/// 10연차에서 n번째 칸에 더하는 간격 (서버 규칙과 같아요)
const OFF: [i64; 10] = [0, 383, 761, 127, 512, 894, 266, 649, 38, 421];
/// 서버 규칙과 같은 계산: 뽑은 시각(밀리초)에 칸 간격을 더한 끝 세 자리
pub fn tier(at_ms: f64, n: usize) -> &'static str {
    let m = (at_ms.floor() as i64 + OFF[n.min(9)]).rem_euclid(1000) as u32;
    let mut acc = 0;
    for (k, _, _, p, _) in ITEMS {
        acc += p;
        if m < acc {
            return k;
        }
    }
    "branch"
}
/// 받침이 있으면 "을", 없으면 "를"
fn eul(w: &str) -> &'static str {
    match w.chars().last() {
        Some(c) if ('가'..='힣').contains(&c) && (c as u32 - 0xAC00) % 28 != 0 => "을",
        _ => "를",
    }
}
fn item(k: &str) -> Option<&'static (&'static str, &'static str, &'static str, u32, &'static str)> {
    ITEMS.iter().find(|x| x.0 == k)
}
fn rarity_cls(r: &str) -> &'static str {
    match r {
        "전설" => "t4",
        "영웅" => "t3",
        "희귀" => "t2",
        _ => "t1",
    }
}

/// 동그라미 위에 얹는 장신구들 (쉼표로 이은 번호들. 없으면 빈 글자). 자리마다 위치가 정해져 있어요
pub fn deco_html(keys: &str) -> String {
    keys.split(',')
        .filter_map(item)
        .map(|(k, n, ..)| format!(r#"<img class="deco dc-{k}" src="app/deco/{k}.webp" alt="{n}" draggable="false" decoding="async">"#))
        .collect()
}
/// 이 사람이 낀 장신구 번호들 (쉼표로 이어요, 프로필 캐시에서)
pub fn deco_of(uid: &str) -> String {
    let p = cloud::call_now("peek", &[JsValue::from_str(uid)]).unwrap_or(JsValue::NULL);
    let list = cloud::get(&p, "decos");
    if !js_sys::Array::is_array(&list) {
        return String::new();
    }
    js_sys::Array::from(&list).iter().filter_map(|x| cloud::get(&x, "k").as_string()).filter(|k| item(k).is_some()).collect::<Vec<_>>().join(",")
}
/// 지금 내가 낀 것 (번호, 뽑기 번호)
fn my_decos() -> Vec<(String, String)> {
    let p = cloud::call_now("peek", &[JsValue::from_str(&my_uid())]).unwrap_or(JsValue::NULL);
    let list = cloud::get(&p, "decos");
    if !js_sys::Array::is_array(&list) {
        return Vec::new();
    }
    js_sys::Array::from(&list)
        .iter()
        .filter_map(|x| Some((cloud::get(&x, "k").as_string()?, cloud::get(&x, "p").as_string()?)))
        .filter(|(k, _)| item(k).is_some())
        .collect()
}

fn my_uid() -> String {
    cloud::cloud().and_then(|c| cloud::get(&c, "uid").as_string()).unwrap_or_default()
}
/// 내 동그라미 + 장신구들
fn me_avatar(keys: &str, cls: &str) -> String {
    let p = cloud::call_now("peek", &[JsValue::from_str(&my_uid())]).unwrap_or(JsValue::NULL);
    let g = |k: &str| cloud::get(&p, k).as_string().unwrap_or_default();
    let name = if g("name").is_empty() { dom::store_get("profile-name", "나") } else { g("name") };
    let photo = if g("photo").is_empty() { dom::store_get("profile-photo", "") } else { g("photo") };
    format!(r#"<span class="dv {cls}">{}{}</span>"#, dom::avatar_html(&name, &photo, "avatar contact"), deco_html(keys))
}
/// 장신구 사진만 (목록·결과 창)
fn thumb(k: &str, cls: &str) -> String {
    let n = item(k).map(|x| x.1).unwrap_or("");
    format!(r#"<img class="{cls}" src="app/deco/{k}.webp" alt="{n}" draggable="false" decoding="async">"#)
}

async fn load_mine() {
    let Ok(v) = cloud::call("gachaList", &[]).await else { return };
    let list: Vec<(String, String)> = js_sys::Array::from(&v).iter().map(|x| (cloud::get(&x, "id").as_string().unwrap_or_default(), tier(cloud::get(&x, "at").as_f64().unwrap_or(0.0), cloud::get(&x, "n").as_f64().unwrap_or(0.0) as usize).to_string())).collect();
    MINE.with(|m| *m.borrow_mut() = list);
}

/* ---------- 뽑기 화면 ---------- */

fn paint_gacha() {
    let Some(root) = by_id("gacha-root") else { return };
    if !cloud::signed_in() {
        return dom::set_html_if(&root, r#"<p class="sns-empty">로그인하면 뽑기를 할 수 있어요</p>"#);
    }
    let bal = crate::money::balance().unwrap_or(0.0);
    let mut odds = String::new();
    for r in ["전설", "영웅", "희귀", "일반"] {
        let items: Vec<_> = ITEMS.iter().filter(|x| x.2 == r).collect();
        let sum: u32 = items.iter().map(|x| x.3).sum();
        odds.push_str(&format!(r#"<h3 class="gc-tierh {}">{r}<em>{}</em></h3><ul class="gc-tiles">"#, rarity_cls(r), pct(sum)));
        for (k, n, _, p, _) in items {
            odds.push_str(&format!(r#"<li class="gc-tile {}">{}<b>{n}</b><small>{}</small></li>"#, rarity_cls(r), thumb(k, "gc-ti"), pct(*p)));
        }
        odds.push_str("</ul>");
    }
    let credits: String = CREDITS.iter().map(|(n, a, l, u)| format!(r#"<li>{n}: <a href="{}" target="_blank" rel="noopener noreferrer">{}</a>, {}</li>"#, esc(u), esc(a), esc(l))).collect();
    dom::set_html_if(
        &root,
        &format!(
            r#"<div class="card gc-hero"><div class="gc-ball" id="gc-ball"><span class="gc-shine"></span></div><p class="gc-sub">동그라미 사진에 얹는 장신구가 나와요. 여러 개를 같이 낄 수 있어요</p><div class="gc-btns"><button type="button" class="gc-go" id="gc-go">1번 뽑기<small>10 멸공</small></button><button type="button" class="gc-go ten" id="gc-go10">10번 뽑기<small>100 멸공</small></button></div><p class="gc-bal">내 멸공 <b>{}</b></p></div>
<h2 class="section-title">나올 수 있는 것</h2><div class="card gc-odds">{odds}</div><details class="gc-cred"><summary>사진 출처</summary><ul>{credits}</ul></details>"#,
            crate::money::w(bal)
        ),
    );
}
fn pct(p: u32) -> String {
    let v = p as f64 / 10.0;
    if v.fract() == 0.0 {
        format!("{v:.0}%")
    } else {
        format!("{v:.1}%")
    }
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
            let k = tier(cloud::get(&v, "at").as_f64().unwrap_or(0.0), 0);
            MINE.with(|m| m.borrow_mut().push((id.clone(), k.to_string())));
            crate::money::log(&dom::obj(&[("t", js_sys::Date::now().into()), ("name", JsValue::from_str("뽑기")), ("cat", "쇼핑".into()), ("icon", "gift".into()), ("amt", (-10.0).into())]));
            show_result(k, &id);
            paint_gacha();
        }
        Err(e) => crate::shell::toast(&format!("뽑지 못했어요. {}", cloud::why(&e.code))),
    }
}
/// 10연차: 100 멸공으로 한 번에 10개
async fn pull10() {
    if BUSY.replace(true) {
        return;
    }
    if let Err(m) = crate::money::ensure(100.0).await {
        BUSY.set(false);
        crate::shell::ask("뽑기를 할 수 없어요", &m, "확인", "").await;
        return;
    }
    let ball = by_id("gc-ball");
    if let Some(b) = &ball {
        let _ = b.class_list().add_1("shake");
    }
    dom::vibrate(14);
    let f = Closure::once_into_js(|| cloud::call_now("gachaPull10", &[]).unwrap_or(JsValue::UNDEFINED));
    let r = cloud::settle(crate::money::wallet_op(f.unchecked_into()).into()).await;
    dom::sleep(900).await;
    if let Some(b) = &ball {
        let _ = b.class_list().remove_1("shake");
    }
    BUSY.set(false);
    match r {
        Ok(v) => {
            let at = cloud::get(&v, "at").as_f64().unwrap_or(0.0);
            let got: Vec<(String, &'static str)> = js_sys::Array::from(&cloud::get(&v, "ids")).iter().enumerate().map(|(n, id)| (id.as_string().unwrap_or_default(), tier(at, n))).collect();
            MINE.with(|m| m.borrow_mut().extend(got.iter().map(|(id, k)| (id.clone(), k.to_string()))));
            crate::money::log(&dom::obj(&[("t", js_sys::Date::now().into()), ("name", JsValue::from_str("10번 뽑기")), ("cat", "쇼핑".into()), ("icon", "gift".into()), ("amt", (-100.0).into())]));
            show_result10(&got);
            paint_gacha();
        }
        Err(e) => crate::shell::toast(&format!("뽑지 못했어요. {}", cloud::why(&e.code))),
    }
}
/// 10연차 결과: 좋은 것부터 10칸, 가장 좋은 것은 크게
fn show_result10(got: &[(String, &'static str)]) {
    let Some(el) = by_id("gc-result") else { return };
    let rank = |k: &str| ITEMS.iter().position(|x| x.0 == k).unwrap_or(99);
    let mut v: Vec<&(String, &'static str)> = got.iter().collect();
    v.sort_by_key(|(_, k)| rank(k));
    let Some((best_id, best)) = v.first().map(|(i, k)| (i.clone(), *k)) else { return };
    let Some((_, bn, br, ..)) = item(best) else { return };
    let tiles: String = v
        .iter()
        .enumerate()
        .map(|(i, (_, k))| {
            let (_, n, r, ..) = item(k).copied().unwrap_or(("", "", "일반", 0, ""));
            format!(r#"<li class="gc-r10 {}" style="animation-delay:{}ms">{}<b>{n}</b></li>"#, rarity_cls(r), 80 * i, thumb(k, "gc-ti"))
        })
        .collect();
    el.set_inner_html(&format!(
        r#"<div class="gc-card ten {}"><p class="gc-tier">10번 뽑기 · 최고 {br}</p>{}<h3>{bn}</h3><ul class="gc-ten">{tiles}</ul><div class="gc-acts"><button type="button" class="gc-btn ghost" data-act="close">확인</button><button type="button" class="gc-btn" data-act="wear" data-item="{best}" data-pull="{}">{bn} 끼기</button></div></div>"#,
        rarity_cls(br),
        thumb(best, "gc-big"),
        esc(&best_id)
    ));
    el.set_hidden(false);
    let _ = el.offset_width();
    let _ = el.class_list().add_1("on");
    dom::vibrate(if *br == "전설" { 40 } else { 16 });
}
/// 결과 창: 장신구 사진과, 내 동그라미에 얹은 모습. 바로 끼거나 닫아요
fn show_result(k: &str, pull_id: &str) {
    let Some(el) = by_id("gc-result") else { return };
    let Some((_, n, r, ..)) = item(k) else { return };
    let mut keys: Vec<String> = my_decos().into_iter().filter(|(x, _)| item(x).map(|i| i.4) != item(k).map(|i| i.4)).map(|(x, _)| x).collect();
    keys.push(k.to_string());
    el.set_inner_html(&format!(
        r#"<div class="gc-card {}"><p class="gc-tier">{r}</p>{}<h3>{n}</h3>{}<div class="gc-acts"><button type="button" class="gc-btn ghost" data-act="close">확인</button><button type="button" class="gc-btn" data-act="wear" data-item="{k}" data-pull="{}">바로 끼기</button></div></div>"#,
        rarity_cls(r),
        thumb(k, "gc-big"),
        me_avatar(&keys.join(","), "md"),
        esc(pull_id)
    ));
    el.set_hidden(false);
    let _ = el.offset_width();
    let _ = el.class_list().add_1("on");
    dom::vibrate(if *r == "전설" { 30 } else { 12 });
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
/// 끼기: 같은 자리에 있던 것은 빼고 새로 끼워요. 빼기: 그것만 빼요
async fn wear(k: String, pull_id: String, on: bool) {
    let slot = item(&k).map(|x| x.4).unwrap_or("");
    let mut list: Vec<(String, String)> = my_decos().into_iter().filter(|(x, _)| if on { item(x).map(|i| i.4) != Some(slot) } else { *x != k }).collect();
    if on {
        list.push((k.clone(), pull_id));
    }
    let arr: js_sys::Array = list.iter().map(|(k, p)| dom::obj(&[("k", JsValue::from_str(k)), ("p", JsValue::from_str(p))])).collect();
    match cloud::call("decoSet", &[arr.into()]).await {
        Ok(_) => {
            let n = item(&k).map(|x| x.1).unwrap_or("");
            crate::shell::toast(&if on { format!("{n}{} 꼈어요", eul(n)) } else { format!("{n}{} 뺐어요", eul(n)) });
            paint_inv();
            crate::sns::repaint_people();
        }
        Err(e) => crate::shell::toast(&format!("바꾸지 못했어요. {}", cloud::why(&e.code))),
    }
}

/* ---------- 인벤 화면 ---------- */

fn paint_inv() {
    let Some(root) = by_id("inv-root") else { return };
    if !cloud::signed_in() {
        return dom::set_html_if(&root, r#"<p class="sns-empty">로그인하면 인벤을 볼 수 있어요</p>"#);
    }
    let now: Vec<String> = my_decos().into_iter().map(|(k, _)| k).collect();
    let mine = MINE.with(|m| m.borrow().clone());
    let cards: String = ITEMS
        .iter()
        .map(|(k, n, r, _, _)| {
            let have: Vec<&(String, String)> = mine.iter().filter(|x| x.1 == *k).collect();
            let foot = if have.is_empty() {
                r#"<span class="iv-lock">아직 없어요</span>"#.to_string()
            } else if now.iter().any(|x| x == k) {
                format!(r#"<button type="button" class="iv-btn on" data-act="off" data-item="{k}">끼는 중</button>"#)
            } else {
                format!(r#"<button type="button" class="iv-btn" data-act="wear" data-item="{k}" data-pull="{}">끼기</button>"#, esc(&have[0].0))
            };
            format!(
                r#"<li class="iv-card {}{}">{}<b>{n}</b><small>{r}{}</small>{foot}</li>"#,
                rarity_cls(r),
                if have.is_empty() { " locked" } else { "" },
                thumb(k, "iv-ti"),
                if have.len() > 1 { format!(" · {}개", have.len()) } else { String::new() }
            )
        })
        .collect();
    let names: Vec<&str> = now.iter().filter_map(|k| item(k).map(|x| x.1)).collect();
    dom::set_html_if(
        &root,
        &format!(
            r#"<div class="card iv-hero">{}<p>{}</p></div><h2 class="section-title">내 장신구</h2><p class="iv-tip">자리(얼굴·머리·목·손목·손·발밑)마다 하나씩 같이 낄 수 있어요</p><ul class="iv-grid">{cards}</ul>{}"#,
            me_avatar(&now.join(","), "xl"),
            if names.is_empty() { "끼고 있는 장신구가 없어요".to_string() } else { format!("<b>{}</b> 끼는 중", esc(&names.join(" · "))) },
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
            } else if dom::closest(&e, "#gc-go10").is_some() {
                spawn_local(pull10());
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
                spawn_local(wear(dom::data(&b, "item"), dom::data(&b, "pull"), true));
            }
            "off" => spawn_local(wear(dom::data(&b, "item"), String::new(), false)),
            _ => hide_result(),
        }
    };
    for id in ["gc-result", "inv-root"] {
        if let Some(r) = by_id(id) {
            dom::on(&r, "click", act);
        }
    }
}
