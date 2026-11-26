//! 공모전 (메뉴 → 공모전): 관리자(owner)만 제목을 정해 공모전을 열고 닫아요.
//! 공모전이 열려 있으면 누구나 사진을 하나 올리고, 남의 사진에 한 표를 던져요 (다른 사진을 누르면 표가 옮겨 가고, 같은 사진을 다시 누르면 취소).
//! 끝난 공모전은 표가 많은 순서로 1·2·3등이 보여요.
//! 서버를 아끼려고 목록은 20개, 사진은 100개만 읽고, 듣기(실시간) 없이 열 때와 누른 뒤에만 다시 읽어요.

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::dom::{self, by_id, esc, safe_img};

#[derive(Clone, Default)]
struct Contest {
    id: String,
    title: String,
    open: bool,
    n: f64,
    at: f64,
}
#[derive(Clone, Default)]
struct Entry {
    by: String,
    img: String,
    votes: f64,
}

thread_local! {
    static LIST: RefCell<Vec<Contest>> = const { RefCell::new(Vec::new()) };
    /// 보고 있는 공모전 (비어 있으면 목록)
    static CUR: RefCell<String> = const { RefCell::new(String::new()) };
    static ENTRIES: RefCell<Vec<Entry>> = const { RefCell::new(Vec::new()) };
    static MY_VOTE: RefCell<String> = const { RefCell::new(String::new()) };
    static LOADING: Cell<bool> = const { Cell::new(false) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
    static BOUND: Cell<bool> = const { Cell::new(false) };
}

const TROPHY: &str = r#"<path d="M8 4.5h8v5a4 4 0 0 1-8 0z"/><path d="M8 6.5H5.5a2.5 2.5 0 0 0 2.6 3M16 6.5h2.5a2.5 2.5 0 0 1-2.6 3M12 13.5V17M8.5 20h7M9.5 17h5"/>"#;
const PLUS: &str = r#"<path d="M12 5v14M5 12h14"/>"#;
const BACK: &str = r#"<path d="M15 5l-7 7 7 7"/>"#;
const HEART: &str = r#"<path d="M12 20.5s-7.5-4.6-7.5-10.4A4.3 4.3 0 0 1 12 7.3a4.3 4.3 0 0 1 7.5 2.8c0 5.8-7.5 10.4-7.5 10.4z"/>"#;

fn svg(d: &str) -> String {
    format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{d}</svg>"#)
}
fn my_uid() -> String {
    cloud::cloud().and_then(|c| cloud::get(&c, "uid").as_string()).unwrap_or_default()
}
fn is_owner() -> bool {
    cloud::cloud().and_then(|c| cloud::get(&cloud::get(&c, "user"), "id").as_string()).is_some_and(|i| i.to_lowercase() == "owner")
}
fn person(uid: &str) -> (String, String) {
    let p = cloud::call_now("peek", &[JsValue::from_str(uid)]).unwrap_or(JsValue::NULL);
    let g = |k: &str| cloud::get(&p, k).as_string().unwrap_or_default();
    let n = g("name");
    (if n.is_empty() { g("id") } else { n }, g("photo"))
}
fn input(id: &str) -> Option<web_sys::HtmlInputElement> {
    wasm_bindgen::JsCast::dyn_into(by_id(id)?).ok()
}

/* ---------- 그리기 ---------- */

fn paint() {
    let Some(root) = by_id("contest-root") else { return };
    if !cloud::signed_in() {
        return dom::set_html_if(&root, r#"<p class="sns-empty">로그인하면 공모전을 볼 수 있어요</p>"#);
    }
    let cur = CUR.with(|c| c.borrow().clone());
    if cur.is_empty() {
        paint_list(&root)
    } else {
        paint_detail(&root, &cur)
    }
}
fn paint_list(root: &web_sys::HtmlElement) {
    let list = LIST.with(|l| l.borrow().clone());
    let mut html = String::new();
    if is_owner() {
        html.push_str(&format!(
            r#"<div class="card ct-new"><input id="ct-title" maxlength="40" autocomplete="off" placeholder="공모전 제목" aria-label="공모전 제목"><button type="button" class="ct-make" id="ct-make">{}열기</button></div>"#,
            svg(PLUS)
        ));
    }
    if list.is_empty() {
        html.push_str(if LOADING.get() { r#"<p class="sns-empty">불러오는 중이에요</p>"# } else { r#"<div class="pl-empty"><b>아직 열린 공모전이 없어요</b><p>공모전이 열리면 사진을 올리고 투표할 수 있어요</p></div>"# });
    } else {
        html.push_str(r#"<ul class="ct-list">"#);
        for c in &list {
            html.push_str(&format!(
                r#"<li><button type="button" class="card ct-row" data-cid="{}"><span class="ct-ic{}">{}</span><span class="ct-tx"><b>{}</b><small>{} · 사진 {}장</small></span><i class="ct-badge{}">{}</i></button></li>"#,
                esc(&c.id),
                if c.open { " on" } else { "" },
                svg(TROPHY),
                esc(&c.title),
                cloud::date_ko(c.at),
                cloud::fmt_num(c.n, 0),
                if c.open { " on" } else { "" },
                if c.open { "진행 중" } else { "종료" }
            ));
        }
        html.push_str("</ul>");
    }
    dom::set_html_if(root, &html);
}
fn paint_detail(root: &web_sys::HtmlElement, cid: &str) {
    let Some(c) = LIST.with(|l| l.borrow().iter().find(|x| x.id == cid).cloned()) else { return };
    let me = my_uid();
    let entries = ENTRIES.with(|e| e.borrow().clone());
    let mine = entries.iter().any(|e| e.by == me);
    let my_vote = MY_VOTE.with(|v| v.borrow().clone());
    let mut html = format!(
        r#"<div class="ct-head"><button type="button" class="glass-btn round ct-back" data-act="back" aria-label="목록">{}</button><span class="ct-badge{}">{}</span></div><h2 class="ct-title">{}</h2><p class="ct-sub">사진 {}장{}</p>"#,
        svg(BACK),
        if c.open { " on" } else { "" },
        if c.open { "진행 중" } else { "종료" },
        esc(&c.title),
        cloud::fmt_num(c.n, 0),
        if c.open { " · 마음에 드는 사진에 한 표를 던져요" } else { " · 표가 많은 순서예요" }
    );
    if c.open && !mine {
        html.push_str(&format!(r#"<button type="button" class="ct-enter" data-act="enter">{}내 사진 올리기</button>"#, svg(PLUS)));
    }
    if is_owner() && c.open {
        html.push_str(r#"<button type="button" class="ct-close" data-act="close">공모전 종료하기</button>"#);
    }
    if entries.is_empty() {
        html.push_str(if LOADING.get() { r#"<p class="sns-empty">불러오는 중이에요</p>"# } else { r#"<p class="sns-empty">아직 올라온 사진이 없어요</p>"# });
    } else {
        html.push_str(r#"<ul class="ct-grid">"#);
        for (i, e) in entries.iter().enumerate() {
            let (n, _) = person(&e.by);
            let pic = safe_img(&e.img, 300000);
            let voted = my_vote == e.by;
            let rank = if !c.open && i < 3 && e.votes > 0.0 { format!(r#"<i class="ct-rank r{}">{}등</i>"#, i + 1, i + 1) } else { String::new() };
            let btn = if e.by == me {
                r#"<span class="ct-mine">내 사진</span>"#.to_string()
            } else if c.open {
                format!(r#"<button type="button" class="ct-vote{}" data-act="vote" data-e="{}" aria-pressed="{voted}">{}{}</button>"#, if voted { " on" } else { "" }, esc(&e.by), svg(HEART), if voted { "투표함" } else { "투표" })
            } else {
                String::new()
            };
            html.push_str(&format!(
                r#"<li class="ct-card{}">{rank}<div class="ct-img" style="background-image:url({pic})" role="img" aria-label="{} 님의 사진"></div><div class="ct-foot"><span class="ct-who" data-uid="{}">{}</span><b class="ct-n">{}표</b></div>{btn}</li>"#,
                if voted { " voted" } else { "" },
                esc(&n),
                esc(&e.by),
                esc(&n),
                cloud::fmt_num(e.votes, 0)
            ));
        }
        html.push_str("</ul>");
    }
    dom::set_html_if(root, &html);
}

/* ---------- 읽기 ---------- */

async fn load_list() {
    LOADING.set(LIST.with(|l| l.borrow().is_empty()));
    paint();
    let r = cloud::call("contestList", &[]).await;
    LOADING.set(false);
    if let Ok(v) = r {
        let list: Vec<Contest> = js_sys::Array::from(&v)
            .iter()
            .map(|x| {
                let s = |k: &str| cloud::get(&x, k).as_string().unwrap_or_default();
                let n = |k: &str| cloud::get(&x, k).as_f64().unwrap_or(0.0);
                Contest { id: s("id"), title: s("title"), open: cloud::get(&x, "open") == JsValue::TRUE, n: n("n"), at: n("at") }
            })
            .collect();
        LIST.with(|l| *l.borrow_mut() = list);
    } else {
        crate::shell::toast("공모전을 불러오지 못했어요");
    }
    paint();
}
async fn load_detail(cid: String) {
    LOADING.set(ENTRIES.with(|e| e.borrow().is_empty()));
    paint();
    let (r, v) = (cloud::call("contestEntries", &[JsValue::from_str(&cid)]).await, cloud::call("contestMyVote", &[JsValue::from_str(&cid)]).await);
    LOADING.set(false);
    if CUR.with(|c| *c.borrow() != cid) {
        return;
    }
    let list: Vec<Entry> = r
        .map(|v| {
            js_sys::Array::from(&v)
                .iter()
                .map(|x| Entry { by: cloud::get(&x, "by").as_string().unwrap_or_default(), img: cloud::get(&x, "img").as_string().unwrap_or_default(), votes: cloud::get(&x, "votes").as_f64().unwrap_or(0.0) })
                .collect()
        })
        .unwrap_or_default();
    let who: js_sys::Array = list.iter().map(|e| JsValue::from_str(&e.by)).collect();
    // 참가 수를 목록에도 맞춰 둬요
    LIST.with(|l| l.borrow_mut().iter_mut().filter(|c| c.id == cid).for_each(|c| c.n = c.n.max(list.len() as f64)));
    ENTRIES.with(|e| *e.borrow_mut() = list);
    MY_VOTE.with(|m| *m.borrow_mut() = v.ok().and_then(|v| v.as_string()).unwrap_or_default());
    paint();
    let _ = cloud::call("fetchProfiles", &[who.into()]).await;
    paint();
}

/// 메뉴에서 공모전을 열 때
pub fn open() {
    bind_once();
    CUR.with(|c| c.borrow_mut().clear());
    paint();
    if cloud::signed_in() {
        spawn_local(load_list());
    }
}

/* ---------- 누르기 ---------- */

fn on_click(e: web_sys::Event) {
    if dom::closest(&e, "#ct-make").is_some() {
        return make();
    }
    if let Some(b) = dom::closest(&e, "[data-cid]") {
        let cid = dom::data(&b, "cid");
        CUR.with(|c| *c.borrow_mut() = cid.clone());
        ENTRIES.with(|x| x.borrow_mut().clear());
        MY_VOTE.with(|x| x.borrow_mut().clear());
        dom::vibrate(6);
        if let Some(s) = by_id("screen-contest") {
            s.set_scroll_top(0);
        }
        return spawn_local(load_detail(cid));
    }
    if let Some(b) = dom::closest(&e, "[data-uid]") {
        let uid = dom::data(&b, "uid");
        return crate::profile::show(&uid, &person(&uid).0, None);
    }
    let Some(b) = dom::closest(&e, "[data-act]") else { return };
    let cid = CUR.with(|c| c.borrow().clone());
    match dom::data(&b, "act").as_str() {
        "back" => {
            CUR.with(|c| c.borrow_mut().clear());
            spawn_local(load_list());
        }
        "enter" => {
            if let Some(f) = by_id("ct-file") {
                f.click();
            }
        }
        "close" => {
            spawn_local(async move {
                if crate::shell::ask("공모전을 끝낼까요?", "끝나면 더는 사진을 올리거나 투표할 수 없고, 표가 많은 순서로 등수가 정해져요.", "종료하기", "취소").await {
                    match cloud::call("contestClose", &[JsValue::from_str(&cid)]).await {
                        Ok(_) => {
                            LIST.with(|l| l.borrow_mut().iter_mut().filter(|c| c.id == cid).for_each(|c| c.open = false));
                            crate::shell::toast("공모전을 끝냈어요");
                            load_detail(cid).await;
                        }
                        Err(e) => crate::shell::toast(&format!("끝내지 못했어요. {}", cloud::why(&e.code))),
                    }
                }
            });
        }
        "vote" => {
            let entry = dom::data(&b, "e");
            if BUSY.replace(true) {
                return;
            }
            let old = MY_VOTE.with(|v| v.borrow().clone());
            dom::vibrate(8);
            // 먼저 화면에 바로 보여주고, 안 되면 되돌려요
            let apply = |entry: &str, old: &str| {
                ENTRIES.with(|x| {
                    for en in x.borrow_mut().iter_mut() {
                        if en.by == old {
                            en.votes = (en.votes - 1.0).max(0.0);
                        }
                        if en.by == entry && entry != old {
                            en.votes += 1.0;
                        }
                    }
                });
                MY_VOTE.with(|v| *v.borrow_mut() = if entry == old { String::new() } else { entry.to_string() });
            };
            apply(&entry, &old);
            paint();
            spawn_local(async move {
                let r = cloud::call("contestVote", &[JsValue::from_str(&cid), JsValue::from_str(&entry), JsValue::from_str(&old)]).await;
                BUSY.set(false);
                if let Err(e) = r {
                    crate::shell::toast(&format!("투표하지 못했어요. {}", cloud::why(&e.code)));
                    load_detail(cid).await;
                } else if entry == old {
                    crate::shell::toast("투표를 취소했어요");
                } else {
                    crate::shell::toast(if old.is_empty() { "투표했어요" } else { "표를 옮겼어요" });
                }
            });
        }
        _ => {}
    }
}
fn make() {
    let t: String = input("ct-title").map(|i| i.value()).unwrap_or_default().trim().chars().take(40).collect();
    if t.is_empty() {
        return crate::shell::toast("공모전 제목을 적어 주세요");
    }
    if t.contains(['<', '>']) {
        return crate::shell::toast("< > 는 쓸 수 없어요");
    }
    if BUSY.replace(true) {
        return;
    }
    spawn_local(async move {
        let r = cloud::call("contestCreate", &[JsValue::from_str(&t)]).await;
        BUSY.set(false);
        match r {
            Ok(_) => {
                if let Some(i) = input("ct-title") {
                    i.set_value("");
                }
                crate::shell::toast("공모전을 열었어요");
                load_list().await;
            }
            Err(e) => crate::shell::toast(&format!("열지 못했어요. {}", cloud::why(&e.code))),
        }
    });
}
fn on_file() {
    let Some(file) = crate::chat::attach::take_file("ct-file") else { return };
    let cid = CUR.with(|c| c.borrow().clone());
    if cid.is_empty() || BUSY.replace(true) {
        return;
    }
    spawn_local(async move {
        let img = crate::chat::attach::read_image(file, 1080.0, 280000, 0.8, false).await;
        let Ok(img) = img else {
            BUSY.set(false);
            return crate::shell::toast("사진을 넣지 못했어요");
        };
        crate::shell::toast("사진을 올리는 중이에요");
        let r = cloud::call("contestEnter", &[JsValue::from_str(&cid), JsValue::from_str(&img)]).await;
        BUSY.set(false);
        match r {
            Ok(_) => {
                crate::shell::toast("사진을 올렸어요");
                LIST.with(|l| l.borrow_mut().iter_mut().filter(|c| c.id == cid).for_each(|c| c.n += 1.0));
                load_detail(cid).await;
            }
            Err(e) => crate::shell::toast(&format!("올리지 못했어요. {}", cloud::why(&e.code))),
        }
    });
}

fn bind_once() {
    if BOUND.replace(true) {
        return;
    }
    if let Some(r) = by_id("contest-root") {
        dom::on(&r, "click", on_click);
        dom::on(&r, "keydown", |e| {
            if wasm_bindgen::JsCast::dyn_ref::<web_sys::KeyboardEvent>(&e).is_some_and(|k| k.key() == "Enter") && dom::closest(&e, "#ct-title").is_some() {
                make();
            }
        });
    }
    if let Some(f) = by_id("ct-file") {
        dom::on(&f, "change", |_| on_file());
    }
}
