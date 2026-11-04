//! 탭 바 뒤의 화면을 사진으로 찍어 두는 곳 (예전 app/backdrop.js를 그대로 옮겼어요).
//! 막대 유리는 뒤의 화면을 휘게 보여줘야 하는데, 브라우저는 "지금 이 자리 뒤에 뭐가 있는지"를 픽셀로 알려주지 않아요.
//! 그래서 화면마다 전체 길이를 한 번 찍어 두고(html2canvas, 외부 라이브러리), 스크롤 위치만큼 밀어서 읽어요.
//!
//! - 화면이 바뀌거나 내용이 변하면 잠깐 뒤에 다시 찍어요 (탭 바를 만지는 동안은 미뤄요)
//! - 사진은 작게 줄인 흐림 버전도 같이 만들어요

use std::cell::RefCell;
use std::collections::HashSet;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, HtmlElement};

const MAX_TEXTURE: f64 = 4096.0;
// 화면 사진은 막대 유리 뒤에서 흐리게 보일 뿐이라 1.5배면 충분해요
const SCALE: f64 = 1.5;
const DEBOUNCE_MS: f64 = 700.0;
// 같은 화면을 다시 찍는 건 아무리 자주 바뀌어도 이만큼은 간격을 둬요. 화면 사진 찍기가 가장 무거운 일이에요
const MIN_GAP_MS: f64 = 6000.0;

struct Bd {
    api: js_sys::Object,
    root: HtmlElement,
    screens: Vec<(String, HtmlElement)>,
    is_busy: js_sys::Function,
    on_update: js_sys::Function,
    active: Option<String>,
    shots: js_sys::Map, // 이름 → { canvas, blur, cssW, cssH, dirty, takenAt } (시험이 window.backdrop.shots를 읽어요)
    queue: Vec<String>,
    working: bool,
    timer: i32,
    supported: bool,
    idle_done: bool,
    idle_waiting: bool,
}

thread_local! {
    static BD: RefCell<Option<Bd>> = const { RefCell::new(None) };
    static RUN: RefCell<Option<Closure<dyn FnMut()>>> = const { RefCell::new(None) };
}

fn win() -> web_sys::Window {
    web_sys::window().expect("window")
}
fn get(o: &JsValue, k: &str) -> JsValue {
    js_sys::Reflect::get(o, &JsValue::from_str(k)).unwrap_or(JsValue::UNDEFINED)
}
fn set(o: &JsValue, k: &str, v: &JsValue) {
    let _ = js_sys::Reflect::set(o, &JsValue::from_str(k), v);
}
fn with<R>(f: impl FnOnce(&mut Bd) -> R) -> Option<R> {
    BD.with(|b| b.try_borrow_mut().ok().and_then(|mut b| b.as_mut().map(f)))
}

impl Bd {
    fn shot(&self, name: &str) -> JsValue {
        self.shots.get(&JsValue::from_str(name))
    }
    fn el(&self, name: &str) -> Option<HtmlElement> {
        self.screens.iter().find(|(k, _)| k == name).map(|(_, e)| e.clone())
    }
    fn set_active(&mut self, name: String) {
        self.active = Some(name.clone());
        set(&self.api, "active", &JsValue::from_str(&name));
        set(&self.api, "activeEl", &self.el(&name).map(JsValue::from).unwrap_or(JsValue::NULL));
        let shot = self.shot(&name);
        let has = get(&shot, "canvas").is_truthy();
        let taken = get(&shot, "takenAt").as_f64().unwrap_or(0.0);
        let now = js_sys::Date::now();
        // 이미 찍어 둔 사진이 있으면 그대로 쓰고, 낡았을 때만 간격을 두고 다시 찍어요
        if has && get(&shot, "dirty").is_truthy() && now - taken < MIN_GAP_MS {
            self.pump(MIN_GAP_MS - (now - taken));
        } else {
            self.ensure(&name, !has);
        }
    }
    // 사진이 없거나 낡았으면 찍도록 줄을 세워요
    fn ensure(&mut self, name: &str, urgent: bool) {
        if !self.supported {
            return;
        }
        let shot = self.shot(name);
        if !shot.is_undefined() && !get(&shot, "dirty").is_truthy() {
            return;
        }
        if !self.queue.iter().any(|q| q == name) {
            if urgent {
                self.queue.insert(0, name.to_string());
            } else {
                self.queue.push(name.to_string());
            }
        }
        self.pump(if urgent { 0.0 } else { 40.0 });
    }
    fn invalidate(&mut self, name: &str) {
        if !self.supported {
            return;
        }
        let shot = self.shot(name);
        if shot.is_undefined() {
            self.shots.set(&JsValue::from_str(name), &crate::dom::obj(&[("dirty", true.into())]));
        } else {
            set(&shot, "dirty", &true.into());
        }
        if !self.queue.iter().any(|q| q == name) {
            self.queue.push(name.to_string());
        }
        // 보이지 않는 화면은 바로 찍지 않고, 그 화면이 보일 때(setActive) 찍어요
        if self.active.as_deref() != Some(name) {
            return;
        }
        let taken = get(&shot, "takenAt").as_f64().unwrap_or(0.0);
        let wait = DEBOUNCE_MS.max(MIN_GAP_MS - (js_sys::Date::now() - taken));
        self.pump(if get(&shot, "canvas").is_truthy() { wait } else { DEBOUNCE_MS });
    }
    fn invalidate_all(&mut self) {
        let names: Vec<String> = self.screens.iter().map(|(k, _)| k.clone()).collect();
        for n in names {
            self.invalidate(&n);
        }
    }
    fn pump(&mut self, delay: f64) {
        win().clear_timeout_with_handle(self.timer);
        self.timer = RUN.with(|r| r.borrow().as_ref().and_then(|c| win().set_timeout_with_callback_and_timeout_and_arguments_0(c.as_ref().unchecked_ref(), delay as i32).ok())).unwrap_or(0);
    }
}

fn run() {
    enum Next {
        Stop,
        Idle,
        Shoot(String),
    }
    let next = with(|b| {
        if b.working || b.queue.is_empty() {
            return Next::Stop;
        }
        // 탭 바가 움직이는 동안이나 화면이 꺼져 있을 때는 메인 스레드를 붙잡지 않아요
        let busy = b.is_busy.call0(&JsValue::NULL).map(|v| v.is_truthy()).unwrap_or(false);
        if busy || crate::dom::doc_hidden() {
            b.pump(400.0);
            return Next::Stop;
        }
        // 폰이 다른 일을 안 하는 틈에 찍어요
        if !b.idle_done {
            if !b.idle_waiting {
                b.idle_waiting = true;
                return Next::Idle;
            }
            return Next::Stop;
        }
        b.idle_done = false;
        b.working = true;
        Next::Shoot(b.queue.remove(0))
    })
    .unwrap_or(Next::Stop);
    match next {
        Next::Stop => {}
        Next::Idle => idle(|| {
            with(|b| {
                b.idle_waiting = false;
                b.idle_done = true;
            });
            run();
        }),
        Next::Shoot(name) => wasm_bindgen_futures::spawn_local(async move {
            match shoot(&name).await {
                Ok(Some(shot)) => {
                    let cb = with(|b| {
                        b.shots.set(&JsValue::from_str(&name), &shot);
                        b.on_update.clone()
                    });
                    if let Some(cb) = cb {
                        let _ = cb.call2(&JsValue::NULL, &JsValue::from_str(&name), &shot);
                    }
                }
                Ok(None) => {}
                Err(err) => web_sys::console::warn_3(&"화면 사진을 찍지 못했어요:".into(), &JsValue::from_str(&name), &err),
            }
            with(|b| {
                b.working = false;
                // 다음 화면은 한참 뒤에 찍어요 (여러 화면을 한꺼번에 찍으면 폰이 멈칫해요)
                if !b.queue.is_empty() {
                    b.pump(1200.0);
                }
            });
        }),
    }
}

fn idle(f: impl FnOnce() + 'static) {
    let c = Closure::once_into_js(f);
    let w = win();
    let ric = get(&w, "requestIdleCallback");
    if let Ok(ric) = ric.dyn_into::<js_sys::Function>() {
        let _ = ric.call2(&w, &c, &crate::dom::obj(&[("timeout", 2500.into())]));
    } else {
        let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(c.unchecked_ref(), 60);
    }
}

async fn shoot(name: &str) -> Result<Option<JsValue>, JsValue> {
    let Some((el, root)) = with(|b| b.el(name).map(|e| (e, b.root.clone()))).flatten() else { return Ok(None) };
    let doc = crate::dom::doc();
    if let Ok(ready) = get(&get(&doc, "fonts"), "ready").dyn_into::<js_sys::Promise>() {
        let _ = wasm_bindgen_futures::JsFuture::from(ready).await;
    }
    let css_w = el.client_width() as f64;
    let css_h = (el.scroll_height() as f64).max(el.client_height() as f64);
    if css_w < 10.0 || css_h < 10.0 {
        return Ok(None);
    }
    let dpr = win().device_pixel_ratio();
    let dpr = if dpr == 0.0 { 1.0 } else { dpr };
    let scale = SCALE.min(dpr).min(MAX_TEXTURE / css_h).min(MAX_TEXTURE / css_w);
    // 앱 바탕색이 투명하면(그라디언트·이미지일 때) 사진이 검게 나오니 문서 바탕색으로 대신해요
    let mut bg = win().get_computed_style(&root).ok().flatten().and_then(|c| c.get_property_value("background-color").ok()).unwrap_or_default();
    let transparent = bg.contains("transparent") || {
        let t = bg.trim_end();
        (t.starts_with("rgba(") || t.starts_with("rgb(")) && t.ends_with(')') && t[..t.len() - 1].rsplit(',').next().is_some_and(|x| x.trim() == "0") && t.contains(',')
    };
    if transparent {
        let v = doc
            .document_element()
            .and_then(|r| win().get_computed_style(&r).ok().flatten())
            .and_then(|c| c.get_property_value("--bg-canvas").ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        bg = if v.is_empty() { "#ffffff".into() } else { v };
    }
    let h2c = get(&win(), "html2canvas").dyn_into::<js_sys::Function>()?;
    let onclone = Closure::<dyn FnMut(JsValue, JsValue)>::new(move |d: JsValue, clone: JsValue| {
        // 숨겨져 있던 화면도 보이게 하고, 스크롤 상자가 아니라 전체 길이로 펼쳐서 찍어요
        let add = |el: &JsValue, css: &str| {
            let st = get(el, "style");
            let old = get(&st, "cssText").as_string().unwrap_or_default();
            set(&st, "cssText", &JsValue::from_str(&format!("{old}{css}")));
        };
        add(&clone, &format!(";position:relative;inset:auto;height:auto;overflow:visible;visibility:visible;opacity:1;width:{}px;", crate::tabbar::js_num(css_w)));
        let wrap = get(&clone, "parentElement");
        if wrap.is_truthy() {
            add(&wrap, ";position:relative;inset:auto;height:auto;");
        }
        if let Ok(d) = d.dyn_into::<web_sys::Document>() {
            if let Ok(Some(app)) = d.query_selector(".app") {
                add(&app, ";height:auto;overflow:visible;");
            }
        }
    });
    // 찍는 화면 밖의 것(다른 화면·탭 바·떠 있는 판)은 복사하지 않아요. 사진에는 이 화면만 나오니 모양은 같고, 복사가 훨씬 가벼워져요
    let target = el.clone();
    let ignore = Closure::<dyn FnMut(JsValue) -> bool>::new(move |n: JsValue| {
        let Ok(n) = n.dyn_into::<web_sys::Element>() else { return false };
        if n.is_same_node(Some(&target)) || n.contains(Some(&target)) {
            return false;
        }
        if target.contains(Some(&n)) {
            return n.tag_name() == "SCRIPT";
        }
        // 화면 바깥: 문서 머리(글꼴·스타일)는 남기고 나머지 몸통은 빼요
        n.closest("head").ok().flatten().is_none()
    });
    let opts = crate::dom::obj(&[
        ("ignoreElements", ignore.as_ref().clone()),
        ("scale", scale.into()),
        ("backgroundColor", bg.into()),
        ("logging", false.into()),
        ("width", css_w.into()),
        ("height", css_h.into()),
        ("windowWidth", css_w.into()),
        ("windowHeight", css_h.into()),
        ("scrollX", 0.into()),
        ("scrollY", 0.into()),
        ("onclone", onclone.as_ref().clone()),
    ]);
    let p: js_sys::Promise = h2c.call2(&JsValue::NULL, &el, &opts)?.dyn_into()?;
    let canvas: HtmlCanvasElement = wasm_bindgen_futures::JsFuture::from(p).await?.dyn_into()?;
    drop(onclone);
    drop(ignore);

    // 흐린 버전: 작게 줄였다가 늘려서 보여요 (샘플링이 부드럽게 이어져서 뿌옇게 보여요)
    let blur: HtmlCanvasElement = doc.create_element("canvas")?.unchecked_into();
    let k = 1.0 / 6.0;
    blur.set_width((canvas.width() as f64 * k + 0.5).floor().max(2.0) as u32);
    blur.set_height((canvas.height() as f64 * k + 0.5).floor().max(2.0) as u32);
    if let Some(b) = blur.get_context("2d")?.and_then(|c| c.dyn_into::<web_sys::CanvasRenderingContext2d>().ok()) {
        b.set_image_smoothing_enabled(true);
        set(&b, "imageSmoothingQuality", &"high".into());
        let _ = b.draw_image_with_html_canvas_element_and_dw_and_dh(&canvas, 0.0, 0.0, blur.width() as f64, blur.height() as f64);
    }
    Ok(Some(crate::dom::obj(&[
        ("canvas", canvas.into()),
        ("blur", blur.into()),
        ("cssW", css_w.into()),
        ("cssH", css_h.into()),
        ("dirty", false.into()),
        ("takenAt", js_sys::Date::now().into()),
    ])))
}

/// 뒤 화면 사진 찍는 곳을 만들어요. 밖에서 부르는 일(setActive·ensure·invalidate·invalidateAll)과 active·activeEl·shots를 가진 JS 객체를 돌려줘요 (window.backdrop)
pub fn create(root: HtmlElement, screens: Vec<(String, HtmlElement)>, is_busy: js_sys::Function, on_update: js_sys::Function) -> JsValue {
    let api = js_sys::Object::new();
    let shots = js_sys::Map::new();
    set(&api, "shots", &shots);
    set(&api, "active", &JsValue::NULL);
    set(&api, "activeEl", &JsValue::NULL);
    let supported = get(&win(), "html2canvas").is_function();
    RUN.with(|r| *r.borrow_mut() = Some(Closure::new(run)));
    BD.with(|b| {
        *b.borrow_mut() = Some(Bd {
            api: api.clone(),
            root,
            screens: screens.clone(),
            is_busy,
            on_update,
            active: None,
            shots,
            queue: Vec::new(),
            working: false,
            timer: 0,
            supported,
            idle_done: false,
            idle_waiting: false,
        })
    });
    if supported {
        let scr = screens.clone();
        let c = Closure::<dyn FnMut(js_sys::Array)>::new(move |records: js_sys::Array| {
            let mut dirty: Vec<String> = Vec::new();
            let mut seen = HashSet::new();
            for r in records.iter() {
                let Ok(r) = r.dyn_into::<web_sys::MutationRecord>() else { continue };
                let Some(target) = r.target() else { continue };
                // 매초 바뀌는 숫자(data-volatile)나 끌고 있는 카드의 움직임은 내용이 바뀐 게 아니에요
                let node: Option<web_sys::Element> = if target.node_type() == 1 { target.clone().dyn_into().ok() } else { target.parent_element() };
                let Some(node) = node else { continue };
                if node.closest("[data-volatile], .lifting").ok().flatten().is_some() {
                    continue;
                }
                for (name, el) in &scr {
                    // 화면 자체의 class 변화는 탭을 옮길 때 켜고 끄는 것이라 내용 변화가 아니에요
                    let is_self_attr = target.is_same_node(Some(el)) && r.type_() == "attributes";
                    if el.contains(Some(&target)) && !is_self_attr && seen.insert(name.clone()) {
                        dirty.push(name.clone());
                    }
                }
            }
            with(|b| {
                for n in &dirty {
                    b.invalidate(n);
                }
            });
        });
        if let Ok(mo) = web_sys::MutationObserver::new(c.as_ref().unchecked_ref()) {
            let o = web_sys::MutationObserverInit::new();
            o.set_subtree(true);
            o.set_child_list(true);
            o.set_character_data(true);
            o.set_attributes(true);
            o.set_attribute_filter(&js_sys::Array::of1(&"class".into()));
            for (_, el) in &screens {
                let _ = mo.observe_with_options(el, &o);
            }
        }
        c.forget();
    }
    let f1 = |f: fn(&mut Bd, String)| {
        Closure::<dyn FnMut(JsValue)>::new(move |n: JsValue| {
            let n = n.as_string().unwrap_or_default();
            with(|b| f(b, n));
        })
        .into_js_value()
    };
    set(&api, "setActive", &f1(|b, n| b.set_active(n)));
    set(&api, "invalidate", &f1(|b, n| b.invalidate(&n)));
    set(
        &api,
        "ensure",
        &Closure::<dyn FnMut(JsValue, JsValue)>::new(|n: JsValue, urgent: JsValue| {
            let n = n.as_string().unwrap_or_default();
            with(|b| b.ensure(&n, urgent.is_truthy()));
        })
        .into_js_value(),
    );
    set(
        &api,
        "invalidateAll",
        &Closure::<dyn FnMut()>::new(|| {
            with(|b| b.invalidate_all());
        })
        .into_js_value(),
    );
    api.into()
}
