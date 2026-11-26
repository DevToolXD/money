//! 릴스 댓글 판 (인스타 릴스처럼): 댓글을 누르면 게시물이 위로 작게 줄고, 아래에서 어두운 판이 올라와요.
//! 판에는 글쓴이의 글과 댓글이 있고, 맨 아래 입력줄에서 댓글·GIF(GIPHY 검색)·멸공 선물을 보내요.
//! 판은 손잡이나 위쪽을 끌어 내리거나, 위에 작아진 게시물을 누르면 닫혀요.
//! 서버를 아끼려고 댓글은 판을 열 때 한 번만 읽고(새로 단 뒤에만 다시), GIF 검색은 0.4초 멈춘 뒤에만 보내요.

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};

use crate::dom::{self, by_id, esc};


thread_local! {
    /// 지금 열린 게시물 (번호, 글쓴이)
    static POST: RefCell<(String, String)> = const { RefCell::new((String::new(), String::new())) };
    /// 판 모드: "" 댓글, "gif", "gift"
    static MODE: RefCell<&'static str> = const { RefCell::new("") };
    static BOUND: Cell<bool> = const { Cell::new(false) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
    /// GIF 검색 차례 (늦게 온 옛 결과는 버려요)
    static SEQ: Cell<u32> = const { Cell::new(0) };
    /// 끌어 내리기: 시작 y, 지금 끈 거리
    static DRAG: Cell<(f64, f64, bool)> = const { Cell::new((0.0, 0.0, false)) };
    /// GIPHY 키 (브라우저에서 쓰는 공개 키예요). 관리 탭에서 넣으면 서버(meta/giphy)에 저장되고, 세션마다 한 번만 읽어요
    static KEY: RefCell<Option<String>> = const { RefCell::new(None) };
    static MANAGE_BOUND: Cell<bool> = const { Cell::new(false) };
}

async fn giphy_key() -> String {
    if let Some(k) = KEY.with(|k| k.borrow().clone()) {
        return k;
    }
    let k = cloud::call("giphyGet", &[]).await.ok().and_then(|v| v.as_string()).unwrap_or_default();
    KEY.with(|x| *x.borrow_mut() = Some(k.clone()));
    k
}
/// 관리 탭: GIPHY 키 넣기
pub fn bind_manage() {
    if MANAGE_BOUND.replace(true) {
        return;
    }
    spawn_local(async {
        let k = giphy_key().await;
        if let Some(i) = input("giphy-key") {
            i.set_value(&k);
        }
    });
    if let Some(b) = by_id("giphy-save") {
        dom::on(&b, "click", |_| {
            let k: String = input("giphy-key").map(|i| i.value()).unwrap_or_default().trim().to_string();
            if !(10..=64).contains(&k.len()) || !k.chars().all(|c| c.is_ascii_alphanumeric()) {
                return crate::shell::toast("키는 영문과 숫자 10~64자예요");
            }
            spawn_local(async move {
                match cloud::call("giphySet", &[JsValue::from_str(&k)]).await {
                    Ok(_) => {
                        KEY.with(|x| *x.borrow_mut() = Some(k));
                        crate::shell::toast("GIF 검색 키를 저장했어요");
                    }
                    Err(e) => crate::shell::toast(&format!("저장하지 못했어요. {}", cloud::why(&e.code))),
                }
            });
        });
    }
}

fn person(uid: &str) -> (String, String) {
    let p = cloud::call_now("peek", &[JsValue::from_str(uid)]).unwrap_or(JsValue::NULL);
    let g = |k: &str| cloud::get(&p, k).as_string().unwrap_or_default();
    let n = g("name");
    (if n.is_empty() { g("id") } else { n }, g("photo"))
}
fn my_uid() -> String {
    cloud::cloud().and_then(|c| cloud::get(&c, "uid").as_string()).unwrap_or_default()
}
fn input(id: &str) -> Option<web_sys::HtmlInputElement> {
    by_id(id)?.dyn_into().ok()
}
fn app() -> Option<web_sys::Element> {
    dom::doc().document_element()
}

/* ---------- 열고 닫기 ---------- */

/// 댓글 판을 열어요 (릴스의 댓글 버튼)
pub fn open(id: &str) {
    if !cloud::signed_in() {
        return crate::shell::toast("로그인하면 댓글을 볼 수 있어요");
    }
    let Some(rc) = by_id("rc") else { return };
    bind_once();
    let by = crate::sns::reel_posts().into_iter().find(|p| p.0 == id).map(|p| p.1).unwrap_or_default();
    POST.with(|p| *p.borrow_mut() = (id.to_string(), by));
    set_mode("");
    let me = my_uid();
    let (n, ph) = person(&me);
    if let Some(m) = by_id("rc-me") {
        m.set_inner_html(&crate::presence::wrap(dom::avatar_html(&n, &ph, "avatar contact rc-av"), &me));
    }
    paint_list(None);
    rc.set_hidden(false);
    let _ = rc.offset_width(); // 숨김을 푼 뒤 한 번만: 올라오는 움직임이 처음부터 보이게
    let _ = rc.class_list().add_1("on");
    if let Some(a) = app() {
        let _ = a.class_list().add_1("rc-open");
    }
    dom::vibrate(6);
    spawn_local(load(id.to_string()));
}
pub fn close() {
    let Some(rc) = by_id("rc") else { return };
    if !rc.class_list().contains("on") {
        return;
    }
    let _ = rc.class_list().remove_1("on");
    if let Some(s) = by_id("rc-sheet") {
        let _ = s.style().remove_property("transform");
    }
    if let Some(a) = app() {
        let _ = a.class_list().remove_1("rc-open");
    }
    if let Some(i) = input("rc-in") {
        let _ = i.blur();
    }
    spawn_local(async move {
        dom::sleep(380).await;
        if !rc.class_list().contains("on") {
            rc.set_hidden(true);
        }
    });
}
/// 판 안의 모드: 댓글 목록 / GIF 고르기 / 선물
fn set_mode(m: &'static str) {
    MODE.with(|x| *x.borrow_mut() = m);
    if let Some(l) = by_id("rc-list") {
        l.set_hidden(!m.is_empty());
    }
    dom::set_hidden("rc-gif", m != "gif");
    dom::set_hidden("rc-gift", m != "gift");
    dom::set_hidden("rc-form", m == "gif");
    if let Some(s) = by_id("rc-sheet") {
        dom::toggle(&s, "tall", m == "gif");
    }
}

/* ---------- 댓글 ---------- */

fn row(uid: &str, at: f64, body: &str, cls: &str) -> String {
    let (n, ph) = person(uid);
    format!(
        r#"<li class="rc-row{cls}"><button type="button" class="rc-who" data-uid="{u}">{av}</button><div class="rc-txt"><p class="rc-name"><b>{n}</b>{bl} <small>{ago}</small></p>{body}</div></li>"#,
        u = esc(uid),
        av = crate::presence::wrap(dom::avatar_html(&n, &ph, "avatar contact rc-av"), uid),
        n = esc(&n),
        bl = crate::profile::blue(uid),
        ago = crate::sns::ago(at),
    )
}
/// 목록 그리기: 맨 위는 글쓴이의 글, 그 아래 댓글들 (None이면 불러오는 중)
fn paint_list(list: Option<&[JsValue]>) {
    let Some(l) = by_id("rc-list") else { return };
    let (id, _) = POST.with(|p| p.borrow().clone());
    let post = crate::sns::reel_posts().into_iter().find(|p| p.0 == id);
    let mut html = String::new();
    if let Some(p) = &post {
        if !p.2.is_empty() {
            html.push_str(&row(&p.1, p.4, &format!(r#"<p class="rc-body">{}</p>"#, esc(&p.2)), " rc-cap"));
        }
    }
    match list {
        None => html.push_str(r#"<li class="rc-none">불러오는 중이에요</li>"#),
        Some([]) => html.push_str(r#"<li class="rc-none"><b>아직 댓글이 없어요</b>첫 댓글을 남겨 보세요</li>"#),
        Some(list) => {
            for r in list {
                let g = |k: &str| cloud::get(r, k).as_string().unwrap_or_default();
                let gif = g("gif");
                let gif = crate::sns::safe_gif(&gif);
                let body = if !gif.is_empty() {
                    format!(r#"<img class="rc-gifimg" src="{}" alt="GIF" loading="lazy" decoding="async">"#, esc(gif))
                } else {
                    format!(r#"<p class="rc-body">{}</p>"#, esc(&g("text")))
                };
                html.push_str(&row(&g("by"), cloud::get(r, "at").as_f64().unwrap_or(0.0), &body, ""));
            }
        }
    }
    l.set_inner_html(&html);
}
async fn load(id: String) {
    let list: Vec<JsValue> = cloud::call("postReplies", &[JsValue::from_str(&id)]).await.map(|v| js_sys::Array::from(&v).to_vec()).unwrap_or_default();
    if POST.with(|p| p.borrow().0 != id) {
        return;
    }
    paint_list(Some(&list));
    let who: js_sys::Array = list.iter().map(|r| cloud::get(r, "by")).collect();
    let _ = cloud::call("fetchProfiles", &[who.into()]).await;
    if POST.with(|p| p.borrow().0 == id) {
        paint_list(Some(&list));
        if let Some(l) = by_id("rc-list") {
            l.set_scroll_top(l.scroll_height());
        }
    }
}
/// 댓글이나 GIF 보내기 (같은 줄을 써요)
fn post_reply(text: String, gif: String) {
    let (id, _) = POST.with(|p| p.borrow().clone());
    if id.is_empty() || BUSY.replace(true) {
        return;
    }
    dom::vibrate(8);
    spawn_local(async move {
        let r = cloud::call("postCreate", &[JsValue::from_str(&text), JsValue::from_str(""), JsValue::from_str(&id), JsValue::from_str(""), JsValue::from_str(&gif)]).await;
        BUSY.set(false);
        match r {
            Ok(_) => {
                crate::algo::bump(&POST.with(|p| p.borrow().1.clone()), 2.0);
                load(id).await
            }
            Err(_) => crate::shell::toast("댓글을 달지 못했어요"),
        }
    });
}
fn send_text() {
    let Some(i) = input("rc-in") else { return };
    let t: String = i.value().trim().chars().take(300).collect();
    if t.is_empty() {
        return;
    }
    i.set_value("");
    post_reply(t, String::new());
}

/* ---------- GIF (GIPHY) ---------- */

fn open_gif() {
    set_mode("gif");
    if let Some(q) = input("rc-q") {
        q.set_value("");
    }
    search(String::new());
}
/// 검색어가 비면 인기 GIF, 있으면 검색 결과 (24개)
fn search(q: String) {
    let Some(grid) = by_id("rc-grid") else { return };
    let seq = SEQ.get() + 1;
    SEQ.set(seq);
    spawn_local(async move {
        let key = giphy_key().await;
        if key.is_empty() {
            grid.set_inner_html(r#"<p class="rc-gnote">GIF 검색이 아직 준비 중이에요</p>"#);
            return;
        }
        if !q.is_empty() {
            dom::sleep(400).await; // 치는 동안에는 보내지 않아요
            if SEQ.get() != seq {
                return;
            }
        }
        let enc = String::from(js_sys::encode_uri_component(&q));
        let url = if q.is_empty() {
            format!("https://api.giphy.com/v1/gifs/trending?api_key={key}&limit=24&rating=pg-13")
        } else {
            format!("https://api.giphy.com/v1/gifs/search?api_key={key}&q={enc}&limit=24&rating=pg-13&lang=ko")
        };
        let r = async {
            let resp = JsFuture::from(web_sys::window().ok_or(JsValue::NULL)?.fetch_with_str(&url)).await?;
            let f: js_sys::Function = js_sys::Reflect::get(&resp, &"json".into())?.dyn_into()?;
            JsFuture::from(f.call0(&resp)?.dyn_into::<js_sys::Promise>()?).await
        }
        .await;
        if SEQ.get() != seq {
            return;
        }
        let Ok(j) = r else {
            grid.set_inner_html(r#"<p class="rc-gnote">GIF를 불러오지 못했어요</p>"#);
            return;
        };
        let items: String = js_sys::Array::from(&cloud::get(&j, "data"))
            .iter()
            .filter_map(|g| {
                let id = cloud::get(&g, "id").as_string()?;
                let url = format!("https://media.giphy.com/media/{id}/200w.webp");
                if crate::sns::safe_gif(&url).is_empty() {
                    return None;
                }
                let fw = cloud::get(&cloud::get(&g, "images"), "fixed_width");
                let num = |k: &str| cloud::get(&fw, k).as_string().and_then(|s| s.parse::<f64>().ok()).unwrap_or(200.0);
                let ratio = (num("height") / num("width").max(1.0)).clamp(0.4, 2.5);
                Some(format!(r#"<button type="button" class="rc-g" data-gif="{u}" style="aspect-ratio:1/{ratio:.3}"><img src="{u}" alt="GIF" loading="lazy" decoding="async"></button>"#, u = esc(&url)))
            })
            .collect();
        grid.set_inner_html(&if items.is_empty() { r#"<p class="rc-gnote">찾는 GIF가 없어요</p>"#.to_string() } else { items });
        grid.set_scroll_top(0);
    });
}

/* ---------- 멸공 선물 ---------- */

fn open_gift() {
    let (_, by) = POST.with(|p| p.borrow().clone());
    if by.is_empty() {
        return;
    }
    if by == my_uid() {
        return crate::shell::toast("내 릴스에는 선물할 수 없어요");
    }
    let (n, _) = person(&by);
    dom::set_text("rc-gt", &format!("{n}님에게 멸공 선물"));
    dom::set_text("rc-gb", &format!("내 멸공 {}", crate::money::w(crate::money::balance().unwrap_or(0.0))));
    if let Some(i) = input("rc-amt") {
        i.set_value("");
    }
    set_mode("gift");
}
fn amount() -> f64 {
    input("rc-amt").map(|i| i.value().chars().filter(|c| c.is_ascii_digit()).collect::<String>()).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0).min(99_999_999.0)
}
fn send_gift() {
    let amt = amount().floor();
    if amt < 1.0 {
        return crate::shell::toast("선물할 멸공을 정해 주세요");
    }
    let (id, by) = POST.with(|p| p.borrow().clone());
    if BUSY.replace(true) {
        return;
    }
    spawn_local(async move {
        if let Err(m) = crate::money::ensure(amt).await {
            BUSY.set(false);
            crate::shell::ask("멸공을 보낼 수 없어요", &m, "확인", "").await;
            return;
        }
        let (id2, by2) = (id.clone(), by.clone());
        let f = Closure::once_into_js(move || cloud::call_now("reelGift", &[JsValue::from_str(&id2), JsValue::from_str(&by2), amt.into()]).unwrap_or(JsValue::UNDEFINED));
        let r = cloud::settle(crate::money::wallet_op(f.unchecked_into()).into()).await;
        BUSY.set(false);
        let (n, _) = person(&by);
        match r {
            Ok(_) => {
                crate::money::log(&dom::obj(&[("t", js_sys::Date::now().into()), ("name", JsValue::from_str(&format!("{n}님 릴스에 선물"))), ("cat", "송금".into()), ("icon", "gift".into()), ("amt", (-amt).into())]));
                dom::vibrate(12);
                crate::algo::bump(&by, 3.0);
                crate::shell::toast(&format!("{n}님에게 멸공 {}을 선물했어요", cloud::fmt_num(amt, 0)));
                set_mode("");
            }
            Err(e) => crate::shell::toast(&format!("선물하지 못했어요. {}", cloud::why(&e.code))),
        }
    });
}

/* ---------- 끌어 내려 닫기 ---------- */

fn drag_start(y: f64) {
    DRAG.set((y, 0.0, true));
    if let Some(s) = by_id("rc-sheet") {
        let _ = s.class_list().add_1("drag");
    }
}
fn drag_move(y: f64) {
    let (y0, _, on) = DRAG.get();
    if !on {
        return;
    }
    let dy = (y - y0).max(0.0);
    DRAG.set((y0, dy, true));
    if let Some(s) = by_id("rc-sheet") {
        let _ = s.style().set_property("transform", &format!("translate3d(0,{dy:.0}px,0)"));
    }
}
fn drag_end() {
    let (_, dy, on) = DRAG.get();
    if !on {
        return;
    }
    DRAG.set((0.0, 0.0, false));
    let Some(s) = by_id("rc-sheet") else { return };
    let _ = s.class_list().remove_1("drag");
    let _ = s.style().remove_property("transform");
    if dy > 90.0 {
        // GIF·선물 중이면 먼저 댓글로 돌아가요
        if MODE.with(|m| !m.borrow().is_empty()) {
            set_mode("");
        } else {
            close();
        }
    }
}

fn bind_once() {
    if BOUND.replace(true) {
        return;
    }
    if let Some(t) = by_id("rc-top") {
        dom::on(&t, "click", |_| close());
    }
    if let Some(g) = by_id("rc-grab") {
        dom::on(&g, "pointerdown", |e| {
            if let Some(p) = e.dyn_ref::<web_sys::PointerEvent>() {
                if let Some(t) = e.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) {
                    let _ = t.set_pointer_capture(p.pointer_id());
                }
                drag_start(p.client_y() as f64);
            }
        });
        dom::on(&g, "pointermove", |e| {
            if let Some(p) = e.dyn_ref::<web_sys::PointerEvent>() {
                drag_move(p.client_y() as f64);
            }
        });
        for ev in ["pointerup", "pointercancel"] {
            dom::on(&g, ev, |_| drag_end());
        }
    }
    if let Some(f) = by_id("rc-form") {
        dom::on(&f, "submit", |e| {
            e.prevent_default();
            send_text();
        });
    }
    if let Some(b) = by_id("rc-gifb") {
        dom::on_tap(&b, open_gif);
    }
    if let Some(b) = by_id("rc-giftb") {
        dom::on_tap(&b, || {
            if MODE.with(|m| *m.borrow() == "gift") {
                set_mode("");
            } else {
                open_gift();
            }
        });
    }
    if let Some(q) = input("rc-q") {
        let q2 = q.clone();
        dom::on(&q, "input", move |_| search(q2.value().trim().chars().take(50).collect()));
    }
    if let Some(g) = by_id("rc-grid") {
        dom::on(&g, "click", |e| {
            if let Some(b) = dom::closest(&e, "[data-gif]") {
                let u = dom::data(&b, "gif");
                set_mode("");
                post_reply(String::new(), u);
            }
        });
    }
    if let Some(c) = by_id("rc-chips") {
        dom::on(&c, "click", |e| {
            if let Some(b) = dom::closest(&e, "[data-v]") {
                let v = dom::data(&b, "v").parse::<f64>().unwrap_or(0.0);
                if let Some(i) = input("rc-amt") {
                    i.set_value(&cloud::fmt_num(amount() + v, 0));
                }
                dom::vibrate(6);
            }
        });
    }
    if let Some(i) = input("rc-amt") {
        let i2 = i.clone();
        dom::on(&i, "input", move |_| {
            let v = amount();
            i2.set_value(&if v > 0.0 { cloud::fmt_num(v, 0) } else { String::new() });
        });
    }
    if let Some(b) = by_id("rc-send") {
        dom::on(&b, "click", |_| send_gift());
    }
    if let Some(l) = by_id("rc-list") {
        dom::on(&l, "click", |e| {
            if let Some(b) = dom::closest(&e, "[data-uid]") {
                let uid = dom::data(&b, "uid");
                crate::profile::show(&uid, &person(&uid).0, None);
            }
        });
    }
    dom::on(&dom::doc(), "keydown", |e| {
        if e.dyn_ref::<web_sys::KeyboardEvent>().is_some_and(|k| k.key() == "Escape") {
            close();
        }
    });
}
