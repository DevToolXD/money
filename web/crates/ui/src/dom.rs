//! 화면 여러 곳에서 쓰는 작은 DOM 도움 (움직임, 찾기, 이벤트 연결, 글자 안전하게)

use js_sys::{Function, Reflect, JSON};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

pub fn doc() -> web_sys::Document {
    web_sys::window().and_then(|w| w.document()).expect("document")
}
/// id로 찾기
pub fn by_id(id: &str) -> Option<web_sys::HtmlElement> {
    doc().get_element_by_id(id)?.dyn_into().ok()
}
/// 선택자로 하나 찾기 (root 안에서)
pub fn qs(root: &web_sys::Element, sel: &str) -> Option<web_sys::HtmlElement> {
    root.query_selector(sel).ok().flatten()?.dyn_into().ok()
}
/// 선택자로 모두 찾기
pub fn qsa(root: &web_sys::Element, sel: &str) -> Vec<web_sys::HtmlElement> {
    let Ok(list) = root.query_selector_all(sel) else { return Vec::new() };
    (0..list.length()).filter_map(|i| list.item(i)?.dyn_into().ok()).collect()
}
/// 앱 틀 (.app): 전체 화면 창과 시트를 여기에 붙여요
pub fn app_el() -> Option<web_sys::HtmlElement> {
    doc().query_selector(".app").ok().flatten()?.dyn_into().ok()
}
pub fn set_hidden(id: &str, hidden: bool) {
    if let Some(e) = by_id(id) {
        e.set_hidden(hidden);
    }
}
pub fn set_text(id: &str, t: &str) {
    if let Some(e) = by_id(id) {
        e.set_text_content(Some(t));
    }
}
/// 입력칸 글자 (없으면 빈 글자)
pub fn input_value(id: &str) -> String {
    doc().get_element_by_id(id).and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok()).map(|i| i.value()).unwrap_or_default()
}
/// 칸에 적은 숫자 읽기 (쉼표는 무시해요). 숫자가 아니면 NaN (예전 JS의 Number()와 같아요)
pub fn read_num(id: &str) -> f64 {
    if doc().get_element_by_id(id).is_none() {
        return f64::NAN;
    }
    js_number(&input_value(id).replace(',', ""))
}
/// JS의 Number(글자): 빈 글자는 0, 숫자가 아니면 NaN
pub fn js_number(s: &str) -> f64 {
    js_sys::Number::from(JsValue::from_str(s.trim())).value_of()
}

/// 이벤트 연결 (화면이 살아 있는 동안 계속 들어요)
pub fn on(target: &web_sys::EventTarget, ev: &str, f: impl FnMut(web_sys::Event) + 'static) {
    let c = Closure::<dyn FnMut(web_sys::Event)>::new(f);
    let _ = target.add_event_listener_with_callback(ev, c.as_ref().unchecked_ref());
    c.forget();
}
/// 이벤트 연결 (passive: false — 끌 때 화면이 같이 움직이지 않게 막아야 할 때)
pub fn on_active(target: &web_sys::EventTarget, ev: &str, f: impl FnMut(web_sys::Event) + 'static) {
    let c = Closure::<dyn FnMut(web_sys::Event)>::new(f);
    let o = web_sys::AddEventListenerOptions::new();
    o.set_passive(false);
    let _ = target.add_event_listener_with_callback_and_add_event_listener_options(ev, c.as_ref().unchecked_ref(), &o);
    c.forget();
}
/// 이벤트가 일어난 곳에서 가장 가까운 선택자 요소
pub fn closest(ev: &web_sys::Event, sel: &str) -> Option<web_sys::HtmlElement> {
    let t: web_sys::Element = ev.target()?.dyn_into().ok()?;
    t.closest(sel).ok().flatten()?.dyn_into().ok()
}
pub fn data(el: &web_sys::Element, key: &str) -> String {
    el.get_attribute(&format!("data-{key}")).unwrap_or_default()
}

/// 움직임을 줄였으면 true (prefers-reduced-motion)
pub fn reduce_motion() -> bool {
    web_sys::window()
        .and_then(|w| w.match_media("(prefers-reduced-motion: reduce)").ok().flatten())
        .map(|m| m.matches())
        .unwrap_or(false)
}
/// el.animate(frames, opts). frames와 opts는 JSON 글자예요. 움직임을 줄였으면 안 움직여요 (None)
pub fn anim(el: &web_sys::Element, frames: &str, opts: &str) -> Option<JsValue> {
    if reduce_motion() {
        return None;
    }
    anim_always(el, frames, opts)
}
/// 움직임 설정과 상관없이 움직여요 (사라지는 움직임이 끝나야 지우는 곳)
pub fn anim_always(el: &web_sys::Element, frames: &str, opts: &str) -> Option<JsValue> {
    let f: Function = Reflect::get(el, &JsValue::from_str("animate")).ok()?.dyn_into().ok()?;
    let frames = JSON::parse(frames).ok()?;
    let mut o = JSON::parse(opts).ok()?;
    if Reflect::get(&o, &JsValue::from_str("fill")).map(|v| v.is_undefined()).unwrap_or(true) {
        let _ = Reflect::set(&o, &JsValue::from_str("fill"), &JsValue::from_str("both"));
    }
    if o.is_null() {
        o = js_sys::Object::new().into();
    }
    f.call2(el, &frames, &o).ok()
}
/// 움직임이 끝나면 f를 불러요 (움직임이 없으면 바로)
pub fn on_finish(a: Option<JsValue>, f: impl FnOnce() + 'static) {
    match a {
        Some(a) => {
            let _ = Reflect::set(&a, &JsValue::from_str("onfinish"), &Closure::once_into_js(f));
        }
        None => f(),
    }
}
/// 애니메이션 취소
pub fn cancel(a: &JsValue) {
    if let Ok(f) = Reflect::get(a, &JsValue::from_str("cancel")).and_then(|f| f.dyn_into::<Function>()) {
        let _ = f.call0(a);
    }
}
/// 글자를 HTML 안에 안전하게 (예전 JS의 esc와 같아요)
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}
/// 클래스 넣고 빼기
pub fn toggle(el: &web_sys::Element, cls: &str, on: bool) {
    let _ = el.class_list().toggle_with_force(cls, on);
}
/// "통" 튀는 움직임을 다시 시작해요 (pop 클래스)
pub fn repop(el: &web_sys::HtmlElement) {
    let _ = el.class_list().remove_1("pop");
    let _ = el.offset_width();
    let _ = el.class_list().add_1("pop");
}
/// 짧게 떨기 (지원하는 기기만)
pub fn vibrate(ms: u32) {
    if let Some(n) = web_sys::window().map(|w| w.navigator()) {
        if let Ok(f) = Reflect::get(&n, &JsValue::from_str("vibrate")).and_then(|f| f.dyn_into::<Function>()) {
            let _ = f.call1(&n, &JsValue::from(ms));
        }
    }
}
/// JS 함수 하나로 감싸기: (L) => 새 L. 연구실 숫자 바꾸기(labChange)에 넘겨요
pub fn js_fn1(f: impl FnMut(JsValue) -> JsValue + 'static) -> JsValue {
    Closure::<dyn FnMut(JsValue) -> JsValue>::new(f).into_js_value()
}
/// {...a, key: v}
pub fn with_key(a: &JsValue, key: &str, v: &JsValue) -> JsValue {
    let o = js_sys::Object::assign(&js_sys::Object::new(), a.unchecked_ref());
    let _ = Reflect::set(&o, &JsValue::from_str(key), v);
    o.into()
}
/// 작은 객체 만들기
pub fn obj(pairs: &[(&str, JsValue)]) -> JsValue {
    let o = js_sys::Object::new();
    for (k, v) in pairs {
        let _ = Reflect::set(&o, &JsValue::from_str(k), v);
    }
    o.into()
}
