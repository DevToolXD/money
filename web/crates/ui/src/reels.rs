//! 릴스 탭: SNS에 올라온 게시물을 화면 하나에 하나씩, 위아래로 넘기며 봐요 (넘기면 딱 맞춰 멈춰요).
//! 사진이 있으면 화면 가득, 글만 있으면 큰 글씨로 보여줘요. 오른쪽에 좋아요·댓글, 아래에 쓴 사람과 글.
//! 게시물은 SNS 탭이 듣고 있는 최근 40개를 그대로 써서 서버를 더 읽지 않아요.

use wasm_bindgen::prelude::*;

use crate::dom::{self, by_id, esc, safe_img};

const HEART: &str = r#"<path d="M12 20.5s-7.5-4.6-7.5-10.4A4.3 4.3 0 0 1 12 7.3a4.3 4.3 0 0 1 7.5 2.8c0 5.8-7.5 10.4-7.5 10.4z"/>"#;
const BUBBLE: &str = r#"<path d="M20 11.5c0 4-3.8 7-8.5 7-1 0-2-.1-2.9-.4L4.5 19.5l1.3-3.3C4.7 15 4 13.4 4 11.5c0-4 3.8-7 8-7s8 3 8 7z"/>"#;
const GRADS: [&str; 5] = [
    "linear-gradient(160deg,#5b6cff,#8f5bd8 55%,#f06292)",
    "linear-gradient(160deg,#0ea5e9,#22c55e)",
    "linear-gradient(160deg,#f97316,#ef4444 60%,#a21caf)",
    "linear-gradient(160deg,#111827,#334155 60%,#0ea5e9)",
    "linear-gradient(160deg,#14b8a6,#6366f1)",
];

fn svg(d: &str) -> String {
    format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{d}</svg>"#)
}
fn person(uid: &str) -> (String, String) {
    let p = cloud::call_now("peek", &[JsValue::from_str(uid)]).unwrap_or(JsValue::NULL);
    let g = |k: &str| cloud::get(&p, k).as_string().unwrap_or_default();
    let n = g("name");
    (if n.is_empty() { g("id") } else { n }, g("photo"))
}

pub fn repaint() {
    let Some(el) = by_id("reels") else { return };
    if !cloud::signed_in() {
        return dom::set_html_if(&el, r#"<div class="rl-empty"><b>로그인하면 릴스를 볼 수 있어요</b></div>"#);
    }
    let posts = crate::sns::reel_posts();
    if posts.is_empty() {
        return dom::set_html_if(&el, r#"<div class="rl-empty"><b>아직 게시물이 없어요</b><p>SNS에서 첫 게시물을 올리면 여기서 넘겨 볼 수 있어요</p></div>"#);
    }
    let html: String = posts
        .iter()
        .enumerate()
        .map(|(i, (id, by, text, img, at, likes, liked, rc))| {
            let (n, ph) = person(by);
            let pic = safe_img(img, 300000);
            let bg = if pic.is_empty() { format!("background:{}", GRADS[i % GRADS.len()]) } else { format!("background-image:url({pic})") };
            let big = if pic.is_empty() && !text.is_empty() { format!(r#"<p class="rl-big">{}</p>"#, esc(text)) } else { String::new() };
            let cap = if !pic.is_empty() && !text.is_empty() { format!(r#"<p class="rl-cap">{}</p>"#, esc(text)) } else { String::new() };
            format!(
                r#"<section class="rl-s{photo}" data-id="{id}"><div class="rl-bg" style="{bg}"></div>{big}<div class="rl-side"><button type="button" class="rl-btn{on}" data-act="like" aria-label="좋아요">{heart}<span>{lk}</span></button><button type="button" class="rl-btn" data-act="replies" aria-label="댓글">{bubble}<span>{rc}</span></button></div><div class="rl-info"><button type="button" class="rl-who" data-uid="{by}">{av}<b>{name}{bl}</b><small>{ago}</small></button>{cap}</div></section>"#,
                photo = if pic.is_empty() { "" } else { " has-photo" },
                id = esc(id),
                on = if *liked { " on" } else { "" },
                heart = svg(HEART),
                lk = cloud::fmt_num(*likes, 0),
                bubble = svg(BUBBLE),
                rc = cloud::fmt_num(*rc, 0),
                by = esc(by),
                av = crate::presence::wrap(dom::avatar_html(&n, &ph, "avatar contact rl-av"), by),
                name = esc(&n),
                bl = crate::profile::blue(by),
                ago = crate::sns::ago(*at),
            )
        })
        .collect();
    dom::set_html_if(&el, &html);
}

fn bind() {
    let Some(el) = by_id("reels") else { return };
    // 스크롤 중에는 아무것도 읽지 않고, 멈추고 0.2초 뒤 한 번만 봐요
    dom::on(&el, "scroll", |_| {
        let Some(w) = web_sys::window() else { return };
        w.clear_timeout_with_handle(SETTLE.get());
        let cb = Closure::once_into_js(settle);
        SETTLE.set(w.set_timeout_with_callback_and_timeout_and_arguments_0(wasm_bindgen::JsCast::unchecked_ref(&cb), 200).unwrap_or(0));
    });
    dom::on(&el, "click", |e| {
        if let Some(b) = dom::closest(&e, "[data-uid]") {
            let uid = dom::data(&b, "uid");
            return crate::profile::show(&uid, &person(&uid).0, None);
        }
        let Some(b) = dom::closest(&e, "[data-act]") else { return double_tap(&e) };
        let Some(s) = dom::closest(&e, ".rl-s") else { return };
        let id = dom::data(&s, "id");
        match dom::data(&b, "act").as_str() {
            "like" => crate::sns::toggle_like(&id),
            _ => crate::reel_comments::open(&id),
        }
    });
}

/// 두 번 톡 누르면 좋아요 (인스타처럼). 아이폰은 dblclick을 잘 안 보내서 두 번 누른 시간을 직접 재요
fn double_tap(e: &web_sys::Event) {
    let Some(s) = dom::closest(e, ".rl-s") else { return };
    let id = dom::data(&s, "id");
    let t = js_sys::Date::now();
    let (lid, lt) = LAST_TAP.with(|l| l.borrow().clone());
    if lid != id || t - lt > 350.0 {
        LAST_TAP.with(|l| *l.borrow_mut() = (id, t));
        return;
    }
    LAST_TAP.with(|l| *l.borrow_mut() = (String::new(), 0.0));
    // 가운데에 큰 하트가 톡 튀어나왔다 사라져요
    if let Ok(h) = dom::doc().create_element("div") {
        h.set_class_name("rl-heart");
        h.set_inner_html(&svg(HEART));
        let _ = s.append_child(&h);
        leptos::prelude::set_timeout(move || h.remove(), std::time::Duration::from_millis(820));
    }
    dom::vibrate(8);
    let liked = crate::sns::reel_posts().iter().any(|p| p.0 == id && p.6);
    if !liked {
        crate::sns::toggle_like(&id);
    }
}

thread_local! {
    static LAST_TAP: std::cell::RefCell<(String, f64)> = const { std::cell::RefCell::new((String::new(), 0.0)) };
    static BOUND: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// 스크롤이 멈추면 보는 중인 게시물을 알려주는 시계, 지금 보는 게시물 번호
    static SETTLE: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
    static WATCHING: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// 스크롤이 멈춘 뒤 한 번만: 지금 화면의 게시물을 본 것으로 적고, 6초 넘게 보면 그 사람에게 관심을 더해요 (추천 알고리즘)
fn settle() {
    let Some(el) = by_id("reels") else { return };
    let h = el.client_height().max(1);
    let i = ((el.scroll_top() as f64) / h as f64).round() as usize;
    let posts = crate::sns::reel_posts();
    let Some(p) = posts.get(i) else { return };
    let (id, by) = (p.0.clone(), p.1.clone());
    if WATCHING.with(|w| *w.borrow() == id) {
        return;
    }
    WATCHING.with(|w| *w.borrow_mut() = id.clone());
    crate::algo::seen(&id);
    leptos::prelude::set_timeout(
        move || {
            if WATCHING.with(|w| *w.borrow() == id) && dom::screen_active("reels") && !dom::doc_hidden() {
                crate::algo::bump(&by, 0.5);
            }
        },
        std::time::Duration::from_millis(6000),
    );
}
/// 릴스 탭을 열 때
pub fn open() {
    if !BOUND.replace(true) {
        bind();
    }
    crate::sns::ensure_watch();
    crate::sns::gifts_refresh(false);
    let before = crate::sns::reel_posts().first().map(|p| p.0.clone());
    crate::sns::rerank(true, false);
    repaint();
    // 추천 순서가 새로 정해졌으면 맨 위부터 봐요
    if crate::sns::reel_posts().first().map(|p| p.0.clone()) != before {
        if let Some(el) = by_id("reels") {
            el.set_scroll_top(0);
        }
    }
    leptos::prelude::set_timeout(settle, std::time::Duration::from_millis(1200));
}
