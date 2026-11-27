//! 상점 (메뉴 → 상점): 애플 스토어 앱의 "제품" 탭과 같은 모양·움직임이에요.
//!   큰 제목과 오른쪽 위 내 동그라미 → 가로로 넘기는 흰 카테고리 칸 → 색 배경의 긴 제품 카드 줄.
//!   카드를 누르면 그 카드가 그 자리에서 화면 가득 커지고(FLIP: transform만 움직여요), 옆으로 넘기며 보다가, 아래로 끌어내리면 제자리로 돌아가요.
//! 파는 것은 프로필 사진(동그라미) 둘레에 두르는 프레임이에요 (디스코드의 아바타 장식처럼). 모두 직접 그린 SVG고, 값은 모두 같아요.
//!   멸공으로 바로 사고, 같은 것은 한 번만 살 수 있어요 (서버 규칙이 값과 중복을 검사해요). 산 프레임은 메뉴 → 인벤에서 걸어요 (한 번에 하나).

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::dom::{self, by_id, esc};

/// (번호, 이름, 종류, 값(멸공), 자리, 카드 위쪽 색, 아래 띠 색, 띠가 밝은지). 값은 서버 규칙(shopPrice)과 같아야 해요. 값은 일단 모두 같아요.
/// 프레임은 모두 같은 자리("frame")라서 한 번에 하나만 걸 수 있어요
pub const PRICE: u32 = 500;
pub const ITEMS: [(&str, &str, &str, u32, &str, &str, &str, bool); 8] = [
    ("gold", "금빛 프레임", "링", PRICE, "frame", "#fbe9a6", "#5a3d00", false),
    ("neon", "네온 프레임", "링", PRICE, "frame", "#cfeef7", "#121033", false),
    ("stars", "별빛 프레임", "링", PRICE, "frame", "#d9d0f5", "#1d1450", false),
    ("laurel", "월계수 프레임", "자연", PRICE, "frame", "#d3ecc8", "#183d1a", false),
    ("sakura", "벚꽃 프레임", "자연", PRICE, "frame", "#fbd3e1", "#5a1c36", false),
    ("flame", "불꽃 프레임", "자연", PRICE, "frame", "#ffd9b8", "#5c1308", false),
    ("cat", "고양이 귀 프레임", "머리", PRICE, "frame", "#ddd9ff", "#241f6b", false),
    ("crown", "왕관 프레임", "머리", PRICE, "frame", "#fff1b5", "#4f3600", false),
];
/// 카테고리 칸 (번호, 이름, 대표 사진, 종류들)
const CATS: [(&str, &str, &str, &[&str]); 4] = [("all", "전체", "gold", &[]), ("ring", "링", "neon", &["링"]), ("nature", "자연", "sakura", &["자연"]), ("head", "머리 장식", "crown", &["머리"])];
/// 알약 칩 (지금은 값이 모두 같아서 없어요)
const CHIPS: [(&str, &str); 0] = [];

thread_local! {
    /// 내 가방 (산 장신구)
    static BAG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static LOADED: Cell<bool> = const { Cell::new(false) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
    static BOUND: Cell<bool> = const { Cell::new(false) };
    /// 고른 카테고리·칩
    static CAT: RefCell<&'static str> = const { RefCell::new("all") };
    static CHIP: RefCell<&'static str> = const { RefCell::new("") };
    /// 크게 본 카드: 몇 번째, 끌기 (시작 x, 시작 y, 시각, 누르는 중, 방향 0 모름·1 가로·2 세로, 끈 거리 x, y, 폭)
    static PIDX: Cell<usize> = const { Cell::new(0) };
    static PDRAG: Cell<(f64, f64, f64, bool, u8, f64, f64, f64)> = const { Cell::new((0.0, 0.0, 0.0, false, 0, 0.0, 0.0, 0.0)) };
    static OPEN: Cell<bool> = const { Cell::new(false) };
}

fn item(k: &str) -> Option<&'static (&'static str, &'static str, &'static str, u32, &'static str, &'static str, &'static str, bool)> {
    ITEMS.iter().find(|x| x.0 == k)
}
fn won(v: u32) -> String {
    format!("{} 멸공", cloud::fmt_num(v as f64, 0))
}
/// 받침이 있으면 "을", 없으면 "를"
fn eul(w: &str) -> &'static str {
    match w.chars().last() {
        Some(c) if ('가'..='힣').contains(&c) && (c as u32 - 0xAC00) % 28 != 0 => "을",
        _ => "를",
    }
}
fn my_uid() -> String {
    cloud::cloud().and_then(|c| cloud::get(&c, "uid").as_string()).unwrap_or_default()
}
fn has(k: &str) -> bool {
    BAG.with(|b| b.borrow().iter().any(|x| x == k))
}

/* ---------- 동그라미에 얹는 장신구 (다른 화면들이 써요) ---------- */

/// 동그라미 위에 얹는 장신구들 (쉼표로 이은 번호들). 자리마다 위치가 정해져 있어요
pub fn deco_html(keys: &str) -> String {
    keys.split(',')
        .filter_map(item)
        .map(|(k, n, ..)| format!(r#"<img class="deco dc-{k}" src="app/deco/{k}.svg" alt="{n}" draggable="false" decoding="async">"#))
        .collect()
}
/// 이 사람이 낀 장신구 번호들 (쉼표로 이어요, 프로필 캐시에서)
pub fn deco_of(uid: &str) -> String {
    let p = cloud::call_now("peek", &[JsValue::from_str(uid)]).unwrap_or(JsValue::NULL);
    let list = cloud::get(&p, "gear");
    if !js_sys::Array::is_array(&list) {
        return String::new();
    }
    js_sys::Array::from(&list).iter().filter_map(|x| x.as_string()).filter(|k| item(k).is_some()).collect::<Vec<_>>().join(",")
}
fn my_gear() -> Vec<String> {
    deco_of(&my_uid()).split(',').filter(|s| !s.is_empty()).map(String::from).collect()
}
fn me_avatar(keys: &str, cls: &str) -> String {
    let p = cloud::call_now("peek", &[JsValue::from_str(&my_uid())]).unwrap_or(JsValue::NULL);
    let g = |k: &str| cloud::get(&p, k).as_string().unwrap_or_default();
    let name = if g("name").is_empty() { dom::store_get("profile-name", "나") } else { g("name") };
    let photo = if g("photo").is_empty() { dom::store_get("profile-photo", "") } else { g("photo") };
    format!(r#"<span class="dv {cls}">{}{}</span>"#, dom::avatar_html(&name, &photo, "avatar contact"), deco_html(keys))
}
/// 프레임을 보여 주는 그림: 샘플 동그라미 위에 프레임을 얹어요 (프레임은 가운데 70%가 사진 자리예요)
fn pic(k: &str, cls: &str) -> String {
    let n = item(k).map(|x| x.1).unwrap_or("");
    format!(r#"<span class="fr-demo {cls}"><i></i><img src="app/deco/{k}.svg" alt="{n}" draggable="false" decoding="async"></span>"#)
}

async fn load_bag() {
    if let Ok(v) = cloud::call("decoBag", &[]).await {
        let items: Vec<String> = js_sys::Array::from(&v).iter().filter_map(|x| x.as_string()).collect();
        BAG.with(|b| *b.borrow_mut() = items);
        LOADED.set(true);
    }
}

/* ---------- 상점 화면 ---------- */

/// 지금 고른 카테고리에 맞는 프레임들
fn shown() -> Vec<&'static str> {
    let cat = CAT.with(|c| *c.borrow());
    let slots: &[&str] = CATS.iter().find(|c| c.0 == cat).map(|c| c.3).unwrap_or(&[]);
    ITEMS.iter().filter(|x| slots.is_empty() || slots.contains(&x.2)).map(|x| x.0).collect()
}
fn card(k: &str, big: bool) -> String {
    let Some((_, n, r, price, _, top, band, light)) = item(k) else { return String::new() };
    let own = has(k);
    format!(
        r#"<article class="sh-card{big}{lt}" data-k="{k}" style="--top:{top};--band:{band}"><div class="sh-pic">{img}</div><div class="sh-band"><small>{tag}</small><b>{n}</b><div class="sh-foot"><span>{price}</span><button type="button" class="sh-buy{on}" data-buy="{k}"{dis}>{btn}</button></div></div></article>"#,
        big = if big { " big" } else { "" },
        lt = if *light { " light" } else { "" },
        img = pic(k, "sh-img"),
        tag = format!("프레임 · {r}"),
        price = won(*price),
        on = if own { " owned" } else { "" },
        dis = if own { " disabled" } else { "" },
        btn = if own { "보유 중" } else { "구입하기" },
    )
}
fn paint() {
    let Some(root) = by_id("shop-root") else { return };
    if !cloud::signed_in() {
        return dom::set_html_if(&root, r#"<p class="sns-empty">로그인하면 상점을 볼 수 있어요</p>"#);
    }
    let cat = CAT.with(|c| *c.borrow());
    let cats: String = CATS
        .iter()
        .map(|(id, n, k, _)| format!(r#"<button type="button" class="sh-cat{}" data-cat="{id}">{}<span>{n}</span></button>"#, if *id == cat { " on" } else { "" }, pic(k, "sh-ci")))
        .collect();
    let list = shown();
    let cards: String = if list.is_empty() { r#"<p class="sh-none">이 종류의 프레임이 없어요</p>"#.to_string() } else { list.iter().map(|k| card(k, false)).collect() };
    let title = CATS.iter().find(|c| c.0 == cat).map(|c| if c.0 == "all" { "프레임 모음".to_string() } else { format!("{} 프레임", c.1) }).unwrap_or_default();
    dom::set_html_if(
        &root,
        &format!(
            r#"<header class="sh-top"><h1 id="title-shop">상점</h1><button type="button" class="sh-me" data-go-inv aria-label="내 프레임">{me}</button></header>
<div class="sh-cats" role="list">{cats}</div>
<h2 class="sh-h red">{title}</h2><div class="sh-row" id="sh-row">{cards}</div>
<p class="sh-bal">내 멸공 <b>{bal}</b> · 가진 프레임 <b>{have}개</b></p>"#,
            me = me_avatar(&my_gear().join(","), "sm"),
            bal = crate::money::w(crate::money::balance().unwrap_or(0.0)),
            have = BAG.with(|b| b.borrow().len()),
        ),
    );
}
pub fn open() {
    bind_once();
    close_pager(false);
    paint();
    if cloud::signed_in() {
        spawn_local(async {
            load_bag().await;
            paint();
        });
    }
}

/* ---------- 카드를 크게 보기 (애플 스토어처럼 그 자리에서 화면 가득) ---------- */

fn pager_place(dx: f64, snap: bool) {
    let Some(t) = by_id("sh-track") else { return };
    let st = t.style();
    let _ = st.set_property("transition", if snap { "transform 0.34s cubic-bezier(.2,.8,.2,1)" } else { "none" });
    // 카드 폭 = 화면 폭 - 48px, 카드 사이 10px. 지금 카드가 가운데 오게
    let _ = st.set_property("transform", &format!("translate3d(calc({} * (30px - 100%) + {dx:.1}px), 0, 0)", PIDX.get()));
}
fn open_pager(k: &str, from: Option<web_sys::DomRect>) {
    let Some(p) = by_id("sh-pager") else { return };
    let list = shown();
    let i = list.iter().position(|x| *x == k).unwrap_or(0);
    PIDX.set(i);
    let cards: String = list.iter().map(|k| card(k, true)).collect();
    p.set_inner_html(&format!(
        r#"<div class="sh-ph"><button type="button" class="sh-pt" data-close>상점</button><span class="sh-me">{}</span></div><div class="sh-pv" id="sh-pv"><div class="sh-track" id="sh-track">{cards}</div></div>"#,
        me_avatar(&my_gear().join(","), "sm")
    ));
    p.set_hidden(false);
    OPEN.set(true);
    pager_place(0.0, false);
    let _ = p.offset_width();
    let _ = p.class_list().add_1("on");
    dom::vibrate(6);
    // FLIP: 누른 작은 카드 자리에서 시작해 큰 카드 자리로 자라나요
    let (Some(from), Some(big)) = (from, p.query_selector(&format!(r#".sh-card[data-k="{k}"]"#)).ok().flatten()) else { return };
    let to = big.get_bounding_client_rect();
    if to.width() < 1.0 || dom::reduce_motion() {
        return;
    }
    let (sx, sy) = (from.width() / to.width(), from.height() / to.height());
    let (tx, ty) = (from.left() - to.left(), from.top() - to.top());
    dom::anim_always(
        &big,
        &format!(r#"[{{"transform":"translate({tx:.1}px,{ty:.1}px) scale({sx:.4},{sy:.4})","borderRadius":"{:.1}px"}},{{"transform":"none","borderRadius":"26px"}}]"#, 22.0 / sx.min(sy)),
        r#"{"duration":480,"easing":"cubic-bezier(.2,.9,.2,1)","fill":"none"}"#,
    );
}
/// 큰 카드를 닫아요. 애니메이션이면 지금 카드가 상점의 작은 카드 자리로 줄어들어요
fn close_pager(animate: bool) {
    let Some(p) = by_id("sh-pager") else { return };
    if !OPEN.replace(false) {
        return;
    }
    let list = shown();
    let k = list.get(PIDX.get()).copied().unwrap_or("");
    let big = p.query_selector(&format!(r#".sh-card[data-k="{k}"]"#)).ok().flatten();
    let small = by_id("sh-row").and_then(|r| r.query_selector(&format!(r#".sh-card[data-k="{k}"]"#)).ok().flatten());
    if let (true, Some(big), Some(small)) = (animate && !dom::reduce_motion(), big, small) {
        // 작은 카드가 보이게 줄을 옮겨 두고, 그 자리로 줄어들어요
        if let Some(row) = by_id("sh-row") {
            let sl = small.dyn_ref::<web_sys::HtmlElement>().map(|s| s.offset_left()).unwrap_or(0) - 16;
            row.set_scroll_left(sl.max(0));
        }
        let from = big.get_bounding_client_rect();
        let to = small.get_bounding_client_rect();
        let (sx, sy) = (to.width() / from.width().max(1.0), to.height() / from.height().max(1.0));
        let (tx, ty) = (to.left() - from.left(), to.top() - from.top());
        let _ = p.class_list().add_1("closing");
        let a = dom::anim_always(
            &big,
            &format!(r#"[{{"transform":"none"}},{{"transform":"translate({tx:.1}px,{ty:.1}px) scale({sx:.4},{sy:.4})"}}]"#),
            r#"{"duration":380,"easing":"cubic-bezier(.3,.7,.2,1)","fill":"forwards"}"#,
        );
        let p2 = p.clone();
        dom::on_finish(a, move || {
            let _ = p2.class_list().remove_2("on", "closing");
            p2.set_hidden(true);
            p2.set_inner_html("");
        });
    } else {
        let _ = p.class_list().remove_2("on", "closing");
        p.set_hidden(true);
        p.set_inner_html("");
    }
}

/* ---------- 사기 ---------- */

async fn buy(k: String) {
    let Some((_, n, _, price, ..)) = item(&k) else { return };
    if has(&k) {
        return crate::shell::toast("이미 가진 프레임이에요");
    }
    if BUSY.replace(true) {
        return;
    }
    if !crate::shell::ask(&format!("{n}{} 살까요?", eul(n)), &format!("{} 멸공이 빠져요. 산 프레임은 인벤에서 걸 수 있어요.", cloud::fmt_num(*price as f64, 0)), "구입하기", "취소").await {
        BUSY.set(false);
        return;
    }
    if let Err(m) = crate::money::ensure(*price as f64).await {
        BUSY.set(false);
        crate::shell::ask("살 수 없어요", &m, "확인", "").await;
        return;
    }
    if !LOADED.get() {
        load_bag().await;
    }
    let items: js_sys::Array = BAG.with(|b| b.borrow().iter().map(|x| JsValue::from_str(x)).collect());
    let (k2, p) = (k.clone(), *price as f64);
    let f = Closure::once_into_js(move || cloud::call_now("decoBuy", &[JsValue::from_str(&k2), p.into(), items.into()]).unwrap_or(JsValue::UNDEFINED));
    let r = cloud::settle(crate::money::wallet_op(f.unchecked_into()).into()).await;
    BUSY.set(false);
    match r {
        Ok(_) => {
            BAG.with(|b| b.borrow_mut().push(k.clone()));
            crate::money::log(&dom::obj(&[("t", js_sys::Date::now().into()), ("name", JsValue::from_str(&format!("{n} 구입"))), ("cat", "쇼핑".into()), ("icon", "gift".into()), ("amt", (-(*price as f64)).into())]));
            dom::vibrate(14);
            crate::shell::toast(&format!("{n}{} 샀어요. 인벤에서 걸어 보세요", eul(n)));
            // 산 카드의 버튼만 바꿔요 (큰 카드 보는 중이어도 그대로)
            for b in dom::qsa(&dom::doc().document_element().expect("html"), &format!(r#"[data-buy="{k}"]"#)) {
                b.set_text_content(Some("보유 중"));
                let _ = b.set_attribute("disabled", "");
                let _ = b.class_list().add_1("owned");
            }
            if !OPEN.get() {
                paint();
            }
        }
        Err(e) => crate::shell::toast(&format!("사지 못했어요. {}", cloud::why(&e.code))),
    }
}

/* ---------- 인벤: 산 프레임만, 하나씩 걸기 ---------- */

async fn wear(k: String, on: bool) {
    let slot = item(&k).map(|x| x.4).unwrap_or("");
    let mut list: Vec<String> = my_gear().into_iter().filter(|x| has(x)).filter(|x| if on { item(x).map(|i| i.4) != Some(slot) } else { *x != k }).collect();
    if on {
        list.push(k.clone());
    }
    let arr: js_sys::Array = list.iter().map(|k| JsValue::from_str(k)).collect();
    match cloud::call("decoSet", &[arr.into()]).await {
        Ok(_) => {
            let n = item(&k).map(|x| x.1).unwrap_or("");
            crate::shell::toast(&if on { format!("{n}{} 걸었어요", eul(n)) } else { format!("{n}{} 내렸어요", eul(n)) });
            paint_inv();
            crate::sns::repaint_people();
        }
        Err(e) => crate::shell::toast(&format!("바꾸지 못했어요. {}", cloud::why(&e.code))),
    }
}
fn paint_inv() {
    let Some(root) = by_id("inv-root") else { return };
    if !cloud::signed_in() {
        return dom::set_html_if(&root, r#"<p class="sns-empty">로그인하면 인벤을 볼 수 있어요</p>"#);
    }
    let now: Vec<String> = my_gear().into_iter().filter(|k| has(k)).collect();
    let bag = BAG.with(|b| b.borrow().clone());
    let cards: String = ITEMS
        .iter()
        .filter(|x| bag.iter().any(|b| b == x.0))
        .map(|(k, n, r, ..)| {
            let foot = if now.iter().any(|x| x == k) {
                format!(r#"<button type="button" class="iv-btn on" data-act="off" data-item="{k}">거는 중</button>"#)
            } else {
                format!(r#"<button type="button" class="iv-btn" data-act="wear" data-item="{k}">걸기</button>"#)
            };
            format!(r#"<li class="iv-card">{}<b>{n}</b><small>{r}</small>{foot}</li>"#, pic(k, "iv-ti"))
        })
        .collect();
    let names: Vec<&str> = now.iter().filter_map(|k| item(k).map(|x| x.1)).collect();
    dom::set_html_if(
        &root,
        &format!(
            r#"<div class="card iv-hero">{}<p>{}</p></div><h2 class="section-title">내 프레임 <small class="iv-n">{}개</small></h2>{}"#,
            me_avatar(&now.join(","), "xl"),
            if names.is_empty() { "걸고 있는 프레임이 없어요".to_string() } else { format!("<b>{}</b> 걸고 있어요", esc(&names.join(" · "))) },
            bag.len(),
            if bag.is_empty() {
                if LOADED.get() { r#"<p class="iv-empty">아직 프레임이 없어요</p>"#.to_string() } else { r#"<p class="iv-empty">불러오는 중이에요</p>"#.to_string() }
            } else {
                format!(r#"<p class="iv-tip">프레임은 한 번에 하나만 걸 수 있어요</p><ul class="iv-grid">{cards}</ul>"#)
            }
        ),
    );
}
pub fn open_inv() {
    bind_once();
    paint_inv();
    if cloud::signed_in() {
        spawn_local(async {
            load_bag().await;
            paint_inv();
        });
    }
}

/* ---------- 누르기·끌기 ---------- */

fn on_shop_click(e: web_sys::Event) {
    if let Some(b) = dom::closest(&e, "[data-buy]") {
        e.stop_propagation();
        return spawn_local(buy(dom::data(&b, "buy")));
    }
    if dom::closest(&e, "[data-go-inv]").is_some() {
        crate::shell::go_screen("inv");
        return open_inv();
    }
    if let Some(b) = dom::closest(&e, "[data-cat]") {
        let id = dom::data(&b, "cat");
        CAT.with(|c| *c.borrow_mut() = CATS.iter().find(|x| x.0 == id).map(|x| x.0).unwrap_or("all"));
        dom::vibrate(5);
        paint();
        if let Some(r) = by_id("sh-row") {
            r.set_scroll_left(0);
        }
        return;
    }
    if let Some(b) = dom::closest(&e, "[data-chip]") {
        let id = dom::data(&b, "chip");
        let now = CHIP.with(|c| *c.borrow());
        let next = CHIPS.iter().find(|x| x.0 == id).map(|x| x.0).unwrap_or("");
        CHIP.with(|c| *c.borrow_mut() = if now == next { "" } else { next });
        dom::vibrate(5);
        paint();
        if let Some(r) = by_id("sh-row") {
            r.set_scroll_left(0);
        }
        return;
    }
    if let Some(c) = dom::closest(&e, ".sh-card") {
        let k = dom::data(&c, "k");
        open_pager(&k, Some(c.get_bounding_client_rect()));
    }
}
fn bind_once() {
    if BOUND.replace(true) {
        return;
    }
    if let Some(r) = by_id("shop-root") {
        dom::on(&r, "click", on_shop_click);
    }
    if let Some(p) = by_id("sh-pager") {
        dom::on(&p, "click", |e| {
            if let Some(b) = dom::closest(&e, "[data-buy]") {
                return spawn_local(buy(dom::data(&b, "buy")));
            }
            if dom::closest(&e, "[data-close]").is_some() {
                close_pager(true);
            }
        });
        // 가로로 끌면 옆 카드, 아래로 끌면 닫기 (처음 움직인 방향으로 정해요)
        let p1 = p.clone();
        dom::on(&p, "pointerdown", move |e| {
            let Some(pe) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
            if !pe.is_primary() || dom::closest(&e, "button").is_some() {
                return;
            }
            PDRAG.set((pe.client_x() as f64, pe.client_y() as f64, js_sys::Date::now(), true, 0, 0.0, 0.0, p1.client_width().max(1) as f64));
        });
        dom::on(&p, "pointermove", |e| {
            let (x0, y0, t0, on, dir, _, _, w) = PDRAG.get();
            if !on {
                return;
            }
            let Some(pe) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
            let (dx, dy) = (pe.client_x() as f64 - x0, pe.client_y() as f64 - y0);
            let dir = if dir == 0 && dx.hypot(dy) > 8.0 { if dx.abs() > dy.abs() { 1 } else { 2 } } else { dir };
            PDRAG.set((x0, y0, t0, true, dir, dx, dy, w));
            match dir {
                1 => {
                    let n = shown().len();
                    let i = PIDX.get();
                    let dx = if (i == 0 && dx > 0.0) || (i + 1 >= n && dx < 0.0) { dx * 0.3 } else { dx };
                    pager_place(dx, false);
                }
                2 if dy > 0.0 => {
                    // 아래로 끌면 카드가 조금 줄며 따라와요
                    if let Some(c) = current_big() {
                        let s = (1.0 - dy / 1600.0).max(0.85);
                        let _ = c.style().set_property("transform", &format!("translate3d(0,{:.1}px,0) scale({s:.3})", dy * 0.6));
                    }
                }
                _ => {}
            }
        });
        for ev in ["pointerup", "pointercancel"] {
            dom::on(&p, ev, |_| {
                let (_, _, t0, on, dir, dx, dy, w) = PDRAG.get();
                if !on {
                    return;
                }
                PDRAG.set((0.0, 0.0, 0.0, false, 0, 0.0, 0.0, 0.0));
                let dt = (js_sys::Date::now() - t0).max(1.0);
                match dir {
                    1 => {
                        let i = PIDX.get();
                        let n = shown().len();
                        if (dx < -w * 0.18 || dx / dt < -0.35) && i + 1 < n {
                            PIDX.set(i + 1);
                            dom::vibrate(5);
                        } else if (dx > w * 0.18 || dx / dt > 0.35) && i > 0 {
                            PIDX.set(i - 1);
                            dom::vibrate(5);
                        }
                        pager_place(0.0, true);
                    }
                    2 => {
                        if dy > 110.0 || dy / dt > 0.5 {
                            close_pager(true);
                        } else if let Some(c) = current_big() {
                            let _ = c.style().set_property("transition", "transform 0.3s cubic-bezier(.2,.8,.2,1)");
                            let _ = c.style().remove_property("transform");
                        }
                    }
                    _ => {}
                }
            });
        }
    }
    if let Some(r) = by_id("inv-root") {
        dom::on(&r, "click", |e| {
            let Some(b) = dom::closest(&e, "[data-act]") else { return };
            match dom::data(&b, "act").as_str() {
                "wear" => spawn_local(wear(dom::data(&b, "item"), true)),
                "off" => spawn_local(wear(dom::data(&b, "item"), false)),
                _ => {}
            }
        });
    }
}
fn current_big() -> Option<web_sys::HtmlElement> {
    let k = shown().get(PIDX.get()).copied()?;
    by_id("sh-pager")?.query_selector(&format!(r#".sh-card[data-k="{k}"]"#)).ok().flatten()?.dyn_into().ok()
}
/// 다른 화면으로 가면 큰 카드는 바로 닫아요
pub fn leave() {
    close_pager(false);
}
