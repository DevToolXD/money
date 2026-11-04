//! 검은 카드 위의 점 무늬: 한 번만 캔버스에 그려요 (점 600개짜리 SVG를 화면에 두면 화면 사진을 찍을 때마다 무거워요).
//! 1291x801 짜리 도안을 카드 크기에 맞춰 그려요: 가운데에서 퍼져 나가는 원 안의 화살표

use std::f64::consts::PI;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::dom::by_id;

fn paint(cv: &web_sys::HtmlCanvasElement) {
    let Some(w) = web_sys::window() else { return };
    let dpr = w.device_pixel_ratio().clamp(1.0, 2.0);
    let cw = if cv.client_width() > 0 { cv.client_width() as f64 } else { 350.0 };
    let ch = if cv.client_height() > 0 { cv.client_height() as f64 } else { 217.0 };
    cv.set_width((cw * dpr).round() as u32);
    cv.set_height((ch * dpr).round() as u32);
    let Some(c) = cv.get_context("2d").ok().flatten().and_then(|c| c.dyn_into::<web_sys::CanvasRenderingContext2d>().ok()) else { return };
    let _ = c.scale(dpr, dpr);
    let k = cw / 1291.0;
    let (cx, cy, rr, step) = (645.0, 855.0, 670.0, 53.0);
    c.set_line_cap("round");
    c.set_line_join("round");
    let mut r = 40.0;
    let mut ring = 0;
    while r <= rr {
        let n = ((PI * r) / step).round().max(6.0);
        let mut i = if ring % 2 == 1 { 0.5 } else { 0.0 };
        while i <= n {
            let a = PI * (1.0 - i / n);
            let x = cx + a.cos() * r * 1.02;
            let y = cy - a.sin() * r;
            i += 1.0;
            if !(-30.0..=1321.0).contains(&x) || y > 830.0 || y < 0.0 {
                continue;
            }
            let d = (3.0 + 52.0 * (PI * (r / rr).powf(0.9)).sin()) * k;
            if d < 1.2 {
                continue;
            }
            let tx = (x / 1291.0).clamp(0.0, 1.0);
            let ty = ((y - 180.0) / 620.0).clamp(0.0, 1.0);
            c.set_fill_style_str(&format!("hsl({:.0} 58% {:.0}%)", 182.0 - 178.0 * (0.7 * tx + 0.3 * ty), 74.0 - 4.0 * ty));
            c.begin_path();
            let _ = c.arc(x * k, y * k, d / 2.0, 0.0, std::f64::consts::TAU);
            c.fill();
            if d < 9.0 {
                continue;
            }
            let (dx, dy) = (a.cos(), -a.sin());
            let l = d * 0.3;
            let hd = d * 0.19;
            let (px, py) = (x * k, y * k);
            c.set_stroke_style_str("#1b1b1d");
            c.set_line_width((d * 0.075).max(1.0));
            c.begin_path();
            c.move_to(px - dx * l, py - dy * l);
            c.line_to(px + dx * l, py + dy * l);
            for sg in [-1.0f64, 1.0] {
                let (ca, sa) = ((sg * 0.75).cos(), (sg * 0.75).sin());
                c.move_to(px + dx * l, py + dy * l);
                c.line_to(px + dx * l - (dx * ca - dy * sa) * hd, py + dy * l - (dx * sa + dy * ca) * hd);
            }
            c.stroke();
        }
        r += step;
        ring += 1;
    }
}

/// 카드 그림을 그리고, 크기가 바뀌면 다시 그려요
pub fn mount() {
    let Some(cv) = by_id("ac-art").and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok()) else { return };
    paint(&cv);
    let cv2 = cv.clone();
    let cb = Closure::<dyn FnMut()>::new(move || paint(&cv2));
    if let Ok(ro) = web_sys::ResizeObserver::new(cb.as_ref().unchecked_ref()) {
        ro.observe(&cv);
        std::mem::forget(ro);
    }
    cb.forget();
}
