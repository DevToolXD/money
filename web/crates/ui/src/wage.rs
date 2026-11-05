//! 직원: 일 탭에서 내 시급과 받은 임금. 회사 정보를 읽을 때 받을 수 있는 임금(정수 시간 단위)을 자동으로 받아요.
//! 급여 금고가 비면 받을 수 없어요. 아직 못 받은 임금은 금고가 채워진 뒤에 받아요.

use std::cell::Cell;

use cloud::{fmt_num, get};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::dom::{by_id, esc};

thread_local! {
    static SEQ: Cell<u32> = const { Cell::new(0) };
    static LAST: Cell<f64> = const { Cell::new(0.0) };
}

fn num(v: &JsValue, k: &str) -> f64 {
    get(v, k).as_f64().unwrap_or(0.0)
}

/// 일 탭의 임금 칸(#wk-wage)을 그려요. 직원일 때만 칸이 있어요
pub fn paint(cid: &str, name: &str) {
    let Some(el) = by_id("wk-wage") else { return };
    let seq = SEQ.get() + 1;
    SEQ.set(seq);
    let (cid, name) = (cid.to_string(), name.to_string());
    spawn_local(async move {
        let Ok(r) = cloud::call("claimWage", &[JsValue::from_str(&cid)]).await else { return };
        if SEQ.get() != seq {
            return;
        }
        LAST.set(js_sys::Date::now());
        let (amt, wage, due, pay, next) = (num(&r, "amt"), num(&r, "wage"), num(&r, "due"), num(&r, "pay"), num(&r, "next"));
        let line = if wage < 1.0 {
            format!("{} 사장님이 아직 임금을 정하지 않았어요", esc(&name))
        } else if amt > 0.0 {
            format!("방금 {} 토큰을 받았어요", fmt_num(amt, 0))
        } else if due >= 1.0 {
            "급여 금고가 비어서 아직 못 받았어요. 채워지면 자동으로 받아요".to_string()
        } else {
            format!("다음 임금까지 약 {}분", ((next / 60000.0).ceil() as i64).max(1))
        };
        if amt > 0.0 {
            crate::shell::toast(&format!("임금 {} 토큰을 받았어요", fmt_num(amt, 0)));
        }
        el.set_inner_html(&format!(
            r#"<div class="wk-wg"><div><b>시급 {} 토큰</b><span>{}</span></div><span class="wk-wg-pay">급여 금고 {} 토큰</span></div>"#,
            fmt_num(wage, 0),
            line,
            fmt_num(pay, 2)
        ));
    });
}

/// 임금 칸을 눌러 다시 받아 보기
pub fn init() {
    if let Some(root) = by_id("work-root") {
        crate::dom::on(&root, "click", |e| {
            if crate::dom::closest(&e, ".wk-wg").is_none() {
                return;
            }
            if js_sys::Date::now() - LAST.get() < 5000.0 {
                return;
            }
            if let (Some(b), Some(cid)) = (by_id("wk-wage"), by_id("wk-wage").and_then(|w| w.get_attribute("data-cid"))) {
                let name = b.get_attribute("data-name").unwrap_or_default();
                paint(&cid, &name);
            }
        });
    }
}
