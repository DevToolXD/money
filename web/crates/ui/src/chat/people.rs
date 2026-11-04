//! 새 대화: 사람 고르기 (20명씩 이어서, 이름·아이디로 찾기), 단톡방 만들기, 단톡방에 초대, 토큰 보낼 사람 고르기

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::{live, render, sheet_open, thread, toast, uid, with_chats, Chat};
use crate::dom::{self, avatar_html, by_id, esc};

#[derive(Clone)]
struct Person {
    uid: String,
    id: String,
    name: String,
    photo: String,
}
impl Person {
    fn from_js(v: &JsValue) -> Person {
        let g = |k| cloud::get(v, k).as_string().unwrap_or_default();
        Person { uid: g("uid"), id: g("id"), name: g("name"), photo: g("photo") }
    }
    fn label(&self) -> &str {
        if self.name.is_empty() {
            &self.id
        } else {
            &self.name
        }
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Chat,
    Gift,
    Invite,
}
struct Multi {
    invite: bool,
    sel: BTreeSet<String>,
    chat_id: String,
    members: Vec<String>,
}
thread_local! {
    static PEOPLE: RefCell<Vec<Person>> = const { RefCell::new(Vec::new()) };
    static MORE: Cell<bool> = const { Cell::new(false) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
    static MODE: Cell<Mode> = const { Cell::new(Mode::Chat) };
    static MULTI: RefCell<Option<Multi>> = const { RefCell::new(None) };
    static SEARCH_SEQ: Cell<u32> = const { Cell::new(0) };
}

fn q() -> String {
    dom::input_value("new-q").trim().to_string()
}
fn row_html(u: &Person, i: usize) -> String {
    format!(
        r#"<li><button class="res-row" type="button" data-i="{i}">{}<span class="res-main"><b>{}</b><span>@{}</span></span></button></li>"#,
        avatar_html(u.label(), &u.photo, "avatar contact"),
        esc(u.label()),
        esc(&u.id)
    )
}
fn empty(t: &str) {
    if let Some(e) = by_id("new-empty") {
        e.set_hidden(false);
        e.set_text_content(Some(t));
    }
}
/// 사람 목록을 20명씩 이어서 불러와요 (끝까지 내리면 다음 20명)
async fn load_people(reset: bool) {
    if BUSY.get() {
        return;
    }
    BUSY.set(true);
    match cloud::call("listUsers", &[reset.into()]).await {
        Ok(r) => {
            if q().is_empty() {
                let list: Vec<Person> = js_sys::Array::from(&cloud::get(&r, "list")).iter().map(|v| Person::from_js(&v)).collect();
                if reset {
                    PEOPLE.with(|p| p.borrow_mut().clear());
                }
                let start = PEOPLE.with(|p| p.borrow().len());
                MORE.set(cloud::get(&r, "more").is_truthy());
                let html: String = list.iter().enumerate().map(|(i, u)| row_html(u, start + i)).collect();
                PEOPLE.with(|p| p.borrow_mut().extend(list));
                if let Some(res) = by_id("new-results") {
                    let _ = res.insert_adjacent_html(if reset { "afterbegin" } else { "beforeend" }, &html);
                }
                let n = PEOPLE.with(|p| p.borrow().len());
                if let Some(e) = by_id("new-empty") {
                    e.set_hidden(n > 0);
                    e.set_text_content(Some("아직 다른 사람이 없어요"));
                }
                paint_multi();
            }
        }
        Err(_) => empty("목록을 불러오지 못했어요. 잠시 뒤 다시 해 주세요"),
    }
    BUSY.set(false);
}
fn title0() -> &'static str {
    if MODE.get() == Mode::Gift {
        "받는 사람"
    } else {
        "새 메시지"
    }
}
fn paint_multi() {
    let (n, invite, on) = MULTI.with(|m| m.borrow().as_ref().map(|m| (m.sel.len(), m.invite, true)).unwrap_or((0, false, false)));
    if let Some(go) = by_id("new-go").and_then(|g| g.dyn_into::<web_sys::HtmlButtonElement>().ok()) {
        go.set_hidden(!on);
        go.set_disabled(n < 1);
        go.set_text_content(Some(&if invite { format!("초대하기 · {n}명") } else { format!("단톡방 만들기 · {n}명") }));
    }
    dom::set_text("new-multi", if on && !invite { "취소" } else { "단톡방" });
    dom::set_text("new-title", if invite { "사람 초대" } else if on { "단톡방 만들기" } else { title0() });
    let people = PEOPLE.with(|p| p.borrow().clone());
    if let Some(res) = by_id("new-results") {
        for r in dom::qsa(&res, ".res-row") {
            let i: usize = dom::data(&r, "i").parse().unwrap_or(usize::MAX);
            let picked = MULTI.with(|m| m.borrow().as_ref().is_some_and(|m| people.get(i).is_some_and(|p| m.sel.contains(&p.uid))));
            dom::toggle(&r, "picked", picked);
        }
    }
}
fn reset_list() {
    if let Some(i) = by_id("new-q").and_then(|i| i.dyn_into::<web_sys::HtmlInputElement>().ok()) {
        i.set_value("");
    }
    if let Some(r) = by_id("new-results") {
        r.set_inner_html("");
    }
    PEOPLE.with(|p| p.borrow_mut().clear());
    empty("불러오는 중이에요");
}
/// 사람 고르기 시트 ("chat": 대화 열기, "gift": 토큰 보낼 사람)
pub fn open_picker(gift: bool) {
    if !live() {
        if cloud::signed_in() {
            return toast("아직 불러오는 중이에요. 잠시 뒤 다시 눌러 주세요");
        }
        let _ = super::ctx().call("showLogin", &[true.into()]);
        return;
    }
    MODE.set(if gift { Mode::Gift } else { Mode::Chat });
    MULTI.with(|m| *m.borrow_mut() = None);
    dom::set_hidden("new-multi", gift);
    dom::set_text("new-title", title0());
    reset_list();
    sheet_open(true, "new-sheet");
    paint_multi();
    leptos::task::spawn_local(load_people(true));
}
/// 초대: 단톡방 안에서 사람을 골라 더해요 (이미 있는 사람은 고를 수 없어요)
fn open_invite(c: &Chat) {
    MULTI.with(|m| *m.borrow_mut() = Some(Multi { invite: true, sel: BTreeSet::new(), chat_id: c.id.clone(), members: c.members.clone() }));
    MODE.set(Mode::Invite);
    dom::set_hidden("new-multi", true);
    reset_list();
    sheet_open(true, "new-sheet");
    leptos::task::spawn_local(load_people(true));
    paint_multi();
}
async fn ensure_chat(u: &Person) -> Result<Chat, cloud::CloudError> {
    let cid = cloud::call("openChat", &[JsValue::from_str(&u.uid)]).await?.as_string().unwrap_or_default();
    let c = with_chats(|cs| {
        if let Some(c) = cs.iter().find(|x| x.id == cid) {
            return c.clone();
        }
        let c = Chat { id: cid.clone(), other: u.uid.clone(), name: u.label().to_string(), photo: u.photo.clone(), live: true, last_at: js_sys::Date::now(), ..Default::default() };
        cs.insert(0, c.clone());
        c
    });
    Ok(c)
}
async fn go() {
    let Some((invite, uids, chat_id)) = MULTI.with(|m| m.borrow().as_ref().map(|m| (m.invite, m.sel.iter().cloned().collect::<Vec<_>>(), m.chat_id.clone()))) else { return };
    if uids.is_empty() {
        return;
    }
    let go_btn = by_id("new-go").and_then(|g| g.dyn_into::<web_sys::HtmlButtonElement>().ok());
    if let Some(g) = &go_btn {
        g.set_disabled(true);
    }
    let arr: js_sys::Array = uids.iter().map(|s| JsValue::from_str(s)).collect();
    let r: Result<(), cloud::CloudError> = async {
        if invite {
            let members = cloud::call("inviteMembers", &[JsValue::from_str(&chat_id), arr.into()]).await?;
            let ms: Vec<String> = js_sys::Array::from(&members).iter().filter_map(|m| m.as_string()).collect();
            with_chats(|cs| cs.iter_mut().filter(|c| c.id == chat_id).for_each(|c| c.members = ms.clone()));
            super::update_open(|c| {
                if c.id == chat_id {
                    c.members = ms.clone();
                }
            });
            toast(&format!("{}명을 초대했어요", uids.len()));
            MULTI.with(|m| *m.borrow_mut() = None);
            sheet_open(false, "");
            return Ok(());
        }
        let people = PEOPLE.with(|p| p.borrow().clone());
        let names: Vec<String> = uids.iter().map(|u| people.iter().find(|p| &p.uid == u).map(|p| p.label().to_string()).unwrap_or_default()).collect();
        let joined = names.join(", ");
        let name = if joined.chars().count() <= 20 { joined } else { dom::cut(&format!("{} 외 {}명", names[0], names.len() - 1), 20) };
        let cid = cloud::call("createGroup", &[JsValue::from_str(&name), arr.into()]).await?.as_string().unwrap_or_default();
        let mut members = vec![uid()];
        members.extend(uids.iter().cloned());
        with_chats(|cs| cs.insert(0, Chat { id: cid.clone(), other: cid.clone(), group: true, members, name, live: true, last_at: js_sys::Date::now(), ..Default::default() }));
        MULTI.with(|m| *m.borrow_mut() = None);
        sheet_open(false, "");
        render();
        thread::open(&cid);
        Ok(())
    }
    .await;
    if r.is_err() {
        toast("단톡방을 만들지 못했어요. 잠시 뒤 다시 해 주세요");
    }
    paint_multi();
}

pub fn init() {
    if let Some(res) = by_id("new-results") {
        if let Some(sc) = res.parent_element() {
            let sc2 = sc.clone();
            dom::on(&sc, "scroll", move |_| {
                if MORE.get() && q().is_empty() && (sc2.scroll_top() + sc2.client_height()) as f64 > sc2.scroll_height() as f64 - 160.0 {
                    leptos::task::spawn_local(load_people(false));
                }
            });
        }
        dom::on(&res, "click", |e| {
            let Some(row) = dom::closest(&e, ".res-row") else { return };
            let i: usize = dom::data(&row, "i").parse().unwrap_or(usize::MAX);
            let Some(u) = PEOPLE.with(|p| p.borrow().get(i).cloned()) else { return };
            let in_multi = MULTI.with(|m| {
                let mut m = m.borrow_mut();
                let Some(m) = m.as_mut() else { return false };
                if m.invite && m.members.contains(&u.uid) {
                    return true;
                }
                if !m.sel.remove(&u.uid) {
                    m.sel.insert(u.uid.clone());
                }
                true
            });
            if in_multi {
                paint_multi();
                return;
            }
            leptos::task::spawn_local(async move {
                match ensure_chat(&u).await {
                    Ok(c) => {
                        sheet_open(false, "");
                        render();
                        if MODE.get() == Mode::Gift {
                            super::gift::open(&c.id, &c.other, &c.name);
                        } else {
                            thread::open(&c.id);
                        }
                    }
                    Err(_) => empty("상대와 연결하지 못했어요. 잠시 뒤 다시 해 주세요"),
                }
            });
        });
    }
    if let Some(b) = by_id("compose") {
        dom::on(&b, "click", |_| open_picker(false));
    }
    if let Some(b) = by_id("ac-send") {
        dom::on(&b, "click", |_| open_picker(true));
    }
    if let Some(b) = by_id("new-cancel") {
        dom::on(&b, "click", |_| sheet_open(false, ""));
    }
    if let Some(i) = by_id("new-q") {
        dom::on(&i, "input", |_| {
            let seq = SEARCH_SEQ.get() + 1;
            SEARCH_SEQ.set(seq);
            let qq = q();
            if qq.is_empty() {
                if let Some(r) = by_id("new-results") {
                    r.set_inner_html("");
                }
                PEOPLE.with(|p| p.borrow_mut().clear());
                empty("불러오는 중이에요");
                leptos::task::spawn_local(load_people(true));
                return;
            }
            leptos::prelude::set_timeout(
                move || {
                    if SEARCH_SEQ.get() != seq {
                        return;
                    }
                    leptos::task::spawn_local(async move {
                        match cloud::call("searchUsers", &[JsValue::from_str(&qq)]).await {
                            Ok(found) => {
                                if SEARCH_SEQ.get() != seq {
                                    return;
                                }
                                let list: Vec<Person> = js_sys::Array::from(&found).iter().map(|v| Person::from_js(&v)).collect();
                                if let Some(r) = by_id("new-results") {
                                    r.set_inner_html(&list.iter().enumerate().map(|(i, u)| row_html(u, i)).collect::<String>());
                                }
                                let n = list.len();
                                PEOPLE.with(|p| *p.borrow_mut() = list);
                                if let Some(e) = by_id("new-empty") {
                                    e.set_hidden(n > 0);
                                    e.set_text_content(Some("일치하는 사람이 없어요"));
                                }
                                paint_multi();
                            }
                            Err(_) => empty("찾지 못했어요. 잠시 뒤 다시 해 주세요"),
                        }
                    });
                },
                std::time::Duration::from_millis(250),
            );
        });
    }
    if let Some(b) = by_id("new-multi") {
        dom::on(&b, "click", |_| {
            MULTI.with(|m| {
                let mut m = m.borrow_mut();
                *m = if m.is_some() { None } else { Some(Multi { invite: false, sel: BTreeSet::new(), chat_id: String::new(), members: Vec::new() }) };
            });
            paint_multi();
        });
    }
    if let Some(b) = by_id("thread-invite") {
        dom::on(&b, "click", |_| {
            if let Some(c) = super::open_chat().filter(|c| c.group) {
                open_invite(&c);
            }
        });
    }
    if let Some(b) = by_id("new-go") {
        dom::on(&b, "click", |_| leptos::task::spawn_local(go()));
    }
}
