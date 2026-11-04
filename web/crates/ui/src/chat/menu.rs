//! 꾹 눌러서 나오는 메뉴: 대화(미리보기 카드 + 큰 메뉴)와 말풍선(복사)

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::{find, open_chat, render, thread, touch_pref, uid, with_chats};
use crate::dom::{self, by_id, esc};

type Run = Rc<dyn Fn()>;
struct Item {
    icon: &'static str,
    label: String,
    danger: bool,
    run: Run,
}
fn icon(k: &str) -> &'static str {
    match k {
        "pin" => r#"<path d="M9 3h6l-1 6 3 3v2H7v-2l3-3zM12 14v7"/>"#,
        "unread" => r#"<path d="M20 12a8 7 0 0 1-12.5 5.8L4 19l1.2-3.4A8 7 0 1 1 20 12z"/><circle cx="19" cy="5" r="2.2" fill="currentColor"/>"#,
        "mute" => r#"<path d="M6 16V11a6 6 0 0 1 9-5.2M18 11v5l2 2H5M10 21h4M4 4l16 16"/>"#,
        "trash" => r#"<path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v6M14 11v6"/>"#,
        _ => r#"<rect x="8" y="8" width="12" height="12" rx="3"/><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2"/>"#,
    }
}
fn item_html(it: &Item, i: usize) -> String {
    format!(
        r#"<button type="button" role="menuitem" data-k="{i}"{}><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round">{}</svg><span>{}</span></button>"#,
        if it.danger { r#" class="danger""# } else { "" },
        icon(it.icon),
        esc(&it.label)
    )
}

thread_local! {
    static ITEMS: RefCell<Vec<Run>> = const { RefCell::new(Vec::new()) };
    static SWALLOW: Cell<bool> = const { Cell::new(false) };
}
fn back() -> Option<web_sys::HtmlElement> {
    by_id("ctx-back")
}
fn hide() {
    if let Some(b) = back() {
        let _ = b.class_list().remove_2("is-open", "mode-stack");
    }
}
fn run_item(e: &web_sys::Event) {
    let Some(btn) = dom::closest(e, "button") else { return };
    hide();
    let k: usize = dom::data(&btn, "k").parse().unwrap_or(usize::MAX);
    let f = ITEMS.with(|i| i.borrow().get(k).cloned());
    if let Some(f) = f {
        f();
    }
}

/// 대화를 꾹 누르면: 대화 미리보기 카드와 큰 메뉴
fn show_chat_menu(id: &str, items: Vec<Item>) {
    let (Some(pv), Some(m2), Some(bk)) = (by_id("ctx-preview"), by_id("ctx-menu2"), back()) else { return };
    let Some(c) = find(id) else { return };
    let _ = pv.set_attribute("data-wall", &thread::wall_of(&c.id));
    // 미리보기는 마지막 글을 먼저 보여주고, 최근 9개를 한 번만 읽어 와요 (꾹 눌렀을 때만 읽기가 생겨요)
    let draw = {
        let c = c.clone();
        let pv = pv.clone();
        move |list: Vec<JsValue>| {
            let mut tmp = super::Chat { name: c.name.clone(), photo: c.photo.clone(), ..Default::default() };
            tmp.msgs = list.into_iter().map(|m| thread::msg_of(m, false, &c.name)).collect();
            let rows: String = (0..tmp.msgs.len()).map(|i| thread::row_of(&tmp, i, -1).html).collect();
            pv.set_inner_html(&format!(r#"<div class="pv">{rows}</div>"#));
        }
    };
    draw(if c.last.is_empty() { Vec::new() } else { vec![dom::obj(&[("id", "p0".into()), ("type", "text".into()), ("text", JsValue::from_str(&c.last)), ("from", JsValue::from_str(&c.last_from))])] });
    let cid = c.id.clone();
    let bk2 = bk.clone();
    leptos::task::spawn_local(async move {
        if let Ok(r) = cloud::call("loadOlder", &[JsValue::from_str(&cid), (js_sys::Date::now() + 60000.0).into(), 9.into()]).await {
            if bk2.class_list().contains("mode-stack") {
                draw(js_sys::Array::from(&cloud::get(&r, "list")).iter().collect());
            }
        }
    });
    let open_id = c.id.clone();
    let f = Closure::<dyn FnMut()>::new(move || {
        hide();
        thread::open(&open_id);
    });
    pv.set_onclick(Some(f.as_ref().unchecked_ref()));
    f.forget();
    m2.set_inner_html(&items.iter().enumerate().map(|(i, it)| item_html(it, i)).collect::<String>());
    ITEMS.with(|x| *x.borrow_mut() = items.into_iter().map(|i| i.run).collect());
    let _ = bk.class_list().add_2("is-open", "mode-stack");
}
fn show_menu(x: f64, y: f64, items: Vec<Item>) {
    let (Some(menu), Some(bk), Some(app)) = (by_id("ctx-menu"), back(), dom::app_el()) else { return };
    menu.set_inner_html(&items.iter().enumerate().map(|(i, it)| item_html(it, i)).collect::<String>());
    ITEMS.with(|x| *x.borrow_mut() = items.into_iter().map(|i| i.run).collect());
    let _ = bk.class_list().add_1("is-open");
    let r = app.get_bounding_client_rect();
    let w = 220.0;
    let h = ITEMS.with(|i| i.borrow().len()) as f64 * 48.0 + 12.0;
    let left = (x - r.left() - w / 2.0).clamp(12.0, (r.width() - w - 12.0).max(12.0));
    let top = (y - r.top() + 8.0).clamp(12.0, (r.height() - h - 12.0).max(12.0));
    let st = menu.style();
    let _ = st.set_property("left", &format!("{left}px"));
    let _ = st.set_property("top", &format!("{top}px"));
    let _ = st.set_property("--ox", &format!("{}px", x - r.left() - left));
}

fn chat_items(id: &str) -> Vec<Item> {
    let Some(c) = find(id) else { return Vec::new() };
    let upd = |id: String, f: fn(&mut super::Chat)| -> Run {
        Rc::new(move || {
            let changed = with_chats(|cs| {
                let c = cs.iter_mut().find(|c| c.id == id)?;
                f(c);
                Some(c.clone())
            });
            if let Some(c) = changed {
                touch_pref(&c);
            }
            render();
        })
    };
    let id = c.id.clone();
    let del_id = c.id.clone();
    vec![
        Item { icon: "pin", label: if c.pin { "고정 해제" } else { "고정" }.into(), danger: false, run: upd(id.clone(), |c| c.pin = !c.pin) },
        Item { icon: "unread", label: if c.unread { "읽음으로 표시" } else { "읽지 않음으로 표시" }.into(), danger: false, run: upd(id.clone(), |c| c.unread = !c.unread) },
        Item { icon: "mute", label: if c.muted { "알림 켜기" } else { "알림 가리기" }.into(), danger: false, run: upd(id, |c| c.muted = !c.muted) },
        Item {
            icon: "trash",
            label: "삭제".into(),
            danger: true,
            run: Rc::new(move || {
                let gone = with_chats(|cs| {
                    let i = cs.iter().position(|c| c.id == del_id)?;
                    let mut c = cs.remove(i);
                    c.hidden = true;
                    Some(c)
                });
                if let Some(c) = gone {
                    touch_pref(&c);
                }
                render();
            }),
        },
    ]
}
fn bubble_items(el: &web_sys::HtmlElement) -> Vec<Item> {
    let i: usize = dom::data(el, "i").parse().unwrap_or(0);
    let t = open_chat().and_then(|c| c.msgs.get(i).map(|m| m.text.clone())).unwrap_or_default();
    vec![Item {
        icon: "copy",
        label: "복사".into(),
        danger: false,
        run: Rc::new(move || {
            if let Some(w) = web_sys::window() {
                let cb = w.navigator().clipboard();
                let _ = cb.write_text(&t);
            }
        }),
    }]
}

/// 꾹 누르기(450ms)와 오른쪽 클릭으로 메뉴를 열어요
fn long_press(root: &web_sys::HtmlElement, sel: &'static str, items: fn(&web_sys::HtmlElement) -> Vec<Item>) {
    let fire = move |el: web_sys::HtmlElement, x: f64, y: f64| {
        SWALLOW.set(true);
        dom::vibrate(12);
        let built = items(&el);
        let id = dom::data(&el, "id");
        if id.is_empty() {
            show_menu(x, y, built);
        } else {
            show_chat_menu(&id, built);
        }
    };
    let r2 = root.clone();
    dom::on(root, "pointerdown", move |e| {
        let Some(pe) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
        let Some(el) = dom::closest(&e, sel) else { return };
        if pe.pointer_type() == "mouse" && pe.button() != 0 {
            return;
        }
        let (sx, sy) = (pe.client_x() as f64, pe.client_y() as f64);
        let el2 = el.clone();
        let cb = Closure::once_into_js(move || fire(el2, sx, sy));
        let w = web_sys::window().expect("window");
        let timer = w.set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 450).unwrap_or(0);
        // 움직이거나 손을 떼면 취소해요 (한 번만 들어요)
        let holder: Rc<RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>>> = Rc::new(RefCell::new(None));
        let (h2, r3) = (holder.clone(), r2.clone());
        let cancel = Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
            if ev.type_() == "pointermove" {
                if let Some(p) = ev.dyn_ref::<web_sys::PointerEvent>() {
                    if (p.client_x() as f64 - sx).hypot(p.client_y() as f64 - sy) < 10.0 {
                        return;
                    }
                }
            }
            if let Some(w) = web_sys::window() {
                w.clear_timeout_with_handle(timer);
            }
            if let Some(c) = h2.borrow_mut().take() {
                for t in ["pointermove", "pointerup", "pointercancel"] {
                    let _ = r3.remove_event_listener_with_callback(t, c.as_ref().unchecked_ref());
                }
                // 지금 부르고 있는 함수라 바로 지우지 않고 잠시 뒤에 놓아줘요
                leptos::prelude::set_timeout(move || drop(c), std::time::Duration::from_millis(0));
            }
        });
        for t in ["pointermove", "pointerup", "pointercancel"] {
            let _ = r2.add_event_listener_with_callback(t, cancel.as_ref().unchecked_ref());
        }
        *holder.borrow_mut() = Some(cancel);
    });
    dom::on(root, "contextmenu", move |e| {
        let Some(el) = dom::closest(&e, sel) else { return };
        e.prevent_default();
        let (x, y) = e.dyn_ref::<web_sys::MouseEvent>().map(|m| (m.client_x() as f64, m.client_y() as f64)).unwrap_or((0.0, 0.0));
        fire(el, x, y);
    });
}

pub fn init() {
    if let Some(bk) = back() {
        let bk2 = bk.clone();
        dom::on(&bk, "pointerdown", move |e| {
            let t = e.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok());
            if t.as_ref().is_some_and(|t| t == bk2.unchecked_ref::<web_sys::Element>() || t.id() == "ctx-stack") {
                hide();
            }
        });
    }
    for id in ["ctx-menu", "ctx-menu2"] {
        if let Some(m) = by_id(id) {
            dom::on(&m, "click", |e| run_item(&e));
        }
    }
    // 꾹 누른 뒤의 클릭은 삼켜요 (메뉴를 연 손가락이 아래 칸을 누르지 않게)
    let doc = dom::doc();
    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_capture(true);
    let down = Closure::<dyn FnMut()>::new(|| SWALLOW.set(false));
    let _ = doc.add_event_listener_with_callback_and_add_event_listener_options("pointerdown", down.as_ref().unchecked_ref(), &opts);
    down.forget();
    let click = Closure::<dyn FnMut(web_sys::Event)>::new(|e: web_sys::Event| {
        if !SWALLOW.get() {
            return;
        }
        SWALLOW.set(false);
        e.stop_propagation();
        e.prevent_default();
    });
    let _ = doc.add_event_listener_with_callback_and_add_event_listener_options("click", click.as_ref().unchecked_ref(), &opts);
    click.forget();
    if let Some(l) = by_id("chat-list") {
        long_press(&l, ".chat-row", |el| chat_items(&dom::data(el, "id")));
    }
    if let Some(p) = by_id("pinned") {
        long_press(&p, ".pin", |el| chat_items(&dom::data(el, "id")));
    }
    if let Some(b) = by_id("bubbles") {
        long_press(&b, ".bubble", bubble_items);
    }
    let _ = uid;
}
