//! 메시지 탭 (예전 app.js의 메시지 부분): 대화 목록, 고정·안 읽음·알림 가리기, 꾹 눌러 메뉴, 찾기, 편집,
//! 대화방(말풍선·카드), 새 대화·단톡방·초대, 배경화면, 보내기 줄 세우기, 사진·그리기·투표·토큰 선물.
//! 서버를 아끼는 구조는 그대로예요: 대화방은 최근 40개만 실시간으로 듣고, 더 옛날 것은 눌러서 불러요.

mod attach;
mod draw;
mod gift;
mod menu;
mod people;
mod poll;
mod thread;

pub use attach::{read_image, take_file};

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use js_sys::{Function, Promise, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{future_to_promise, JsFuture};

use crate::ctx::Ctx;
use crate::dom::{self, avatar_html, by_id, esc};

pub(crate) const WALLS: [&str; 5] = ["white", "sunset", "aurora", "night", "mint"];
const CHEV: &str = r#"<svg viewBox="0 0 8 14" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M1 1l6 6-6 6"/></svg>"#;

/// 대화방 안의 한 줄: (내가 보냈는지, 미리보기 글, 보낸 사람 이름, 서버 값 그대로)
#[derive(Clone)]
pub(crate) struct Msg {
    pub me: bool,
    pub text: String,
    pub by: String,
    pub raw: JsValue,
}

#[derive(Clone, Default)]
pub(crate) struct Chat {
    pub id: String,
    pub other: String,
    pub group: bool,
    pub members: Vec<String>,
    pub live: bool,
    pub name: String,
    pub photo: String,
    pub last: String,
    pub last_from: String,
    pub time: String,
    pub last_at: f64,
    pub unread: bool,
    pub pin: bool,
    pub muted: bool,
    pub hidden: bool,
    pub msgs: Vec<Msg>,
    pub older: Vec<JsValue>,
    pub more: bool,
    pub older_loaded: bool,
}

thread_local! {
    static CTX: RefCell<Ctx> = RefCell::new(Ctx::default());
    static CHATS: RefCell<Vec<Chat>> = const { RefCell::new(Vec::new()) };
    /// 지금 열려 있는 대화 (목록에서 빠져도 여기에 남아요)
    static OPEN: RefCell<Option<Chat>> = const { RefCell::new(None) };
    static ONLY_UNREAD: Cell<bool> = const { Cell::new(false) };
    static LIVE: Cell<bool> = const { Cell::new(false) };
    static SEND_CHAIN: RefCell<Option<Promise>> = const { RefCell::new(None) };
    static LAST_SEND: Cell<f64> = const { Cell::new(0.0) };
}
pub(crate) fn ctx() -> Ctx {
    CTX.with(|c| c.borrow().clone())
}
pub(crate) fn toast(m: &str) {
    ctx().toast(m);
}
pub(crate) fn live() -> bool {
    LIVE.get()
}
pub(crate) fn uid() -> String {
    cloud::cloud().map(|c| cloud::get(&c, "uid")).and_then(|u| u.as_string()).unwrap_or_default()
}
pub(crate) fn sheet_open(on: bool, id: &str) {
    let el = by_id(id).map(JsValue::from).unwrap_or(JsValue::UNDEFINED);
    let _ = ctx().call("sheetOpen", &[on.into(), el]);
}
pub(crate) fn paint_status() {
    if let Ok(f) = Reflect::get(&js_sys::global(), &JsValue::from_str("__paintStatus")).and_then(|f| f.dyn_into::<Function>()) {
        let _ = f.call0(&JsValue::NULL);
    }
}

/* ---------- 대화 목록 ---------- */

pub(crate) fn with_chats<R>(f: impl FnOnce(&mut Vec<Chat>) -> R) -> R {
    CHATS.with(|c| f(&mut c.borrow_mut()))
}
pub(crate) fn open_chat() -> Option<Chat> {
    OPEN.with(|o| o.borrow().clone())
}
pub(crate) fn open_id() -> Option<String> {
    OPEN.with(|o| o.borrow().as_ref().map(|c| c.id.clone()))
}
/// 열린 대화를 바꿔요 (목록 안의 같은 대화도 같이 바꿔요)
pub(crate) fn update_open(f: impl Fn(&mut Chat)) {
    let id = OPEN.with(|o| {
        let mut o = o.borrow_mut();
        let c = o.as_mut()?;
        f(c);
        Some(c.id.clone())
    });
    if let Some(id) = id {
        with_chats(|cs| cs.iter_mut().filter(|c| c.id == id).for_each(&f));
    }
}
pub(crate) fn set_open(c: Option<Chat>) {
    OPEN.with(|o| *o.borrow_mut() = c);
}
pub(crate) fn find(id: &str) -> Option<Chat> {
    with_chats(|cs| cs.iter().find(|c| c.id == id).cloned())
}

/// 고정·알림 가리기·숨김은 이 기기에 저장해요
fn prefs() -> JsValue {
    js_sys::JSON::parse(&dom::store_get("prefs", "{}")).ok().filter(|v| v.is_object()).unwrap_or_else(|| js_sys::Object::new().into())
}
pub(crate) fn touch_pref(c: &Chat) {
    if !c.live {
        return;
    }
    let p = prefs();
    let _ = Reflect::set(&p, &JsValue::from_str(&c.id), &dom::obj(&[("pin", c.pin.into()), ("muted", c.muted.into()), ("hidden", c.hidden.into())]));
    if let Ok(s) = js_sys::JSON::stringify(&p) {
        dom::store_set("prefs", &String::from(s));
    }
}
fn pref(id: &str, k: &str) -> bool {
    cloud::get(&cloud::get(&prefs(), id), k).is_truthy()
}

/// "오후 3:05" (오늘) 또는 "11월 3일"
pub(crate) fn fmt_time(t: f64) -> String {
    if t <= 0.0 {
        return String::new();
    }
    let d = js_sys::Date::new(&JsValue::from_f64(t));
    let h = d.get_hours();
    let hm = format!("{} {}:{:02}", if h < 12 { "오전" } else { "오후" }, if h % 12 == 0 { 12 } else { h % 12 }, d.get_minutes());
    if String::from(d.to_date_string()) == String::from(js_sys::Date::new_0().to_date_string()) {
        hm
    } else {
        format!("{}월 {}일", d.get_month() + 1, d.get_date())
    }
}

pub fn render() {
    let (Some(list), Some(pinned), Some(empty)) = (by_id("chat-list"), by_id("pinned"), by_id("chat-empty")) else { return };
    let q = dom::input_value("chat-search").trim().to_string();
    let only = ONLY_UNREAD.get();
    let chats = with_chats(|c| c.clone());
    let pins: Vec<&Chat> = if !q.is_empty() || only { Vec::new() } else { chats.iter().filter(|c| c.pin).collect() };
    pinned.set_hidden(pins.is_empty());
    pinned.set_inner_html(
        &pins
            .iter()
            .map(|c| format!(r#"<button class="pin{}" type="button" data-id="{}">{}{}{}</button>"#, if c.unread { " is-unread" } else { "" }, esc(&c.id), if c.unread { r#"<i class="dot"></i>"# } else { "" }, avatar_html(&c.name, &c.photo, "avatar contact"), esc(&c.name)))
            .collect::<String>(),
    );
    let shown: Vec<&Chat> = chats.iter().filter(|c| !pins.iter().any(|p| p.id == c.id) && (c.name.contains(&q) || c.last.contains(&q)) && (!only || c.unread)).collect();
    list.set_inner_html(&shown.iter().map(|c| {
        format!(
            r#"<li><div class="chat-row" role="button" tabindex="0" data-id="{}">{}{}<div class="chat-main"><div class="chat-top"><b>{}</b><span class="meta">{}{}{CHEV}</span></div><p class="chat-prev">{}</p></div><button class="del" type="button" data-del="{}">삭제</button></div></li>"#,
            esc(&c.id),
            if c.unread { r#"<i class="dot"></i>"# } else { "" },
            avatar_html(&c.name, &c.photo, "avatar contact"),
            esc(&c.name),
            if c.muted { r#"<svg class="mute-ico" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M6 16V11a6 6 0 0 1 9-5.2M18 11v5l2 2H5M4 4l16 16"/></svg>"# } else { "" },
            c.time,
            esc(&c.last),
            esc(&c.id)
        )
    }).collect::<String>());
    let g = js_sys::global();
    let flag = |k: &str| Reflect::get(&g, &JsValue::from_str(k)).map(|v| v.is_truthy()).unwrap_or(false);
    let t = if !chats.is_empty() {
        "결과가 없어요"
    } else if live() {
        "메시지가 없어요. 위쪽 버튼으로 대화를 시작해 보세요"
    } else if cloud::signed_in() || dom::store_get("auth-hint", "null") != "null" {
        if flag("cloudDown") {
            "서버 데이터베이스가 꺼져 있어서 대화를 못 불러왔어요"
        } else if flag("__booted") {
            "서버에 연결하지 못했어요. 새로고침해 주세요"
        } else {
            "불러오는 중이에요"
        }
    } else {
        "로그인하면 대화를 시작할 수 있어요"
    };
    empty.set_text_content(Some(t));
    empty.set_hidden(!shown.is_empty() || !pins.is_empty());
    dom::set_text("stat-chats", &chats.len().to_string());
}

/* ---------- 서버에서 대화 목록 듣기 (로그인한 뒤) ---------- */

/// 로그인한 뒤 부르면 대화 목록을 실시간으로 들어요
pub fn start() {
    LIVE.set(true);
    with_chats(|c| c.clear());
    render();
    let cb = Closure::<dyn FnMut(JsValue)>::new(on_chats);
    let _ = cloud::call_now("watchChats", &[cb.into_js_value()]);
}
fn peek(uid: &str) -> JsValue {
    cloud::call_now("peek", &[JsValue::from_str(uid)]).unwrap_or(JsValue::UNDEFINED)
}
fn on_chats(docs: JsValue) {
    let me = uid();
    let open = open_id();
    let old: HashMap<String, Chat> = with_chats(|cs| cs.iter().map(|c| (c.id.clone(), c.clone())).collect());
    let mut missing: Vec<String> = Vec::new();
    let mut next: Vec<Chat> = Vec::new();
    for d in js_sys::Array::from(&docs).iter() {
        let g = |k| cloud::get(&d, k);
        let id = g("id").as_string().unwrap_or_default();
        let last = g("last").as_string().unwrap_or_default();
        // 아무도 글을 보내지 않은 빈 대화는 목록에 안 보여요
        if pref(&id, "hidden") || last.is_empty() {
            continue;
        }
        let group = g("group").as_bool() == Some(true);
        let members: Vec<String> = js_sys::Array::from(&g("members")).iter().filter_map(|m| m.as_string()).collect();
        let other = if group { id.clone() } else { members.iter().find(|u| **u != me).cloned().unwrap_or_default() };
        let p = if group { JsValue::UNDEFINED } else { peek(&other) };
        let has_p = !(p.is_null() || p.is_undefined());
        if !group && !has_p {
            missing.push(other.clone());
        }
        let o = old.get(&id);
        let last_at = g("lastAt").as_f64().unwrap_or(0.0);
        let last_from = g("lastFrom").as_string().unwrap_or_default();
        let read = cloud::get(&g("reads"), &me).as_f64().unwrap_or(0.0);
        let pname = cloud::get(&p, "name").as_string().filter(|s| !s.is_empty()).or_else(|| cloud::get(&p, "id").as_string());
        let name = if group {
            g("name").as_string().filter(|s| !s.is_empty()).unwrap_or_else(|| "단톡방".into())
        } else if has_p {
            pname.unwrap_or_default()
        } else {
            o.map(|o| o.name.clone()).unwrap_or_else(|| "…".into())
        };
        let photo = if group {
            String::new()
        } else if has_p {
            cloud::get(&p, "photo").as_string().unwrap_or_default()
        } else {
            o.map(|o| o.photo.clone()).unwrap_or_default()
        };
        let mut c = o.cloned().unwrap_or_default();
        c.id = id.clone();
        c.other = other;
        c.group = group;
        c.members = members;
        c.live = true;
        c.name = name;
        c.photo = photo;
        c.last = last;
        c.time = fmt_time(last_at);
        c.unread = !last_from.is_empty() && last_from != me && read < last_at && open.as_deref() != Some(&id);
        c.last_from = last_from;
        c.last_at = last_at;
        c.pin = pref(&id, "pin");
        c.muted = pref(&id, "muted");
        next.push(c);
    }
    next.sort_by(|a, b| b.last_at.partial_cmp(&a.last_at).unwrap_or(std::cmp::Ordering::Equal));
    // 방금 만든 대화(아직 글이 없는 것)가 목록에서 사라지지 않게 유지해요
    if let Some(oid) = &open {
        if !next.iter().any(|c| &c.id == oid) {
            if let Some(o) = old.get(oid).filter(|o| o.live) {
                next.push(o.clone());
            }
        }
    }
    // 열린 대화는 듣고 있는 메시지를 그대로 두고, 목록 쪽 값만 맞춰요
    if let Some(oid) = &open {
        if let Some(n) = next.iter_mut().find(|c| &c.id == oid) {
            if let Some(o) = open_chat() {
                n.msgs = o.msgs;
                n.older = o.older;
                n.more = o.more;
                n.older_loaded = o.older_loaded;
            }
            set_open(Some(n.clone()));
        }
    }
    with_chats(|cs| *cs = next);
    render();
    // 사람 정보가 없는 대화는 한 번에 모아서 읽어 와요 (30분 동안 기억해서 다시 읽지 않아요)
    if !missing.is_empty() {
        let arr: js_sys::Array = missing.iter().map(|s| JsValue::from_str(s)).collect();
        leptos::task::spawn_local(async move {
            if let Ok(n) = cloud::call("fetchProfiles", &[arr.into()]).await {
                if n.as_f64().unwrap_or(0.0) > 0.0 || n.is_truthy() {
                    refresh_names();
                }
            }
        });
    }
}
/// 캐시에 사람 정보가 들어오면 이름과 사진을 다시 채워요
pub fn refresh_names() {
    with_chats(|cs| {
        for c in cs.iter_mut() {
            let p = peek(&c.other);
            if !(p.is_null() || p.is_undefined()) {
                c.name = cloud::get(&p, "name").as_string().filter(|s| !s.is_empty()).or_else(|| cloud::get(&p, "id").as_string()).unwrap_or_default();
                c.photo = cloud::get(&p, "photo").as_string().unwrap_or_default();
            }
        }
    });
    render();
}

/* ---------- 보내기 줄 세우기: 서버 규칙이 0.6초보다 빨리 연달아 보내는 걸 막아서, 0.7초 간격으로 보내요 ---------- */

pub(crate) async fn queue_send(job: impl FnOnce() -> Result<JsValue, cloud::CloudError> + 'static) -> Result<JsValue, JsValue> {
    let prev = SEND_CHAIN.with(|c| c.borrow_mut().take());
    let run = future_to_promise(async move {
        if let Some(p) = prev {
            let _ = JsFuture::from(p).await;
        }
        let wait = 700.0 - (js_sys::Date::now() - LAST_SEND.get());
        if wait > 0.0 {
            dom::sleep(wait as i32).await;
        }
        let out = match job() {
            Ok(v) => match v.dyn_into::<Promise>() {
                Ok(p) => JsFuture::from(p).await,
                Err(v) => Ok(v),
            },
            Err(e) => Err(JsValue::from_str(&e.message)),
        };
        LAST_SEND.set(js_sys::Date::now());
        out
    });
    let r2 = run.clone();
    let settled = future_to_promise(async move {
        let _ = JsFuture::from(r2).await;
        Ok(JsValue::UNDEFINED)
    });
    SEND_CHAIN.with(|c| *c.borrow_mut() = Some(settled));
    JsFuture::from(run).await
}
/// 대화방에 카드(사진·그림·투표·요청)나 글을 보내요
pub(crate) async fn send_payload(c: &Chat, payload: JsValue) -> Result<JsValue, JsValue> {
    let (id, other) = (c.id.clone(), c.other.clone());
    let pv = thread::preview(&payload);
    queue_send(move || cloud::call_now("send", &[JsValue::from_str(&id), JsValue::from_str(&other), payload, JsValue::from_str(&pv)])).await
}

/* ---------- 시작 ---------- */

pub fn init(c: Ctx) {
    CTX.with(|x| *x.borrow_mut() = c);
    render();
    // 메시지 목록과 대화 창: 맨 아래보다 더 내리면 막히지 않고 부드럽게 돌아와요
    if let Some(sc) = crate::shell::screen("messages") {
        crate::bounce::attach(&sc);
    }
    if let Some(b) = by_id("bubbles") {
        crate::bounce::attach(&b);
    }
    if let Some(s) = by_id("chat-search") {
        dom::on(&s, "input", |_| render());
    }
    if let Some(list) = by_id("chat-list") {
        let l2 = list.clone();
        dom::on(&list, "click", move |e| {
            if let Some(del) = dom::closest(&e, "[data-del]") {
                let id = dom::data(&del, "del");
                with_chats(|cs| cs.retain(|c| c.id != id));
                render();
                return;
            }
            if let Some(row) = dom::closest(&e, ".chat-row") {
                if !l2.class_list().contains("editing") {
                    thread::open(&dom::data(&row, "id"));
                }
            }
        });
    }
    if let Some(p) = by_id("pinned") {
        dom::on(&p, "click", |e| {
            if let Some(pin) = dom::closest(&e, ".pin") {
                thread::open(&dom::data(&pin, "id"));
            }
        });
    }
    if let Some(ed) = by_id("edit-chats") {
        let e2 = ed.clone();
        dom::on(&ed, "click", move |_| {
            let Some(list) = by_id("chat-list") else { return };
            let on = list.class_list().toggle("editing").unwrap_or(false);
            e2.set_text_content(Some(if on { "완료" } else { "편집" }));
        });
    }
    if let Some(f) = by_id("filter-unread") {
        let f2 = f.clone();
        dom::on(&f, "click", move |_| {
            ONLY_UNREAD.set(!ONLY_UNREAD.get());
            let _ = f2.set_attribute("aria-pressed", &ONLY_UNREAD.get().to_string());
            render();
        });
    }
    menu::init();
    thread::init();
    people::init();
    attach::init();
    gift::init();
    poll::init();
    draw::init();
}

/// 다른 탭으로 가면 열린 대화를 닫아요
pub fn leave(tab: &str) {
    if tab != "messages" && open_chat().is_some() {
        thread::close();
    }
}
