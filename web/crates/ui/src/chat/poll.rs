//! 사진 투표: 질문과 선택지(2~4개, 글이나 사진)를 만들어 보내요

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::{open_chat, send_payload, sheet_open, toast};
use crate::dom::{self, by_id, esc, safe_img};

#[derive(Clone, Default)]
struct Opt {
    t: String,
    img: String,
}
thread_local! {
    static POLL: RefCell<Vec<Opt>> = const { RefCell::new(Vec::new()) };
    static PICK: Cell<usize> = const { Cell::new(0) };
}
const PLUS: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>"#;

fn draw_opts() {
    let opts = POLL.with(|p| p.borrow().clone());
    if let Some(box_) = by_id("poll-opts") {
        box_.set_inner_html(
            &opts
                .iter()
                .enumerate()
                .map(|(i, o)| {
                    let img = safe_img(&o.img, 60000);
                    let has = !o.img.is_empty();
                    format!(
                        r#"<div class="poll-edit"><button class="poll-photo{}" data-p="{i}" type="button" aria-label="사진 고르기" {}>{}</button><input type="text" data-t="{i}" maxlength="30" placeholder="선택지 {}" value="{}"></div>"#,
                        if has { " has" } else { "" },
                        if has { format!(r#"style="background-image:url({img})""#) } else { String::new() },
                        if has { "" } else { PLUS },
                        i + 1,
                        esc(&o.t)
                    )
                })
                .collect::<String>(),
        );
    }
    dom::set_hidden("poll-add", opts.len() >= 4);
}

pub fn open() {
    POLL.with(|p| *p.borrow_mut() = vec![Opt::default(), Opt::default()]);
    if let Some(q) = by_id("poll-q").and_then(|q| q.dyn_into::<web_sys::HtmlInputElement>().ok()) {
        q.set_value("");
    }
    dom::set_text("poll-err", "");
    draw_opts();
    sheet_open(true, "poll-sheet");
}

async fn send() {
    let q = dom::cut(dom::input_value("poll-q").trim(), 60);
    let opts: Vec<Opt> = POLL.with(|p| p.borrow().iter().filter(|o| !o.t.trim().is_empty() || !o.img.is_empty()).cloned().collect());
    if q.is_empty() {
        return dom::set_text("poll-err", "질문을 적어 주세요");
    }
    if opts.len() < 2 {
        return dom::set_text("poll-err", "선택지를 두 개 이상 만들어 주세요");
    }
    let Some(c) = open_chat() else { return };
    let arr: js_sys::Array = opts.iter().map(|o| dom::obj(&[("t", JsValue::from_str(&dom::cut(o.t.trim(), 30))), ("img", JsValue::from_str(&o.img))])).collect();
    let payload = dom::obj(&[("type", "poll".into()), ("poll", dom::obj(&[("q", JsValue::from_str(&q)), ("options", arr.into())])), ("votes", js_sys::Object::new().into())]);
    match send_payload(&c, payload).await {
        Ok(_) => sheet_open(false, ""),
        Err(_) => dom::set_text("poll-err", "보내지 못했어요. 잠시 뒤 다시 해 주세요"),
    }
}

pub fn init() {
    if let Some(o) = by_id("poll-opts") {
        dom::on(&o, "input", |e| {
            let Some(t) = e.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) else { return };
            let Ok(i) = dom::data(&t, "t").parse::<usize>() else { return };
            POLL.with(|p| {
                if let Some(o) = p.borrow_mut().get_mut(i) {
                    o.t = t.value();
                }
            });
        });
        dom::on(&o, "click", |e| {
            let Some(b) = dom::closest(&e, "[data-p]") else { return };
            PICK.set(dom::data(&b, "p").parse().unwrap_or(0));
            if let Some(f) = by_id("file-poll") {
                f.click();
            }
        });
    }
    if let Some(f) = by_id("file-poll") {
        dom::on(&f, "change", |_| {
            let Some(file) = super::attach::take_file("file-poll") else { return };
            leptos::task::spawn_local(async move {
                match super::attach::read_image(file, 320.0, 45000, 0.62, false).await {
                    Ok(img) => {
                        let i = PICK.get();
                        POLL.with(|p| {
                            if let Some(o) = p.borrow_mut().get_mut(i) {
                                o.img = img;
                            }
                        });
                        draw_opts();
                    }
                    Err(_) => toast("사진을 열지 못했어요"),
                }
            });
        });
    }
    if let Some(b) = by_id("poll-add") {
        dom::on(&b, "click", |_| {
            if POLL.with(|p| p.borrow().len()) < 4 {
                POLL.with(|p| p.borrow_mut().push(Opt::default()));
                draw_opts();
            }
        });
    }
    if let Some(b) = by_id("poll-cancel") {
        dom::on(&b, "click", |_| sheet_open(false, ""));
    }
    if let Some(b) = by_id("poll-send") {
        dom::on(&b, "click", |_| leptos::task::spawn_local(send()));
    }
}
