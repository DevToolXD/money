//! 상태: 어느 탭에서나 떠 있는 동그란 버튼. 누르면 돌면서 배고픔·목마름·에너지가 튀어나와요.
//! 낮아지면 화면 가장자리가 붉어지고, 배고픔이나 목마름이 0%가 되면 쓰러져서 쉬어요 (예전 app.js의 상태 부분).
//! 줄어드는 빠르기는 firestore.rules의 쓰러짐 검사와 같아야 해요.

use std::cell::{Cell, RefCell};

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::dom::{self, anim_always, by_id};

/// (키, 이름, 그림): 배고픔 24시간, 목마름 18시간, 에너지 30일에 100%가 다 닳아요. 에너지가 0이어도 쓰러지지 않아요
const STATS: [(&str, &str, &str); 3] = [
    ("h", "배고픔", r#"<path d="M7.500 4v5a2.500 2.500 0 0 0 5 0V4M10 4v16M17 20V4c-2.200 1.500-3 4-3 7h3"/>"#),
    ("t", "목마름", r#"<path d="M12 3.500C9 8 6.500 10.800 6.500 14.500a5.500 5.500 0 0 0 11 0C17.500 10.800 15 8 12 3.500z"/>"#),
    ("e", "에너지", r#"<path d="M13 3L6 13.500h5L10 21l7-10.500h-5z"/>"#),
];
const SIZE: f64 = 46.0;
const GAP: f64 = 70.0;

thread_local! {
    /// 서버에 기록된 상태 {h,t,e,at,deaths}. 값은 at부터 시간이 흐른 만큼 줄어든 것으로 계산해요
    static VIT: RefCell<JsValue> = const { RefCell::new(JsValue::NULL) };
    static VIT_TIMER: Cell<i32> = const { Cell::new(0) };
    static LOCK_TIMER: Cell<i32> = const { Cell::new(0) };
    static DYING: Cell<bool> = const { Cell::new(false) };
    static SHOWN: RefCell<[Option<i64>; 3]> = const { RefCell::new([None; 3]) };
    static OPEN: Cell<bool> = const { Cell::new(false) };
    static ANIMS: RefCell<Vec<JsValue>> = const { RefCell::new(Vec::new()) };
    static POS: Cell<(f64, f64)> = const { Cell::new((0.9, 0.36)) };
    static DRAG: RefCell<Option<(i32, f64, f64, f64, f64, bool)>> = const { RefCell::new(None) };
}
fn win() -> web_sys::Window {
    web_sys::window().expect("window")
}
fn vit() -> JsValue {
    VIT.with(|v| v.borrow().clone())
}
fn n(v: &JsValue, k: &str) -> f64 {
    cloud::get(v, k).as_f64().unwrap_or(0.0)
}
fn at_ms(v: &JsValue) -> f64 {
    cloud::ts_ms(&cloud::get(v, "at"))
}
/// 지금 상태 (배고픔, 목마름, 에너지). 에너지가 0이 된 뒤에는 배고픔·목마름이 두 배로 닳아요 (firestore.rules 와 같아요)
fn now_vals(v: &JsValue) -> [f64; 3] {
    let hrs = ((js_sys::Date::now() - at_ms(v)) / 3600000.0).max(0.0);
    let eff = hrs; // 베타: 에너지가 0이어도 두 배로 닳지 않아요
    [
        (n(v, "h") - eff * (100.0 / 24.0)).clamp(0.0, 100.0),
        (n(v, "t") - eff * (100.0 / 18.0)).clamp(0.0, 100.0),
        (n(v, "e") - hrs * (100.0 / (30.0 * 24.0))).clamp(0.0, 100.0),
    ]
}

/* ---------- 쓰러짐: 잠금 화면 ---------- */

fn stop_lock() {
    win().clear_interval_with_handle(LOCK_TIMER.get());
    dom::set_hidden("lock", true);
}
fn show_lock() {
    dom::set_hidden("lock", false);
    dom::set_text("lock-n", &format!("{}번째 쓰러졌어요", n(&vit(), "deaths")));
    let paint = || {
        let left = (at_ms(&vit()) - js_sys::Date::now()).max(0.0);
        if left <= 0.0 {
            stop_lock();
            start();
            return;
        }
        let hh = (left / 3600000.0).floor() as i64;
        let mm = ((left % 3600000.0) / 60000.0).floor() as i64;
        let ss = ((left % 60000.0) / 1000.0).floor() as i64;
        dom::set_text("lock-t", &format!("{}{mm:02}분 {ss:02}초 뒤에 일어나요", if hh > 0 { format!("{hh}시간 ") } else { String::new() }));
    };
    win().clear_interval_with_handle(LOCK_TIMER.get());
    paint();
    let c = Closure::<dyn FnMut()>::new(paint);
    LOCK_TIMER.set(win().set_interval_with_callback_and_timeout_and_arguments_0(c.as_ref().unchecked_ref(), 1000).unwrap_or(0));
    c.forget();
}

fn paint() {
    let v = vit();
    if v.is_null() || v.is_undefined() {
        return;
    }
    let Some(vig) = by_id("vignette") else { return };
        let vals = now_vals(&v);
    for (i, (k, _, _)) in STATS.iter().enumerate() {
        let x = vals[i].round().max(0.0) as i64;
        let changed = SHOWN.with(|s| {
            let mut s = s.borrow_mut();
            let c = s[i] != Some(x);
            s[i] = Some(x);
            c
        });
        if changed {
            if let Some(el) = dom::doc().query_selector(&format!("#fab-tiles [data-k=\"{k}\"]")).ok().flatten() {
                el.set_text_content(Some(&format!("{x}%")));
            }
        }
    }
    let low = vals[0].min(vals[1]).min(vals[2]);
    // 배고픔·목마름이 0이 되어도 쓰러지지 않아요 (쓰러짐은 없앴어요)
    let dead = false;
    let _ = low;
    // 목마름이 낮으면 붉은 가장자리, 배고픔이 낮으면 화면이 양옆으로 떨리고, 에너지가 낮으면 초점이 풀렸다 돌아와요
    let t = vals[1];
    vig.set_class_name(if t <= 5.0 { "vignette v5" } else if t <= 10.0 { "vignette v10" } else if t <= 20.0 { "vignette v20" } else { "vignette" });
    fx(vals[0], vals[2]);
    if dead && !DYING.get() {
        DYING.set(true);
        let deaths = n(&v, "deaths") + 1.0;
        leptos::task::spawn_local(async move {
            match cloud::call("die", &[deaths.into()]).await {
                Ok(nv) => {
                    VIT.with(|x| *x.borrow_mut() = nv);
                    if let Some(vig) = by_id("vignette") {
                        vig.set_class_name("vignette");
                    }
                    show_lock();
                }
                Err(e) => cloud::warn(&format!("쓰러짐을 기록하지 못했어요: {}", e.code)),
            }
            DYING.set(false);
        });
    }
}
fn start() {
    win().clear_interval_with_handle(VIT_TIMER.get());
    dom::set_hidden("status-fab", false);
    SHOWN.with(|s| *s.borrow_mut() = [None; 3]);
    paint();
    let c = Closure::<dyn FnMut()>::new(|| {
        if !dom::doc_hidden() {
            paint();
        }
    });
    VIT_TIMER.set(win().set_interval_with_callback_and_timeout_and_arguments_0(c.as_ref().unchecked_ref(), 5000).unwrap_or(0));
    c.forget();
}
/// 로그인한 뒤: 서버에서 상태를 읽어요
const OFF: bool = true;
pub fn load() {
    // 새로 만드는 동안은 몸 상태(배고픔·목마름·에너지)를 쉬어요: 상태 버튼과 화면 효과가 나오지 않아요
    if OFF {
        return;
    }
    leptos::task::spawn_local(async {
        match cloud::call("loadVitals", &[]).await {
            Ok(v) => {
                VIT.with(|x| *x.borrow_mut() = v.clone());
                // 예전에 쓰러져 쉬던 중이어도 잠그지 않아요
                start();
            }
            Err(e) => cloud::warn(&format!("상태를 불러오지 못했어요: {}", e.code)),
        }
    });
}
/// 다른 화면(인벤토리에서 먹기 등)이 바뀐 상태를 알려줘요
pub fn set(v: JsValue) {
    VIT.with(|x| *x.borrow_mut() = v);
    start();
}
pub fn get() -> JsValue {
    vit()
}

/* ---------- 동그란 버튼 ---------- */

fn tiles() -> Vec<web_sys::HtmlElement> {
    by_id("fab-tiles").map(|t| dom::qsa(&t, ".fab-tile")).unwrap_or_default()
}
/// 버튼에서 위나 아래로 일렬로 세워요 (화면 가운데 쪽으로)
fn toggle(on: bool) {
    let (Some(bubble), Some(app)) = (by_id("status-bubble"), dom::app_el()) else { return };
    OPEN.set(on);
    dom::toggle(&bubble, "is-open", on);
    let _ = bubble.set_attribute("aria-expanded", &on.to_string());
    ANIMS.with(|a| a.borrow_mut().drain(..).for_each(|x| dom::cancel(&x)));
    let a = app.get_bounding_client_rect();
    let r = bubble.get_bounding_client_rect();
    let th = if r.top() + SIZE / 2.0 < a.top() + a.height() * 0.5 { 90.0 } else { -90.0 }; // 90도는 아래쪽
    for (i, t) in tiles().into_iter().enumerate() {
        let rad = 62.0 + i as f64 * GAP;
        let from = th - 260.0; // 돌면서 나와요
        let tf = |deg: f64, d: f64, sc: f64| format!("translate(-50%, -50%) rotate({deg}deg) translateX({d}px) rotate({}deg) scale({sc})", -deg);
        let f0 = format!(r#"{{"transform":"{}","opacity":0}}"#, tf(from, 0.0, 0.2));
        let f1 = format!(r#"{{"transform":"{}","opacity":1}}"#, tf(th, rad, 1.0));
        let frames = if on { format!("[{f0},{f1}]") } else { format!("[{f1},{f0}]") };
        let _ = t.class_list().remove_1("show");
        let _ = t.style().set_property("visibility", "visible");
        let anim = anim_always(&t, &frames, &format!(r#"{{"duration":{},"delay":{},"easing":"cubic-bezier(0.22, 1.2, 0.36, 1)","fill":"both"}}"#, if on { 620 } else { 420 }, i * 70));
        if let Some(x) = &anim {
            ANIMS.with(|a| a.borrow_mut().push(x.clone()));
        }
        if on {
            let t2 = t.clone();
            leptos::prelude::set_timeout(
                move || {
                    if OPEN.get() {
                        let _ = t2.class_list().add_1("show");
                    }
                },
                std::time::Duration::from_millis(520 + i as u64 * 70),
            ); // 퍼센트는 다 나온 뒤에 떠요
        } else if let Some(x) = anim {
            let t2 = t.clone();
            let _ = Reflect::set(
                &x,
                &JsValue::from_str("onfinish"),
                &Closure::once_into_js(move || {
                    if !OPEN.get() {
                        let _ = t2.style().set_property("visibility", "hidden");
                    }
                }),
            );
        }
    }
}
fn place() {
    let (Some(fab), Some(app)) = (by_id("status-fab"), dom::app_el()) else { return };
    let (x, y) = POS.get();
    let (w, h) = (app.client_width() as f64, app.client_height() as f64);
    let _ = fab.style().set_property("left", &format!("{}px", (x * w - SIZE / 2.0).clamp(2.0, (w - SIZE - 2.0).max(2.0))));
    let _ = fab.style().set_property("top", &format!("{}px", (y * h - SIZE / 2.0).clamp(2.0, (h - SIZE - 2.0).max(2.0))));
}

pub fn init() {
    let Some(fab) = by_id("status-fab") else { return };
    // 스크롤하는 동안에는 상태 버튼을 살짝 숨겨서 글자 위로 겹치지 않게 해요. 멈추면 다시 보여요
    let idle = std::rc::Rc::new(Cell::new(0));
    let on_scroll = {
        let fab = fab.clone();
        move |_e: web_sys::Event| {
            // 끌고 있거나 펼쳐 둔 동안에는 숨기지 않아요 (아이폰에서 끌 때 화면이 같이 튕기면 버튼이 사라져 보였어요)
            if DRAG.with(|d| d.borrow().is_some()) || OPEN.get() {
                return;
            }
            let _ = &fab; // 상태 동그라미는 스크롤해도 숨기지 않아요 (늘 보여요)
            win().clear_timeout_with_handle(idle.get());
            let f2 = fab.clone();
            let cb = Closure::once_into_js(move || {
                let _ = f2.class_list().remove_1("fab-away");
            });
            idle.set(win().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 450).unwrap_or(0));
        }
    };
    for sc in dom::qsa(&dom::doc().document_element().expect("html"), ".screen") {
        dom::on(&sc, "scroll", on_scroll.clone());
    }
    dom::on(&win(), "scroll", on_scroll);
    if let Some(t) = by_id("fab-tiles") {
        t.set_inner_html(
            &STATS
                .iter()
                .map(|(k, label, d)| format!(r#"<span class="fab-tile" role="img" aria-label="{label}"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">{d}</svg><b data-k="{k}">--%</b></span>"#))
                .collect::<String>(),
        );
    }
    for t in tiles() {
        let _ = t.style().set_property("visibility", "hidden");
    }
    // 끌어서 옮길 수 있고, 놓은 자리에 그대로 있어요. 위치는 이 기기에 기억해요
    if let Ok(v) = js_sys::JSON::parse(&dom::store_get("status-pos", "null")) {
        let (x, y) = (cloud::get(&v, "x").as_f64(), cloud::get(&v, "y").as_f64());
        if let (Some(x), Some(y)) = (x, y) {
            if (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y) {
                POS.set((x, y));
            }
        }
    }
    place();
    dom::on(&win(), "resize", |_| place());
    let Some(bubble) = by_id("status-bubble") else { return };
    let b2 = bubble.clone();
    dom::on(&bubble, "pointerdown", move |e| {
        let Some(pe) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
        let _ = b2.set_pointer_capture(pe.pointer_id());
        let r = b2.get_bounding_client_rect();
        let (x, y) = (pe.client_x() as f64, pe.client_y() as f64);
        DRAG.with(|d| *d.borrow_mut() = Some((pe.pointer_id(), x - r.left(), y - r.top(), x, y, false)));
    });
    dom::on(&bubble, "pointermove", |e| {
        let Some(pe) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
        let Some((id, dx, dy, x0, y0, moved)) = DRAG.with(|d| *d.borrow()) else { return };
        if pe.pointer_id() != id {
            return;
        }
        let (x, y) = (pe.client_x() as f64, pe.client_y() as f64);
        if !moved && (x - x0).hypot(y - y0) < 6.0 {
            return;
        }
        DRAG.with(|d| *d.borrow_mut() = Some((id, dx, dy, x0, y0, true)));
        let Some(app) = dom::app_el() else { return };
        let a = app.get_bounding_client_rect();
        POS.set(((x - a.left() - dx + SIZE / 2.0) / a.width(), (y - a.top() - dy + SIZE / 2.0) / a.height()));
        place();
    });
    dom::on(&bubble, "pointerup", |e| {
        let Some(pe) = e.dyn_ref::<web_sys::PointerEvent>() else { return };
        let Some((id, .., moved)) = DRAG.with(|d| *d.borrow()) else { return };
        if pe.pointer_id() != id {
            return;
        }
        DRAG.with(|d| *d.borrow_mut() = None);
        if moved {
            let (x, y) = POS.get();
            dom::store_set("status-pos", &format!(r#"{{"x":{x},"y":{y}}}"#));
        } else {
            toggle(!OPEN.get());
        }
    });
    dom::on(&bubble, "pointercancel", |_| DRAG.with(|d| *d.borrow_mut() = None));
    // 시험과 다른 화면이 쓰는 이름: window.__vitals.get() / set(v)
    let api = js_sys::Object::new();
    let _ = Reflect::set(&api, &JsValue::from_str("get"), &Closure::<dyn Fn() -> JsValue>::new(get).into_js_value());
    let _ = Reflect::set(&api, &JsValue::from_str("set"), &Closure::<dyn Fn(JsValue)>::new(set).into_js_value());
    let _ = Reflect::set(&js_sys::global(), &JsValue::from_str("__vitals"), &api);
}

thread_local! {
    static FX_T: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}
/// 1초마다 불려요. 배고프면 5초(아주 배고프면 3초)마다 0.5초 떨리고, 지치면 6초(바닥이면 4초)마다 1.2초 흐려져요
fn fx(h: f64, e: f64) {
    if dom::reduce_motion() {
        return;
    }
    let t = FX_T.with(|c| {
        let v = c.get().wrapping_add(1);
        c.set(v);
        v
    });
    let later = |ms: i32, f: Box<dyn FnOnce()>| {
        let cb = Closure::once_into_js(f);
        let _ = win().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), ms);
    };
    if h <= 20.0 && t % (if h <= 5.0 { 3 } else { 5 }) == 0 {
        if let Some(sc) = dom::doc().query_selector(".screen.is-active").ok().flatten() {
            let _ = sc.class_list().add_1("vit-shaking");
            later(520, Box::new(move || {
                let _ = sc.class_list().remove_1("vit-shaking");
            }));
        }
    }
    if e <= 20.0 && t % (if e <= 3.0 { 4 } else { 6 }) == 0 {
        let fx = by_id("vit-fx").or_else(|| {
            let app = dom::app_el()?;
            let d = dom::doc().create_element("div").ok()?;
            d.set_id("vit-fx");
            d.set_class_name("vit-fx");
            let _ = d.set_attribute("aria-hidden", "true");
            let _ = app.append_child(&d);
            d.dyn_into::<web_sys::HtmlElement>().ok()
        });
        if let Some(fx) = fx {
            let _ = fx.class_list().toggle_with_force("strong", e <= 3.0);
            let _ = fx.class_list().add_1("on");
            later(1200, Box::new(move || {
                let _ = fx.class_list().remove_1("on");
            }));
        }
    }
}
