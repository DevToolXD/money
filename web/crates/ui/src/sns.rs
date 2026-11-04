//! SNS (무제 탭): 스레드 앱의 홈 화면처럼. 아래쪽 탭은 없고 홈 피드만 있어요.
//! - 맨 위: 내 동그라미 + "새로운 소식이 있나요?" (누르면 글쓰기 판이 올라와요)
//! - 글: 동그라미, 이름, 시간, 글, 사진, 좋아요·댓글·리포스트·공유. 댓글이 있으면 동그라미 아래로 가는 선이 이어져요
//! - 글을 누르면 그 글과 댓글이 열리고, 아래에서 답글을 남겨요
//! 서버를 아끼려고 피드는 이 탭을 처음 열 때부터 최근 40개만 들어요.

use std::cell::{Cell, RefCell};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::dom::{self, by_id, esc};

#[derive(Clone)]
struct Post {
    id: String,
    by: String,
    text: String,
    img: String,
    at: f64,
    likes: f64,
    reposts: f64,
    rc: f64,
    liked: bool,
    reposted: bool,
}
impl Post {
    fn from(v: &JsValue) -> Self {
        let s = |k: &str| cloud::get(v, k).as_string().unwrap_or_default();
        let n = |k: &str| cloud::get(v, k).as_f64().unwrap_or(0.0);
        let b = |k: &str| cloud::get(v, k).as_bool().unwrap_or(false);
        Post { id: s("id"), by: s("by"), text: s("text"), img: s("img"), at: n("at"), likes: n("likes"), reposts: n("reposts"), rc: n("rc"), liked: b("liked"), reposted: b("reposted") }
    }
}

thread_local! {
    static FEED: RefCell<Vec<Post>> = const { RefCell::new(Vec::new()) };
    static WATCHING: Cell<bool> = const { Cell::new(false) };
    static OPEN: RefCell<Option<(Post, Vec<Post>)>> = const { RefCell::new(None) };
    static REPLY_TO: RefCell<String> = const { RefCell::new(String::new()) };
    static DRAFT_IMG: RefCell<String> = const { RefCell::new(String::new()) };
    static POSTING: Cell<bool> = const { Cell::new(false) };
}

/* ---------- 아이콘 (직접 그린 윤곽선) ---------- */
fn svg(d: &str) -> String {
    format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{d}</svg>"#)
}
const HEART: &str = r#"<path d="M12 20s-7.5-4.6-7.5-10.2A4.3 4.3 0 0 1 12 7.3a4.3 4.3 0 0 1 7.5 2.5C19.5 15.4 12 20 12 20z"/>"#;
const BUBBLE: &str = r#"<path d="M20 11.5a8 8 0 0 1-11.6 7.1L4 20l1.4-4.2A8 8 0 1 1 20 11.5z"/>"#;
const REPOST: &str = r#"<path d="M7 7h10l-2.5-2.5M17 17H7l2.5 2.5"/><path d="M17 7v4M7 17v-4"/>"#;
const SHARE: &str = r#"<path d="M3.5 11.3 20 4l-7.3 16.5-2.3-6.9z"/><path d="M10.4 13.6 20 4"/>"#;
const MORE: &str = r#"<circle cx="6" cy="12" r="1.2" fill="currentColor"/><circle cx="12" cy="12" r="1.2" fill="currentColor"/><circle cx="18" cy="12" r="1.2" fill="currentColor"/>"#;
const PHOTO: &str = r#"<rect x="3.5" y="5" width="17" height="14" rx="3"/><circle cx="9" cy="10" r="1.6"/><path d="M20.5 15.5l-4.5-4.5-8.5 8"/>"#;
const BACK: &str = r#"<path d="M15 5l-7 7 7 7"/>"#;
// 우리 앱의 표시: 겹친 두 말풍선 (어느 서비스의 로고도 아니에요)
const MARK: &str = r#"<path d="M14.5 9.5a6 6 0 1 1-3.4 5.4"/><path d="M9.5 14.5a6 6 0 1 1 3.4-5.4"/>"#;

/* ---------- 이름·사진 ---------- */
fn person(uid: &str) -> (String, String) {
    let p = cloud::call_now("peek", &[JsValue::from_str(uid)]).unwrap_or(JsValue::NULL);
    let name = cloud::get(&p, "name").as_string().filter(|s| !s.is_empty()).or_else(|| cloud::get(&p, "id").as_string()).unwrap_or_else(|| "…".into());
    (name, cloud::get(&p, "photo").as_string().unwrap_or_default())
}
fn need_people(list: &[Post]) {
    let missing: js_sys::Array = list.iter().filter(|p| person(&p.by).0 == "…").map(|p| JsValue::from_str(&p.by)).collect();
    if missing.length() == 0 {
        return;
    }
    spawn_local(async move {
        if cloud::call("fetchProfiles", &[missing.into()]).await.is_ok() {
            paint_feed();
            paint_open();
        }
    });
}
fn ago(at: f64) -> String {
    let s = ((js_sys::Date::now() - at) / 1000.0).max(0.0);
    if s < 60.0 {
        "방금".into()
    } else if s < 3600.0 {
        format!("{}분", (s / 60.0) as i64)
    } else if s < 86400.0 {
        format!("{}시간", (s / 3600.0) as i64)
    } else if s < 86400.0 * 7.0 {
        format!("{}일", (s / 86400.0) as i64)
    } else {
        cloud::date_ko(at)
    }
}
fn count(n: f64) -> String {
    if n <= 0.0 {
        String::new()
    } else {
        crate::money::w(n).replace(" 토큰", "")
    }
}

/* ---------- 그리기 ---------- */
fn post_html(p: &Post, big: bool, thread_line: bool) -> String {
    let (name, photo) = person(&p.by);
    let me = cloud::cloud().map(|c| cloud::get(&c, "uid").as_string().unwrap_or_default()).unwrap_or_default();
    let img = dom::safe_img(&p.img, 300000);
    format!(
        r#"<article class="sp{}" data-id="{id}"><div class="sp-l">{av}{line}</div><div class="sp-r"><div class="sp-h"><b>{name}</b><span class="sp-t">{t}</span>{more}</div>{text}{img}<div class="sp-a"><button type="button" class="sp-b{lk}" data-like="{id}" aria-label="좋아요" aria-pressed="{liked}">{heart}<span>{nl}</span></button><button type="button" class="sp-b" data-reply="{id}" aria-label="댓글">{bub}<span>{nr}</span></button><button type="button" class="sp-b{rp}" data-repost="{id}" aria-label="리포스트" aria-pressed="{reposted}">{rep}<span>{nrp}</span></button><button type="button" class="sp-b" data-share="{id}" aria-label="공유">{share}</button></div></div></article>"#,
        if big { " big" } else { "" },
        id = esc(&p.id),
        av = dom::avatar_html(&name, &photo, "avatar contact sp-av"),
        line = if thread_line { r#"<i class="sp-line" aria-hidden="true"></i>"# } else { "" },
        name = esc(&name),
        t = ago(p.at),
        more = if p.by == me { format!(r#"<button type="button" class="sp-more" data-more="{}" aria-label="더 보기">{}</button>"#, esc(&p.id), svg(MORE)) } else { String::new() },
        text = if p.text.is_empty() { String::new() } else { format!(r#"<p class="sp-x">{}</p>"#, esc(&p.text)) },
        img = if img.is_empty() { String::new() } else { format!(r#"<img class="sp-img" src="{img}" alt="" loading="lazy" decoding="async">"#) },
        lk = if p.liked { " on" } else { "" },
        liked = p.liked,
        heart = svg(HEART),
        nl = count(p.likes),
        bub = svg(BUBBLE),
        nr = count(p.rc),
        rp = if p.reposted { " on" } else { "" },
        reposted = p.reposted,
        rep = svg(REPOST),
        nrp = count(p.reposts),
        share = svg(SHARE),
    )
}

fn root() -> Option<web_sys::HtmlElement> {
    by_id("sns")
}

fn build() {
    let Some(sc) = crate::shell::screen("untitled3") else { return };
    if root().is_some() {
        return;
    }
    sc.set_inner_html(&format!(
        r#"<div class="sns" id="sns"><header class="sns-top"><span class="sns-mark">{}</span></header><button type="button" class="sns-compose" id="sns-compose"><span id="sns-me"></span><span class="sns-ph">새로운 소식이 있나요?</span><span class="sns-pic">{}</span></button><div class="sns-feed" id="sns-feed"><p class="sns-empty">불러오는 중이에요</p></div></div>
        <section class="sns-sheet" id="sns-sheet" hidden aria-label="새로운 스레드"><header class="sns-sh"><button type="button" class="sns-cancel" data-sheet-close>취소</button><b id="sns-sheet-title">새로운 스레드</b><span></span></header><div class="sns-parent" id="sns-parent"></div><div class="sns-write"><span id="sns-me2"></span><div class="sns-wr"><b id="sns-myname"></b><textarea id="sns-text" maxlength="500" rows="3" placeholder="새로운 소식이 있나요?"></textarea><div class="sns-prev" id="sns-prev"></div><label class="sns-attach" aria-label="사진 넣기">{}<input id="sns-file" type="file" accept="image/*" hidden></label></div></div><footer class="sns-sf"><span>누구에게나 답글 허용</span><button type="button" class="sns-post" id="sns-post" disabled>게시</button></footer></section>
        <section class="sns-detail" id="sns-detail" hidden aria-label="스레드"><header class="sns-dh"><button type="button" class="sns-back" data-detail-close aria-label="뒤로">{}</button><b>스레드</b><span></span></header><div class="sns-dbody" id="sns-dbody"></div><button type="button" class="sns-replybar" id="sns-replybar"></button></section>"#,
        svg(MARK),
        svg(PHOTO),
        svg(PHOTO),
        svg(BACK)
    ));
    // 글쓰기 판과 글 보기는 화면 위에 덮이도록 앱 틀로 옮겨요
    if let Some(app) = dom::app_el() {
        for id in ["sns-sheet", "sns-detail"] {
            if let Some(el) = by_id(id) {
                let _ = app.append_child(&el);
                dom::on(&el, "click", on_click);
            }
        }
    }
    let Some(r) = by_id("sns") else { return };
    dom::on(&r, "click", on_click);
    if let Some(t) = by_id("sns-text") {
        dom::on(&t, "input", |_| paint_post_btn());
    }
    if let Some(f) = by_id("sns-file") {
        dom::on(&f, "change", |_| {
            let Some(file) = crate::chat::attach::take_file("sns-file") else { return };
            spawn_local(async move {
                match crate::chat::attach::read_image(file, 1080.0, 280000, 0.78, false).await {
                    Ok(img) => {
                        DRAFT_IMG.with(|d| *d.borrow_mut() = img);
                        paint_draft_img();
                    }
                    Err(_) => crate::shell::toast("사진을 넣지 못했어요"),
                }
            });
        });
    }
}

fn me_html(cls: &str) -> String {
    let uid = cloud::cloud().map(|c| cloud::get(&c, "uid").as_string().unwrap_or_default()).unwrap_or_default();
    let (name, photo) = person(&uid);
    let name = if name == "…" { cloud::cloud().map(|c| cloud::get(&cloud::get(&c, "user"), "name").as_string().unwrap_or_default()).unwrap_or_default() } else { name };
    dom::avatar_html(&name, &photo, cls)
}

/// 무제 탭을 열 때
pub fn open() {
    build();
    let Some(feed) = by_id("sns-feed") else { return };
    if !cloud::signed_in() {
        dom::set_hidden("sns-compose", true);
        feed.set_inner_html(r#"<p class="sns-empty">로그인하면 글을 보고 쓸 수 있어요</p>"#);
        return;
    }
    dom::set_hidden("sns-compose", false);
    if let Some(m) = by_id("sns-me") {
        m.set_inner_html(&me_html("avatar contact sp-av"));
    }
    if WATCHING.replace(true) {
        return;
    }
    let cb = Closure::<dyn FnMut(JsValue)>::new(|list: JsValue| {
        let posts: Vec<Post> = js_sys::Array::from(&list).iter().map(|v| Post::from(&v)).collect();
        need_people(&posts);
        FEED.with(|f| *f.borrow_mut() = posts);
        paint_feed();
    });
    let _ = cloud::call_now("postsWatch", &[cb.into_js_value()]);
}

fn paint_feed() {
    let Some(feed) = by_id("sns-feed") else { return };
    let html = FEED.with(|f| f.borrow().iter().map(|p| post_html(p, false, false)).collect::<String>());
    feed.set_inner_html(if html.is_empty() { r#"<p class="sns-empty">아직 글이 없어요. 첫 글을 남겨 보세요</p>"# } else { &html });
}

fn find(id: &str) -> Option<Post> {
    let in_feed = FEED.with(|f| f.borrow().iter().find(|p| p.id == id).cloned());
    in_feed.or_else(|| OPEN.with(|o| o.borrow().as_ref().and_then(|(p, rs)| if p.id == id { Some(p.clone()) } else { rs.iter().find(|r| r.id == id).cloned() })))
}
fn patch(id: &str, f: impl Fn(&mut Post)) {
    FEED.with(|x| x.borrow_mut().iter_mut().filter(|p| p.id == id).for_each(&f));
    OPEN.with(|o| {
        if let Some((p, rs)) = o.borrow_mut().as_mut() {
            if p.id == id {
                f(p);
            }
            rs.iter_mut().filter(|r| r.id == id).for_each(&f);
        }
    });
}

fn on_click(e: web_sys::Event) {
    if dom::closest(&e, "#sns-compose").is_some() {
        return open_sheet("");
    }
    if dom::closest(&e, "[data-sheet-close]").is_some() {
        return close_sheet();
    }
    if dom::closest(&e, "#sns-post").is_some() {
        return post();
    }
    if dom::closest(&e, "[data-detail-close]").is_some() {
        return close_detail();
    }
    if dom::closest(&e, "#sns-replybar").is_some() {
        let id = OPEN.with(|o| o.borrow().as_ref().map(|(p, _)| p.id.clone())).unwrap_or_default();
        return open_sheet(&id);
    }
    if let Some(b) = dom::closest(&e, "[data-like]") {
        return toggle(&dom::data(&b, "like"), true);
    }
    if let Some(b) = dom::closest(&e, "[data-repost]") {
        return toggle(&dom::data(&b, "repost"), false);
    }
    if let Some(b) = dom::closest(&e, "[data-reply]") {
        let id = dom::data(&b, "reply");
        return open_sheet(&id);
    }
    if let Some(b) = dom::closest(&e, "[data-share]") {
        let text = find(&dom::data(&b, "share")).map(|p| p.text).unwrap_or_default();
        if let Some(w) = web_sys::window() {
            let _ = w.navigator().clipboard().write_text(&text);
        }
        return crate::shell::toast("글을 복사했어요");
    }
    if let Some(b) = dom::closest(&e, "[data-more]") {
        let id = dom::data(&b, "more");
        spawn_local(async move {
            if !crate::shell::ask("글을 삭제할까요?", "삭제하면 되돌릴 수 없어요", "삭제", "취소").await {
                return;
            }
            match cloud::call("postDelete", &[JsValue::from_str(&id)]).await {
                Ok(_) => {
                    crate::shell::toast("글을 삭제했어요");
                    if OPEN.with(|o| o.borrow().as_ref().is_some_and(|(p, _)| p.id == id)) {
                        close_detail();
                    } else {
                        OPEN.with(|o| {
                            if let Some((_, rs)) = o.borrow_mut().as_mut() {
                                rs.retain(|r| r.id != id);
                            }
                        });
                        paint_open();
                    }
                }
                Err(_) => crate::shell::toast("삭제하지 못했어요"),
            }
        });
        return;
    }
    if let Some(a) = dom::closest(&e, ".sp") {
        let in_detail = dom::closest(&e, "#sns-detail").is_some();
        if !in_detail {
            open_detail(&dom::data(&a, "id"));
        }
    }
}

fn toggle(id: &str, like: bool) {
    let Some(p) = find(id) else { return };
    let on = if like { !p.liked } else { !p.reposted };
    // 먼저 화면을 바꾸고, 서버에 실패하면 되돌려요
    let apply = move |p: &mut Post, on: bool| {
        if like {
            if p.liked != on {
                p.likes += if on { 1.0 } else { -1.0 };
            }
            p.liked = on;
        } else {
            if p.reposted != on {
                p.reposts += if on { 1.0 } else { -1.0 };
            }
            p.reposted = on;
        }
    };
    patch(id, |p| apply(p, on));
    paint_feed();
    paint_open();
    if like && on {
        dom::vibrate(8);
    }
    let id = id.to_string();
    spawn_local(async move {
        if cloud::call(if like { "postLike" } else { "postRepost" }, &[JsValue::from_str(&id), on.into()]).await.is_err() {
            patch(&id, |p| apply(p, !on));
            paint_feed();
            paint_open();
            crate::shell::toast("반영하지 못했어요");
        }
    });
}

/* ---------- 글쓰기 판 ---------- */
fn open_sheet(reply_to: &str) {
    let Some(sh) = by_id("sns-sheet") else { return };
    REPLY_TO.with(|r| *r.borrow_mut() = reply_to.to_string());
    DRAFT_IMG.with(|d| d.borrow_mut().clear());
    dom::set_text("sns-sheet-title", if reply_to.is_empty() { "새로운 스레드" } else { "답글" });
    if let Some(m) = by_id("sns-me2") {
        m.set_inner_html(&me_html("avatar contact sp-av"));
    }
    let uid = cloud::cloud().map(|c| cloud::get(&c, "uid").as_string().unwrap_or_default()).unwrap_or_default();
    let my = person(&uid).0;
    let my = if my == "…" { cloud::cloud().map(|c| cloud::get(&cloud::get(&c, "user"), "name").as_string().unwrap_or_default()).unwrap_or_default() } else { my };
    dom::set_text("sns-myname", &my);
    if let Some(par) = by_id("sns-parent") {
        par.set_inner_html(&find(reply_to).map(|p| post_html(&p, false, true)).unwrap_or_default());
    }
    if let Some(t) = by_id("sns-text").and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok()) {
        t.set_value("");
        let who = find(reply_to).map(|p| person(&p.by).0);
        t.set_placeholder(&who.map(|n| format!("{n}님에게 답글 남기기")).unwrap_or_else(|| "새로운 소식이 있나요?".into()));
        let t2 = t.clone();
        leptos::prelude::request_animation_frame(move || {
            let _ = t2.focus();
        });
    }
    paint_draft_img();
    paint_post_btn();
    sh.set_hidden(false);
    let _ = sh.class_list().remove_1("out");
    let _ = sh.offset_width();
    let _ = sh.class_list().add_1("in");
}
fn close_sheet() {
    let Some(sh) = by_id("sns-sheet") else { return };
    let _ = sh.class_list().remove_1("in");
    let _ = sh.class_list().add_1("out");
    let cb = Closure::once_into_js(move || {
        if sh.class_list().contains("out") {
            sh.set_hidden(true);
        }
    });
    let _ = web_sys::window().map(|w| w.set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 380));
}
fn draft_text() -> String {
    by_id("sns-text").and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok()).map(|t| t.value()).unwrap_or_default().trim().to_string()
}
fn paint_post_btn() {
    let empty = draft_text().is_empty() && DRAFT_IMG.with(|d| d.borrow().is_empty());
    if let Some(b) = by_id("sns-post").and_then(|b| b.dyn_into::<web_sys::HtmlButtonElement>().ok()) {
        b.set_disabled(empty || POSTING.get());
    }
}
fn paint_draft_img() {
    let img = DRAFT_IMG.with(|d| d.borrow().clone());
    if let Some(p) = by_id("sns-prev") {
        let src = dom::safe_img(&img, 300000);
        p.set_inner_html(&if src.is_empty() { String::new() } else { format!(r#"<img src="{src}" alt="">"#) });
    }
    paint_post_btn();
}
fn post() {
    if POSTING.get() {
        return;
    }
    let text = draft_text();
    let img = DRAFT_IMG.with(|d| d.borrow().clone());
    if text.is_empty() && img.is_empty() {
        return;
    }
    let parent = REPLY_TO.with(|r| r.borrow().clone());
    POSTING.set(true);
    paint_post_btn();
    spawn_local(async move {
        let r = cloud::call("postCreate", &[JsValue::from_str(&text), JsValue::from_str(&img), JsValue::from_str(&parent)]).await;
        POSTING.set(false);
        match r {
            Ok(_) => {
                close_sheet();
                crate::shell::toast(if parent.is_empty() { "게시했어요" } else { "답글을 남겼어요" });
                if !parent.is_empty() {
                    patch(&parent, |p| p.rc += 1.0);
                    paint_feed();
                    if OPEN.with(|o| o.borrow().as_ref().is_some_and(|(p, _)| p.id == parent)) {
                        load_replies(&parent).await;
                    }
                }
            }
            Err(_) => {
                crate::shell::toast("게시하지 못했어요");
                paint_post_btn();
            }
        }
    });
}

/* ---------- 글 하나와 댓글 ---------- */
fn open_detail(id: &str) {
    let Some(p) = find(id) else { return };
    let Some(d) = by_id("sns-detail") else { return };
    OPEN.with(|o| *o.borrow_mut() = Some((p.clone(), Vec::new())));
    paint_open();
    d.set_hidden(false);
    let _ = d.class_list().remove_1("out");
    let _ = d.offset_width();
    let _ = d.class_list().add_1("in");
    let id = id.to_string();
    spawn_local(async move { load_replies(&id).await });
}
async fn load_replies(id: &str) {
    let Ok(list) = cloud::call("postReplies", &[JsValue::from_str(id)]).await else { return };
    let rs: Vec<Post> = js_sys::Array::from(&list).iter().map(|v| Post::from(&v)).collect();
    need_people(&rs);
    OPEN.with(|o| {
        if let Some((p, old)) = o.borrow_mut().as_mut() {
            if p.id == id {
                p.rc = rs.len() as f64;
                *old = rs;
            }
        }
    });
    paint_open();
}
fn paint_open() {
    let Some(body) = by_id("sns-dbody") else { return };
    let Some((p, rs)) = OPEN.with(|o| o.borrow().clone()) else { return };
    let replies: String = rs.iter().map(|r| post_html(r, false, false)).collect();
    body.set_inner_html(&format!(
        r#"{}<div class="sns-rh"><b>답글</b><span>{}개</span></div>{}"#,
        post_html(&p, true, false),
        rs.len(),
        if replies.is_empty() { r#"<p class="sns-empty">아직 답글이 없어요</p>"#.to_string() } else { replies }
    ));
    let who = person(&p.by).0;
    if let Some(b) = by_id("sns-replybar") {
        b.set_inner_html(&format!(r#"{}<span>{}님에게 답글 남기기</span>"#, me_html("avatar contact sp-av sm"), esc(&who)));
    }
}
fn close_detail() {
    let Some(d) = by_id("sns-detail") else { return };
    let _ = d.class_list().remove_1("in");
    let _ = d.class_list().add_1("out");
    let cb = Closure::once_into_js(move || {
        if d.class_list().contains("out") {
            d.set_hidden(true);
            OPEN.with(|o| *o.borrow_mut() = None);
        }
    });
    let _ = web_sys::window().map(|w| w.set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 380));
}
