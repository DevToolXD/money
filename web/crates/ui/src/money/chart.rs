//! 쓴 돈 꺾은선 그래프: 1년(한 달씩 12개) · 1개월(하루씩 30개) · 1일(한 시간씩 24개)

use std::cell::RefCell;

use wasm_bindgen::prelude::*;

use super::{all_tx, sheet_open, w};
use crate::dom::{self, by_id};

struct Pt {
    l: String,
    v: f64,
}
thread_local! {
    static RANGE: RefCell<&'static str> = const { RefCell::new("m") };
    static PTS: RefCell<Vec<Pt>> = const { RefCell::new(Vec::new()) };
}
fn date(y: i32, m: i32, d: i32) -> js_sys::Date {
    js_sys::Date::new_with_year_month_day(y as u32, m, d)
}
/// 하루 동안 쓴 토큰 (0시 기준 날짜별)
fn spent_between(lo: f64, hi: f64) -> f64 {
    all_tx().iter().filter(|e| e.amt < 0.0 && e.t >= lo && e.t < hi).map(|e| -e.amt).sum()
}
fn series(r: &str) -> Vec<Pt> {
    let t = js_sys::Date::new_0();
    let (y, m, d) = (t.get_full_year() as i32, t.get_month() as i32, t.get_date() as i32);
    let mut out = Vec::new();
    match r {
        "m" => {
            for i in (0..30).rev() {
                let a = date(y, m, d - i);
                let b = date(y, m, d - i + 1);
                out.push(Pt { l: format!("{}/{}", a.get_month() + 1, a.get_date()), v: spent_between(a.get_time(), b.get_time()) });
            }
        }
        "y" => {
            for k in (0..12).rev() {
                let first = date(y, m - k, 1);
                let next = date(first.get_full_year() as i32, first.get_month() as i32 + 1, 1);
                out.push(Pt { l: format!("{}월", first.get_month() + 1), v: spent_between(first.get_time(), next.get_time()) });
            }
        }
        _ => {
            let end = js_sys::Date::now();
            for h in (0..24).rev() {
                let hi = end - h as f64 * 3600e3;
                let lo = hi - 3600e3;
                let v: f64 = all_tx().iter().filter(|e| e.amt < 0.0 && e.t > lo && e.t <= hi).map(|e| -e.amt).sum();
                out.push(Pt { l: format!("{}시", js_sys::Date::new(&JsValue::from_f64(lo)).get_hours()), v });
            }
        }
    }
    out
}
fn label(r: &str) -> &'static str {
    match r {
        "y" => "1년 동안 쓴 토큰",
        "m" => "1개월 동안 쓴 토큰",
        _ => "1일 동안 쓴 토큰",
    }
}
fn xy(n: usize, i: usize, v: f64, max: f64) -> (f64, f64) {
    (14.0 + (i as f64 * 312.0) / (n.max(2) - 1) as f64, 180.0 - (v / max) * 150.0)
}

fn draw(r: &'static str) {
    RANGE.with(|x| *x.borrow_mut() = r);
    for b in dom::qsa(&dom::doc().document_element().expect("html"), "#chart-seg button") {
        let _ = b.set_attribute("aria-selected", &(dom::data(&b, "r") == r).to_string());
    }
    let pts = series(r);
    let total: f64 = pts.iter().map(|p| p.v).sum();
    dom::set_text("chart-label", label(r));
    dom::set_text("chart-total", &w(total));
    let max = pts.iter().map(|p| p.v).fold(1.0f64, f64::max);
    let n = pts.len();
    let line: String = pts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let (x, y) = xy(n, i, p.v, max);
            format!("{}{x:.1} {y:.1}", if i == 0 { "M" } else { " L" })
        })
        .collect();
    let (x0, _) = xy(n, 0, 0.0, max);
    let (xn, _) = xy(n, n - 1, 0.0, max);
    let dots: String = pts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let (x, y) = xy(n, i, p.v, max);
            format!(r#"<circle cx="{x:.1}" cy="{y:.1}" r="3.500" class="c-dot" style="animation-delay:{}s"/>"#, 0.5 + (i as f64 / n as f64) * 0.9)
        })
        .collect();
    let Some(chart) = by_id("chart") else { return };
    chart.set_inner_html(&format!(
        r##"<defs><linearGradient id="cg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#3182f6" stop-opacity="0.28"/><stop offset="1" stop-color="#3182f6" stop-opacity="0"/></linearGradient></defs>
      <path d="{line} L{xn} 190 L{x0} 190Z" fill="url(#cg)" class="c-area"/>
      <path d="{line}" pathLength="1" class="c-line"/>
      {dots}
      <line class="c-guide" x1="0" x2="0" y1="20" y2="190"/>"##
    ));
    let every = n.div_ceil(6);
    let axis: String = pts
        .iter()
        .enumerate()
        .filter(|(i, _)| (i % every == 0 && n - 1 - i >= every.div_ceil(2)) || *i == n - 1)
        .map(|(i, p)| format!(r#"<span style="left:{}%">{}</span>"#, (xy(n, i, 0.0, max).0 / 340.0) * 100.0, p.l))
        .collect();
    dom::set_text("chart-axis", "");
    if let Some(ax) = by_id("chart-axis") {
        ax.set_inner_html(&axis);
    }
    dom::set_hidden("chart-tip", true);
    PTS.with(|p| *p.borrow_mut() = pts);
    let _ = chart.class_list().remove_1("go");
    let _ = chart.get_bounding_client_rect();
    let _ = chart.class_list().add_1("go");
}

fn scrub(e: &web_sys::PointerEvent) {
    let Some(chart) = by_id("chart") else { return };
    let r = chart.get_bounding_client_rect();
    let x = ((e.client_x() as f64 - r.left()) / r.width()) * 340.0;
    PTS.with(|p| {
        let p = p.borrow();
        let n = p.len();
        if n == 0 {
            return;
        }
        let max = p.iter().map(|q| q.v).fold(1.0f64, f64::max);
        let xs = |i| xy(n, i, 0.0, max).0;
        let best = (0..n).min_by(|a, b| (xs(*a) - x).abs().partial_cmp(&(xs(*b) - x).abs()).unwrap_or(std::cmp::Ordering::Equal)).unwrap_or(0);
        if let Some(g) = dom::qs(&chart, ".c-guide") {
            let _ = g.set_attribute("x1", &xs(best).to_string());
            let _ = g.set_attribute("x2", &xs(best).to_string());
            let _ = g.style().set_property("opacity", "1");
        }
        if let Some(tip) = by_id("chart-tip") {
            tip.set_hidden(false);
            tip.set_text_content(Some(&format!("{} · {}", p[best].l, w(p[best].v))));
            let _ = tip.style().set_property("left", &format!("{}%", ((xs(best) / 340.0) * 100.0).clamp(18.0, 82.0)));
        }
    });
}

pub fn open() {
    sheet_open(true, "chart-sheet");
    draw(RANGE.with(|r| *r.borrow()));
}

pub fn init() {
    if let Some(chart) = by_id("chart") {
        dom::on(&chart, "pointerdown", |e| {
            if let Some(p) = wasm_bindgen::JsCast::dyn_ref::<web_sys::PointerEvent>(&e) {
                scrub(p);
            }
        });
        dom::on(&chart, "pointermove", |e| {
            if let Some(p) = wasm_bindgen::JsCast::dyn_ref::<web_sys::PointerEvent>(&e) {
                if p.buttons() != 0 || p.pointer_type() == "touch" {
                    scrub(p);
                }
            }
        });
    }
    if let Some(seg) = by_id("chart-seg") {
        dom::on(&seg, "click", |e| {
            if let Some(b) = dom::closest(&e, "button") {
                let r = match dom::data(&b, "r").as_str() {
                    "y" => "y",
                    "d" => "d",
                    _ => "m",
                };
                draw(r);
            }
        });
    }
}
