//! 대화방: 말풍선과 카드(사진·그림·토큰·투표), 바뀐 줄만 갈아끼우기, 이전 메시지 더 보기, 글 쓰기, 배경화면

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use js_sys::{Function, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::{ctx, find, open_chat, paint_status, render, set_open, toast, uid, update_open, with_chats, Chat, Msg, WALLS};
use crate::dom::{self, avatar_html, by_id, esc, safe_img};

const WAVE: &str = "M5 10v4M9 6v12M13 3v18M17 8v8M21 11v2";
const ARROW: &str = "M12 19V5M5 12l7-7 7 7";

thread_local! {
    static UNSUB: RefCell<Option<Function>> = const { RefCell::new(None) };
    static READ_MARK: RefCell<String> = const { RefCell::new(String::new()) };
    /// 단톡방에서 보낸 사람의 이름 (한 번 읽으면 기억해요)
    static SENDERS: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    static SEQ: Cell<u32> = const { Cell::new(0) };
}

fn ko(v: f64) -> String {
    cloud::fmt_num(v, 3)
}
fn s(v: &JsValue, k: &str) -> String {
    cloud::get(v, k).as_string().unwrap_or_default()
}
/// 목록에 보이는 한 줄 글
pub fn preview(m: &JsValue) -> String {
    let amt = || ko(js_sys::Number::from(cloud::get(m, "amount")).value_of());
    match s(m, "type").as_str() {
        "photo" => "사진".into(),
        "draw" => "그림".into(),
        "gift" => format!("토큰 {} 선물", amt()),
        "request" => format!("토큰 {} 요청", amt()),
        "poll" => format!("투표: {}", s(&cloud::get(m, "poll"), "q")),
        _ => s(m, "text"),
    }
}
pub(crate) fn msg_of(m: JsValue, group: bool, chat_name: &str) -> Msg {
    let from = s(&m, "from");
    let by = if group { SENDERS.with(|n| n.borrow().get(&from).cloned()).unwrap_or_default() } else { chat_name.to_string() };
    Msg { me: from == uid(), text: preview(&m), by, raw: m }
}

/// 글이 아닌 메시지(사진, 그림, 토큰, 투표)는 말풍선 대신 카드로 그려요
fn card_html(m: &JsValue, me: bool, i: usize) -> String {
    let t = s(m, "type");
    match t.as_str() {
        "photo" | "draw" => {
            let img = s(m, "img");
            let src = safe_img(&img, 140000);
            if src.is_empty() {
                r#"<div class="bubble">열 수 없는 사진이에요</div>"#.into()
            } else {
                let d = t == "draw";
                format!(r#"<button class="media{}" data-view="{i}" type="button"><img src="{src}" alt="{}" decoding="async" loading="lazy"></button>"#, if d { " drawn" } else { "" }, if d { "그림" } else { "사진" })
            }
        }
        "gift" | "request" => {
            let gift = t == "gift";
            let amount = js_sys::Number::from(cloud::get(m, "amount")).value_of();
            let amount = if amount.is_finite() { amount.floor().max(0.0) } else { 0.0 };
            let done = cloud::get(m, "done").is_truthy();
            let btn = if done && gift && cloud::get(m, "cancel").is_truthy() {
                format!(r#"<span class="cash-state">{}</span>"#, if me { "선물을 취소했어요" } else { "상대가 취소했어요" })
            } else if done {
                format!(r#"<span class="cash-state">{}</span>"#, match (gift, me) {
                    (true, true) => "상대가 받았어요",
                    (true, false) => "받았어요",
                    (false, true) => "상대가 보냈어요",
                    (false, false) => "보냈어요",
                })
            } else if me && gift {
                format!(r#"<span class="cash-state">상대가 받으면 완료돼요</span><button class="cash-btn ghost" data-act="cancel" data-i="{i}" type="button">선물 취소</button>"#)
            } else if me {
                r#"<span class="cash-state">요청했어요</span>"#.to_string()
            } else {
                format!(r#"<button class="cash-btn" data-act="{}" data-i="{i}" type="button">{}</button>"#, if gift { "accept" } else { "pay" }, if gift { "받기" } else { "보내기" })
            };
            format!(r#"<div class="cash"><p class="cash-tag">{}</p><div class="cash-amt"><b>{}</b><span>토큰</span></div>{btn}</div>"#, if gift { "토큰 선물" } else { "토큰 요청" }, ko(amount))
        }
        "poll" => {
            let poll = cloud::get(m, "poll");
            let opts = cloud::get(&poll, "options");
            if !js_sys::Array::is_array(&opts) {
                return String::new();
            }
            let votes_obj = cloud::get(m, "votes");
            let votes: Vec<f64> = if votes_obj.is_object() { js_sys::Object::values(votes_obj.unchecked_ref::<js_sys::Object>()).iter().filter_map(|v: JsValue| v.as_f64()).collect() } else { Vec::new() };
            let mine = cloud::get(&votes_obj, &uid()).as_f64();
            let total = votes.len();
            let rows: String = js_sys::Array::from(&opts)
                .iter()
                .enumerate()
                .map(|(k, o)| {
                    let n = votes.iter().filter(|v| **v == k as f64).count();
                    let pct = if total > 0 { ((n as f64 / total as f64) * 100.0 + 0.5).floor() } else { 0.0 };
                    let img = s(&o, "img");
                    let img = safe_img(&img, 60000);
                    let t = s(&o, "t");
                    let label = if t.is_empty() { format!("선택지 {}", k + 1) } else { t };
                    format!(
                        r#"<button class="poll-opt{}" data-vote="{k}" data-i="{i}" type="button"><i class="po-bar" style="width:{pct}%"></i>{}<span class="po-t">{}</span><span class="po-n">{n}</span></button>"#,
                        if mine == Some(k as f64) { " on" } else { "" },
                        if img.is_empty() { String::new() } else { format!(r#"<img src="{img}" alt="" decoding="async" loading="lazy">"#) },
                        esc(&label)
                    )
                })
                .collect();
            format!(r#"<div class="poll"><p class="poll-q">{}</p>{rows}<p class="poll-meta">{total}명 투표했어요{}</p></div>"#, esc(&s(&poll, "q")), if mine.is_none() { " · 눌러서 투표" } else { "" })
        }
        _ => String::new(),
    }
}

pub(crate) struct Row {
    key: String,
    sig: String,
    pub html: String,
}
/// 한 줄(보낸 사람 이름표 + 말풍선이나 카드)을 만들어요. sig는 이 줄이 바뀌었는지 알아보는 표시예요
pub(crate) fn row_of(c: &Chat, i: usize, fresh: i64) -> Row {
    let msgs = &c.msgs;
    let m = &msgs[i];
    let next = msgs.get(i + 1);
    let last = next.is_none_or(|n| n.me != m.me || n.by != m.by);
    let ty = s(&m.raw, "type");
    let card = if !ty.is_empty() && ty != "text" { card_html(&m.raw, m.me, i) } else { String::new() };
    let b = if card.is_empty() {
        format!(r#"<div class="bubble{}{}{}" data-i="{i}">{}</div>"#, if m.me { " me" } else { "" }, if last { " last" } else { "" }, if i as i64 == fresh { " fresh" } else { "" }, esc(&m.text))
    } else {
        card
    };
    let prev = if i > 0 { msgs.get(i - 1) } else { None };
    let name_tag = if c.group && !m.me && prev.is_none_or(|p| p.me != m.me || p.by != m.by) && !m.by.is_empty() { format!(r#"<span class="sender">{}</span>"#, esc(&m.by)) } else { String::new() };
    let line = if m.me {
        format!(r#"<div class="line me">{b}</div>"#)
    } else {
        format!(
            r#"<div class="line">{}<div class="bcol">{name_tag}{b}</div></div>"#,
            if last { avatar_html(if m.by.is_empty() { &c.name } else { &m.by }, &c.photo, "avatar contact") } else { r#"<span style="width:38px"></span>"#.into() }
        )
    };
    let id = s(&m.raw, "id");
    let key = if id.is_empty() { i.to_string() } else { id };
    let votes = cloud::get(&m.raw, "votes");
    let vs = if votes.is_truthy() { js_sys::JSON::stringify(&votes).map(String::from).unwrap_or_default() } else { String::new() };
    let sig = format!("{key}|{}|{}|{vs}|{i}", if last { 1 } else { 0 }, if cloud::get(&m.raw, "done").is_truthy() { 1 } else { 0 });
    let html = format!(r#"<div class="mrow" data-k="{}" data-sig="{}">{line}</div>"#, esc(&key), esc(&sig));
    Row { key, sig, html }
}
/// 이미 그려진 줄은 건드리지 않고 바뀐 줄만 갈아끼우고 새 줄만 붙여요 (사진이 있는 대화에서 다시 그리느라 버벅이지 않게)
fn sync_rows(bubbles: &web_sys::HtmlElement, items: &[Row]) {
    let cur = dom::qsa(bubbles, ":scope > .mrow");
    let mut i = 0;
    while i < items.len() && i < cur.len() && dom::data(&cur[i], "k") == items[i].key {
        if dom::data(&cur[i], "sig") != items[i].sig {
            if let Ok(t) = dom::doc().create_element("template") {
                t.set_inner_html(&items[i].html);
                if let Some(n) = t.unchecked_ref::<web_sys::HtmlTemplateElement>().content().first_element_child() {
                    let _ = cur[i].replace_with_with_node_1(&n);
                }
            }
        }
        i += 1;
    }
    for k in (i..cur.len()).rev() {
        cur[k].remove(); // 순서가 달라졌으면 뒤쪽만 지우고 다시 붙여요
    }
    let read = dom::qs(bubbles, ":scope > .read");
    for it in &items[i..] {
        match &read {
            Some(r) => {
                let _ = r.insert_adjacent_html("beforebegin", &it.html);
            }
            None => {
                let _ = bubbles.insert_adjacent_html("beforeend", &it.html);
            }
        }
    }
}
/// 대화 화면: 메시지 목록을 다시 그려요 (fresh는 방금 생긴 말풍선 번호, -2면 스크롤하지 않아요)
pub(crate) fn render_thread(fresh: i64) {
    let (Some(c), Some(bubbles)) = (open_chat(), by_id("bubbles")) else { return };
    let stamp = format!("오늘 {}", if c.time.contains(':') { c.time.as_str() } else { "오후 2:14" });
    let head = match dom::qs(&bubbles, ":scope > .stamp") {
        Some(h) => h,
        None => {
            let _ = bubbles.insert_adjacent_html("afterbegin", r#"<div class="stamp"></div>"#);
            match dom::qs(&bubbles, ":scope > .stamp") {
                Some(h) => h,
                None => return,
            }
        }
    };
    if head.text_content().unwrap_or_default() != stamp {
        head.set_text_content(Some(&stamp));
    }
    let more = dom::qs(&bubbles, ":scope > .more");
    if c.more && more.is_none() {
        let _ = head.insert_adjacent_html("afterend", r#"<button class="more" type="button">이전 메시지 더 보기</button>"#);
    } else if !c.more {
        if let Some(m) = more {
            m.remove();
        }
    }
    let rows: Vec<Row> = (0..c.msgs.len()).map(|i| row_of(&c, i, fresh)).collect();
    sync_rows(&bubbles, &rows);
    let read = dom::qs(&bubbles, ":scope > .read");
    let mine_last = c.msgs.last().is_some_and(|m| m.me);
    if mine_last && read.is_none() {
        let _ = bubbles.insert_adjacent_html("beforeend", r#"<div class="read">전송됨</div>"#);
    } else if !mine_last {
        if let Some(r) = read {
            r.remove();
        }
    }
    if fresh != -2 {
        let o = web_sys::ScrollToOptions::new();
        o.set_top(bubbles.scroll_height() as f64);
        o.set_behavior(if fresh >= 0 && !dom::reduce_motion() { web_sys::ScrollBehavior::Smooth } else { web_sys::ScrollBehavior::Auto });
        bubbles.scroll_to_with_scroll_to_options(&o);
    }
}

/* ---------- 배경화면 ---------- */

pub(crate) fn walls() -> JsValue {
    js_sys::JSON::parse(&dom::store_get("walls", "{}")).ok().filter(|v| v.is_object()).unwrap_or_else(|| js_sys::Object::new().into())
}
pub(crate) fn wall_of(id: &str) -> String {
    cloud::get(&walls(), id).as_string().filter(|w| WALLS.contains(&w.as_str())).unwrap_or_else(|| "white".into())
}
fn apply_wall() {
    let (Some(c), Some(th)) = (open_chat(), by_id("thread")) else { return };
    let wall = wall_of(&c.id);
    let _ = th.set_attribute("data-wall", &wall);
    paint_status();
    for o in dom::qsa(&dom::doc().document_element().expect("html"), "[data-pick]") {
        let _ = o.set_attribute("aria-pressed", &(dom::data(&o, "pick") == wall).to_string());
    }
}

/* ---------- 열기, 닫기 ---------- */

pub fn open(id: &str) {
    let Some(mut c) = find(id) else { return };
    c.unread = false;
    with_chats(|cs| cs.iter_mut().filter(|x| x.id == c.id).for_each(|x| x.unread = false));
    dom::set_text("thread-name", &c.name);
    if let Some(tav) = by_id("thread-avatar") {
        let tp = safe_img(&c.photo, 40000);
        tav.set_text_content(Some(&if tp.is_empty() { dom::initials(&c.name) } else { String::new() }));
        let _ = tav.style().set_property("background-image", &if tp.is_empty() { String::new() } else { format!("url({tp})") });
        dom::toggle(&tav, "photo", !tp.is_empty());
    }
    dom::set_text("thread-badge", &with_chats(|cs| cs.iter().filter(|c| c.unread).count()).to_string());
    if let Some(f) = UNSUB.with(|u| u.borrow_mut().take()) {
        let _ = f.call0(&JsValue::NULL);
    }
    c.msgs.clear();
    c.older.clear();
    c.more = false;
    c.older_loaded = false;
    set_open(Some(c.clone()));
    apply_wall();
    if let Some(b) = by_id("bubbles") {
        b.set_inner_html("");
    }
    render_thread(-2);
    let seq = SEQ.get() + 1;
    SEQ.set(seq);
    // 최근 40개만 실시간으로 받아요. 더 옛날 것은 "이전 메시지 더 보기"로 불러요
    let (cid, group, cname) = (c.id.clone(), c.group, c.name.clone());
    let cb = Closure::<dyn FnMut(JsValue, JsValue)>::new(move |list: JsValue, has_more: JsValue| {
        let (cid, cname) = (cid.clone(), cname.clone());
        leptos::task::spawn_local(async move {
            let list: Vec<JsValue> = js_sys::Array::from(&list).iter().collect();
            // 단톡방이면 보낸 사람 이름을 먼저 읽어요 (말풍선 위에 이름을 써요)
            if group {
                let mut seen = std::collections::HashSet::new();
                for m in &list {
                    let u = s(m, "from");
                    if seen.insert(u.clone()) && !SENDERS.with(|n| n.borrow().contains_key(&u)) {
                        let name = cloud::call("nameOf", &[JsValue::from_str(&u)]).await.ok().and_then(|v| v.as_string()).unwrap_or_default();
                        SENDERS.with(|n| n.borrow_mut().insert(u, name));
                    }
                }
            }
            if SEQ.get() != seq || super::open_id().as_deref() != Some(&cid) {
                return;
            }
            let Some(c) = open_chat() else { return };
            let prev_len = c.msgs.len() as i64 - c.older.len() as i64;
            let mut all: Vec<JsValue> = c.older.clone();
            all.extend(list.iter().cloned());
            let grew = list.len() as i64 > prev_len && prev_len > 0;
            let msgs: Vec<Msg> = all.iter().map(|m| msg_of(m.clone(), group, &cname)).collect();
            let n = all.len() as i64;
            let more = has_more.is_truthy();
            update_open(|x| {
                x.msgs = msgs.clone();
                if !x.older_loaded {
                    x.more = more;
                }
            });
            render_thread(if grew || prev_len == 0 {
                if prev_len == 0 {
                    -1
                } else {
                    n - 1
                }
            } else {
                -2
            });
            if let Some(last) = list.last() {
                let lid = s(last, "id");
                if s(last, "from") != uid() && READ_MARK.with(|r| *r.borrow() != lid) {
                    READ_MARK.with(|r| *r.borrow_mut() = lid);
                    let _ = cloud::call_now("markRead", &[JsValue::from_str(&cid)]);
                }
            }
        });
    });
    let unsub = cloud::call_now("watchMessages", &[JsValue::from_str(&c.id), cb.into_js_value()]).ok().and_then(|f| f.dyn_into::<Function>().ok());
    UNSUB.with(|u| *u.borrow_mut() = unsub);
    dom::set_hidden("thread-invite", !c.group);
    if let Some(t) = dom::doc().query_selector("#attach [data-a=\"token\"]").ok().flatten().and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()) {
        t.set_hidden(c.group); // 단톡방에서는 토큰을 주고받지 않아요
    }
    if let Some(app) = dom::app_el() {
        let _ = app.class_list().add_1("in-thread");
    }
    paint_status();
    if let Some(th) = by_id("thread") {
        let _ = th.class_list().add_1("is-open");
    }
    render();
}

pub fn close() {
    if let Some(f) = UNSUB.with(|u| u.borrow_mut().take()) {
        let _ = f.call0(&JsValue::NULL);
    }
    SEQ.set(SEQ.get() + 1);
    READ_MARK.with(|r| r.borrow_mut().clear());
    set_open(None);
    if let Some(th) = by_id("thread") {
        let _ = th.class_list().remove_1("is-open");
    }
    if let Some(app) = dom::app_el() {
        let _ = app.class_list().remove_1("in-thread");
    }
    paint_status();
    if let Some(i) = by_id("composer-input") {
        let _ = i.blur();
    }
    let tb = Reflect::get(&js_sys::global(), &JsValue::from_str("tabbar")).unwrap_or(JsValue::UNDEFINED);
    if let Ok(f) = Reflect::get(&tb, &JsValue::from_str("requestRender")).and_then(|f| f.dyn_into::<Function>()) {
        let _ = f.call0(&tb);
    }
}

/// 이전 메시지 더 보기
async fn load_older() {
    let Some(c) = open_chat() else { return };
    let Some(oldest) = c.msgs.first().map(|m| m.raw.clone()) else { return };
    let Some(bubbles) = by_id("bubbles") else { return };
    let h0 = bubbles.scroll_height();
    match cloud::call("loadOlder", &[JsValue::from_str(&c.id), cloud::get(&oldest, "at")]).await {
        Ok(r) => {
            if super::open_id().as_deref() != Some(&c.id) {
                return;
            }
            let list: Vec<JsValue> = js_sys::Array::from(&cloud::get(&r, "list")).iter().collect();
            let more = cloud::get(&r, "more").is_truthy();
            let name = c.name.clone();
            update_open(|x| {
                let mut older = list.clone();
                older.extend(x.older.iter().cloned());
                x.older = older;
                x.older_loaded = true;
                x.more = more;
                let mut msgs: Vec<Msg> = list.iter().map(|m| msg_of(m.clone(), false, &name)).collect();
                msgs.extend(x.msgs.iter().cloned());
                x.msgs = msgs;
            });
            for r in dom::qsa(&bubbles, ":scope > .mrow") {
                r.remove(); // 앞에 끼워 넣는 거라 줄을 다시 붙여요
            }
            render_thread(-2);
            bubbles.set_scroll_top(bubbles.scroll_height() - h0);
        }
        Err(_) => toast("이전 메시지를 불러오지 못했어요"),
    }
}

pub fn init() {
    if let Some(b) = by_id("bubbles") {
        dom::on(&b, "click", |e| {
            if dom::closest(&e, ".more").is_some() && open_chat().is_some() {
                leptos::task::spawn_local(load_older());
            }
        });
    }
    if let Some(i) = by_id("composer-input") {
        let i2 = i.clone();
        dom::on(&i, "input", move |_| {
            let v = i2.dyn_ref::<web_sys::HtmlInputElement>().map(|x| x.value()).or_else(|| i2.dyn_ref::<web_sys::HtmlTextAreaElement>().map(|x| x.value())).unwrap_or_default();
            let ready = !v.trim().is_empty();
            if let Some(b) = by_id("send") {
                dom::toggle(&b, "ready", ready);
            }
            if let Some(p) = by_id("send-icon").and_then(|s| s.first_element_child()) {
                let _ = p.set_attribute("d", if ready { ARROW } else { WAVE });
            }
        });
    }
    if let Some(f) = by_id("composer") {
        dom::on(&f, "submit", |e| {
            e.prevent_default();
            let Some(inp) = by_id("composer-input") else { return };
            let v = inp.dyn_ref::<web_sys::HtmlInputElement>().map(|x| x.value()).or_else(|| inp.dyn_ref::<web_sys::HtmlTextAreaElement>().map(|x| x.value())).unwrap_or_default();
            let text = dom::cut(v.trim(), 1000);
            let Some(c) = open_chat() else { return };
            if text.is_empty() {
                return;
            }
            if let Some(x) = inp.dyn_ref::<web_sys::HtmlInputElement>() {
                x.set_value("");
            } else if let Some(x) = inp.dyn_ref::<web_sys::HtmlTextAreaElement>() {
                x.set_value("");
            }
            if let Ok(ev) = web_sys::Event::new("input") {
                let _ = inp.dispatch_event(&ev);
            }
            leptos::task::spawn_local(async move {
                let (id, other) = (c.id.clone(), c.other.clone());
                let payload = dom::obj(&[("type", "text".into()), ("text", JsValue::from_str(&text))]);
                let t2 = text.clone();
                if super::queue_send(move || cloud::call_now("send", &[JsValue::from_str(&id), JsValue::from_str(&other), payload, JsValue::from_str(&t2)])).await.is_err() {
                    toast("보내지 못했어요");
                }
            });
        });
    }
    if let Some(b) = by_id("thread-back") {
        dom::on(&b, "click", |_| close());
    }
    // 배경화면
    // 1:1 대화 위쪽 사진을 누르면 그 사람 카드가 떠요
    if let Some(a) = by_id("thread-avatar") {
        dom::on(&a, "click", |_| {
            if let Some(c) = super::open_chat() {
                if c.group {
                    open_group_sheet(&c);
                } else if !c.other.is_empty() {
                    crate::profile::show(&c.other, &c.name, None);
                }
            }
        });
    }
    if let Some(b) = by_id("thread-name-btn") {
        dom::on(&b, "click", |_| match super::open_chat() {
            Some(c) if c.group => open_group_sheet(&c),
            _ => super::sheet_open(true, "wall-sheet"),
        });
    }
    bind_group();
    for b in dom::qsa(&dom::doc().document_element().expect("html"), "[data-pick]") {
        let b2 = b.clone();
        dom::on(&b, "click", move |_| {
            let Some(c) = open_chat() else { return };
            let pick = dom::data(&b2, "pick");
            let w = walls();
            if WALLS.contains(&pick.as_str()) {
                let _ = Reflect::set(&w, &JsValue::from_str(&c.id), &JsValue::from_str(&pick));
            }
            if let Ok(s) = js_sys::JSON::stringify(&w) {
                dom::store_set("walls", &String::from(s));
            }
            apply_wall();
            let _ = ctx().call("saveSettings", &[]);
        });
    }
    if let Some(b) = by_id("wall-done") {
        dom::on(&b, "click", |_| super::sheet_open(false, ""));
    }
}

/* ---------- 단톡방 설정: 이름, 사진, 배경 ---------- */

fn paint_head(c: &Chat) {
    dom::set_text("thread-name", &c.name);
    if let Some(tav) = by_id("thread-avatar") {
        let tp = safe_img(&c.photo, 40000);
        tav.set_text_content(Some(&if tp.is_empty() { dom::initials(&c.name) } else { String::new() }));
        let _ = tav.style().set_property("background-image", &if tp.is_empty() { String::new() } else { format!("url({tp})") });
        dom::toggle(&tav, "photo", !tp.is_empty());
    }
    if let Some(av) = by_id("grp-av") {
        let tp = safe_img(&c.photo, 40000);
        av.set_text_content(Some(&if tp.is_empty() { dom::initials(&c.name) } else { String::new() }));
        let _ = av.style().set_property("background-image", &if tp.is_empty() { String::new() } else { format!("url({tp})") });
        dom::toggle(&av, "photo", !tp.is_empty());
    }
}
fn open_group_sheet(c: &Chat) {
    paint_head(c);
    if let Some(i) = by_id("grp-name").and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok()) {
        i.set_value(&c.name);
    }
    super::sheet_open(true, "group-sheet");
}
fn save_group_name() {
    let Some(c) = open_chat() else { return };
    let Some(i) = by_id("grp-name").and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok()) else { return };
    let n = dom::cut(i.value().trim(), 20);
    if n.is_empty() || n == c.name {
        i.set_value(&c.name);
        return;
    }
    let id = c.id.clone();
    wasm_bindgen_futures::spawn_local(async move {
        match cloud::call("setGroup", &[JsValue::from_str(&id), dom::obj(&[("name", JsValue::from_str(&n))])]).await {
            Ok(_) => {
                let n2 = n.clone();
                super::update_open(move |c| c.name = n2.clone());
                if let Some(c) = open_chat() {
                    paint_head(&c);
                }
            }
            Err(_) => super::toast("이름을 바꾸지 못했어요"),
        }
    });
}
fn bind_group() {
    if let Some(i) = by_id("grp-name") {
        dom::on(&i, "change", |_| save_group_name());
        dom::on(&i, "keydown", |e| {
            if e.dyn_ref::<web_sys::KeyboardEvent>().map(|k| k.key() == "Enter").unwrap_or(false) {
                e.prevent_default();
                save_group_name();
                if let Some(t) = e.target().and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok()) {
                    let _ = t.blur();
                }
            }
        });
    }
    if let Some(b) = by_id("group-done") {
        dom::on(&b, "click", |_| {
            save_group_name();
            super::sheet_open(false, "");
        });
    }
    if let Some(b) = by_id("grp-photo-btn") {
        dom::on(&b, "click", |_| {
            if let Some(f) = by_id("grp-file") {
                f.click();
            }
        });
    }
    if let Some(f) = by_id("grp-file") {
        dom::on(&f, "change", |_| {
            let Some(file) = super::attach::take_file("grp-file") else { return };
            let Some(c) = open_chat() else { return };
            wasm_bindgen_futures::spawn_local(async move {
                let img = match super::attach::read_image(file, 320.0, 38000, 0.8, true).await {
                    Ok(i) => i,
                    Err(_) => return super::toast("사진을 넣지 못했어요"),
                };
                match cloud::call("setGroup", &[JsValue::from_str(&c.id), dom::obj(&[("photo", JsValue::from_str(&img))])]).await {
                    Ok(_) => {
                        super::update_open(move |c| c.photo = img.clone());
                        if let Some(c) = open_chat() {
                            paint_head(&c);
                        }
                    }
                    Err(_) => super::toast("사진을 바꾸지 못했어요"),
                }
            });
        });
    }
}
