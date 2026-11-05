//! SNS 탭: 위쪽(큰 제목·알림 종·내 동그라미), 사람들 동그라미 줄(누르면 사람 카드), 유리 게시물 카드 피드,
//! 탭 바 바로 위 둥근 유리 + 버튼(새 게시물), 댓글 달기, 좋아요, 댓글 모두 보기, 공유.
//! 서버를 아끼려고 피드는 최근 40개만 듣고, 사람 정보는 30분 캐시를 써요. 사용자 글·사진은 모두 esc()/safe_img()를 거쳐요.

use std::cell::{Cell, RefCell};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::dom::{self, by_id, esc, safe_img};

#[derive(Clone, Default)]
struct Post {
    id: String,
    by: String,
    text: String,
    img: String,
    at: f64,
    likes: f64,
    liked: bool,
    rc: f64,
    place: String,
    last: String,
}
impl Post {
    fn from_js(v: &JsValue) -> Post {
        let s = |k: &str| cloud::get(v, k).as_string().unwrap_or_default();
        let n = |k: &str| cloud::get(v, k).as_f64().unwrap_or(0.0);
        Post { id: s("id"), by: s("by"), text: s("text"), img: s("img"), at: n("at"), likes: n("likes"), liked: cloud::get(v, "liked") == JsValue::TRUE, rc: n("rc"), place: s("topic"), last: s("last") }
    }
}

thread_local! {
    static PEOPLE: RefCell<Vec<(String, String, String)>> = const { RefCell::new(Vec::new()) }; // 번호, 이름, 사진
    static POSTS: RefCell<Vec<Post>> = const { RefCell::new(Vec::new()) };
    static LAST: RefCell<std::collections::HashMap<String, (String, String)>> = RefCell::new(std::collections::HashMap::new()); // 글 → (쓴 사람, 마지막 댓글)
    static UNSUB: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
    static DRAFT_IMG: RefCell<String> = const { RefCell::new(String::new()) };
    static SHARE: RefCell<(String, Vec<String>)> = const { RefCell::new((String::new(), Vec::new())) }; // 보낼 글, 고른 곳
    static OPEN: RefCell<String> = const { RefCell::new(String::new()) }; // 댓글 모두 보기 중인 글
    static LOADED: Cell<bool> = const { Cell::new(false) };
    static BOUND: Cell<bool> = const { Cell::new(false) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
}

const HEART: &str = r#"<path d="M12 20.5s-7.5-4.6-7.5-10.4A4.3 4.3 0 0 1 12 7.3a4.3 4.3 0 0 1 7.5 2.8c0 5.8-7.5 10.4-7.5 10.4z"/>"#;
const BUBBLE: &str = r#"<path d="M20 11.5c0 4-3.8 7-8.5 7-1 0-2-.1-2.9-.4L4.5 19.5l1.3-3.3C4.7 15 4 13.4 4 11.5c0-4 3.8-7 8-7s8 3 8 7z"/>"#;
const SHARE_I: &str = r#"<path d="M14 5l6 6-6 6"/><path d="M20 11h-7a8 8 0 0 0-8 8"/>"#;
const PIN: &str = r#"<path d="M12 21s-6.5-5.6-6.5-11a6.5 6.5 0 0 1 13 0C18.5 15.4 12 21 12 21z"/><circle cx="12" cy="10" r="2.2"/>"#;
const MORE: &str = r#"<circle cx="5" cy="12" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="19" cy="12" r="1.2"/>"#;
const X: &str = r#"<path d="M7 7l10 10M17 7L7 17"/>"#;

fn svg(d: &str) -> String {
    format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{d}</svg>"#)
}
fn my_uid() -> String {
    cloud::cloud().map(|c| cloud::get(&c, "uid").as_string().unwrap_or_default()).unwrap_or_default()
}
/// 사람 이름과 사진 (캐시)
fn person(uid: &str) -> (String, String) {
    let p = cloud::call_now("peek", &[JsValue::from_str(uid)]).unwrap_or(JsValue::NULL);
    let g = |k: &str| cloud::get(&p, k).as_string().unwrap_or_default();
    let n = g("name");
    (if n.is_empty() { g("id") } else { n }, g("photo"))
}
fn me() -> (String, String) {
    let (n, p) = person(&my_uid());
    let n = if n.is_empty() { cloud::cloud().and_then(|c| cloud::get(&cloud::get(&c, "user"), "name").as_string()).unwrap_or_default() } else { n };
    (n, p)
}
fn ago(at: f64) -> String {
    let s = ((js_sys::Date::now() - at) / 1000.0).max(0.0);
    if s < 60.0 {
        "방금".into()
    } else if s < 3600.0 {
        format!("{}분 전", (s / 60.0) as i64)
    } else if s < 86400.0 {
        format!("{}시간 전", (s / 3600.0) as i64)
    } else {
        format!("{}일 전", (s / 86400.0) as i64)
    }
}

/* ---------- 그리기 ---------- */

fn paint_top() {
    if let Some(m) = by_id("sns-me") {
        let (n, ph) = me();
        m.set_inner_html(&if cloud::signed_in() { dom::avatar_html(&n, &ph, "avatar contact sns-me-av") } else { String::new() });
    }
    let Some(el) = by_id("sns-people") else { return };
    let html: String = PEOPLE.with(|p| {
        p.borrow()
            .iter()
            .map(|(uid, name, photo)| format!(r#"<button type="button" class="sns-p" data-uid="{}" role="listitem">{}<span class="sns-n">{}</span></button>"#, esc(uid), dom::avatar_html(name, photo, "avatar contact sns-av"), esc(name)))
            .collect()
    });
    el.set_inner_html(&if html.is_empty() && !cloud::signed_in() { r#"<p class="sns-empty">로그인하면 사람들이 보여요</p>"#.into() } else { html });
}

fn card(p: &Post) -> String {
    let (name, photo) = person(&p.by);
    let img = safe_img(&p.img, 300000);
    let place = if p.place.is_empty() { String::new() } else { format!(r#"<span class="sns-chip">{}{}</span>"#, svg(PIN), esc(&p.place)) };
    let media = if img.is_empty() {
        if place.is_empty() { String::new() } else { format!(r#"<div class="sns-noimg">{place}</div>"#) }
    } else {
        format!(r#"<div class="sns-img" style="background-image:url({img})" role="img" aria-label="게시물 사진">{place}</div>"#)
    };
    let last = LAST.with(|l| l.borrow().get(&p.id).cloned());
    let reply = match last {
        Some((by, t)) if p.rc > 0.0 => format!(r#"<p class="sns-rp"><b>{}:</b> {}</p>"#, esc(&person(&by).0), esc(&t)),
        _ => String::new(),
    };
    let text = if p.text.is_empty() { String::new() } else { format!(r#"<p class="sns-tx">{}</p>"#, esc(&p.text)) };
    let cm = if text.is_empty() && reply.is_empty() {
        String::new()
    } else {
        format!(r#"<div class="sns-cm{}">{text}{reply}</div>"#, if img.is_empty() && place.is_empty() { " flat" } else { "" })
    };
    format!(
        r#"<article class="sns-card" data-id="{id}"><header class="sns-ch"><button type="button" class="sns-who" data-uid="{by}">{av}<span><b>{name}</b><small>{ago}</small></span></button>{more}</header>{media}{cm}<footer class="sns-act"><button type="button" class="sns-like{on}" data-act="like" aria-label="{ll}" aria-pressed="{pr}">{heart}<span>{likes}</span></button><button type="button" data-act="replies" aria-label="댓글 {rc}개">{bubble}<span>{rc}</span></button><button type="button" data-act="share" aria-label="공유">{share}</button></footer></article>"#,
        id = esc(&p.id),
        by = esc(&p.by),
        av = dom::avatar_html(&name, &photo, "avatar contact sns-cav"),
        name = esc(&name),
        ago = ago(p.at),
        more = if p.by == my_uid() { format!(r#"<button type="button" class="sns-more" data-act="more" aria-label="더 보기">{}</button>"#, svg(MORE)) } else { String::new() },
        cm = cm,
        on = if p.liked { " on" } else { "" },
        ll = if p.liked { "좋아요 취소" } else { "좋아요" },
        pr = p.liked,
        heart = format!(r#"<svg viewBox="0 0 24 24" fill="{}" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" aria-hidden="true">{HEART}</svg>"#, if p.liked { "currentColor" } else { "none" }),
        likes = if p.likes > 0.0 { (p.likes as i64).to_string() } else { String::new() },
        rc = p.rc as i64,
        bubble = svg(BUBBLE),
        share = svg(SHARE_I),
    )
}

fn paint_feed() {
    let Some(el) = by_id("sns-feed") else { return };
    if !cloud::signed_in() {
        el.set_inner_html("");
        return;
    }
    let html: String = POSTS.with(|p| p.borrow().iter().map(card).collect());
    el.set_inner_html(&if html.is_empty() { r#"<p class="sns-none">아직 게시물이 없어요. + 를 눌러 첫 게시물을 올려 보세요</p>"#.into() } else { html });
}

/// 댓글이 있는 글의 마지막 댓글을 읽어 와요 (아직 모르는 것만)
fn load_last() {
    let need: Vec<(String, String)> = POSTS.with(|p| p.borrow().iter().filter(|p| p.rc > 0.0 && !p.last.is_empty()).map(|p| (p.id.clone(), p.last.clone())).collect());
    let need: Vec<(String, String)> = need.into_iter().filter(|(id, last)| LAST.with(|l| l.borrow().get(id).map(|_| false).unwrap_or(true)) || last.is_empty()).collect();
    for (id, last) in need.into_iter().take(12) {
        spawn_local(async move {
            if let Ok(v) = cloud::call("postGet", &[JsValue::from_str(&last)]).await {
                if !v.is_null() {
                    let by = cloud::get(&v, "by").as_string().unwrap_or_default();
                    let t = cloud::get(&v, "text").as_string().unwrap_or_default();
                    let _ = cloud::call("fetchProfiles", &[js_sys::Array::of1(&JsValue::from_str(&by)).into()]).await;
                    LAST.with(|l| l.borrow_mut().insert(id, (by, t)));
                    paint_feed();
                }
            }
        });
    }
}

fn watch() {
    if UNSUB.with(|u| u.borrow().is_some()) {
        return;
    }
    let cb = Closure::<dyn FnMut(JsValue)>::new(|list: JsValue| {
        let posts: Vec<Post> = js_sys::Array::from(&list).iter().map(|v| Post::from_js(&v)).collect();
        let who: js_sys::Array = posts.iter().map(|p| JsValue::from_str(&p.by)).collect();
        POSTS.with(|p| *p.borrow_mut() = posts);
        paint_feed();
        load_last();
        spawn_local(async move {
            if cloud::call("fetchProfiles", &[who.into()]).await.map(|n| n.as_f64().unwrap_or(0.0) > 0.0).unwrap_or(false) {
                paint_feed();
            }
        });
    })
    .into_js_value();
    if let Ok(f) = cloud::call_now("postsWatch", &[cb]) {
        if let Ok(f) = f.dyn_into::<js_sys::Function>() {
            UNSUB.with(|u| *u.borrow_mut() = Some(f));
        }
    }
}

/* ---------- 누르기 ---------- */

fn patch(id: &str, f: impl Fn(&mut Post)) {
    POSTS.with(|p| p.borrow_mut().iter_mut().filter(|p| p.id == id).for_each(f));
}

fn on_feed_click(e: web_sys::Event) {
    if let Some(b) = dom::closest(&e, ".sns-who") {
        let uid = dom::data(&b, "uid");
        crate::profile::show(&uid, &person(&uid).0, None);
        return;
    }
    let Some(b) = dom::closest(&e, "[data-act]") else { return };
    let Some(cardel) = dom::closest(&e, ".sns-card") else { return };
    let id = dom::data(&cardel, "id");
    match dom::data(&b, "act").as_str() {
        "like" => {
            let on = !POSTS.with(|p| p.borrow().iter().find(|p| p.id == id).map(|p| p.liked).unwrap_or(false));
            patch(&id, |p| {
                p.liked = on;
                p.likes = (p.likes + if on { 1.0 } else { -1.0 }).max(0.0);
            });
            dom::vibrate(8);
            paint_feed();
            spawn_local(async move {
                if cloud::call("postLike", &[JsValue::from_str(&id), on.into()]).await.is_err() {
                    crate::shell::toast("좋아요를 못 했어요");
                }
            });
        }
        "replies" => open_replies(&id),
        "share" => open_share(&id),
        "more" => {
            spawn_local(async move {
                if crate::chat::ctx().ask("게시물을 지울까요?", "지운 게시물은 되돌릴 수 없어요", "지우기", "취소").await {
                    match cloud::call("postDelete", &[JsValue::from_str(&id)]).await {
                        Ok(_) => crate::shell::toast("게시물을 지웠어요"),
                        Err(_) => crate::shell::toast("지우지 못했어요"),
                    }
                }
            });
        }
        _ => {}
    }
}

/// 댓글 달기 (카드 안 칸이나 댓글 모두 보기 판)
fn send_reply(id: String, input: web_sys::HtmlInputElement) {
    let t = input.value().trim().to_string();
    if t.is_empty() || BUSY.replace(true) {
        return;
    }
    input.set_value("");
    let _ = input.blur();
    spawn_local(async move {
        let r = cloud::call("postCreate", &[JsValue::from_str(&t), JsValue::from_str(""), JsValue::from_str(&id), JsValue::from_str("")]).await;
        BUSY.set(false);
        match r {
            Ok(_) => {
                LAST.with(|l| l.borrow_mut().insert(id.clone(), (my_uid(), t)));
                if OPEN.with(|o| *o.borrow() == id) {
                    load_replies(id).await;
                }
            }
            Err(_) => crate::shell::toast("댓글을 달지 못했어요"),
        }
    });
}

/* ---------- 댓글 모두 보기 ---------- */

fn open_replies(id: &str) {
    OPEN.with(|o| *o.borrow_mut() = id.to_string());
    if let Some(l) = by_id("sns-rl") {
        l.set_inner_html(r#"<p class="sns-none">불러오는 중이에요</p>"#);
    }
    if let Some(f) = by_id("sns-rf") {
        let _ = f.set_attribute("data-id", id);
    }
    sheet(true, "sns-replies");
    spawn_local(load_replies(id.to_string()));
}
async fn load_replies(id: String) {
    let list: Vec<Post> = cloud::call("postReplies", &[JsValue::from_str(&id)]).await.map(|v| js_sys::Array::from(&v).iter().map(|v| Post::from_js(&v)).collect()).unwrap_or_default();
    let who: js_sys::Array = list.iter().map(|p| JsValue::from_str(&p.by)).collect();
    let _ = cloud::call("fetchProfiles", &[who.into()]).await;
    if !OPEN.with(|o| *o.borrow() == id) {
        return;
    }
    let html: String = list
        .iter()
        .map(|r| {
            let (n, ph) = person(&r.by);
            format!(r#"<li class="sns-r"><button type="button" class="sns-who" data-uid="{}">{}</button><div><b>{}</b> <small>{}</small><p>{}</p></div></li>"#, esc(&r.by), dom::avatar_html(&n, &ph, "avatar contact sns-cav"), esc(&n), ago(r.at), esc(&r.text))
        })
        .collect();
    if let Some(l) = by_id("sns-rl") {
        l.set_inner_html(&if html.is_empty() { r#"<p class="sns-none">첫 댓글을 달아 보세요</p>"#.into() } else { html });
    }
}

/* ---------- 보내기: 시뮬레이터 안 메시지로 (대화방·단톡방·사람) ---------- */

/// (키, 이름, 사진, 단톡방인지): 키는 "c:대화번호"나 "u:사람번호"
fn share_targets() -> Vec<(String, String, String, bool)> {
    let me = my_uid();
    let chats: Vec<(String, String, String, bool, String)> = crate::chat::with_chats(|cs| cs.iter().map(|c| (c.id.clone(), c.name.clone(), c.photo.clone(), c.group, c.other.clone())).collect());
    let have: Vec<String> = chats.iter().filter(|c| !c.3).map(|c| c.4.clone()).collect();
    let mut out: Vec<(String, String, String, bool)> = chats.into_iter().map(|c| (format!("c:{}", c.0), c.1, c.2, c.3)).collect();
    PEOPLE.with(|p| {
        for (uid, name, photo) in p.borrow().iter() {
            if *uid != me && !have.contains(uid) {
                out.push((format!("u:{uid}"), name.clone(), photo.clone(), false));
            }
        }
    });
    out
}
fn paint_share() {
    let sel = SHARE.with(|s| s.borrow().1.clone());
    let html: String = share_targets()
        .iter()
        .map(|(k, n, ph, g)| {
            format!(
                r#"<li><button type="button" class="sns-sr{}" data-key="{}" aria-pressed="{}">{}<span class="sns-srn">{}<small>{}</small></span><i class="sns-ck" aria-hidden="true">{}</i></button></li>"#,
                if sel.contains(k) { " on" } else { "" },
                esc(k),
                sel.contains(k),
                dom::avatar_html(n, ph, "avatar contact sns-cav"),
                esc(n),
                if *g { "단톡방" } else { "" },
                svg(r#"<path d="M5 12.5l4.5 4.5L19 7.5"/>"#)
            )
        })
        .collect();
    if let Some(l) = by_id("sns-sl") {
        l.set_inner_html(&if html.is_empty() { r#"<p class="sns-none">보낼 사람이 아직 없어요</p>"#.into() } else { html });
    }
    if let Some(b) = by_id("sns-send") {
        let _ = if sel.is_empty() { b.set_attribute("disabled", "") } else { b.remove_attribute("disabled") };
        b.set_text_content(Some(&if sel.is_empty() { "보내기".to_string() } else { format!("보내기 {}", sel.len()) }));
    }
}
fn open_share(id: &str) {
    SHARE.with(|s| *s.borrow_mut() = (id.to_string(), Vec::new()));
    paint_share();
    sheet(true, "sns-share");
}
fn share_row_click(e: web_sys::Event) {
    e.stop_propagation(); // 다시 그리면 눌린 칸이 사라져서 "바깥을 눌렀다"고 읽히지 않게
    let Some(b) = dom::closest(&e, "[data-key]") else { return };
    let k = dom::data(&b, "key");
    SHARE.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(i) = s.1.iter().position(|x| *x == k) {
            s.1.remove(i);
        } else if s.1.len() < 10 {
            s.1.push(k);
        }
    });
    dom::vibrate(6);
    paint_share();
}
fn send_share() {
    let (id, sel) = SHARE.with(|s| s.borrow().clone());
    let Some(p) = POSTS.with(|p| p.borrow().iter().find(|p| p.id == id).cloned()) else { return };
    if sel.is_empty() || BUSY.replace(true) {
        return;
    }
    if let Some(b) = by_id("sns-send") {
        let _ = b.set_attribute("disabled", "");
    }
    let names: std::collections::HashMap<String, (String, String)> = PEOPLE.with(|x| x.borrow().iter().map(|(u, n, ph)| (u.clone(), (n.clone(), ph.clone()))).collect());
    spawn_local(async move {
        let who = person(&p.by).0;
        let body = if p.text.is_empty() { "사진".to_string() } else { dom::cut(&p.text, 300) };
        let text = format!("{who}님의 게시물\n{body}");
        let mut done = 0;
        for k in sel {
            let chat = if let Some(cid) = k.strip_prefix("c:") {
                crate::chat::with_chats(|cs| cs.iter().find(|c| c.id == cid).cloned())
            } else if let Some(uid) = k.strip_prefix("u:") {
                let (n, ph) = names.get(uid).cloned().unwrap_or_default();
                match crate::chat::people::chat_with(uid, "", &n, &ph).await {
                    Ok(cid) => crate::chat::with_chats(|cs| cs.iter().find(|c| c.id == cid).cloned()),
                    Err(_) => None,
                }
            } else {
                None
            };
            let Some(chat) = chat else { continue };
            let payload = dom::obj(&[("type", "text".into()), ("text", JsValue::from_str(&text))]);
            if crate::chat::send_payload(&chat, payload).await.is_ok() {
                done += 1;
            }
        }
        BUSY.set(false);
        sheet(false, "sns-share");
        crate::shell::toast(if done > 0 { "메시지로 보냈어요" } else { "보내지 못했어요" });
        let _ = cloud::call("postShare", &[JsValue::from_str(&id)]).await;
    });
}

/* ---------- 새 게시물 ---------- */

fn sheet(on: bool, id: &str) {
    let Some(s) = by_id(id) else { return };
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
fn area() -> Option<web_sys::HtmlTextAreaElement> {
    by_id("sns-text")?.dyn_into().ok()
}
fn paint_draft() {
    let img = DRAFT_IMG.with(|d| d.borrow().clone());
    if let Some(p) = by_id("sns-prev") {
        let s = safe_img(&img, 300000);
        p.set_inner_html(&if s.is_empty() { String::new() } else { format!(r#"<div class="sns-pv" style="background-image:url({s})"><button type="button" class="sns-pvx" id="sns-pvx" aria-label="사진 빼기">{}</button></div>"#, svg(X)) });
    }
    let ok = area().map(|a| !a.value().trim().is_empty()).unwrap_or(false) || !img.is_empty();
    if let Some(b) = by_id("sns-post") {
        let _ = if ok { b.remove_attribute("disabled") } else { b.set_attribute("disabled", "") };
    }
}
fn open_compose() {
    if !cloud::signed_in() {
        crate::shell::toast("로그인하고 올릴 수 있어요");
        return;
    }
    if let Some(m) = by_id("sns-cme") {
        let (n, ph) = me();
        m.set_inner_html(&format!(r#"{}<b>{}</b>"#, dom::avatar_html(&n, &ph, "avatar contact sns-cav"), esc(&n)));
    }
    paint_draft();
    sheet(true, "sns-compose");
    if let Some(a) = area() {
        let _ = a.focus();
    }
}
fn post() {
    let text = area().map(|a| a.value().trim().to_string()).unwrap_or_default();
    let img = DRAFT_IMG.with(|d| d.borrow().clone());
    let place = input("sns-place").map(|i| i.value().trim().to_string()).unwrap_or_default();
    if (text.is_empty() && img.is_empty()) || BUSY.replace(true) {
        return;
    }
    if let Some(b) = by_id("sns-post") {
        let _ = b.set_attribute("disabled", "");
    }
    spawn_local(async move {
        let r = cloud::call("postCreate", &[JsValue::from_str(&text), JsValue::from_str(&img), JsValue::from_str(""), JsValue::from_str(&place)]).await;
        BUSY.set(false);
        match r {
            Ok(_) => {
                if let Some(a) = area() {
                    a.set_value("");
                }
                if let Some(i) = input("sns-place") {
                    i.set_value("");
                }
                DRAFT_IMG.with(|d| d.borrow_mut().clear());
                sheet(false, "sns-compose");
                crate::shell::toast("게시물을 올렸어요");
                if let Some(sc) = crate::shell::screen("untitled3") {
                    sc.set_scroll_top(0);
                }
            }
            Err(_) => {
                crate::shell::toast("올리지 못했어요. 잠시 뒤 다시 해 주세요");
                paint_draft();
            }
        }
    });
}

fn bind() {
    if let Some(el) = by_id("sns-people") {
        dom::on(&el, "click", |e| {
            let Some(b) = dom::closest(&e, "[data-uid]") else { return };
            let uid = dom::data(&b, "uid");
            let name = PEOPLE.with(|p| p.borrow().iter().find(|x| x.0 == uid).map(|x| x.1.clone()).unwrap_or_default());
            crate::profile::show(&uid, &name, None);
        });
    }
    if let Some(b) = by_id("sns-bell") {
        dom::on(&b, "click", |_| crate::shell::toast("새 알림이 없어요"));
    }
    if let Some(f) = by_id("sns-feed") {
        dom::on(&f, "click", on_feed_click);
    }
    if let Some(b) = by_id("sns-fab") {
        dom::on(&b, "click", |_| open_compose());
    }
    for id in ["sns-compose", "sns-replies", "sns-share"] {
        if let Some(s) = by_id(id) {
            let id2 = id.to_string();
            dom::on(&s, "click", move |e| {
                if dom::closest(&e, "[data-close]").is_some() || dom::closest(&e, ".sns-panel").is_none() {
                    sheet(false, &id2);
                    if id2 == "sns-replies" {
                        OPEN.with(|o| o.borrow_mut().clear());
                    }
                    return;
                }
                if let Some(b) = dom::closest(&e, ".sns-who") {
                    let uid = dom::data(&b, "uid");
                    crate::profile::show(&uid, &person(&uid).0, None);
                }
            });
        }
    }
    if let Some(l) = by_id("sns-sl") {
        dom::on(&l, "click", share_row_click);
    }
    if let Some(b) = by_id("sns-send") {
        dom::on(&b, "click", |_| send_share());
    }
    if let Some(f) = by_id("sns-rf") {
        dom::on(&f, "submit", |e| {
            e.prevent_default();
            let id = OPEN.with(|o| o.borrow().clone());
            if let Some(i) = input("sns-ri") {
                send_reply(id, i);
            }
        });
    }
    if let Some(t) = by_id("sns-text") {
        dom::on(&t, "input", |_| paint_draft());
    }
    if let Some(b) = by_id("sns-post") {
        dom::on(&b, "click", |_| post());
    }
    if let Some(p) = by_id("sns-prev") {
        dom::on(&p, "click", |e| {
            if dom::closest(&e, "#sns-pvx").is_some() {
                DRAFT_IMG.with(|d| d.borrow_mut().clear());
                paint_draft();
            }
        });
    }
    if let Some(f) = by_id("sns-file") {
        dom::on(&f, "change", |_| {
            let Some(file) = crate::chat::attach::take_file("sns-file") else { return };
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

/// SNS 탭을 열 때
pub fn open() {
    if !BOUND.replace(true) {
        bind();
    }
    paint_top();
    paint_feed();
    if !cloud::signed_in() {
        return;
    }
    watch();
    if LOADED.replace(true) {
        return;
    }
    spawn_local(async {
        let list = cloud::call("listUsers", &[true.into()]).await.ok().map(|v| js_sys::Array::from(&cloud::get(&v, "list")).to_vec()).unwrap_or_default();
        let ppl: Vec<(String, String, String)> = list
            .iter()
            .map(|v| {
                let s = |k: &str| cloud::get(v, k).as_string().unwrap_or_default();
                let n = s("name");
                (s("uid"), if n.is_empty() { s("id") } else { n }, s("photo"))
            })
            .collect();
        PEOPLE.with(|p| *p.borrow_mut() = ppl);
        paint_top();
    });
}

