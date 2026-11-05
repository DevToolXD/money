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
    topic: String,
    first: bool,
    sh: f64,
    last: String,
}
impl Post {
    fn from(v: &JsValue) -> Self {
        let s = |k: &str| cloud::get(v, k).as_string().unwrap_or_default();
        let n = |k: &str| cloud::get(v, k).as_f64().unwrap_or(0.0);
        let b = |k: &str| cloud::get(v, k).as_bool().unwrap_or(false);
        Post { id: s("id"), by: s("by"), text: s("text"), img: s("img"), at: n("at"), likes: n("likes"), reposts: n("reposts"), rc: n("rc"), liked: b("liked"), reposted: b("reposted"), topic: s("topic"), first: b("first"), sh: n("sh"), last: s("last") }
    }
}

thread_local! {
    static FEED: RefCell<Vec<Post>> = const { RefCell::new(Vec::new()) };
    static WATCHING: Cell<bool> = const { Cell::new(false) };
    static OPEN: RefCell<Option<(Post, Vec<Post>)>> = const { RefCell::new(None) };
    static REPLY_TO: RefCell<String> = const { RefCell::new(String::new()) };
    static DRAFT_IMG: RefCell<String> = const { RefCell::new(String::new()) };
    static POSTING: Cell<bool> = const { Cell::new(false) };
    /// 답글 안으로 들어갈 때 이전 글을 쌓아 둬요 (뒤로 가면 하나씩 돌아가요)
    static STACK: RefCell<Vec<Post>> = const { RefCell::new(Vec::new()) };
    /// 내가 팔로우한 사람들
    static FOLLOWS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// 피드 보기: 0 전체, 1 팔로우 중, 2 내 글
    static MODE: Cell<u8> = const { Cell::new(0) };
    /// 찾기 글자 (비어 있으면 다 보여요)
    static QUERY: RefCell<String> = const { RefCell::new(String::new()) };
    static PULL: Cell<Option<f64>> = const { Cell::new(None) };
    static REFRESHING: Cell<bool> = const { Cell::new(false) };
    /// 피드에 미리 보여줄 마지막 답글 (답글 번호 → 답글, 없어졌으면 None). 한 번 읽으면 기억해요
    static LAST_REPLY: RefCell<std::collections::HashMap<String, Option<Post>>> = RefCell::new(std::collections::HashMap::new());
}

/* ---------- 아이콘 (직접 그린 윤곽선) ---------- */
fn svg(d: &str) -> String {
    format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{d}</svg>"#)
}
const HEART: &str = r#"<path d="M12 20.3c-.3 0-8.8-4.9-8.8-11A4.9 4.9 0 0 1 12 6.6a4.9 4.9 0 0 1 8.8 2.7c0 6.1-8.5 11-8.8 11z"/>"#;
const BUBBLE: &str = r#"<path d="M20.4 12a8.4 8.4 0 0 1-12.5 7.3L3.6 20.4l1.1-4.3A8.4 8.4 0 1 1 20.4 12z"/>"#;
const REPOST: &str = r#"<path d="M18.5 13.5v1a4 4 0 0 1-4 4H8"/><path d="M10.5 16l-2.5 2.5 2.5 2.5"/><path d="M5.5 10.5v-1a4 4 0 0 1 4-4H16"/><path d="M13.5 8l2.5-2.5L13.5 3"/>"#;
const SHARE: &str = r#"<path d="M21 12 4 4.5l2.6 7.5L4 19.5z"/><path d="M6.6 12H21"/>"#;
const MORE: &str = r#"<circle cx="6" cy="12" r="1.2" fill="currentColor"/><circle cx="12" cy="12" r="1.2" fill="currentColor"/><circle cx="18" cy="12" r="1.2" fill="currentColor"/>"#;
const PHOTO: &str = r#"<rect x="3.5" y="5" width="17" height="14" rx="3"/><circle cx="9" cy="10" r="1.6"/><path d="M20.5 15.5l-4.5-4.5-8.5 8"/>"#;
const BACK: &str = r#"<path d="M15 5l-7 7 7 7"/>"#;
const MENU: &str = r#"<path d="M4 9h16M4 15h16"/>"#;
const SEARCH: &str = r#"<circle cx="11" cy="11" r="6.5"/><path d="M16 16l4 4"/>"#;
const PLUS: &str = r#"<path d="M12 5v14M5 12h14"/>"#;
const STAR: &str = r#"<path d="M12 4l2.4 4.9 5.4.8-3.9 3.8.9 5.4L12 16.4l-4.8 2.5.9-5.4-3.9-3.8 5.4-.8z"/>"#;
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
fn my_uid() -> String {
    cloud::cloud().map(|c| cloud::get(&c, "uid").as_string().unwrap_or_default()).unwrap_or_default()
}
fn post_html(p: &Post, big: bool, thread_line: bool) -> String {
    post_html_x(p, big, thread_line, false)
}
/// inline: 피드에서 원래 글 아래에 들여 쓴 답글 미리보기
fn post_html_x(p: &Post, big: bool, thread_line: bool, inline: bool) -> String {
    let (name, photo) = person(&p.by);
    let me = my_uid();
    let img = dom::safe_img(&p.img, 300000);
    let follow = p.by != me && !me.is_empty() && !FOLLOWS.with(|f| f.borrow().contains(&p.by));
    let n = |v: f64| if v > 0.0 { format!("<span>{}</span>", count(v)) } else { String::new() };
    format!(
        r#"{first}<article class="sp{big}{inl}" data-id="{id}"><div class="sp-l"><span class="sp-avw">{av}{badge}</span>{line}</div><div class="sp-r"><div class="sp-h"><b>{name}</b>{topic}<span class="sp-t">{t}</span><button type="button" class="sp-more" data-more="{id}" data-mine="{mine}" aria-label="더 보기">{more}</button></div>{text}{img}<div class="sp-a"><button type="button" class="sp-b{lk}" data-like="{id}" aria-label="좋아요" aria-pressed="{liked}">{heart}{nl}</button><button type="button" class="sp-b" data-reply="{id}" aria-label="댓글">{bub}{nr}</button><button type="button" class="sp-b{rp}" data-repost="{id}" aria-label="리포스트" aria-pressed="{reposted}">{rep}{nrp}</button><button type="button" class="sp-b" data-share="{id}" aria-label="보내기">{share}{nsh}</button></div></div></article>"#,
        first = if p.first && !big && !inline { format!(r#"<p class="sp-first" aria-label="첫 스레드">{}첫 스레드</p>"#, svg(STAR)) } else { String::new() },
        big = if big { " big" } else { "" },
        inl = if inline { " sp-in" } else if thread_line { " has-in" } else { "" },
        id = esc(&p.id),
        av = dom::avatar_html(&name, &photo, "avatar contact sp-av"),
        badge = if follow { format!(r#"<button type="button" class="sp-follow" data-follow="{}" aria-label="팔로우">{}</button>"#, esc(&p.by), svg(PLUS)) } else { String::new() },
        line = if thread_line { r#"<i class="sp-line" aria-hidden="true"></i>"# } else { "" },
        name = esc(&name),
        topic = if p.topic.is_empty() { String::new() } else { format!(r#"<span class="sp-tp"><em>›</em><i aria-hidden="true"></i>{}</span>"#, esc(&p.topic)) },
        t = ago(p.at),
        mine = p.by == me,
        more = svg(MORE),
        text = if p.text.is_empty() { String::new() } else { format!(r#"<p class="sp-x">{}</p>"#, esc(&p.text)) },
        img = if img.is_empty() { String::new() } else { format!(r#"<div class="sp-media"><img class="sp-img" src="{img}" alt="" loading="lazy" decoding="async"></div>"#) },
        lk = if p.liked { " on" } else { "" },
        liked = p.liked,
        heart = svg(HEART),
        nl = n(p.likes),
        bub = svg(BUBBLE),
        nr = n(p.rc),
        rp = if p.reposted { " on" } else { "" },
        reposted = p.reposted,
        rep = svg(REPOST),
        nrp = n(p.reposts),
        share = svg(SHARE),
        nsh = n(p.sh),
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
        r#"<div class="sns" id="sns"><header class="sns-top" id="sns-top"><button type="button" class="sns-ic" data-menu aria-label="보기 메뉴">{menu}</button><span class="sns-mark" id="sns-mark">{}</span><button type="button" class="sns-ic" data-search aria-label="찾기">{search}</button></header><div class="sns-search" id="sns-search" hidden><label class="sns-q">{search}<input id="sns-q" type="search" placeholder="글이나 사람 찾기" autocomplete="off"></label><button type="button" class="sns-cancel" data-search-close>취소</button></div><div class="sns-menu" id="sns-menu" hidden><button type="button" data-mode="0">전체</button><button type="button" data-mode="1">팔로우 중</button><button type="button" data-mode="2">내 글</button><button type="button" data-refresh>새로고침</button></div><div class="sns-body" id="sns-body"><button type="button" class="sns-compose" id="sns-compose"><span id="sns-me"></span><span class="sns-cw"><b id="sns-myname0"></b><span class="sns-ph">새로운 소식을 공유해보세요.</span></span><span class="sns-pic">{}</span></button><div class="sns-feed" id="sns-feed"><p class="sns-empty">불러오는 중이에요</p></div></div></div><button type="button" class="sns-fab" id="sns-fab" aria-label="새 글 쓰기">{plus}</button>
        <section class="sns-sheet" id="sns-sheet" hidden aria-label="새로운 스레드"><header class="sns-sh"><button type="button" class="sns-cancel" data-sheet-close>취소</button><b id="sns-sheet-title">새로운 스레드</b><span></span></header><div class="sns-parent" id="sns-parent"></div><div class="sns-write"><span id="sns-me2"></span><div class="sns-wr"><div class="sns-who"><b id="sns-myname"></b><input id="sns-topic" class="sns-topic" maxlength="20" placeholder="› 주제 추가" autocomplete="off" aria-label="주제"></div><textarea id="sns-text" maxlength="500" rows="3" placeholder="새로운 소식이 있나요?"></textarea><div class="sns-prev" id="sns-prev"></div><label class="sns-attach" aria-label="사진 넣기">{}<input id="sns-file" type="file" accept="image/*" hidden></label></div></div><footer class="sns-sf"><span>누구에게나 답글 허용</span><button type="button" class="sns-post" id="sns-post" disabled>게시</button></footer></section>
        <section class="sns-detail" id="sns-detail" hidden aria-label="스레드"><header class="sns-dh"><button type="button" class="sns-back" data-detail-close aria-label="뒤로">{}</button><b>스레드</b><span></span></header><div class="sns-dbody" id="sns-dbody"></div><button type="button" class="sns-replybar" id="sns-replybar"></button></section>"#,
        svg(MARK),
        svg(PHOTO),
        svg(PHOTO),
        svg(BACK),
        menu = svg(MENU),
        search = svg(SEARCH),
        plus = svg(PLUS),
    ));
    // 글쓰기 판과 글 보기는 화면 위에 덮이도록 앱 틀로 옮겨요
    if let Some(app) = dom::app_el() {
        for id in ["sns-sheet", "sns-detail", "sns-fab"] {
            if let Some(el) = by_id(id) {
                let _ = app.append_child(&el);
                dom::on(&el, "click", on_click);
            }
        }
    }
    let Some(r) = by_id("sns") else { return };
    dom::on(&r, "click", on_click);
    if let Some(q) = by_id("sns-q") {
        let q2 = q.clone();
        dom::on(&q, "input", move |_| {
            let v = js_sys::Reflect::get(&q2, &"value".into()).ok().and_then(|v| v.as_string()).unwrap_or_default();
            QUERY.with(|x| *x.borrow_mut() = v.trim().to_lowercase());
            paint_feed();
        });
    }
    // 스크롤: 내리면 오른쪽 아래에 글쓰기 버튼이 떠올라요. 맨 위에서 더 당기면 새로고침해요
    dom::on(&sc, "scroll", move |_| {
        let y = by_id("screen-untitled3").map(|s| s.scroll_top()).unwrap_or(0);
        if let Some(f) = by_id("sns-fab") {
            dom::toggle(&f, "show", y > 160);
        }
    });
    let sc2 = sc.clone();
    dom::on(&sc, "touchstart", move |e| {
        if sc2.scroll_top() <= 0 && !REFRESHING.get() {
            PULL.set(touch_y(&e));
        }
    });
    let sc3 = sc.clone();
    dom::on_active(&sc, "touchmove", move |e| {
        let (Some(y0), Some(y)) = (PULL.get(), touch_y(&e)) else { return };
        let d = y - y0;
        if d <= 0.0 || sc3.scroll_top() > 0 {
            set_pull(0.0, false);
            return;
        }
        e.prevent_default();
        set_pull(110.0 * (1.0 - 1.0 / (d * 0.5 / 110.0 + 1.0)), false);
    });
    dom::on(&sc, "touchend", |_| end_pull());
    dom::on(&sc, "touchcancel", |_| end_pull());
    if let Some(t) = by_id("sns-text") {
        let t2 = t.clone();
        dom::on(&t, "input", move |_| {
            grow(&t2);
            paint_post_btn();
        });
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

fn touch_y(e: &web_sys::Event) -> Option<f64> {
    let t = e.dyn_ref::<web_sys::TouchEvent>()?.touches().get(0)?;
    Some(t.client_y() as f64)
}
/// 당긴 만큼 글이 내려오고, 위쪽 표시가 흔들리며 다시 그려져요
fn set_pull(px: f64, back: bool) {
    if let Some(b) = by_id("sns-body") {
        let st = b.style();
        let _ = st.set_property("transition", if back { "transform 0.45s cubic-bezier(0.25, 1, 0.5, 1)" } else { "none" });
        let _ = st.set_property("transform", &if px > 0.0 { format!("translateY({px}px)") } else { String::new() });
    }
    if let Some(m) = by_id("sns-mark") {
        let _ = m.style().set_property("--pull", &format!("{}", (px / 64.0).min(1.0)));
    }
}
fn end_pull() {
    let Some(_) = PULL.take() else { return };
    let shown = by_id("sns-body").and_then(|b| b.style().get_property_value("transform").ok()).unwrap_or_default();
    let px: f64 = shown.trim_start_matches("translateY(").trim_end_matches("px)").parse().unwrap_or(0.0);
    if px < 56.0 {
        return set_pull(0.0, true);
    }
    refresh();
}
/// 새로고침: 표시가 돌며 다시 그려지는 동안 글을 새로 읽어요
fn refresh() {
    if REFRESHING.replace(true) {
        return;
    }
    set_pull(56.0, true);
    if let Some(m) = by_id("sns-mark") {
        let _ = m.class_list().add_1("spin");
    }
    dom::vibrate(8);
    spawn_local(async {
        load_follows().await;
        let missing: js_sys::Array = FEED.with(|f| f.borrow().iter().map(|p| JsValue::from_str(&p.by)).collect());
        let _ = cloud::call("fetchProfiles", &[missing.into()]).await;
        dom::sleep(700).await;
        paint_feed();
        if let Some(m) = by_id("sns-mark") {
            let _ = m.class_list().remove_1("spin");
        }
        set_pull(0.0, true);
        REFRESHING.set(false);
    });
}
async fn load_follows() {
    if let Ok(v) = cloud::call("myFollows", &[]).await {
        let list: Vec<String> = js_sys::Array::from(&v).iter().filter_map(|x| x.as_string()).collect();
        FOLLOWS.with(|f| *f.borrow_mut() = list);
    }
}
/// 쓰는 만큼 글쓰기 칸이 늘어나요
fn grow(t: &web_sys::HtmlElement) {
    let st = t.style();
    let _ = st.set_property("height", "auto");
    let _ = st.set_property("height", &format!("{}px", t.scroll_height().max(42)));
}
fn my_name() -> String {
    let n = person(&my_uid()).0;
    if n == "…" {
        cloud::cloud().map(|c| cloud::get(&cloud::get(&c, "user"), "name").as_string().unwrap_or_default()).unwrap_or_default()
    } else {
        n
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
    dom::set_text("sns-myname0", &my_name());
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
    spawn_local(async {
        load_follows().await;
        paint_feed();
    });
}

fn paint_feed() {
    let Some(feed) = by_id("sns-feed") else { return };
    let (mode, q, me) = (MODE.get(), QUERY.with(|x| x.borrow().clone()), my_uid());
    let html = FEED.with(|f| {
        f.borrow()
            .iter()
            .filter(|p| match mode {
                1 => FOLLOWS.with(|x| x.borrow().contains(&p.by)),
                2 => p.by == me,
                _ => true,
            })
            .filter(|p| q.is_empty() || p.text.to_lowercase().contains(&q) || person(&p.by).0.to_lowercase().contains(&q) || p.topic.to_lowercase().contains(&q))
            .map(|p| {
                // 답글이 있으면 마지막 답글을 아래에 들여 써서 미리 보여줘요 (선이 둥글게 이어져요)
                let reply = (p.rc > 0.0 && !p.last.is_empty()).then(|| LAST_REPLY.with(|m| m.borrow().get(&p.last).cloned())).flatten().flatten().filter(|r| r.id == p.last);
                match reply {
                    Some(r) => format!(r#"<div class="sp-grp">{}{}</div>"#, post_html_x(p, false, true, false), post_html_x(&r, false, false, true)),
                    None => format!(r#"<div class="sp-grp">{}</div>"#, post_html(p, false, false)),
                }
            })
            .collect::<String>()
    });
    load_last_replies();
    let empty = if !q.is_empty() {
        "찾는 글이 없어요"
    } else {
        match mode {
            1 => "팔로우한 사람의 글이 없어요. 동그라미의 +를 눌러 팔로우해 보세요",
            2 => "아직 쓴 글이 없어요",
            _ => "아직 글이 없어요. 첫 글을 남겨 보세요",
        }
    };
    feed.set_inner_html(&if html.is_empty() { format!(r#"<p class="sns-empty">{empty}</p>"#) } else { html });
}

/// 피드 글의 마지막 답글을 아직 모르면 한 번씩 읽어요 (같은 답글은 다시 읽지 않아요)
fn load_last_replies() {
    let need: Vec<String> = FEED.with(|f| {
        f.borrow()
            .iter()
            .filter(|p| p.rc > 0.0 && !p.last.is_empty())
            .map(|p| p.last.clone())
            .filter(|rid| !LAST_REPLY.with(|m| m.borrow().contains_key(rid)))
            .collect()
    });
    if need.is_empty() {
        return;
    }
    LAST_REPLY.with(|m| {
        let mut m = m.borrow_mut();
        for rid in &need {
            m.insert(rid.clone(), None);
        }
    });
    spawn_local(async move {
        for rid in need {
            let r = cloud::call("postGet", &[JsValue::from_str(&rid)]).await.ok().filter(|v| !v.is_null()).map(|v| Post::from(&v));
            if let Some(r) = &r {
                need_people(std::slice::from_ref(r));
            }
            LAST_REPLY.with(|m| m.borrow_mut().insert(rid, r));
        }
        paint_feed();
    });
}

fn find(id: &str) -> Option<Post> {
    let in_feed = FEED.with(|f| f.borrow().iter().find(|p| p.id == id).cloned())
        .or_else(|| LAST_REPLY.with(|m| m.borrow().values().flatten().find(|r| r.id == id).cloned()));
    in_feed.or_else(|| OPEN.with(|o| o.borrow().as_ref().and_then(|(p, rs)| if p.id == id { Some(p.clone()) } else { rs.iter().find(|r| r.id == id).cloned() })))
}
fn patch(id: &str, f: impl Fn(&mut Post)) {
    FEED.with(|x| x.borrow_mut().iter_mut().filter(|p| p.id == id).for_each(&f));
    LAST_REPLY.with(|m| m.borrow_mut().values_mut().flatten().filter(|p| p.id == id).for_each(&f));
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
    if dom::closest(&e, "[data-menu]").is_some() {
        if let Some(m) = by_id("sns-menu") {
            m.set_hidden(!m.hidden());
            for b in dom::qsa(&m, "[data-mode]") {
                dom::toggle(&b, "on", dom::data(&b, "mode") == MODE.get().to_string());
            }
        }
        return;
    }
    if let Some(b) = dom::closest(&e, "[data-mode]") {
        MODE.set(dom::data(&b, "mode").parse().unwrap_or(0));
        dom::set_hidden("sns-menu", true);
        return paint_feed();
    }
    if dom::closest(&e, "[data-refresh]").is_some() {
        dom::set_hidden("sns-menu", true);
        return refresh();
    }
    if dom::closest(&e, "[data-search]").is_some() {
        dom::set_hidden("sns-search", false);
        if let Some(q) = by_id("sns-q") {
            let _ = q.focus();
        }
        return;
    }
    if dom::closest(&e, "[data-search-close]").is_some() {
        dom::set_hidden("sns-search", true);
        QUERY.with(|x| x.borrow_mut().clear());
        if let Some(q) = by_id("sns-q") {
            let _ = js_sys::Reflect::set(&q, &"value".into(), &"".into());
        }
        return paint_feed();
    }
    if let Some(b) = dom::closest(&e, "[data-follow]") {
        let who = dom::data(&b, "follow");
        FOLLOWS.with(|f| f.borrow_mut().push(who.clone()));
        let _ = b.class_list().add_1("done");
        dom::vibrate(8);
        let name = person(&who).0;
        spawn_local(async move {
            if cloud::call("follow", &[JsValue::from_str(&who), true.into()]).await.is_ok() {
                crate::shell::toast(&format!("{name}님을 팔로우했어요"));
                dom::sleep(450).await;
                paint_feed();
                paint_open();
            } else {
                FOLLOWS.with(|f| f.borrow_mut().retain(|x| *x != who));
                crate::shell::toast("팔로우하지 못했어요");
                paint_feed();
            }
        });
        return;
    }
    if dom::closest(&e, "#sns-fab").is_some() {
        return open_sheet("");
    }
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
        let id = dom::data(&b, "share");
        let text = find(&id).map(|p| p.text).unwrap_or_default();
        if let Some(w) = web_sys::window() {
            let _ = w.navigator().clipboard().write_text(&text);
        }
        patch(&id, |p| p.sh += 1.0);
        paint_open();
        spawn_local(async move {
            let _ = cloud::call("postShare", &[JsValue::from_str(&id)]).await;
        });
        return crate::shell::toast("글을 복사했어요");
    }
    if let Some(b) = dom::closest(&e, "[data-more]") {
        let id = dom::data(&b, "more");
        if dom::data(&b, "mine") != "true" {
            // 남의 글: 글 내용을 복사해요
            let text = find(&id).map(|p| p.text).unwrap_or_default();
            if let Some(w) = web_sys::window() {
                let _ = w.navigator().clipboard().write_text(&text);
            }
            return crate::shell::toast("글을 복사했어요");
        }
        spawn_local(async move {
            if !crate::shell::ask("글을 삭제할까요?", "삭제하면 되돌릴 수 없어요", "삭제", "취소").await {
                return;
            }
            match cloud::call("postDelete", &[JsValue::from_str(&id)]).await {
                Ok(parent) => {
                    crate::shell::toast("글을 삭제했어요");
                    // 답글이었으면 원래 글의 댓글 수를 줄여요
                    if let Some(par) = parent.as_string().filter(|p| !p.is_empty()) {
                        // 피드는 서버 숫자를 실시간으로 받아요. 글 화면(실시간이 아님)만 직접 줄여요
                        OPEN.with(|o| {
                            if let Some((p, _)) = o.borrow_mut().as_mut().filter(|(p, _)| p.id == par) {
                                p.rc = (p.rc - 1.0).max(0.0);
                            }
                        });
                        STACK.with(|s| s.borrow_mut().iter_mut().filter(|p| p.id == par).for_each(|p| p.rc = (p.rc - 1.0).max(0.0)));
                        paint_feed();
                    }
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
        // 피드의 글, 또는 글 화면 안의 답글(큰 글이 아닌 것)을 누르면 그 글 안으로 들어가요
        let in_parent_preview = dom::closest(&e, "#sns-parent").is_some();
        if !a.class_list().contains("big") && !in_parent_preview {
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
    if let Some(t) = by_id("sns-topic") {
        let _ = js_sys::Reflect::set(&t, &"value".into(), &"".into());
        t.set_hidden(!reply_to.is_empty()); // 답글에는 주제가 없어요
    }
    if let Some(par) = by_id("sns-parent") {
        par.set_inner_html(&find(reply_to).map(|p| post_html(&p, false, true)).unwrap_or_default());
    }
    if let Some(t) = by_id("sns-text").and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok()) {
        t.set_value("");
        grow(&t);
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
        let topic = by_id("sns-topic").and_then(|t| js_sys::Reflect::get(&t, &"value".into()).ok()).and_then(|v| v.as_string()).unwrap_or_default();
        let r = cloud::call("postCreate", &[JsValue::from_str(&text), JsValue::from_str(&img), JsValue::from_str(&parent), JsValue::from_str(&topic)]).await;
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
    // 이미 글을 보고 있으면(답글을 누른 것) 지금 글을 쌓아 두고 그 답글 안으로 들어가요
    let now = OPEN.with(|o| o.borrow().as_ref().map(|(p, _)| p.clone()));
    let inside = !d.hidden() && now.is_some();
    if let Some(cur) = now.filter(|_| inside) {
        if cur.id == id {
            return;
        }
        STACK.with(|s| s.borrow_mut().push(cur));
    } else {
        STACK.with(|s| s.borrow_mut().clear());
    }
    OPEN.with(|o| *o.borrow_mut() = Some((p.clone(), Vec::new())));
    paint_open();
    if let Some(body) = by_id("sns-dbody") {
        body.set_scroll_top(0);
        if inside {
            let _ = body.class_list().remove_1("swap");
            let _ = body.offset_width();
            let _ = body.class_list().add_1("swap");
        }
    }
    if inside {
        let id = id.to_string();
        spawn_local(async move { load_replies(&id).await });
        return;
    }
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
    // 답글 안에 들어와 있었으면 한 단계만 돌아가요
    if let Some(prev) = STACK.with(|s| s.borrow_mut().pop()) {
        let id = prev.id.clone();
        OPEN.with(|o| *o.borrow_mut() = Some((prev, Vec::new())));
        paint_open();
        spawn_local(async move { load_replies(&id).await });
        return;
    }
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
