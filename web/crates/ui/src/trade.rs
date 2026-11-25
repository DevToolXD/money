//! 거래 탭: 물물교환이나 멸공으로 사고팔 물건을 올려요 (사진·무엇을 내놓는지·무엇과 바꾸고 싶은지·멸공 값).
//! 생김새는 SNS와 같은 유리 카드 피드이고, 거래는 "제안하기"로 글쓴이와 메시지를 열어서 해요.
//! 서버를 아끼려고 탭을 열 때 최근 40개만 읽고(15초 안에는 다시 읽지 않아요), 올리기·완료·지우기 뒤에만 다시 읽어요.

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::dom::{self, by_id, esc, safe_img};

#[derive(Clone, Default)]
struct Item {
    id: String,
    by: String,
    title: String,
    want: String,
    price: f64,
    text: String,
    img: String,
    done: bool,
    at: f64,
}

thread_local! {
    static ITEMS: RefCell<Vec<Item>> = const { RefCell::new(Vec::new()) };
    static DRAFT_IMG: RefCell<String> = const { RefCell::new(String::new()) };
    static BOUND: Cell<bool> = const { Cell::new(false) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
    static LOADING: Cell<bool> = const { Cell::new(false) };
    static AT: Cell<f64> = const { Cell::new(0.0) };
}

const SWAP: &str = r#"<path d="M7 7h12l-3-3M17 17H5l3 3"/>"#;
const COIN: &str = r#"<circle cx="12" cy="12" r="8.5"/><path d="M9 9.5h6M12 9.5v6"/>"#;
const SEND: &str = r#"<path d="M20.5 3.5L10 14"/><path d="M20.5 3.5l-6.5 17-4-6.5-6.5-4z"/>"#;
const CHECK: &str = r#"<path d="M5 12.5l4.5 4.5L19 7.5"/>"#;
const TRASH: &str = r#"<path d="M4.5 7h15M9.5 7V5h5v2M6.5 7l1 13h9l1-13"/>"#;
const X: &str = r#"<path d="M7 7l10 10M17 7L7 17"/>"#;

fn svg(d: &str) -> String {
    format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{d}</svg>"#)
}
fn my_uid() -> String {
    cloud::cloud().and_then(|c| cloud::get(&c, "uid").as_string()).unwrap_or_default()
}
fn person(uid: &str) -> (String, String, String) {
    let p = cloud::call_now("peek", &[JsValue::from_str(uid)]).unwrap_or(JsValue::NULL);
    let g = |k: &str| cloud::get(&p, k).as_string().unwrap_or_default();
    let n = g("name");
    (if n.is_empty() { g("id") } else { n }, g("id"), g("photo"))
}

fn card(t: &Item) -> String {
    let me = my_uid();
    let (n, _, ph) = person(&t.by);
    let pic = safe_img(&t.img, 300000);
    let mine = t.by == me;
    let foot = if mine {
        format!(
            r#"<button type="button" class="tr-go{}" data-act="done">{}{}</button><button type="button" class="pl-del" data-act="del" aria-label="글 지우기">{}</button>"#,
            if t.done { " on" } else { "" },
            svg(CHECK),
            if t.done { "거래 완료됨" } else { "거래 완료로 표시" },
            svg(TRASH)
        )
    } else if t.done {
        r#"<span class="tr-closed">거래가 끝났어요</span>"#.to_string()
    } else {
        format!(r#"<button type="button" class="tr-go primary" data-act="offer">{}제안하기</button>"#, svg(SEND))
    };
    format!(
        r#"<article class="sns-card tr-card{done}" data-id="{id}"><header class="pl-head"><button type="button" class="sns-who" data-uid="{by}">{av}<span><b>{n}{bl}</b><small>{ago}</small></span></button>{badge}</header>{img}<h2 class="pl-title">{title}</h2><div class="pl-chips">{want}{price}</div>{text}<footer class="pl-foot">{foot}</footer></article>"#,
        done = if t.done { " is-done" } else { "" },
        id = esc(&t.id),
        by = esc(&t.by),
        av = crate::presence::wrap(dom::avatar_html(&n, &ph, "avatar contact sns-cav"), &t.by),
        n = esc(&n),
        bl = crate::profile::blue(&t.by),
        ago = crate::sns::ago(t.at),
        badge = if t.done { r#"<span class="tr-badge">완료</span>"# } else { r#"<span class="pl-soon">거래 중</span>"# },
        img = if pic.is_empty() { String::new() } else { format!(r#"<div class="tr-img" style="background-image:url({pic})" role="img" aria-label="물건 사진"></div>"#) },
        title = esc(&t.title),
        want = if t.want.is_empty() { String::new() } else { format!(r#"<span class="pl-chip">{}{} 와(과) 바꿔요</span>"#, svg(SWAP), esc(&t.want)) },
        price = if t.price > 0.0 { format!(r#"<span class="pl-chip tr-price">{}{} 멸공</span>"#, svg(COIN), cloud::fmt_num(t.price, 0)) } else { String::new() },
        text = if t.text.is_empty() { String::new() } else { format!(r#"<p class="pl-text">{}</p>"#, esc(&t.text)) },
    )
}

pub fn repaint() {
    let Some(f) = by_id("trade-feed") else { return };
    if !cloud::signed_in() {
        return dom::set_html_if(&f, r#"<p class="sns-empty">로그인하면 거래 글을 볼 수 있어요</p>"#);
    }
    let html: String = ITEMS.with(|p| p.borrow().iter().map(card).collect());
    let was_empty = f.child_element_count() == 0 || f.query_selector(".tr-card").ok().flatten().is_none();
    dom::set_html_if(&f, &if html.is_empty() {
        if LOADING.get() { r#"<p class="sns-empty">불러오는 중이에요</p>"#.into() } else { r#"<div class="pl-empty"><b>아직 거래 글이 없어요</b><p>아래 + 버튼으로 바꾸고 싶은 물건을 올려 보세요</p></div>"#.into() }
    } else {
        html.clone()
    });
    if was_empty && !html.is_empty() {
        dom::reveal(&f);
    }
}

async fn load() {
    AT.set(js_sys::Date::now());
    LOADING.set(ITEMS.with(|p| p.borrow().is_empty()));
    repaint();
    let r = cloud::call("tradesList", &[]).await;
    LOADING.set(false);
    let Ok(list) = r else {
        crate::shell::toast("거래 글을 불러오지 못했어요");
        return repaint();
    };
    let v: Vec<Item> = js_sys::Array::from(&list)
        .iter()
        .map(|it| {
            let s = |k: &str| cloud::get(&it, k).as_string().unwrap_or_default();
            let n = |k: &str| cloud::get(&it, k).as_f64().unwrap_or(0.0);
            Item { id: s("id"), by: s("by"), title: s("title"), want: s("want"), price: n("price"), text: s("text"), img: s("img"), done: cloud::get(&it, "done").is_truthy(), at: n("at") }
        })
        .collect();
    let uids: js_sys::Array = v.iter().map(|t| JsValue::from_str(&t.by)).collect();
    ITEMS.with(|p| *p.borrow_mut() = v);
    repaint();
    let _ = cloud::call("fetchProfiles", &[uids.into()]).await;
    repaint();
}

fn on_feed(e: web_sys::Event) {
    if let Some(b) = dom::closest(&e, "[data-uid]") {
        let uid = dom::data(&b, "uid");
        return crate::profile::show(&uid, &person(&uid).0, None);
    }
    let Some(b) = dom::closest(&e, "[data-act]") else { return };
    let Some(card) = dom::closest(&e, ".tr-card") else { return };
    let (id, act) = (dom::data(&card, "id"), dom::data(&b, "act"));
    let Some(item) = ITEMS.with(|p| p.borrow().iter().find(|x| x.id == id).cloned()) else { return };
    if BUSY.replace(true) {
        return;
    }
    dom::vibrate(10);
    spawn_local(async move {
        match act.as_str() {
            "offer" => {
                let (n, uid_id, ph) = person(&item.by);
                match crate::chat::people::chat_with(&item.by, &uid_id, &n, &ph).await {
                    Ok(cid) => {
                        crate::chat::thread::open(&cid);
                        // 무슨 글을 보고 연락했는지 입력칸에 적어 둬요 (보내기는 직접 눌러요)
                        if let Some(i) = by_id("composer-input").and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok()) {
                            i.set_value(&format!("거래 글 \"{}\" 보고 연락드려요", item.title));
                        }
                    }
                    Err(_) => crate::shell::toast("대화를 열지 못했어요"),
                }
            }
            "done" => match cloud::call("tradeDone", &[JsValue::from_str(&id), (!item.done).into()]).await {
                Ok(_) => {
                    crate::shell::toast(if item.done { "다시 거래 중으로 바꿨어요" } else { "거래 완료로 표시했어요" });
                    load().await;
                }
                Err(e) => crate::shell::toast(&format!("바꾸지 못했어요. {}", cloud::why(&e.code))),
            },
            _ => {
                if crate::shell::ask("이 거래 글을 지울까요?", "지우면 되돌릴 수 없어요.", "지우기", "취소").await {
                    match cloud::call("tradeDelete", &[JsValue::from_str(&id)]).await {
                        Ok(_) => {
                            crate::shell::toast("거래 글을 지웠어요");
                            load().await;
                        }
                        Err(e) => crate::shell::toast(&format!("지우지 못했어요. {}", cloud::why(&e.code))),
                    }
                }
            }
        }
        BUSY.set(false);
    });
}

/* ---------- 새 거래 글 ---------- */
fn sheet(on: bool) {
    let Some(s) = by_id("trade-compose") else { return };
    if on {
        s.set_hidden(false);
        let _ = s.offset_width();
        let _ = s.class_list().add_1("on");
    } else {
        let _ = s.class_list().remove_1("on");
        let s2 = s.clone();
        spawn_local(async move {
            dom::sleep(320).await;
            if !s2.class_list().contains("on") {
                s2.set_hidden(true);
            }
        });
    }
}
fn input(id: &str) -> Option<web_sys::HtmlInputElement> {
    by_id(id)?.dyn_into().ok()
}
fn val(id: &str) -> String {
    input(id).map(|i| i.value()).or_else(|| by_id(id).and_then(|e| e.dyn_into::<web_sys::HtmlTextAreaElement>().ok()).map(|t| t.value())).unwrap_or_default().trim().to_string()
}
fn paint_draft() {
    let img = DRAFT_IMG.with(|d| d.borrow().clone());
    if let Some(p) = by_id("tr-prev") {
        let s = safe_img(&img, 300000);
        dom::set_html_if(&p, &if s.is_empty() {
            format!(r#"<button type="button" class="tr-addimg" id="tr-addimg">{}<span>사진 넣기</span></button>"#, svg(r#"<rect x="3.5" y="5" width="17" height="14" rx="3"/><circle cx="9" cy="10" r="1.6"/><path d="M20.5 16l-5-5-8 8"/>"#))
        } else {
            format!(r#"<div class="sns-pv" style="background-image:url({s})"><button type="button" class="sns-pvx" id="tr-pvx" aria-label="사진 빼기">{}</button></div>"#, svg(X))
        });
    }
    if let Some(b) = by_id("tr-post") {
        let ok = !val("tr-title").is_empty() && (!val("tr-want").is_empty() || val("tr-price").parse::<f64>().is_ok_and(|p| p > 0.0));
        let _ = if ok { b.remove_attribute("disabled") } else { b.set_attribute("disabled", "") };
    }
}
pub fn open_compose() {
    if !cloud::signed_in() {
        return crate::shell::toast("로그인하고 거래 글을 올릴 수 있어요");
    }
    paint_draft();
    sheet(true);
}
fn post() {
    let (title, want, text) = (val("tr-title"), val("tr-want"), val("tr-text"));
    let price: f64 = val("tr-price").chars().filter(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0.0);
    if title.is_empty() || (want.is_empty() && price <= 0.0) {
        return crate::shell::toast("무엇을 내놓는지와, 바꾸고 싶은 것이나 멸공 값을 적어 주세요");
    }
    if [&title, &want, &text].iter().any(|s| s.contains(['<', '>'])) {
        return crate::shell::toast("< > 는 쓸 수 없어요");
    }
    if BUSY.replace(true) {
        return;
    }
    let img = DRAFT_IMG.with(|d| d.borrow().clone());
    let v = dom::obj(&[
        ("title", JsValue::from_str(&title.chars().take(40).collect::<String>())),
        ("want", JsValue::from_str(&want.chars().take(40).collect::<String>())),
        ("price", price.min(100_000_000.0).into()),
        ("text", JsValue::from_str(&text.chars().take(300).collect::<String>())),
        ("img", JsValue::from_str(&img)),
    ]);
    spawn_local(async move {
        let r = cloud::call("tradeCreate", &[v]).await;
        BUSY.set(false);
        match r {
            Ok(_) => {
                for id in ["tr-title", "tr-want", "tr-price"] {
                    if let Some(i) = input(id) {
                        i.set_value("");
                    }
                }
                if let Some(t) = by_id("tr-text").and_then(|e| e.dyn_into::<web_sys::HtmlTextAreaElement>().ok()) {
                    t.set_value("");
                }
                DRAFT_IMG.with(|d| d.borrow_mut().clear());
                sheet(false);
                crate::shell::toast("거래 글을 올렸어요");
                dom::vibrate(16);
                load().await;
            }
            Err(e) => crate::shell::toast(&format!("올리지 못했어요. {}", cloud::why(&e.code))),
        }
    });
}

fn bind() {
    if let Some(f) = by_id("trade-feed") {
        dom::on(&f, "click", on_feed);
    }
    if let Some(s) = by_id("trade-compose") {
        dom::on(&s, "click", |e| {
            if dom::closest(&e, "[data-close]").is_some() || dom::closest(&e, ".sns-panel").is_none() {
                return sheet(false);
            }
            if dom::closest(&e, "#tr-addimg").is_some() {
                if let Some(f) = by_id("tr-file") {
                    f.click();
                }
                return;
            }
            if dom::closest(&e, "#tr-pvx").is_some() {
                DRAFT_IMG.with(|d| d.borrow_mut().clear());
                return paint_draft();
            }
            if dom::closest(&e, "#tr-post").is_some() {
                post();
            }
        });
        dom::on(&s, "input", |_| paint_draft());
    }
    if let Some(f) = by_id("tr-file") {
        dom::on(&f, "change", |_| {
            let Some(file) = crate::chat::attach::take_file("tr-file") else { return };
            spawn_local(async move {
                match crate::chat::attach::read_image(file, 1080.0, 280000, 0.78, false).await {
                    Ok(img) => {
                        DRAFT_IMG.with(|d| *d.borrow_mut() = img);
                        paint_draft();
                    }
                    Err(_) => crate::shell::toast("사진을 넣지 못했어요"),
                }
            });
        });
    }
}

/// 거래 탭을 열 때 (미리 읽어 둔 걸 먼저 보여주고, 5초가 지났으면 조용히 새로 읽어요)
pub fn open() {
    crate::sns::bind_once();
    if !BOUND.replace(true) {
        bind();
    }
    repaint();
    if cloud::signed_in() && js_sys::Date::now() - AT.get() > 5_000.0 {
        spawn_local(load());
    }
}
