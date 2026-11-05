//! 하단 탭 바: 막대 유리, 아이콘·글자, 선택 알약과 렌즈를 캔버스 하나에 WebGL로 직접 그려요. (예전 app/tabbar.js를 그대로 옮겼어요)
//!
//! - 막대(알약형 탭 줄, 둥근 검색 버튼)는 그 자체가 리퀴드 글라스예요. Backdrop이 찍어 둔 뒤 화면 사진을 스크롤 위치만큼 밀어 읽어서
//!   휘어 보이게 해요. 사진이 아직 없으면 CSS 유리 판(backdrop-filter)이 대신해요.
//! - 선택 알약과 렌즈는 같은 물체예요. 누르면 알약이 떠올라 렌즈가 되고, 도착하기 직전에 다시 알약으로 내려앉아요.
//! - 움직임은 스프링을 해석해로 풀어서, 프레임 간격이 들쭉날쭉해도 떨리지 않아요.
//! - WebGL을 못 쓰면 DOM 아이콘과 CSS 알약으로 대신 보여줘요.
//!
//! 셰이더(tabbar.frag)는 글자 하나 바꾸지 않고 그대로 옮겼어요.

use crate::icons::{self, Part};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d as C2d, HtmlCanvasElement, HtmlElement, WebGlRenderingContext as GL, WebGlTexture, WebGlUniformLocation};

const NS: &str = "http://www.w3.org/2000/svg";
const PAD_X: f64 = 36.0;
const PAD_TOP: f64 = 30.0;
const PAD_BOTTOM: f64 = 48.0;
const LAND_PX: f64 = 8.0;
const LENS_INSET: f64 = 4.0;
const GLASS_FADE: f64 = 0.28;

#[derive(Clone, Copy)]
struct Preset {
    w: f64,
    z: f64,
}
const LIFT: Preset = Preset { w: 16.1, z: 0.5 };
const MOVE: Preset = Preset { w: 21.0, z: 1.0 };
const TRAIL: Preset = Preset { w: 30.0, z: 1.0 };
const STRETCH: Preset = Preset { w: 14.0, z: 0.55 };
const PRESS: Preset = Preset { w: 26.0, z: 1.0 };
const EXCITE: Preset = Preset { w: 13.0, z: 1.0 };
const PAGE: Preset = Preset { w: 20.0, z: 1.0 }; // 핫바 넘기기: 튕기지 않고 부드럽게 미끄러져요

// 핫바: 옆으로 넘기면 바 안의 내용만 다음 쪽으로 바뀌어요 (프로필 버튼은 그대로)
const PAGES: [[(&str, &str); 4]; 2] = [
    [("messages", "메시지"), ("money", "돈"), ("work", "일"), ("shop", "상점")],
    [("ranking", "랭킹"), ("inventory", "인벤토리"), ("business", "사업"), ("untitled3", "SNS")],
];

const VERT: &str = "\nattribute vec2 aPos;\nvoid main() { gl_Position = vec4(aPos, 0.0, 1.0); }\n";
const FRAG: &str = include_str!("tabbar.frag");

fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    hi.min(lo.max(v))
}
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
fn now() -> f64 {
    js_sys::Reflect::get(&js_sys::global(), &"performance".into())
        .ok()
        .and_then(|p| js_sys::Reflect::get(&p, &"now".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()).and_then(|f| f.call0(&p).ok()))
        .and_then(|v| v.as_f64())
        .unwrap_or_else(js_sys::Date::now)
}
fn num(o: &JsValue, k: &str) -> f64 {
    js_sys::Reflect::get(o, &JsValue::from_str(k)).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}
fn win() -> web_sys::Window {
    web_sys::window().expect("window")
}

/* ---------- 스프링 ---------- */

// 매 프레임 dt만큼을 정확히 풀어요. 목표가 바뀌어도 위치와 속도는 이어져요.
struct Spring {
    x: f64,
    v: f64,
    target: f64,
    w: f64,
    z: f64,
    eps: f64,
}
impl Spring {
    fn new(value: f64, p: Preset, eps: f64) -> Self {
        Spring { x: value, v: 0.0, target: value, w: p.w, z: p.z, eps }
    }
    fn use_(&mut self, p: Preset) {
        self.w = p.w;
        self.z = p.z;
    }
    fn snap(&mut self, value: f64) {
        self.x = value;
        self.target = value;
        self.v = 0.0;
    }
    fn step(&mut self, dt: f64) {
        let y0 = self.x - self.target;
        let v0 = self.v;
        let (w, z) = (self.w, self.z);
        let (y, v);
        if z < 1.0 {
            let wd = w * (1.0 - z * z).sqrt();
            let e = (-z * w * dt).exp();
            let c = (wd * dt).cos();
            let s = (wd * dt).sin();
            y = e * (y0 * c + ((v0 + z * w * y0) / wd) * s);
            v = e * (v0 * c - ((z * w * v0 + w * w * y0) / wd) * s);
        } else {
            let e = (-w * dt).exp();
            y = e * (y0 + (v0 + w * y0) * dt);
            v = e * (v0 - w * (v0 + w * y0) * dt);
        }
        self.x = self.target + y;
        self.v = v;
    }
    fn settled(&self) -> bool {
        (self.x - self.target).abs() < self.eps && self.v.abs() < self.eps * 20.0
    }
}

/* ---------- 색 ---------- */

type Rgba = [f64; 4];

// "#3182f6", "rgb(...)", "rgba(...)" 를 [r, g, b, a] (0~1)로 바꿔요. 낯선 표기는 캔버스에 맡겨요.
fn parse_color(text: &str) -> Rgba {
    let t = text.trim();
    if let Some(h) = t.strip_prefix('#') {
        if (3..=8).contains(&h.len()) && h.chars().all(|c| c.is_ascii_hexdigit()) {
            let h: String = if h.len() == 3 || h.len() == 4 { h.chars().flat_map(|c| [c, c]).collect() } else { h.to_string() };
            let n = |i: usize| h.get(i..i + 2).and_then(|x| u8::from_str_radix(x, 16).ok()).map(|x| x as f64 / 255.0).unwrap_or(f64::NAN);
            return [n(0), n(2), n(4), if h.len() == 8 { n(6) } else { 1.0 }];
        }
    }
    let low = t.to_ascii_lowercase();
    if (low.starts_with("rgb(") || low.starts_with("rgba(")) && t.ends_with(')') {
        let inner = &t[t.find('(').unwrap_or(0) + 1..t.len() - 1];
        if !inner.contains(')') {
            let v: Vec<f64> = inner
                .split(|c: char| c.is_whitespace() || c == ',' || c == '/')
                .filter(|x| !x.is_empty())
                .map(|x| if let Some(p) = x.strip_suffix('%') { js_float(p) / 100.0 } else { js_float(x) })
                .collect();
            let g = |i: usize| v.get(i).copied().unwrap_or(f64::NAN);
            return [g(0) / 255.0, g(1) / 255.0, g(2) / 255.0, if v.len() > 3 { g(3) } else { 1.0 }];
        }
    }
    COLOR_CTX.with(|c| {
        let Some(ctx) = c else { return [0.0, 0.0, 0.0, 0.0] };
        ctx.clear_rect(0.0, 0.0, 1.0, 1.0);
        ctx.set_fill_style_str("#000");
        ctx.set_fill_style_str(t);
        ctx.fill_rect(0.0, 0.0, 1.0, 1.0);
        let d = ctx.get_image_data(0.0, 0.0, 1.0, 1.0).map(|d| d.data().0).unwrap_or_default();
        let g = |i: usize| d.get(i).copied().unwrap_or(0) as f64 / 255.0;
        [g(0), g(1), g(2), g(3)]
    })
}
fn js_float(s: &str) -> f64 {
    js_sys::Number::parse_float(s)
}
thread_local! {
    static COLOR_CTX: Option<C2d> = {
        let doc = crate::dom::doc();
        let c: HtmlCanvasElement = doc.create_element("canvas").unwrap().unchecked_into();
        c.set_width(1);
        c.set_height(1);
        let o = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&o, &"willReadFrequently".into(), &true.into());
        c.get_context_with_context_options("2d", &o).ok().flatten().map(|x| x.unchecked_into())
    };
    static BAR: RefCell<Option<Rc<RefCell<Bar>>>> = const { RefCell::new(None) };
    static FRAME: RefCell<Option<Closure<dyn FnMut(f64)>>> = const { RefCell::new(None) };
}

/* ---------- 탭 바 ---------- */

struct Label {
    cx: f64,
    cy: f64,
    font: String,
}
struct Geo {
    cx: f64,
    cy: f64,
    w: f64,
    h: f64,
    ix: f64,
    iy: f64,
    isize: f64,
    label: Option<Label>,
}
struct Size {
    pill_hw: f64,
    pill_hh: f64,
    lens_hw: f64,
    lens_hh: f64,
    pill_exp: f64,
    lens_exp: f64,
}
#[derive(Clone, Copy)]
struct Plat {
    cx: f64,
    cy: f64,
    hw: f64,
    hh: f64,
}
struct Press {
    id: i32,
    x0: f64,
    tab: HtmlElement,
    tab_index: usize,
    dragging: bool,
    was_current: bool,
    t0: f64,
    lx: f64,
    lt: f64,
    flick: bool,
}
struct Shot {
    tex: WebGlTexture,
    blur: WebGlTexture,
    css_w: f64,
    css_h: f64,
}
struct Theme {
    ink: Rgba,
    accent: Rgba,
    pill: Rgba,
    tint: Rgba,
}
struct Gfx {
    gl: GL,
    u: Vec<(&'static str, Option<WebGlUniformLocation>)>,
    tex: WebGlTexture,
    tex_a: WebGlTexture,
    tex_b: WebGlTexture,
    empty: WebGlTexture,
    plain: WebGlTexture, // 사진이 아직 없을 때 비추는 화면 바탕색 (1×1)
}
impl Gfx {
    fn u(&self, n: &str) -> Option<&WebGlUniformLocation> {
        self.u.iter().find(|(k, _)| *k == n).and_then(|(_, v)| v.as_ref())
    }
}

/// 탭 바가 밖으로 알리는 일 (빌린 것을 놓은 뒤에 보내요: 받는 쪽이 다시 탭 바를 부를 수 있어서요)
enum Out {
    Change(HtmlElement, usize),
    Reselect(HtmlElement),
}

struct Bar {
    nav: HtmlElement,
    backdrop: JsValue,
    page_root: HtmlElement,
    shots: Vec<(String, Shot)>,
    plat_a: Option<HtmlElement>,
    plat_b: Option<HtmlElement>,
    grab_group: String,
    canvas: HtmlCanvasElement,
    tabs: Vec<HtmlElement>,
    pill_el: Option<HtmlElement>,
    reduced: Option<web_sys::MediaQueryList>,
    current: usize,
    press: Option<Press>,
    releasing: bool,
    last_pointer_at: f64,
    geo: Vec<Geo>,
    sizes: Vec<Size>,
    running: bool,
    last_time: f64,
    ready: bool,
    glass_on: bool,
    glass_mix: f64,
    x: Spring,
    lift: Spring,
    grab: Spring,
    stretch: Spring,
    excite: Spring,
    page: usize,
    sel_page: usize,
    page_s: Spring,
    g: Option<Gfx>,
    dpr: f64,
    cw: f64,
    ch: f64,
    theme: Theme,
    bar_a: Option<Plat>,
    bar_b: Option<Plat>,
    app_off: (f64, f64),
    masks: Vec<HtmlCanvasElement>,
    out: Vec<Out>,
    want_frame: bool,
    plain_px: [u8; 4],
}

fn with<R>(f: impl FnOnce(&mut Bar) -> R) -> Option<R> {
    let rc = BAR.with(|b| b.borrow().clone())?;
    let r = {
        let mut b = rc.try_borrow_mut().ok()?;
        f(&mut b)
    };
    flush();
    Some(r)
}

// 미뤄 둔 알림과 다음 프레임 요청을 보내요
fn flush() {
    let Some(rc) = BAR.with(|b| b.borrow().clone()) else { return };
    let (outs, frame, nav) = {
        let Ok(mut b) = rc.try_borrow_mut() else { return };
        (std::mem::take(&mut b.out), std::mem::take(&mut b.want_frame), b.nav.clone())
    };
    if frame {
        FRAME.with(|f| {
            if let Some(c) = f.borrow().as_ref() {
                let _ = win().request_animation_frame(c.as_ref().unchecked_ref());
            }
        });
    }
    for o in outs {
        let (name, detail) = match o {
            Out::Change(tab, i) => ("tabchange", crate::dom::obj(&[("tab", tab.into()), ("index", (i as f64).into())])),
            Out::Reselect(tab) => ("tabreselect", crate::dom::obj(&[("tab", tab.into())])),
        };
        let init = web_sys::CustomEventInit::new();
        init.set_detail(&detail);
        init.set_bubbles(true);
        if let Ok(ev) = web_sys::CustomEvent::new_with_event_init_dict(name, &init) {
            let _ = nav.dispatch_event(&ev);
        }
    }
}

fn rect(el: &web_sys::Element) -> web_sys::DomRect {
    el.get_bounding_client_rect()
}

impl Bar {
    fn reduced(&self) -> bool {
        self.reduced.as_ref().is_some_and(|m| m.matches())
    }

    /* ----- DOM 아이콘 (대체 화면용이면서 접근성용) ----- */
    fn build_icons(&self) {
        let doc = crate::dom::doc();
        for tab in &self.tabs {
            let Some(holder) = tab.query_selector(".tab-icon").ok().flatten() else { continue };
            let Some(parts) = icons::icon(&tab.get_attribute("data-tab").unwrap_or_default()) else { continue };
            let Ok(svg) = doc.create_element_ns(Some(NS), "svg") else { continue };
            let _ = svg.set_attribute("viewBox", &format!("0 0 {0} {0}", icons::SIZE));
            let _ = svg.set_attribute("aria-hidden", "true");
            for part in parts {
                let Ok(path) = doc.create_element_ns(Some(NS), "path") else { continue };
                let _ = path.set_attribute("d", part.d);
                let _ = path.set_attribute("fill", if part.fill { "currentColor" } else { "none" });
                if part.alpha != 1.0 {
                    let _ = path.set_attribute("fill-opacity", &js_num(part.alpha));
                }
                if part.evenodd {
                    let _ = path.set_attribute("fill-rule", "evenodd");
                }
                if part.stroke != 0.0 {
                    let _ = path.set_attribute("stroke", "currentColor");
                    let _ = path.set_attribute("stroke-width", &js_num(part.stroke));
                    let _ = path.set_attribute("stroke-linecap", "round");
                    let _ = path.set_attribute("stroke-linejoin", "round");
                }
                if let Some(t) = part.transform {
                    let _ = path.set_attribute("transform", &format!("matrix({})", t.iter().map(|v| js_num(*v)).collect::<Vec<_>>().join(" ")));
                }
                let _ = svg.append_child(&path);
            }
            holder.set_inner_html("");
            let _ = holder.append_child(&svg);
        }
    }

    /* ----- WebGL ----- */
    fn init_gl(&self) -> Option<Gfx> {
        let o = crate::dom::obj(&[
            ("alpha", true.into()),
            ("premultipliedAlpha", true.into()),
            ("antialias", false.into()),
            ("depth", false.into()),
            ("stencil", false.into()),
            ("powerPreference", "low-power".into()),
        ]);
        let gl: GL = self.canvas.get_context_with_context_options("webgl", &o).ok().flatten()?.dyn_into().ok()?;
        let compile = |ty: u32, src: &str| {
            let sh = gl.create_shader(ty)?;
            gl.shader_source(&sh, src);
            gl.compile_shader(&sh);
            if !gl.get_shader_parameter(&sh, GL::COMPILE_STATUS).as_bool().unwrap_or(false) {
                web_sys::console::warn_2(&"셰이더 컴파일 실패:".into(), &gl.get_shader_info_log(&sh).unwrap_or_default().into());
                return None;
            }
            Some(sh)
        };
        let vs = compile(GL::VERTEX_SHADER, VERT)?;
        let fs = compile(GL::FRAGMENT_SHADER, FRAG)?;
        let prog = gl.create_program()?;
        gl.attach_shader(&prog, &vs);
        gl.attach_shader(&prog, &fs);
        gl.link_program(&prog);
        if !gl.get_program_parameter(&prog, GL::LINK_STATUS).as_bool().unwrap_or(false) {
            web_sys::console::warn_2(&"셰이더 연결 실패:".into(), &gl.get_program_info_log(&prog).unwrap_or_default().into());
            return None;
        }
        gl.use_program(Some(&prog));
        let buf = gl.create_buffer()?;
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&buf));
        let verts = js_sys::Float32Array::from(&[-1.0f32, -1.0, 3.0, -1.0, -1.0, 3.0][..]);
        gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &verts, GL::STATIC_DRAW);
        let loc = gl.get_attrib_location(&prog, "aPos") as u32;
        gl.enable_vertex_attrib_array(loc);
        gl.vertex_attrib_pointer_with_i32(loc, 2, GL::FLOAT, false, 0, 0);
        let names = [
            "uMask", "uMaskA", "uMaskB", "uSlide", "uSlideW", "uClip", "uBack", "uBlur", "uPx", "uDpr", "uSize", "uGlass", "uGlassMix", "uPageOff", "uPageSize", "uBarA", "uBarB", "uPress", "uLens", "uExp", "uLift", "uInk", "uAccent", "uPill", "uTint", "uDark",
        ];
        let u = names.iter().map(|n| (*n, gl.get_uniform_location(&prog, n))).collect();
        let tex = make_texture(&gl, None)?;
        let tex_a = make_texture(&gl, None)?;
        let tex_b = make_texture(&gl, None)?;
        gl.disable(GL::BLEND);
        gl.clear_color(0.0, 0.0, 0.0, 0.0);
        let empty = make_texture(&gl, None)?;
        let plain = make_texture(&gl, None)?;
        Some(Gfx { gl, u, tex, tex_a, tex_b, empty, plain })
    }

    // WebGL을 쓰는지에 따라 보이는 것이 달라져요 (못 쓰면 DOM 아이콘, CSS 유리 판, CSS 알약)
    fn use_gl(&mut self, g: Option<Gfx>) {
        let on = g.is_some();
        self.g = g;
        let cl = self.nav.class_list();
        let _ = cl.toggle_with_force("is-gl", on);
        let _ = cl.toggle_with_force("is-fallback", !on);
        if !on {
            self.glass_on = false;
            self.glass_mix = 0.0;
            let _ = cl.remove_1("has-glass");
        }
    }

    // Backdrop이 화면 사진을 새로 찍을 때마다 불려요
    fn set_shot(&mut self, name: String, shot: &JsValue) {
        let Some(g) = &self.g else { return };
        let gl = &g.gl;
        let max = gl.get_parameter(GL::MAX_TEXTURE_SIZE).ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
        let Ok(canvas) = js_sys::Reflect::get(shot, &"canvas".into()).and_then(|c| c.dyn_into::<HtmlCanvasElement>().map_err(|e| e)) else { return };
        let Ok(blur) = js_sys::Reflect::get(shot, &"blur".into()).and_then(|c| c.dyn_into::<HtmlCanvasElement>().map_err(|e| e)) else { return };
        if canvas.width() as f64 > max || canvas.height() as f64 > max {
            return;
        }
        let (Some(tex), Some(bt)) = (make_texture(gl, Some(&canvas)), make_texture(gl, Some(&blur))) else { return };
        let s = Shot { tex, blur: bt, css_w: num(shot, "cssW"), css_h: num(shot, "cssH") };
        if let Some(slot) = self.shots.iter_mut().find(|(k, _)| *k == name) {
            gl.delete_texture(Some(&slot.1.tex));
            gl.delete_texture(Some(&slot.1.blur));
            slot.1 = s;
        } else {
            self.shots.push((name, s));
        }
        self.request_render();
    }

    // 낡은 화면 사진을 모두 버려요 (테마가 바뀌었을 때처럼 사진이 틀린 색일 때)
    fn drop_shots(&mut self) {
        if let Some(g) = &self.g {
            for (_, s) in &self.shots {
                g.gl.delete_texture(Some(&s.tex));
                g.gl.delete_texture(Some(&s.blur));
            }
        }
        self.shots.clear();
        self.request_render();
    }

    // 알약이 떠 있거나 움직이는 동안은 무거운 일(화면 사진 찍기)을 미뤄요
    fn busy(&self) -> bool {
        self.press.is_some() || self.releasing || self.lift.x > 0.02 || self.excite.x > 0.02 || !self.page_s.settled() || self.running
    }

    /* ----- 배치와 그리기 재료 ----- */
    fn read_theme(&mut self) {
        let Ok(Some(cs)) = win().get_computed_style(&self.nav) else { return };
        let get = |n: &str| {
            let v = cs.get_property_value(n).unwrap_or_default();
            parse_color(if v.is_empty() { "#000" } else { &v })
        };
        self.theme = Theme { ink: get("--tab-ink"), accent: get("--tab-accent"), pill: get("--tab-pill"), tint: get("--tab-tint") };
    }

    fn layout(&mut self) {
        let nr = rect(&self.nav);
        if nr.width() < 10.0 || nr.height() < 10.0 {
            return;
        }
        let dpr = clamp(win().device_pixel_ratio(), 1.0, 3.0);
        self.dpr = if dpr == 0.0 { 1.0 } else { dpr };
        let dpr = self.dpr;
        self.cw = nr.width() + PAD_X * 2.0;
        self.ch = nr.height() + PAD_TOP + PAD_BOTTOM;
        self.read_theme();

        self.geo = self
            .tabs
            .iter()
            .map(|tab| {
                let r = rect(tab);
                let ir = tab.query_selector(".tab-icon").ok().flatten().map(|i| rect(&i));
                let lab = tab.query_selector(".tab-label").ok().flatten();
                let (ix, iy, isize) = ir.map(|ir| (ir.left() - nr.left() + PAD_X, ir.top() - nr.top() + PAD_TOP, ir.width())).unwrap_or_default();
                let label = lab.and_then(|lab| {
                    let text = lab.text_content().unwrap_or_default().trim().to_string();
                    if text.is_empty() {
                        return None;
                    }
                    let lr = rect(&lab);
                    let c = win().get_computed_style(&lab).ok().flatten()?;
                    let p = |n: &str| c.get_property_value(n).unwrap_or_default();
                    Some(Label {
                        cx: lr.left() - nr.left() + lr.width() / 2.0 + PAD_X,
                        cy: lr.top() - nr.top() + lr.height() / 2.0 + PAD_TOP,
                        font: format!("{} {} {}", p("font-weight"), p("font-size"), p("font-family")),
                    })
                });
                Geo {
                    cx: r.left() - nr.left() + r.width() / 2.0 + PAD_X,
                    cy: r.top() - nr.top() + r.height() / 2.0 + PAD_TOP,
                    w: r.width(),
                    h: r.height(),
                    ix,
                    iy,
                    isize,
                    label,
                }
            })
            .collect();

        // 탭마다 알약과 렌즈의 크기
        let bar_h = nr.height();
        self.sizes = self
            .geo
            .iter()
            .map(|g| {
                let r = g.w.min(g.h) / 2.0;
                Size {
                    pill_hw: r,
                    pill_hh: r,
                    lens_hw: bar_h * 0.56, // 렌즈는 옆으로 길지 않은 동그라미예요
                    lens_hh: bar_h * 0.56,
                    pill_exp: 2.0,
                    lens_exp: 2.0,
                }
            })
            .collect();

        if self.g.is_some() {
            let st = self.canvas.style();
            let _ = st.set_property("width", &format!("{}px", js_num(self.cw)));
            let _ = st.set_property("height", &format!("{}px", js_num(self.ch)));
            let _ = st.set_property("left", &format!("{}px", js_num(-PAD_X)));
            let _ = st.set_property("top", &format!("{}px", js_num(-PAD_TOP)));
            // 크기를 같은 값으로 다시 넣어도 캔버스가 지워져서 깜빡이니, 달라졌을 때만 바꿔요
            let w = js_round(self.cw * dpr) as u32;
            let h = js_round(self.ch * dpr) as u32;
            if self.canvas.width() != w {
                self.canvas.set_width(w);
            }
            if self.canvas.height() != h {
                self.canvas.set_height(h);
            }
            self.draw_mask();
            if let Some(g) = &self.g {
                g.gl.viewport(0, 0, w as i32, h as i32);
            }
        }

        if self.geo.is_empty() {
            return;
        }
        // 멈춰 있을 때는 선택된 탭 위로 맞춰요
        if self.press.is_none() && !self.releasing {
            let cx = self.geo[self.current].cx;
            self.x.snap(cx);
            self.lift.snap(0.0);
            self.grab.snap(0.0);
            self.stretch.snap(0.0);
            self.excite.snap(0.0);
            self.page_s.snap(self.page as f64);
        } else {
            let i = self.press.as_ref().map(|p| p.tab_index).unwrap_or(self.current);
            self.x.target = self.geo[i].cx;
        }
        // 막대 유리 모양 (캔버스 좌표). offset* 값은 눌림 때 CSS가 키우는 배율에 영향을 받지 않아요
        let plat = |el: &Option<HtmlElement>| {
            el.as_ref().map(|el| Plat {
                cx: el.offset_left() as f64 + el.offset_width() as f64 / 2.0 + PAD_X,
                cy: el.offset_top() as f64 + el.offset_height() as f64 / 2.0 + PAD_TOP,
                hw: el.offset_width() as f64 / 2.0,
                hh: el.offset_height() as f64 / 2.0,
            })
        };
        self.bar_a = plat(&self.plat_a);
        self.bar_b = plat(&self.plat_b);
        let app = rect(&self.page_root);
        self.app_off = (nr.left() - app.left(), nr.top() - app.top());

        self.place_fallback_pill();
        self.ready = true;
        // 캔버스가 지워졌을 수 있으니 같은 프레임 안에서 바로 다시 그려요
        self.render();
        self.request_render();
    }

    // 글자와 아이콘의 "덮개"를 흰색으로 그려요. 색은 셰이더가 입혀요.
    fn draw_mask(&mut self) {
        let dpr = self.dpr;
        if self.masks.is_empty() {
            let doc = crate::dom::doc();
            self.masks = (0..3).filter_map(|_| doc.create_element("canvas").ok().map(|c| c.unchecked_into())).collect();
        }
        let (cw, chh) = (self.canvas.width(), self.canvas.height());
        let paint = |mc: &HtmlCanvasElement, items: &[(&Geo, Option<&'static [Part]>, Option<(&Label, &str)>)]| {
            mc.set_width(cw);
            mc.set_height(chh);
            let Some(ctx) = mc.get_context("2d").ok().flatten().and_then(|c| c.dyn_into::<C2d>().ok()) else { return };
            let _ = ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
            ctx.clear_rect(0.0, 0.0, cw as f64, chh as f64);
            let _ = ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);
            ctx.set_fill_style_str("#fff");
            ctx.set_stroke_style_str("#fff");
            ctx.set_line_cap("round");
            ctx.set_line_join("round");
            for (g, parts, label) in items {
                if let Some(parts) = parts {
                    let k = g.isize / icons::SIZE;
                    for part in parts.iter() {
                        ctx.save();
                        let _ = ctx.translate(g.ix, g.iy);
                        let _ = ctx.scale(k, k);
                        if let Some(t) = part.transform {
                            let _ = ctx.transform(t[0], t[1], t[2], t[3], t[4], t[5]);
                        }
                        let Ok(path) = web_sys::Path2d::new_with_path_string(part.d) else {
                            ctx.restore();
                            continue;
                        };
                        if part.fill {
                            ctx.set_global_alpha(part.alpha);
                            ctx.fill_with_path_2d_and_winding(&path, if part.evenodd { web_sys::CanvasWindingRule::Evenodd } else { web_sys::CanvasWindingRule::Nonzero });
                            ctx.set_global_alpha(1.0);
                        }
                        if part.stroke != 0.0 {
                            ctx.set_line_width(part.stroke);
                            ctx.stroke_with_path(&path);
                        }
                        ctx.restore();
                    }
                }
                if let Some((l, text)) = label {
                    ctx.set_font(&l.font);
                    ctx.set_text_align("center");
                    ctx.set_text_baseline("middle");
                    let _ = ctx.fill_text(text, l.cx, l.cy);
                }
            }
        };
        let last = self.tabs.len() - 1;
        let last_name = self.tabs[last].get_attribute("data-tab").unwrap_or_default();
        paint(&self.masks[0], &[(&self.geo[last], icons::icon(&last_name), None)]);
        for p in 0..2 {
            let items: Vec<_> = PAGES[p]
                .iter()
                .enumerate()
                .filter_map(|(i, (id, label))| {
                    let g = self.geo.get(i)?;
                    Some((g, icons::icon(id), g.label.as_ref().map(|l| (l, *label))))
                })
                .collect();
            paint(&self.masks[p + 1], &items);
        }
        let Some(g) = &self.g else { return };
        let gl = &g.gl;
        for (i, tex) in [&g.tex, &g.tex_a, &g.tex_b].into_iter().enumerate() {
            gl.bind_texture(GL::TEXTURE_2D, Some(tex));
            gl.pixel_storei(GL::UNPACK_FLIP_Y_WEBGL, 0);
            gl.pixel_storei(GL::UNPACK_PREMULTIPLY_ALPHA_WEBGL, 0);
            let _ = gl.tex_image_2d_with_u32_and_u32_and_canvas(GL::TEXTURE_2D, 0, GL::RGBA as i32, GL::RGBA, GL::UNSIGNED_BYTE, &self.masks[i]);
        }
    }

    /* ----- 입력 ----- */
    fn tab_of(&self, e: &web_sys::Event) -> Option<HtmlElement> {
        let t: web_sys::Element = e.target()?.dyn_into().ok()?;
        let tab: HtmlElement = t.closest(".tab").ok().flatten()?.dyn_into().ok()?;
        if self.nav.contains(Some(&tab)) {
            Some(tab)
        } else {
            None
        }
    }
    fn index_of(&self, tab: &HtmlElement) -> Option<usize> {
        self.tabs.iter().position(|t| t == tab)
    }

    fn on_down(&mut self, e: &web_sys::PointerEvent) {
        if e.pointer_type() == "mouse" && e.button() != 0 {
            return;
        }
        let Some(tab) = self.tab_of(e) else { return };
        if self.press.is_some() || self.geo.is_empty() {
            return;
        }
        let Some(index) = self.index_of(&tab) else { return };
        let t = now();
        self.last_pointer_at = t;
        let _ = tab.set_pointer_capture(e.pointer_id());
        let x0 = num(e, "clientX");
        let group = tab.get_attribute("data-group").unwrap_or_default();
        self.press = Some(Press { id: e.pointer_id(), x0, tab, tab_index: index, dragging: false, was_current: self.is_selected(index), t0: t, lx: x0, lt: t, flick: false });
        let _ = self.nav.set_attribute("data-press", &group);
        self.grab_group = if group.is_empty() { "main".into() } else { group };
        let _ = self.nav.class_list().add_1("is-pressed");
        self.releasing = false;
        self.grab.target = 1.0;
        self.lift.target = if self.reduced() { 0.0 } else { 1.0 };
        self.x.use_(MOVE);
        self.x.target = self.geo[index].cx;
        if self.reduced() {
            self.x.snap(self.geo[index].cx);
        }
        self.request_render();
    }

    fn on_move(&mut self, e: &web_sys::PointerEvent) {
        let n = self.geo.len();
        let Some(p) = self.press.as_mut() else { return };
        if e.pointer_id() != p.id {
            return;
        }
        let cx = num(e, "clientX");
        let dx = cx - p.x0;
        let t = now();
        let v = (cx - p.lx) / (t - p.lt).max(1.0); // px/ms
        p.lx = cx;
        p.lt = t;
        // 빠르게 휙 넘기면 선택을 옮기는 게 아니라 핫바를 넘겨요. 렌즈는 제자리로 돌아가요
        if !p.flick && p.tab_index < 4 && dx.abs() > 10.0 && v.abs() > 0.4 {
            p.flick = true;
            self.x.use_(MOVE);
            self.x.target = self.geo[p.tab_index].cx;
        }
        if p.flick {
            return;
        }
        if !p.dragging && dx.abs() > 6.0 {
            p.dragging = true;
            self.x.use_(TRAIL);
        }
        if p.dragging {
            // 첫 탭과 마지막 칸 밖으로는 끌려가지 않아요
            let lo = self.geo[0].cx;
            let hi = self.geo[n - 1].cx;
            self.x.target = clamp(self.geo[p.tab_index].cx + dx, lo, hi);
            self.request_render();
        }
    }

    fn on_up(&mut self, e: &web_sys::PointerEvent, commit: bool) {
        match &self.press {
            Some(p) if p.id == e.pointer_id() => {}
            _ => return,
        }
        let p = self.press.take().unwrap();
        self.last_pointer_at = now();
        let _ = self.nav.class_list().remove_1("is-pressed");
        let _ = p.tab.release_pointer_capture(e.pointer_id());
        self.grab.target = 0.0;

        let dx = num(e, "clientX") - p.x0;
        let flipped = commit && p.tab_index < 4 && (p.flick || (dx.abs() >= 40.0 && now() - p.t0 < 420.0));
        let mut index = p.tab_index;
        if p.dragging && !flipped {
            let ahead = self.x.target + self.x.v * 0.1;
            index = self.nearest(ahead);
        }
        if !commit || flipped {
            index = self.current;
        }
        self.x.use_(MOVE);
        self.x.target = self.geo[index].cx;
        self.releasing = true;
        if flipped {
            self.flip_page();
            self.request_render();
            return;
        }
        if commit && self.is_selected(index) && !p.dragging && p.was_current {
            self.out.push(Out::Reselect(self.tabs[index].clone()));
        }
        if commit {
            self.select(index);
        }
        self.request_render();
    }

    // 키보드나 보조 기기로 누른 경우만 여기서 처리해요 (포인터로 누른 클릭은 이미 처리했어요)
    fn on_click(&mut self, e: &web_sys::Event) {
        let Some(tab) = self.tab_of(e) else { return };
        if now() - self.last_pointer_at < 700.0 {
            return;
        }
        let Some(index) = self.index_of(&tab) else { return };
        if self.is_selected(index) {
            self.out.push(Out::Reselect(tab));
            return;
        }
        self.go(index);
    }

    fn on_key(&self, e: &web_sys::KeyboardEvent) {
        let dir: i64 = match e.key().as_str() {
            "ArrowRight" => 1,
            "ArrowLeft" => -1,
            _ => 0,
        };
        if dir == 0 {
            return;
        }
        let act = crate::dom::doc().active_element();
        let Some(i) = self.tabs.iter().position(|t| act.as_ref().is_some_and(|a| a.is_same_node(Some(t)))) else { return };
        let next = (i as i64 + dir).clamp(0, self.tabs.len() as i64 - 1) as usize;
        let _ = self.tabs[next].focus();
        e.prevent_default();
    }

    fn nearest(&self, x: f64) -> usize {
        let mut best = 0;
        for (i, g) in self.geo.iter().enumerate() {
            if (g.cx - x).abs() < (self.geo[best].cx - x).abs() {
                best = i;
            }
        }
        best
    }

    /* ----- 선택 ----- */
    // 이 칸이 지금 선택돼 있나 (같은 칸이어도 핫바가 다르면 다른 탭이에요)
    fn is_selected(&self, index: usize) -> bool {
        index == self.current && (index >= 4 || self.sel_page == self.page)
    }
    fn mark_current(&self) {
        for (i, t) in self.tabs.iter().enumerate() {
            if self.is_selected(i) {
                let _ = t.set_attribute("aria-current", "page");
            } else {
                let _ = t.remove_attribute("aria-current");
            }
        }
    }
    fn select(&mut self, index: usize) {
        if self.is_selected(index) {
            return;
        }
        let crossing = index < 4 && self.sel_page != self.page;
        self.current = index;
        if index < 4 {
            self.sel_page = self.page;
        }
        if crossing {
            self.x.snap(self.geo[index].cx);
        }
        self.mark_current();
        self.place_fallback_pill();
        self.out.push(Out::Change(self.tabs[index].clone(), index));
    }

    // 핫바를 다음 쪽으로 넘겨요: 바 안의 내용만 짧고 굵게 튕기며 옆으로 밀려요
    fn flip_page(&mut self) {
        self.page = 1 - self.page;
        self.apply_page_dom();
        self.mark_current();
        self.place_fallback_pill();
        self.page_s.target = self.page as f64;
        if self.reduced() {
            self.page_s.snap(self.page as f64);
        } else {
            crate::dom::vibrate(8); // 짧고 가볍게 한 번
        }
        self.request_render();
    }

    // 버튼의 이름과 아이콘을 지금 보이는 쪽 것으로 바꿔요 (눌림·접근성·대체 화면용)
    fn apply_page_dom(&self) {
        for (i, (id, label)) in PAGES[self.page].iter().enumerate() {
            let Some(tab) = self.tabs.get(i) else { continue };
            let _ = tab.set_attribute("data-tab", id);
            if let Some(lab) = tab.query_selector(".tab-label").ok().flatten() {
                lab.set_text_content(Some(label));
            }
        }
        self.build_icons();
    }

    // 코드나 키보드로 옮길 때: 렌즈가 떠올라 미끄러지고 내려앉아요
    fn go(&mut self, index: usize) {
        self.x.use_(MOVE);
        self.x.target = self.geo[index].cx;
        if self.reduced() {
            self.x.snap(self.geo[index].cx);
        } else {
            self.lift.target = 1.0;
            self.releasing = true;
        }
        self.select(index);
        self.request_render();
    }

    fn place_fallback_pill(&self) {
        let Some(pill) = &self.pill_el else { return };
        let Some(t) = self.tabs.get(self.current) else { return };
        let nr = rect(&self.nav);
        let r = rect(t);
        pill.set_hidden(self.current < 4 && self.sel_page != self.page);
        let d = r.width().min(r.height());
        let st = pill.style();
        let _ = st.set_property("width", &format!("{}px", js_num(d)));
        let _ = st.set_property("height", &format!("{}px", js_num(d)));
        let _ = st.set_property("transform", &format!("translate({}px, {}px)", js_num(r.left() - nr.left() + (r.width() - d) / 2.0), js_num(r.top() - nr.top() + (r.height() - d) / 2.0)));
    }

    /* ----- 매 프레임 ----- */
    fn request_render(&mut self) {
        if self.running {
            return;
        }
        self.running = true;
        self.last_time = 0.0;
        self.want_frame = true;
    }

    fn frame(&mut self, t: f64) {
        if !self.running {
            return;
        }
        let dt = if self.last_time != 0.0 { clamp((t - self.last_time) / 1000.0, 0.0, 0.05) } else { 1.0 / 60.0 };
        self.last_time = t;

        // 도착하기 조금 전부터 렌즈를 내려놓기 시작해요
        if self.releasing && (self.x.x - self.x.target).abs() < LAND_PX {
            self.lift.target = 0.0;
            self.releasing = false;
        }
        self.excite.target = self.lift.target;

        self.x.step(dt);
        self.lift.step(dt);
        self.grab.step(dt);
        self.excite.step(dt);
        self.page_s.step(dt);

        // 빨리 움직일수록 살짝 길쭉해져요
        self.stretch.target = clamp(self.x.v.abs() / 800.0, 0.0, 1.0);
        self.stretch.step(dt);

        // 막대 유리가 처음 나타날 때는 CSS 유리 판에서 서서히 넘어가요
        if self.glass_on && self.glass_mix < 1.0 {
            self.glass_mix = if self.reduced() { 1.0 } else { (self.glass_mix + dt / GLASS_FADE).min(1.0) };
        }

        self.render();

        let idle = self.press.is_none()
            && !self.releasing
            && self.x.settled()
            && self.lift.settled()
            && self.grab.settled()
            && self.stretch.settled()
            && self.excite.settled()
            && self.page_s.settled()
            && (!self.glass_on || self.glass_mix >= 1.0);
        if idle {
            let (a, b, c, d, e) = (self.x.target, self.lift.target, self.grab.target, self.excite.target, self.page_s.target);
            self.x.snap(a);
            self.lift.snap(b);
            self.grab.snap(c);
            self.stretch.snap(0.0);
            self.excite.snap(d);
            self.page_s.snap(e);
            self.render();
            self.running = false;
        } else {
            self.want_frame = true;
        }
    }

    // 렌즈 중심이 x일 때 막대 유리 가장자리까지 쓸 수 있는 반폭이에요 (두 막대 유리 중 더 넉넉한 쪽)
    fn reach(&self, x: f64) -> f64 {
        let mut room = f64::NEG_INFINITY;
        for bar in [self.bar_a, self.bar_b].into_iter().flatten() {
            room = room.max((x - (bar.cx - bar.hw)).min(bar.cx + bar.hw - x));
        }
        room - LENS_INSET
    }

    // 현재 위치에서 알약/렌즈의 크기를 탭 사이로 이어서 계산해요
    fn shape(&self) -> (f64, f64, f64, f64, f64, f64) {
        let x = self.x.x;
        let g = &self.geo;
        let mut i = 0;
        while i + 2 < g.len() && x > g[i + 1].cx {
            i += 1;
        }
        let span = g[i + 1].cx - g[i].cx;
        let span = if span == 0.0 || span.is_nan() { 1.0 } else { span };
        let t = clamp((x - g[i].cx) / span, 0.0, 1.0);
        let (a, b) = (&self.sizes[i], &self.sizes[i + 1]);
        let pill_hw = lerp(a.pill_hw, b.pill_hw, t);
        let pill_hh = lerp(a.pill_hh, b.pill_hh, t);
        let lens_hw = lerp(a.lens_hw, b.lens_hw, t);
        let lens_hh = lerp(a.lens_hh, b.lens_hh, t);
        let pill_exp = lerp(a.pill_exp, b.pill_exp, t);
        let lens_exp = lerp(a.lens_exp, b.lens_exp, t);
        let cy = lerp(g[i].cy, g[i + 1].cy, t);

        let lift = clamp(self.lift.x, 0.0, 1.0);
        let bulge = 1.0 + 0.03 * clamp(self.grab.x, 0.0, 1.0);
        let s = clamp(self.stretch.x, 0.0, 1.0);
        let mut hw = lerp(pill_hw, lens_hw, lift) * bulge * (1.0 + 0.38 * s); // 움직이는 동안 옆으로 넓은 둥근 사각형이 됐다가 멈추면 동그랗게 돌아와요
        let mut hh = lerp(pill_hh, lens_hh, lift) * bulge * (1.0 - 0.08 * s);
        hw = hw.min(pill_hw.max(self.reach(x) + 14.0 * lift));
        let mut cx = x;
        if self.current < 4 {
            let d = self.sel_page as f64 - self.page_s.x;
            let vis = clamp(1.0 - d.abs() * 1.8, 0.0, 1.0);
            let bhw = self.bar_a.map(|b| b.hw).unwrap_or(f64::NAN);
            cx = if vis < 0.03 { -1000.0 } else { x + d * bhw * 2.0 };
            hw *= vis;
            hh *= vis;
        }
        (cx, cy - 0.5 * lift, hw.max(0.5), hh.max(0.5), lerp(pill_exp, lens_exp, lift) + 1.4 * s, lift)
    }

    fn render(&mut self) {
        if self.g.is_none() || !self.ready || self.geo.len() < 2 {
            return;
        }
        let (scx, scy, shw, shh, sexp, slift) = self.shape();
        let lift = clamp(slift, 0.0, 1.0);

        // 뒤 화면 사진 (지금 보이는 화면의 것). 새 화면의 사진이 아직 없으면 마지막으로 쓰던 사진을 그대로 써요
        let active = if self.backdrop.is_null() || self.backdrop.is_undefined() { None } else { js_sys::Reflect::get(&self.backdrop, &"active".into()).ok().and_then(|v| v.as_string()) };
        // 이 화면의 사진이 아직 없으면 다른 화면 사진을 빌리지 않고(다른 창이 비쳐 보였어요), 이 화면의 바탕색만 비춰요. 유리는 그대로 유리예요
        let entry = active.as_ref().and_then(|a| self.shots.iter().position(|(k, _)| k == a));
        // 사진이 하나도 없을 때(처음 켤 때, 테마가 바뀌어 버렸을 때, 그래픽 메모리를 되찾은 뒤)도 일반 판으로 돌아가지 않고 바탕색으로 유리를 그려요
        let plain = entry.is_none();
        if plain {
            let el = js_sys::Reflect::get(&self.backdrop, &"activeEl".into()).ok().and_then(|e| e.dyn_into::<web_sys::Element>().ok());
            let mut bg = el.and_then(|e| win().get_computed_style(&e).ok().flatten()).and_then(|c| c.get_property_value("background-color").ok()).unwrap_or_default();
            let c = parse_color(&bg);
            if c[3] < 0.5 {
                bg = crate::dom::doc().document_element().and_then(|r| win().get_computed_style(&r).ok().flatten()).and_then(|c| c.get_property_value("--bg-canvas").ok()).unwrap_or_else(|| "#f2f2f7".into());
            }
            let c = parse_color(if bg.trim().is_empty() { "#f2f2f7" } else { &bg });
            let px = [(c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, 255];
            if self.plain_px != px {
                self.plain_px = px;
                if let Some(g) = &self.g {
                    g.gl.bind_texture(GL::TEXTURE_2D, Some(&g.plain));
                    let _ = g.gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(GL::TEXTURE_2D, 0, GL::RGBA as i32, 1, 1, 0, GL::RGBA, GL::UNSIGNED_BYTE, Some(&px));
                }
            }
        }
        let glass_on = (entry.is_some() || plain) && self.bar_a.is_some() && self.bar_b.is_some();
        if glass_on != self.glass_on {
            self.glass_on = glass_on;
            if !glass_on {
                self.glass_mix = 0.0;
            }
            let _ = self.nav.class_list().toggle_with_force("has-glass", glass_on);
        }
        let scroll = if glass_on {
            js_sys::Reflect::get(&self.backdrop, &"activeEl".into()).ok().and_then(|e| e.dyn_into::<web_sys::Element>().ok()).map(|e| e.scroll_top() as f64).unwrap_or(0.0)
        } else {
            0.0
        };
        let dark = crate::dom::doc().document_element().and_then(|r| r.get_attribute("data-theme")).as_deref() == Some("dark");
        let g = self.g.as_ref().unwrap();
        let gl = &g.gl;
        let f = |v: f64| v as f32;
        let u1f = |n: &str, a: f64| gl.uniform1f(g.u(n), f(a));
        let u2f = |n: &str, a: f64, b: f64| gl.uniform2f(g.u(n), f(a), f(b));
        let u3f = |n: &str, a: f64, b: f64, c: f64| gl.uniform3f(g.u(n), f(a), f(b), f(c));
        let u4f = |n: &str, a: f64, b: f64, c: f64, d: f64| gl.uniform4f(g.u(n), f(a), f(b), f(c), f(d));
        let a = self.bar_a.unwrap_or(Plat { cx: f64::NAN, cy: f64::NAN, hw: f64::NAN, hh: f64::NAN });
        let b = self.bar_b.unwrap_or(Plat { cx: f64::NAN, cy: f64::NAN, hw: f64::NAN, hh: f64::NAN });

        gl.clear(GL::COLOR_BUFFER_BIT);
        gl.active_texture(GL::TEXTURE0);
        gl.bind_texture(GL::TEXTURE_2D, Some(&g.tex));
        gl.uniform1i(g.u("uMask"), 0);
        gl.active_texture(GL::TEXTURE3);
        gl.bind_texture(GL::TEXTURE_2D, Some(&g.tex_a));
        gl.uniform1i(g.u("uMaskA"), 3);
        gl.active_texture(GL::TEXTURE4);
        gl.bind_texture(GL::TEXTURE_2D, Some(&g.tex_b));
        gl.uniform1i(g.u("uMaskB"), 4);
        gl.active_texture(GL::TEXTURE0);
        u1f("uSlide", self.page_s.x);
        u1f("uSlideW", a.hw * 2.0);
        u2f("uClip", a.cx - a.hw, a.cx + a.hw);

        let ent = entry.map(|e| &self.shots[e].1);
        gl.active_texture(GL::TEXTURE1);
        let back = if plain { &g.plain } else { &g.empty };
        gl.bind_texture(GL::TEXTURE_2D, Some(ent.map(|e| &e.tex).unwrap_or(back)));
        gl.uniform1i(g.u("uBack"), 1);
        gl.active_texture(GL::TEXTURE2);
        gl.bind_texture(GL::TEXTURE_2D, Some(ent.map(|e| &e.blur).unwrap_or(back)));
        gl.uniform1i(g.u("uBlur"), 2);
        gl.active_texture(GL::TEXTURE0);
        u1f("uGlass", if glass_on { 1.0 } else { 0.0 });
        u1f("uGlassMix", self.glass_mix);
        u4f("uBarA", a.cx, a.cy, a.hw, a.hh);
        u4f("uBarB", b.cx, b.cy, b.hw, b.hh);
        if glass_on {
            let (pw, ph) = ent.map(|e| (e.css_w, e.css_h)).unwrap_or((self.cw, self.ch));
            u2f("uPageOff", self.app_off.0 - PAD_X, self.app_off.1 - PAD_TOP + scroll);
            u2f("uPageSize", pw, ph);
            let grab = clamp(self.grab.x, 0.0, 1.0);
            u2f("uPress", if self.grab_group == "main" { grab } else { 0.0 }, if self.grab_group == "side" { grab } else { 0.0 });
        }
        u2f("uPx", self.canvas.width() as f64, self.canvas.height() as f64);
        u1f("uDpr", self.canvas.width() as f64 / self.cw);
        u2f("uSize", self.cw, self.ch);
        u4f("uLens", scx, scy, shw, shh);
        u1f("uExp", sexp);
        u1f("uLift", lift);
        let th = &self.theme;
        u3f("uInk", th.ink[0], th.ink[1], th.ink[2]);
        u3f("uAccent", th.accent[0], th.accent[1], th.accent[2]);
        u4f("uPill", th.pill[0], th.pill[1], th.pill[2], th.pill[3]);
        u4f("uTint", th.tint[0], th.tint[1], th.tint[2], th.tint[3]);
        u1f("uDark", if dark { 1.0 } else { 0.0 });
        gl.draw_arrays(GL::TRIANGLES, 0, 3);
    }
}

fn make_texture(gl: &GL, source: Option<&HtmlCanvasElement>) -> Option<WebGlTexture> {
    let tex = gl.create_texture()?;
    gl.bind_texture(GL::TEXTURE_2D, Some(&tex));
    gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MIN_FILTER, GL::LINEAR as i32);
    gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MAG_FILTER, GL::LINEAR as i32);
    gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE as i32);
    gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE as i32);
    gl.pixel_storei(GL::UNPACK_FLIP_Y_WEBGL, 0);
    gl.pixel_storei(GL::UNPACK_PREMULTIPLY_ALPHA_WEBGL, 0);
    match source {
        Some(c) => {
            let _ = gl.tex_image_2d_with_u32_and_u32_and_canvas(GL::TEXTURE_2D, 0, GL::RGBA as i32, GL::RGBA, GL::UNSIGNED_BYTE, c);
        }
        None => {
            let _ = gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(GL::TEXTURE_2D, 0, GL::RGBA as i32, 1, 1, 0, GL::RGBA, GL::UNSIGNED_BYTE, Some(&[0, 0, 0, 0]));
        }
    }
    Some(tex)
}

/// JS가 숫자를 글자로 바꾸는 것과 같게 (1.0 → "1")
pub(crate) fn js_num(v: f64) -> String {
    js_sys::Number::from(v).to_string_with_radix(10).map(String::from).unwrap_or_default()
}
fn js_round(v: f64) -> f64 {
    (v + 0.5).floor()
}

fn on<E: JsCast + 'static>(t: &web_sys::EventTarget, ev: &str, f: impl FnMut(E) + 'static) {
    let mut f = f;
    crate::dom::on(t, ev, move |e: web_sys::Event| f(e.unchecked_into()));
}

/// 탭 바를 만들어요. 밖에서 부르는 일(busy·requestRender·dropShots·setShot·flipPage)을 가진 JS 객체를 돌려줘요 (window.tabbar)
pub fn create(nav: HtmlElement, backdrop: JsValue, page_root: HtmlElement, use_gl: bool) -> JsValue {
    let q = |s: &str| nav.query_selector(s).ok().flatten().and_then(|e| e.dyn_into::<HtmlElement>().ok());
    let tabs: Vec<HtmlElement> = crate::dom::qsa(&nav, ":scope > .tab");
    let canvas: HtmlCanvasElement = q(".tabbar-canvas").and_then(|c| c.dyn_into().ok()).unwrap_or_else(|| crate::dom::doc().create_element("canvas").unwrap().unchecked_into());
    let current = tabs.iter().position(|t| t.has_attribute("aria-current")).unwrap_or(0);
    let bar = Bar {
        plat_a: q(".platter-main"),
        plat_b: q(".platter-side"),
        pill_el: q(".pill"),
        reduced: win().match_media("(prefers-reduced-motion: reduce)").ok().flatten(),
        nav: nav.clone(),
        backdrop,
        page_root: page_root.clone(),
        shots: Vec::new(),
        grab_group: "main".into(),
        canvas: canvas.clone(),
        tabs,
        current,
        press: None,
        releasing: false,
        last_pointer_at: f64::NEG_INFINITY,
        geo: Vec::new(),
        sizes: Vec::new(),
        running: false,
        last_time: 0.0,
        ready: false,
        glass_on: false,
        glass_mix: 0.0,
        x: Spring::new(0.0, MOVE, 0.05),
        lift: Spring::new(0.0, LIFT, 0.002),
        grab: Spring::new(0.0, PRESS, 0.002),
        stretch: Spring::new(0.0, STRETCH, 0.002),
        excite: Spring::new(0.0, EXCITE, 0.002),
        page: 0,
        sel_page: 0,
        page_s: Spring::new(0.0, PAGE, 0.001),
        g: None,
        dpr: 1.0,
        cw: 0.0,
        ch: 0.0,
        theme: Theme { ink: [0.0; 4], accent: [0.0; 4], pill: [0.0; 4], tint: [0.0; 4] },
        bar_a: None,
        bar_b: None,
        app_off: (0.0, 0.0),
        masks: Vec::new(),
        out: Vec::new(),
        want_frame: false,
        plain_px: [0; 4],
    };
    let rc = Rc::new(RefCell::new(bar));
    BAR.with(|b| *b.borrow_mut() = Some(rc.clone()));
    FRAME.with(|f| {
        *f.borrow_mut() = Some(Closure::new(|t: f64| {
            with(|b| b.frame(t));
        }))
    });
    {
        let b = rc.borrow();
        b.build_icons();
    }
    // 입력
    let nt: &web_sys::EventTarget = nav.as_ref();
    on(nt, "contextmenu", |e: web_sys::Event| e.prevent_default());
    on(nt, "pointerdown", |e: web_sys::PointerEvent| {
        with(|b| b.on_down(&e));
    });
    on(nt, "pointermove", |e: web_sys::PointerEvent| {
        with(|b| b.on_move(&e));
    });
    on(nt, "pointerup", |e: web_sys::PointerEvent| {
        with(|b| b.on_up(&e, true));
    });
    on(nt, "pointercancel", |e: web_sys::PointerEvent| {
        with(|b| b.on_up(&e, false));
    });
    {
        let c = Closure::<dyn FnMut(web_sys::Event)>::new(|e: web_sys::Event| {
            with(|b| b.on_click(&e));
        });
        let _ = nav.add_event_listener_with_callback_and_bool("click", c.as_ref().unchecked_ref(), true);
        c.forget();
    }
    on(nt, "keydown", |e: web_sys::KeyboardEvent| {
        with(|b| b.on_key(&e));
    });
    // 그래픽 메모리가 모자라 컨텍스트를 잃으면 대체 화면으로 돌아가고, 돌아오면 다시 올려요
    on(canvas.as_ref(), "webglcontextlost", |e: web_sys::Event| {
        e.prevent_default();
        with(|b| b.use_gl(None));
    });
    on(canvas.as_ref(), "webglcontextrestored", |_: web_sys::Event| {
        let bd = with(|b| {
            b.shots.clear();
            let g = b.init_gl();
            b.use_gl(g);
            b.layout();
            b.backdrop.clone()
        });
        if let Some(bd) = bd {
            if let Ok(m) = js_sys::Reflect::get(&bd, &"shots".into()).and_then(|m| m.dyn_into::<js_sys::Map>().map_err(|e| e)) {
                m.for_each(&mut |shot, name| {
                    if js_sys::Reflect::get(&shot, &"canvas".into()).is_ok_and(|c| c.is_truthy()) {
                        if let Some(n) = name.as_string() {
                            with(|b| b.set_shot(n, &shot));
                        }
                    }
                });
            }
        }
    });
    // 화면이 스크롤되면 막대 유리가 보여주는 자리도 따라가요
    {
        let c = Closure::<dyn FnMut(web_sys::Event)>::new(|_: web_sys::Event| {
            with(|b| b.request_render());
        });
        let o = web_sys::AddEventListenerOptions::new();
        o.set_capture(true);
        o.set_passive(true);
        let _ = page_root.add_event_listener_with_callback_and_add_event_listener_options("scroll", c.as_ref().unchecked_ref(), &o);
        c.forget();
    }
    let refresh = || {
        with(|b| {
            b.read_theme();
            b.request_render();
        });
    };
    if let Ok(Some(mq)) = win().match_media("(prefers-color-scheme: dark)") {
        on(mq.as_ref(), "change", move |_: web_sys::Event| refresh());
    }
    {
        let c = Closure::<dyn FnMut(JsValue)>::new(move |_| refresh());
        if let Ok(mo) = web_sys::MutationObserver::new(c.as_ref().unchecked_ref()) {
            let o = web_sys::MutationObserverInit::new();
            o.set_attributes(true);
            o.set_attribute_filter(&js_sys::Array::of1(&"data-theme".into()));
            if let Some(r) = crate::dom::doc().document_element() {
                let _ = mo.observe_with_options(&r, &o);
            }
        }
        c.forget();
    }

    {
        let mut b = rc.borrow_mut();
        let g = if use_gl { b.init_gl() } else { None };
        b.use_gl(g);
    }

    // 막대 크기가 달라질 때뿐 아니라 앱 틀 높이가 달라질 때도 막대의 화면 안 위치가 바뀌어요
    {
        let c = Closure::<dyn FnMut(JsValue)>::new(|_| {
            with(|b| b.layout());
        });
        if let Ok(ro) = web_sys::ResizeObserver::new(c.as_ref().unchecked_ref()) {
            ro.observe(&nav);
            ro.observe(&page_root);
        }
        c.forget();
    }
    rc.borrow_mut().layout();
    flush();
    // 글꼴이 늦게 도착하면 글자를 다시 그려요
    if let Ok(fonts) = js_sys::Reflect::get(&crate::dom::doc(), &"fonts".into()) {
        if let Ok(ready) = js_sys::Reflect::get(&fonts, &"ready".into()).and_then(|p| p.dyn_into::<js_sys::Promise>().map_err(|e| e)) {
            let c = Closure::<dyn FnMut(JsValue)>::new(|_| {
                with(|b| b.layout());
            });
            let _ = ready.then(&c);
            c.forget();
        }
        if let Ok(t) = fonts.dyn_into::<web_sys::EventTarget>() {
            on(&t, "loadingdone", |_: web_sys::Event| {
                with(|b| b.layout());
            });
        }
    }

    // 밖에서 부르는 일
    let api = js_sys::Object::new();
    let set = |k: &str, v: JsValue| {
        let _ = js_sys::Reflect::set(&api, &JsValue::from_str(k), &v);
    };
    set("busy", Closure::<dyn FnMut() -> JsValue>::new(|| JsValue::from_bool(with(|b| b.busy()).unwrap_or(true))).into_js_value());
    set("requestRender", Closure::<dyn FnMut()>::new(|| {
        with(|b| b.request_render());
    }).into_js_value());
    set("dropShots", Closure::<dyn FnMut()>::new(|| {
        with(|b| b.drop_shots());
    }).into_js_value());
    set("setShot", Closure::<dyn FnMut(JsValue, JsValue)>::new(|name: JsValue, shot: JsValue| {
        let n = name.as_string().unwrap_or_default();
        with(|b| b.set_shot(n, &shot));
    }).into_js_value());
    set("flipPage", Closure::<dyn FnMut()>::new(|| {
        with(|b| b.flip_page());
    }).into_js_value());
    api.into()
}
