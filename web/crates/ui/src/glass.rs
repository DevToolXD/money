//! 리퀴드 글라스 (CSS로 그린 유리 전부): Kyant Liquid Glass의 계산을 웹으로 옮겼어요. (예전 app/glass.js를 그대로 옮겼어요)
//! 원본: https://github.com/Kyant0/AndroidLiquidGlass (Copyright 2025 Kyant, Apache License 2.0)
//!
//! - 굴절 (lens): 가장자리에서 안쪽까지 원호 모양으로 뒤 화면을 끌어와요. 버튼 lens(12, 24), 큰 판 lens(24, 48, 깊이)
//!   브라우저가 SVG 필터를 backdrop-filter에 쓸 수 있을 때만 해요 (크롬·안드로이드). 아이폰 사파리는 흐림만 써요
//! - 누르기 (InteractiveHighlight): 누른 자리에서 빛이 번지고, 살짝 부풀고, 끄는 쪽으로 따라와요 (스프링)
//! - 테두리 선은 그리지 않아요 (가는 선이 보이지 않게 해 달라고 했어요)
//! 탭 바 유리는 tabbar.rs의 WebGL이 그려요.

use std::cell::RefCell;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement};

// 유리로 그리는 것들 (어둡게 덮는 뒤판·잠금 화면처럼 유리가 아닌 것은 빼요). 새 유리를 만들면 여기에 넣어요
pub const SEL: &str = ".glass-btn,.t-glass,.plus,.field-glass,.ctx-menu,.lg-card,.lg-btn,.alert-card,.alert-btn,.chart-seg,.t-btn.grey,.attach,.a-ico,.poll-opt,.sheet.dark,.draw-undo,.pen-btn,.status-bubble,.fab-tile,.found-logo,.pk-glass,.pk-orb,.pk-row,.pk-soon-orb,.pk-card,.pk-step,.biz-more,.biz-tag,.bz-sheet-t,.bz-sheet-g,.bz-cancel,.bz-input,.bz-btn,.gg,.gseg,.lab-stats span,.lab-input,.lab-row,.lab-reagent,.notice-opt,.inv-empty-glass,.mg-sug-row,.pk-ic,.sns-card,.sns-fab,.sns-panel";
const PRESSABLE: &str = "button, a, [role='button'], .pk-ic, .a-ico";

#[derive(Default)]
struct St {
    key: String,
    hl: Option<HtmlElement>,
    p: f64,
    v: f64,
    target: f64,
    x: f64,
    y: f64,
    ox: f64,
    oy: f64,
    sx: f64,
    sy: f64,
    vx: f64,
    vy: f64,
    down: bool,
    // 물방울 (상태 버튼): 움직이는 쪽은 둥글고 뒤쪽은 뾰족해져요
    drop: bool,
    tear: f64,
    tear_v: f64,
    tear_t: f64,
    ang: f64,
    lx: f64,
    ly: f64,
    lt: f64,
}

struct G {
    lens: bool,
    rm: Option<web_sys::MediaQueryList>,
    els: Vec<(HtmlElement, St)>,
    active: Vec<usize>,
    raf: i32,
    last: f64,
    svg: Option<Element>,
    lens_cache: HashMap<String, String>,
    resize: Option<web_sys::ResizeObserver>,
}

thread_local! {
    static G_: RefCell<Option<G>> = const { RefCell::new(None) };
    static LOOP: RefCell<Option<Closure<dyn FnMut(f64)>>> = const { RefCell::new(None) };
}

fn win() -> web_sys::Window {
    web_sys::window().expect("window")
}
fn with<R>(f: impl FnOnce(&mut G) -> R) -> Option<R> {
    G_.with(|g| g.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(f)))
}
fn num(o: &JsValue, k: &str) -> f64 {
    js_sys::Reflect::get(o, &JsValue::from_str(k)).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}
fn jn(v: f64) -> String {
    crate::tabbar::js_num(v)
}
fn fixed(v: f64, d: u8) -> String {
    js_sys::Number::from(v).to_fixed(d).map(String::from).unwrap_or_default()
}

/* ---------- Kyant 계산 ---------- */
fn sd_round(x: f64, y: f64, hw: f64, hh: f64, r: f64) -> f64 {
    let ax = x.abs() - (hw - r);
    let ay = y.abs() - (hh - r);
    ax.max(0.0).hypot(ay.max(0.0)) - r + ax.max(ay).min(0.0)
}
fn grad_round(x: f64, y: f64, hw: f64, hh: f64, r: f64) -> (f64, f64) {
    let ax = x.abs() - (hw - r);
    let ay = y.abs() - (hh - r);
    let sx = if x < 0.0 { -1.0 } else { 1.0 };
    let sy = if y < 0.0 { -1.0 } else { 1.0 };
    if ax >= 0.0 || ay >= 0.0 {
        let mx = ax.max(0.0);
        let my = ay.max(0.0);
        let l = mx.hypot(my);
        let l = if l == 0.0 { 1.0 } else { l };
        return (sx * mx / l, sy * my / l);
    }
    if ay <= ax {
        (sx, 0.0)
    } else {
        (0.0, sy)
    }
}
fn circle_map(x: f64) -> f64 {
    1.0 - (1.0 - x * x).max(0.0).sqrt()
}
fn js_round(v: f64) -> f64 {
    (v + 0.5).floor()
}

impl G {
    /* ---------- 굴절 지도 (SVG 변위 필터, 크롬만) ---------- */
    fn lens_filter(&mut self, w: f64, h: f64, r: f64, height: f64, amount: f64, depth: bool) -> String {
        let key = format!("{}x{}x{}x{}x{}x{}", jn(w), jn(h), jn(r), jn(height), jn(amount), depth as u8);
        if let Some(v) = self.lens_cache.get(&key) {
            return v.clone();
        }
        let doc = crate::dom::doc();
        if self.svg.is_none() {
            if let Ok(s) = doc.create_element_ns(Some("http://www.w3.org/2000/svg"), "svg") {
                let _ = s.set_attribute("width", "0");
                let _ = s.set_attribute("height", "0");
                let _ = s.set_attribute("aria-hidden", "true");
                let _ = s.set_attribute("style", "position:absolute;width:0;height:0;overflow:hidden;pointer-events:none");
                if let Some(b) = doc.body() {
                    let _ = b.append_child(&s);
                }
                self.svg = Some(s);
            }
        }
        let cw = js_round(w).max(1.0) as u32;
        let chh = js_round(h).max(1.0) as u32;
        let hw = w / 2.0;
        let hh = h / 2.0;
        let gr = (r * 1.5).min(hw).min(hh);
        let scale = amount * 2.0 + 2.0;
        let mut data = vec![0u8; (cw * chh * 4) as usize];
        let byte = |v: f64| js_round(v).clamp(0.0, 255.0) as u8;
        for py in 0..chh {
            for px in 0..cw {
                let x = px as f64 + 0.5 - hw;
                let y = py as f64 + 0.5 - hh;
                let (mut dx, mut dy) = (0.0, 0.0);
                let sd = sd_round(x, y, hw, hh, r);
                if sd <= 0.0 && -sd < height {
                    let d = circle_map(1.0 + sd / height) * -amount;
                    let (mut gx, mut gy) = grad_round(x, y, hw, hh, gr);
                    if depth {
                        let l = x.hypot(y);
                        let l = if l == 0.0 { 1.0 } else { l };
                        gx += x / l;
                        gy += y / l;
                        let m = gx.hypot(gy);
                        let m = if m == 0.0 { 1.0 } else { m };
                        gx /= m;
                        gy /= m;
                    }
                    dx = d * gx;
                    dy = d * gy;
                }
                let i = ((py * cw + px) * 4) as usize;
                data[i] = byte((0.5 + dx / scale) * 255.0);
                data[i + 1] = byte((0.5 + dy / scale) * 255.0);
                data[i + 2] = 128;
                data[i + 3] = 255;
            }
        }
        let c: web_sys::HtmlCanvasElement = doc.create_element("canvas").unwrap().unchecked_into();
        c.set_width(cw);
        c.set_height(chh);
        if let Some(ctx) = c.get_context("2d").ok().flatten().and_then(|x| x.dyn_into::<web_sys::CanvasRenderingContext2d>().ok()) {
            if let Ok(img) = web_sys::ImageData::new_with_u8_clamped_array_and_sh(wasm_bindgen::Clamped(&data), cw, chh) {
                let _ = ctx.put_image_data(&img, 0.0, 0.0);
            }
        }
        let url = c.to_data_url_with_type("image/png").unwrap_or_default();
        let id = format!("lg-f{}", self.lens_cache.len());
        if let (Some(svg), Ok(f)) = (&self.svg, doc.create_element_ns(Some("http://www.w3.org/2000/svg"), "filter")) {
            let _ = f.set_attribute("id", &id);
            let _ = f.set_attribute("x", "0");
            let _ = f.set_attribute("y", "0");
            let _ = f.set_attribute("width", &jn(w));
            let _ = f.set_attribute("height", &jn(h));
            let _ = f.set_attribute("filterUnits", "userSpaceOnUse");
            let _ = f.set_attribute("color-interpolation-filters", "sRGB");
            f.set_inner_html(&format!(
                "<feImage href=\"{url}\" x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" result=\"map\"/><feDisplacementMap in=\"SourceGraphic\" in2=\"map\" scale=\"{}\" xChannelSelector=\"R\" yChannelSelector=\"G\"/>",
                jn(w),
                jn(h),
                jn(scale)
            ));
            let _ = svg.append_child(&f);
        }
        let v = format!("url(#{id})");
        self.lens_cache.insert(key, v.clone());
        v
    }

    fn paint(&mut self, i: usize) {
        let el = self.els[i].0.clone();
        let w = el.offset_width() as f64;
        let h = el.offset_height() as f64;
        if w < 4.0 || h < 4.0 {
            return;
        }
        let rr = win().get_computed_style(&el).ok().flatten().and_then(|c| c.get_property_value("border-top-left-radius").ok()).map(|s| js_sys::Number::parse_float(&s)).unwrap_or(0.0);
        let rr = if rr.is_nan() { 0.0 } else { rr };
        let r = rr.min(w / 2.0).min(h / 2.0);
        let big = h > 80.0 && w > 120.0; // 큰 판: 시트·카드·메뉴
        let key = format!("{}x{}x{}", jn(w), jn(h), jn(r));
        if self.els[i].1.key == key {
            return;
        }
        self.els[i].1.key = key;
        if self.lens {
            let f = if big { self.lens_filter(w, h, r, 24.0, 48.0, true) } else { self.lens_filter(w, h, r, 12.0, 24.0, false) };
            let blur = if big { 16 } else { 2 };
            let v = format!("{f} blur({blur}px) saturate(150%)");
            let st = el.style();
            let _ = st.set_property("-webkit-backdrop-filter", &v);
            let _ = st.set_property("backdrop-filter", &v);
        }
    }

    fn index(&self, el: &Element) -> Option<usize> {
        // 요소에 번호를 적어 두어서 찾는 데 시간이 들지 않아요
        let i = js_sys::Reflect::get(el, &"_lgi".into()).ok()?.as_f64()? as usize;
        self.els.get(i).filter(|(e, _)| e.is_same_node(Some(el))).map(|_| i)
    }

    fn apply(&mut self, i: usize) {
        let rm = self.rm.as_ref().is_some_and(|m| m.matches());
        let (el, s) = (&self.els[i].0, &self.els[i].1);
        let p = s.p.max(0.0);
        let w = el.offset_width() as f64;
        let w = if w == 0.0 { 1.0 } else { w };
        let h = el.offset_height() as f64;
        let h = if h == 0.0 { 1.0 } else { h };
        if let Some(hl) = &s.hl {
            let st = hl.style();
            let _ = st.set_property("--lg-p", &fixed(p, 3));
            let _ = st.set_property("--lg-x", &format!("{}px", jn(w.min(s.x.max(0.0)))));
            let _ = st.set_property("--lg-y", &format!("{}px", jn(h.min(s.y.max(0.0)))));
            let _ = st.set_property("--lg-r", &format!("{}px", jn(w.min(h) * 1.5)));
        }
        if rm {
            return;
        }
        if s.drop {
            // 물방울: 끄는 동안 납작하게 늘어나지 않고, 앞은 둥글고 뒤(오는 쪽)는 뾰족해져요. 멈추면 다시 동그래져요
            let t = s.tear.clamp(0.0, 1.0);
            let base = 1.0 + (4.0 / h) * p;
            let st = el.style();
            let _ = st.set_property("scale", &if p > 0.001 { fixed(base, 4) } else { String::new() });
            let _ = st.set_property("translate", "");
            if t > 0.002 {
                // 뾰족한 왼쪽 아래 모서리를 뒤(-x)로 돌리고, 움직이는 방향으로 살짝 길게 늘인 뒤, 움직이는 방향으로 돌려요
                let a = s.ang.to_degrees();
                let _ = st.set_property("border-radius", &format!("50% 50% 50% {}%", fixed(50.0 * (1.0 - 0.92 * t), 2)));
                let _ = st.set_property("transform", &format!("rotate({}deg) scale({}, {}) rotate(45deg)", fixed(a, 2), fixed(1.0 + 0.22 * t, 4), fixed(1.0 - 0.07 * t, 4)));
                let _ = st.set_property("--tear-neg", &format!("{}deg", fixed(-(a + 45.0), 2)));
            } else {
                let _ = st.remove_property("border-radius");
                let _ = st.remove_property("transform");
                let _ = st.remove_property("--tear-neg");
            }
            return;
        }
        // 부풀기: 높이 기준 4px, 끄는 쪽으로 늘어나요. 따라오기: tanh로 부드럽게 멈춰요
        let md = w.min(h);
        let tx = md * (0.05 * s.ox / md).tanh();
        let ty = md * (0.05 * s.oy / md).tanh();
        let base = 1.0 + (4.0 / h) * p;
        let ang = s.oy.atan2(s.ox);
        let mx = w.max(h);
        let ds = 4.0 / h;
        let sx = base + ds * (ang.cos() * s.ox / mx).abs() * (w / h).min(1.0);
        let sy = base + ds * (ang.sin() * s.oy / mx).abs() * (h / w).min(1.0);
        let on = p > 0.001 || s.ox != 0.0 || s.oy != 0.0;
        let st = el.style();
        let _ = st.set_property("translate", &if on { format!("{}px {}px", fixed(tx, 2), fixed(ty, 2)) } else { String::new() });
        let _ = st.set_property("scale", &if on { format!("{} {}", fixed(sx, 4), fixed(sy, 4)) } else { String::new() });
    }

    /* ---------- 누르기: 빛 번짐 + 부풀기 + 끌기 (스프링 0.5, 300) ---------- */
    fn step(&mut self, dt: f64) {
        let list = self.active.clone();
        for i in list {
            {
                let s = &mut self.els[i].1;
                let k: f64 = 300.0;
                let c = 2.0 * 0.5 * k.sqrt();
                let a = -k * (s.p - s.target) - c * s.v;
                s.v += a * dt;
                s.p += s.v * dt;
                // 끄는 거리도 같은 스프링으로 돌아가요
                if !s.down {
                    let ax = -k * s.ox - c * s.vx;
                    let ay = -k * s.oy - c * s.vy;
                    s.vx += ax * dt;
                    s.vy += ay * dt;
                    s.ox += s.vx * dt;
                    s.oy += s.vy * dt;
                }
                if s.drop {
                    // 손가락이 잠깐 멈추면 다시 동그래져요. 살짝 출렁이며 돌아와요
                    if js_sys::Date::now() - s.lt > 80.0 {
                        s.tear_t = 0.0;
                    }
                    let kt: f64 = 260.0;
                    let ct = 2.0 * 0.6 * kt.sqrt();
                    let at = -kt * (s.tear - s.tear_t) - ct * s.tear_v;
                    s.tear_v += at * dt;
                    s.tear += s.tear_v * dt;
                }
            }
            self.apply(i);
            let s = &mut self.els[i].1;
            if !s.down && s.p.abs() < 0.001 && s.v.abs() < 0.01 && s.ox.hypot(s.oy) < 0.1 && s.tear.abs() < 0.002 && s.tear_v.abs() < 0.01 {
                s.p = 0.0;
                s.tear = 0.0;
                s.tear_v = 0.0;
                s.ox = 0.0;
                s.oy = 0.0;
                self.apply(i);
                self.active.retain(|x| *x != i);
            }
        }
    }
    fn kick(&mut self) {
        if self.raf == 0 {
            self.raf = LOOP.with(|l| l.borrow().as_ref().and_then(|c| win().request_animation_frame(c.as_ref().unchecked_ref()).ok())).unwrap_or(0);
        }
    }
}

fn frame(t: f64) {
    with(|g| {
        let dt = if g.last != 0.0 { (t - g.last) / 1000.0 } else { 0.016 }.min(0.032);
        g.last = t;
        g.step(dt);
        g.raf = 0;
        if g.active.is_empty() {
            g.last = 0.0;
        } else {
            g.kick();
        }
    });
}

fn bind_press(el: &HtmlElement) {
    let _ = el.class_list().add_1("lg-press");
    let t: &web_sys::EventTarget = el.as_ref();
    let e1 = el.clone();
    crate::dom::on(t, "pointerdown", move |e| {
        if js_sys::Reflect::get(&e, &"pointerType".into()).ok().and_then(|v| v.as_string()).as_deref() == Some("mouse") && num(&e, "button") != 0.0 {
            return;
        }
        let r = e1.get_bounding_client_rect();
        let (cx, cy) = (num(&e, "clientX"), num(&e, "clientY"));
        with(|g| {
            let Some(i) = g.index(&e1) else { return };
            let s = &mut g.els[i].1;
            s.down = true;
            s.sx = cx;
            s.sy = cy;
            s.x = cx - r.left();
            s.y = cy - r.top();
            s.ox = 0.0;
            s.oy = 0.0;
            s.target = 1.0;
            s.lx = cx;
            s.ly = cy;
            s.lt = js_sys::Date::now();
            if !g.active.contains(&i) {
                g.active.push(i);
            }
            g.kick();
        });
    });
    let e2 = el.clone();
    crate::dom::on(t, "pointermove", move |e| {
        with(|g| {
            let Some(i) = g.index(&e2) else { return };
            if !g.els[i].1.down {
                return;
            }
            let r = e2.get_bounding_client_rect();
            let (cx, cy) = (num(&e, "clientX"), num(&e, "clientY"));
            let s = &mut g.els[i].1;
            s.x = cx - r.left();
            s.y = cy - r.top();
            s.ox = cx - s.sx;
            s.oy = cy - s.sy;
            if s.drop {
                // 빠르기로 뾰족한 정도를 정해요 (0.9px/ms면 가장 뾰족해요)
                let t = js_sys::Date::now();
                let (dx, dy) = (cx - s.lx, cy - s.ly);
                let dist = dx.hypot(dy);
                if dist > 0.5 {
                    let sp = dist / (t - s.lt).max(8.0);
                    s.tear_t = (sp / 0.9).clamp(0.0, 1.0);
                    s.ang = dy.atan2(dx);
                    s.lx = cx;
                    s.ly = cy;
                    s.lt = t;
                }
            }
        });
    });
    for ev in ["pointerup", "pointercancel", "pointerleave"] {
        let e3 = el.clone();
        crate::dom::on(t, ev, move |_| {
            with(|g| {
                let Some(i) = g.index(&e3) else { return };
                let s = &mut g.els[i].1;
                if !s.down {
                    return;
                }
                s.down = false;
                s.target = 0.0;
                g.kick();
            });
        });
    }
}

/* ---------- 유리 붙이기 ---------- */
fn attach(el: &HtmlElement) {
    let already = with(|g| g.index(el).is_some()).unwrap_or(true);
    if already || el.closest(".tabbar").ok().flatten().is_some() {
        return;
    }
    let field = matches!(el.tag_name().as_str(), "INPUT" | "TEXTAREA");
    let mut st = St { drop: el.id() == "status-bubble", ..St::default() };
    let _ = el.class_list().add_1("lg");
    if !field {
        if win().get_computed_style(el).ok().flatten().and_then(|c| c.get_property_value("position").ok()).as_deref() == Some("static") {
            let _ = el.class_list().add_1("lg-rel");
        }
        if let Ok(hl) = crate::dom::doc().create_element("lg-hl") {
            let _ = hl.set_attribute("aria-hidden", "true");
            let _ = el.append_child(&hl);
            st.hl = hl.dyn_into().ok();
        }
    } else {
        let _ = el.class_list().add_1("lg-field");
    }
    with(|g| {
        g.els.push((el.clone(), st));
        let i = g.els.len() - 1;
        let _ = js_sys::Reflect::set(el, &"_lgi".into(), &(i as f64).into());
        g.paint(i);
        if let Some(ro) = &g.resize {
            ro.observe(el);
        }
    });
    if !field && el.matches(PRESSABLE).unwrap_or(false) {
        bind_press(el);
    }
}
fn scan(root: &Element) {
    if root.matches(SEL).unwrap_or(false) {
        if let Some(h) = root.dyn_ref::<HtmlElement>() {
            attach(h);
        }
    }
    for el in crate::dom::qsa(root, SEL) {
        attach(&el);
    }
}

/// 시작: 지금 있는 유리를 그리고, 새로 생기는 것도 지켜봐요
pub fn start() {
    let search = win().location().search().unwrap_or_default();
    let ua = win().navigator().user_agent().unwrap_or_default();
    let has = |s: &str| ua.contains(s);
    let chrome = ua.split("Chrome/").nth(1).is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit()));
    let supports = js_sys::Reflect::get(&js_sys::global(), &"CSS".into())
        .ok()
        .and_then(|css| {
            let f: js_sys::Function = js_sys::Reflect::get(&css, &"supports".into()).ok()?.dyn_into().ok()?;
            f.call2(&css, &"backdrop-filter".into(), &"url(#a)".into()).ok()
        })
        .is_some_and(|v| v.is_truthy());
    let lens_off = search.contains("?lens=off") || search.contains("&lens=off");
    let lens = !lens_off && chrome && !["iPhone", "iPad", "iPod", "CriOS", "FxiOS", "EdgiOS"].iter().any(|s| has(s)) && supports;
    LOOP.with(|l| *l.borrow_mut() = Some(Closure::new(frame)));
    let ro = {
        let c = Closure::<dyn FnMut(js_sys::Array)>::new(|entries: js_sys::Array| {
            for e in entries.iter() {
                let t = js_sys::Reflect::get(&e, &"target".into()).ok().and_then(|t| t.dyn_into::<Element>().ok());
                if let Some(t) = t {
                    with(|g| {
                        if let Some(i) = g.index(&t) {
                            g.paint(i);
                        }
                    });
                }
            }
        });
        let ro = web_sys::ResizeObserver::new(c.as_ref().unchecked_ref()).ok();
        c.forget();
        ro
    };
    G_.with(|g| {
        *g.borrow_mut() = Some(G {
            lens,
            rm: win().match_media("(prefers-reduced-motion: reduce)").ok().flatten(),
            els: Vec::new(),
            active: Vec::new(),
            raf: 0,
            last: 0.0,
            svg: None,
            lens_cache: HashMap::new(),
            resize: ro,
        })
    });
    let doc = crate::dom::doc();
    if lens {
        if let Some(r) = doc.document_element() {
            let _ = r.class_list().add_1("lg-lens");
        }
    }
    let Some(body) = doc.body() else { return };
    scan(&body);
    let c = Closure::<dyn FnMut(js_sys::Array)>::new(|records: js_sys::Array| {
        for r in records.iter() {
            let Ok(r) = r.dyn_into::<web_sys::MutationRecord>() else { continue };
            let added = r.added_nodes();
            for i in 0..added.length() {
                if let Some(n) = added.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
                    if n.tag_name() != "LG-HL" {
                        scan(&n);
                    }
                }
            }
            // 화면 코드가 글자를 통째로 바꾸면(textContent) 빛 자리도 지워져서, 다시 붙여요
            let removed = r.removed_nodes();
            for i in 0..removed.length() {
                let Some(n) = removed.item(i).and_then(|n| n.dyn_into::<Element>().ok()) else { continue };
                if n.tag_name() != "LG-HL" || n.is_connected() {
                    continue;
                }
                let Some(target) = r.target().and_then(|t| t.dyn_into::<Element>().ok()) else { continue };
                let mine = with(|g| g.index(&target).and_then(|i| g.els[i].1.hl.clone())).flatten().is_some_and(|h| h.is_same_node(Some(&n)));
                if mine {
                    let _ = target.append_child(&n);
                }
            }
            // 클래스가 바뀌어 유리가 된 것 (.t-btn에 grey가 붙는 것처럼)
            if r.type_() == "attributes" {
                if let Some(t) = r.target().and_then(|t| t.dyn_into::<HtmlElement>().ok()) {
                    let known = with(|g| g.index(&t).is_some()).unwrap_or(true);
                    if !known && t.matches(SEL).unwrap_or(false) {
                        attach(&t);
                    }
                }
            }
        }
    });
    if let Ok(mo) = web_sys::MutationObserver::new(c.as_ref().unchecked_ref()) {
        let o = web_sys::MutationObserverInit::new();
        o.set_child_list(true);
        o.set_subtree(true);
        o.set_attributes(true);
        o.set_attribute_filter(&js_sys::Array::of1(&"class".into()));
        let _ = mo.observe_with_options(&body, &o);
    }
    c.forget();
    let api = crate::dom::obj(&[
        ("lens", lens.into()),
        ("count", Closure::<dyn FnMut() -> JsValue>::new(|| JsValue::from(with(|g| g.els.len()).unwrap_or(0) as f64)).into_js_value()),
    ]);
    let _ = js_sys::Reflect::set(&js_sys::global(), &"LiquidGlass".into(), &api);
}
