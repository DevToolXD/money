//! 메시지의 + 메뉴 → GIF: GIPHY에서 찾아서 대화로 보내요 (릴스 댓글과 같은 키·같은 주소 모양).
//! 보내면 말풍선 없이 GIF만 카드로 보여요. 서버 규칙은 GIPHY 주소 모양과 세로 비율(r)만 받아요.

use std::cell::Cell;

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use super::{open_chat, queue_send, sheet_open, toast};
use crate::dom::{self, by_id, esc};

thread_local! {
    static SEQ: Cell<u32> = const { Cell::new(0) };
    static BOUND: Cell<bool> = const { Cell::new(false) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
}

pub fn open() {
    bind();
    if let Some(i) = by_id("gp-q").and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok()) {
        i.set_value("");
    }
    sheet_open(true, "gp-sheet");
    search(String::new());
}

fn search(q: String) {
    let Some(grid) = by_id("gp-grid") else { return };
    let seq = SEQ.get() + 1;
    SEQ.set(seq);
    spawn_local(async move {
        if !q.is_empty() {
            dom::sleep(400).await; // 치는 동안에는 보내지 않아요
            if SEQ.get() != seq {
                return;
            }
        }
        let r = crate::reel_comments::gif_items(&q).await;
        if SEQ.get() != seq {
            return;
        }
        match r {
            Err(note) => grid.set_inner_html(&format!(r#"<p class="rc-gnote">{note}</p>"#)),
            Ok(items) => {
                let html: String = items
                    .iter()
                    .map(|(u, ratio)| format!(r#"<button type="button" class="rc-g" data-gif="{u}" data-r="{r}" style="aspect-ratio:1/{ratio:.3}"><img src="{u}" alt="GIF" loading="lazy" decoding="async"></button>"#, u = esc(u), r = (ratio * 1000.0).round() as i64))
                    .collect();
                grid.set_inner_html(&html);
                grid.set_scroll_top(0);
            }
        }
    });
}

async fn send(url: String, r: i64) {
    if BUSY.replace(true) {
        return;
    }
    let Some(c) = open_chat() else {
        BUSY.set(false);
        return;
    };
    sheet_open(false, "");
    let (id, other) = (c.id.clone(), c.other.clone());
    let payload = dom::obj(&[("type", "gif".into()), ("gif", JsValue::from_str(&url)), ("r", (r.clamp(400, 2500) as f64).into())]);
    let ok = queue_send(move || cloud::call_now("send", &[JsValue::from_str(&id), JsValue::from_str(&other), payload, JsValue::from_str("GIF")])).await.is_ok();
    BUSY.set(false);
    if ok {
        dom::vibrate(8);
    } else {
        toast("GIF를 보내지 못했어요");
    }
}

fn bind() {
    if BOUND.replace(true) {
        return;
    }
    if let Some(q) = by_id("gp-q").and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok()) {
        let q2 = q.clone();
        dom::on(&q, "input", move |_| search(q2.value().trim().chars().take(50).collect()));
    }
    if let Some(g) = by_id("gp-grid") {
        dom::on(&g, "click", |e| {
            if let Some(b) = dom::closest(&e, "[data-gif]") {
                let url = dom::data(&b, "gif");
                if crate::sns::safe_gif(&url).is_empty() {
                    return;
                }
                let r: i64 = dom::data(&b, "r").parse().unwrap_or(800);
                spawn_local(send(url, r));
            }
        });
    }
}
