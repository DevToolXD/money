//! 토큰 선물과 요청: 검은 시트. + − 는 10씩, 큰 + − 는 100씩 (꾹 누르면 계속), 숫자를 누르면 키패드로 직접 입력해요

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::{ctx, queue_send, sheet_open, toast};
use crate::dom::{self, anim, anim_always, by_id, on_finish};

#[derive(Clone, Default)]
struct Gift {
    cid: String,
    to: String,
    name: String,
    amt: f64,
}
thread_local! {
    static G: RefCell<Gift> = RefCell::new(Gift { amt: 100.0, ..Default::default() });
    static SHOWN: Cell<Option<f64>> = const { Cell::new(None) };
    static FRESH: Cell<bool> = const { Cell::new(false) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
}
fn ko(v: f64) -> String {
    cloud::fmt_num(v, 3)
}
fn amt() -> f64 {
    G.with(|g| g.borrow().amt)
}
fn set_amt(v: f64) {
    G.with(|g| g.borrow_mut().amt = v);
}

/// 숫자가 바뀌면 옛 숫자가 잔상을 남기며 위(늘 때)나 아래(줄 때)로 흩어지고, 새 숫자가 반대쪽에서 들어와요
fn roll(dir: f64, old: &str) {
    if dom::reduce_motion() {
        return;
    }
    let Some(num) = by_id("g-num") else { return };
    let Some(holder) = num.parent_element() else { return };
    let ghosts = dom::qsa(&holder, ".gnum-ghost");
    if ghosts.len() > 6 {
        for g in &ghosts {
            g.remove();
        }
    }
    for (i, alpha) in [0.5, 0.26].iter().enumerate() {
        let Ok(g) = dom::doc().create_element("span") else { continue };
        g.set_class_name("gnum-ghost");
        g.set_text_content(Some(old));
        let _ = g.set_attribute("aria-hidden", "true");
        let _ = holder.append_child(&g);
        let a = anim_always(
            &g,
            &format!(r#"[{{"opacity":{alpha},"transform":"translateY(0)","filter":"blur(0px)"}},{{"opacity":0,"transform":"translateY({}px)","filter":"blur({}px)"}}]"#, -dir * (22.0 + i as f64 * 12.0), 3 + i * 2),
            &format!(r#"{{"duration":{},"delay":{},"easing":"cubic-bezier(0.22, 1, 0.36, 1)","fill":"both"}}"#, 340 + i * 90, i * 40),
        );
        on_finish(a, move || g.remove());
    }
    anim(&num, &format!(r#"[{{"opacity":0.25,"transform":"translateY({}px)","filter":"blur(3px)"}},{{"opacity":1,"transform":"translateY(0)","filter":"blur(0px)"}}]"#, dir * 16.0), r#"{"duration":300,"easing":"cubic-bezier(0.22, 1, 0.36, 1)"}"#);
}
fn show_amt(animate: bool) {
    let Some(num) = by_id("g-num") else { return };
    let old = num.text_content().unwrap_or_default();
    let a = amt();
    num.set_text_content(Some(&ko(a)));
    if let Some(p) = num.parent_element().and_then(|p| p.dyn_into::<web_sys::HtmlElement>().ok()) {
        let size = if a >= 1e7 {
            "46px"
        } else if a >= 1e6 {
            "58px"
        } else if a >= 1e5 {
            "68px"
        } else {
            "80px"
        };
        let _ = p.style().set_property("font-size", size);
    }
    dom::set_text("gift-bal", &format!("내 토큰 {}", crate::money::w(crate::money::balance().unwrap_or(0.0))));
    if let Some(s) = SHOWN.get() {
        if animate && s != a {
            roll(if a > s { 1.0 } else { -1.0 }, &old);
        }
    }
    SHOWN.set(Some(a));
}

/// 선물 시트 열기 (cid: 대화, to: 받는 사람 uid)
pub fn open(cid: &str, to: &str, name: &str) {
    G.with(|g| {
        let mut g = g.borrow_mut();
        let amt = if g.amt > 0.0 { g.amt } else { 100.0 };
        *g = Gift { cid: cid.into(), to: to.into(), name: name.into(), amt };
    });
    dom::set_text("gift-to", &format!("{name}님에게"));
    dom::set_hidden("keypad", true);
    FRESH.set(true);
    dom::set_text("g-keytoggle", "키패드 보기");
    show_amt(false);
    sheet_open(true, "gift-sheet");
}
fn step(dir: f64, size: f64) {
    set_amt((amt() + dir * size).clamp(0.0, 99999999.0));
    show_amt(true);
    dom::vibrate(6);
}
fn toggle_keys(on: bool) {
    dom::set_hidden("keypad", !on);
    dom::set_text("g-keytoggle", if on { "키패드 숨기기" } else { "키패드 보기" });
}
fn keypad_hidden() -> bool {
    by_id("keypad").map(|k| k.hidden()).unwrap_or(true)
}

async fn send() {
    if BUSY.get() {
        return;
    }
    let g = G.with(|g| g.borrow().clone());
    if g.amt < 1.0 {
        return toast("보낼 토큰을 정해 주세요");
    }
    if let Err(m) = crate::money::ensure(g.amt).await {
        ctx().ask("토큰을 보낼 수 없어요", &m, "확인", "").await;
        return;
    }
    BUSY.set(true);
    let (cid, to, amount) = (g.cid.clone(), g.to.clone(), g.amt);
    let r = queue_send(move || {
        let f = Closure::once_into_js(move || cloud::call_now("sendGift", &[JsValue::from_str(&cid), JsValue::from_str(&to), amount.into()]).unwrap_or(JsValue::UNDEFINED));
        Ok(crate::money::wallet_op(f.unchecked_into()).into())
    })
    .await;
    match r {
        Ok(_) => {
            crate::money::log(&dom::obj(&[("t", js_sys::Date::now().into()), ("name", JsValue::from_str(&format!("{}님에게 선물", g.name))), ("cat", "송금".into()), ("icon", "gift".into()), ("amt", (-g.amt).into())]));
            sheet_open(false, "");
            toast(&format!("{}님에게 토큰 {}을 보냈어요", g.name, ko(g.amt)));
        }
        Err(_) => toast("보내지 못했어요. 잠시 뒤 다시 해 주세요"),
    }
    BUSY.set(false);
}
async fn request() {
    if BUSY.get() {
        return;
    }
    let g = G.with(|g| g.borrow().clone());
    if g.amt < 1.0 {
        return toast("요청할 토큰을 정해 주세요");
    }
    BUSY.set(true);
    let (cid, to, amount) = (g.cid.clone(), g.to.clone(), g.amt);
    let r = queue_send(move || {
        let payload = dom::obj(&[("type", "request".into()), ("amount", amount.into()), ("done", false.into())]);
        cloud::call_now("send", &[JsValue::from_str(&cid), JsValue::from_str(&to), payload, JsValue::from_str(&format!("토큰 {} 요청", ko(amount)))])
    })
    .await;
    match r {
        Ok(_) => {
            sheet_open(false, "");
            toast(&format!("{}님에게 토큰 {}을 요청했어요", g.name, ko(g.amt)));
        }
        Err(_) => toast("요청하지 못했어요"),
    }
    BUSY.set(false);
}

pub fn init() {
    for (id, dir, size) in [("g-minus", -1.0, 10.0), ("g-plus", 1.0, 10.0), ("g-minus100", -1.0, 100.0), ("g-plus100", 1.0, 100.0)] {
        let Some(btn) = by_id(id) else { continue };
        let hold: std::rc::Rc<Cell<(i32, i32)>> = Default::default();
        let h2 = hold.clone();
        dom::on(&btn, "pointerdown", move |_| {
            step(dir, size);
            let w = web_sys::window().expect("window");
            let h3 = h2.clone();
            let rep = Closure::once_into_js(move || {
                let tick = Closure::<dyn FnMut()>::new(move || step(dir, size));
                let w = web_sys::window().expect("window");
                let iv = w.set_interval_with_callback_and_timeout_and_arguments_0(tick.as_ref().unchecked_ref(), 110).unwrap_or(0);
                tick.forget();
                h3.set((h3.get().0, iv));
            });
            let t = w.set_timeout_with_callback_and_timeout_and_arguments_0(rep.unchecked_ref(), 380).unwrap_or(0);
            h2.set((t, 0));
        });
        for t in ["pointerup", "pointerleave", "pointercancel"] {
            let h = hold.clone();
            dom::on(&btn, t, move |_| {
                let (a, b) = h.get();
                if let Some(w) = web_sys::window() {
                    w.clear_timeout_with_handle(a);
                    w.clear_interval_with_handle(b);
                }
                h.set((0, 0));
            });
        }
    }
    for id in ["g-amt", "g-keytoggle"] {
        if let Some(b) = by_id(id) {
            dom::on(&b, "click", |_| toggle_keys(keypad_hidden()));
        }
    }
    if let Some(k) = by_id("keypad") {
        dom::on(&k, "click", |e| {
            let Some(b) = dom::closest(&e, "[data-k]") else { return };
            let key = dom::data(&b, "k");
            let mut s = if FRESH.get() { String::new() } else { goods::shapes::num(amt()) };
            FRESH.set(false);
            s = if key == "back" {
                s.chars().take(s.chars().count().saturating_sub(1)).collect()
            } else if s == "0" {
                key.trim_start_matches('0').to_string()
            } else {
                s + &key
            };
            set_amt(dom::js_number(if s.is_empty() { "0" } else { &s }).min(99999999.0));
            show_amt(true);
            dom::vibrate(5);
        });
    }
    if let Some(b) = by_id("g-send") {
        dom::on(&b, "click", |_| leptos::task::spawn_local(send()));
    }
    if let Some(b) = by_id("g-request") {
        dom::on(&b, "click", |_| leptos::task::spawn_local(request()));
    }
}
