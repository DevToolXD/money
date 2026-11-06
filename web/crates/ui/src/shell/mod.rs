//! 앱 틀 (예전 app.js의 나머지): 화면 바꾸기, 위쪽 바, 테마, 알림(toast)·확인 창(ask)·시트, 설정 저장,
//! 탭 바와 뒤 화면 사진(backdrop) 연결, 로그인한 뒤 시작 흐름. 다른 화면(러스트 모듈)에 넘기는 도움 묶음(ctx)도 여기서 만들어요.

mod account;
pub(crate) mod company;
mod login;
mod ranking;

use std::cell::{Cell, RefCell};

use js_sys::{Function, Object, Promise, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::ctx::Ctx;
use crate::dom::{self, by_id, qs, qsa, safe_img};

pub(crate) const WALLS: [&str; 5] = ["white", "sunset", "aurora", "night", "mint"];

thread_local! {
    static TABBAR: RefCell<JsValue> = const { RefCell::new(JsValue::NULL) };
    static BACKDROP: RefCell<JsValue> = const { RefCell::new(JsValue::NULL) };
    static TOAST_T: Cell<i32> = const { Cell::new(0) };
    static SAVE_T: Cell<i32> = const { Cell::new(0) };
    static STARTED: Cell<bool> = const { Cell::new(false) };
    static BOOTED: Cell<bool> = const { Cell::new(false) };
    static CTX_OBJ: RefCell<JsValue> = const { RefCell::new(JsValue::NULL) };
}
fn win() -> web_sys::Window {
    web_sys::window().expect("window")
}
fn global(k: &str) -> JsValue {
    Reflect::get(&js_sys::global(), &JsValue::from_str(k)).unwrap_or(JsValue::UNDEFINED)
}
fn set_global(k: &str, v: &JsValue) {
    let _ = Reflect::set(&js_sys::global(), &JsValue::from_str(k), v);
}
/// obj.name(...args) (없으면 아무것도 안 해요)
pub(crate) fn invoke(obj: &JsValue, name: &str, args: &[JsValue]) -> JsValue {
    let Ok(f) = Reflect::get(obj, &JsValue::from_str(name)).and_then(|f| f.dyn_into::<Function>()) else { return JsValue::UNDEFINED };
    let a: js_sys::Array = args.iter().cloned().collect();
    f.apply(obj, &a).unwrap_or(JsValue::UNDEFINED)
}
pub(crate) fn tabbar() -> JsValue {
    TABBAR.with(|t| t.borrow().clone())
}
fn backdrop() -> JsValue {
    BACKDROP.with(|t| t.borrow().clone())
}
pub(crate) fn app() -> web_sys::HtmlElement {
    dom::app_el().expect(".app")
}
fn nav() -> Option<web_sys::HtmlElement> {
    dom::doc().query_selector(".tabbar").ok().flatten()?.dyn_into().ok()
}
pub(crate) fn screen(name: &str) -> Option<web_sys::HtmlElement> {
    dom::doc().query_selector(&format!(".screen[data-screen=\"{name}\"]")).ok().flatten()?.dyn_into().ok()
}
fn screens() -> Vec<web_sys::HtmlElement> {
    qsa(&dom::doc().document_element().expect("html"), ".screen")
}

/* ---------- 테마, 화면 바꾸기, 위쪽 바 ---------- */

/// 색 토큰(design-system)과 탭 바가 같은 테마를 보게 해요
fn apply_theme() {
    let dark = win().match_media("(prefers-color-scheme: dark)").ok().flatten().is_some_and(|m| m.matches());
    if let Some(r) = dom::doc().document_element() {
        let _ = r.set_attribute("data-theme", if dark { "dark" } else { "light" });
    }
}
pub(crate) fn show_screen(name: &str) {
    let app = app();
    for el in screens() {
        let on = dom::data(&el, "screen") == name;
        dom::toggle(&el, "is-active", on);
        if on {
            let _ = app.set_attribute("data-screen", name);
        }
        let _ = Reflect::set(&el, &JsValue::from_str("inert"), &JsValue::from_bool(!on));
    }
    login::paint_status();
}
fn set_title(name: &str) {
    if let Some(s) = screen(name) {
        dom::set_text("topbar-title", &dom::data(&s, "title"));
    }
}
/// 큰 제목이 접히면 가운데 작은 제목과 유리가 나타나요
pub(crate) fn update_chrome(el: &web_sys::HtmlElement) {
    if !el.class_list().contains("is-active") {
        return;
    }
    let (Some(top), Some(title)) = (by_id("topbar"), by_id("topbar-title")) else { return };
    let y = el.scroll_top() as f64;
    let ts = title.style();
    let Some(h1) = qs(el, ".large-title h1") else {
        // 큰 제목이 없는 화면(메시지)은 작은 제목이 늘 보여요. 돈 탭처럼 data-notitle이면 제목 없이 유리만 나타나요
        let p0 = (y / 16.0).clamp(0.0, 1.0);
        let _ = top.style().set_property("--p", &p0.to_string());
        dom::toggle(&top, "on", p0 > 0.0);
        let _ = ts.set_property("opacity", if el.has_attribute("data-notitle") { "0" } else { "1" });
        let _ = ts.set_property("transform", "none");
        return;
    };
    let p = ((y - 28.0) / 20.0).clamp(0.0, 1.0);
    let _ = top.style().set_property("--p", &p.to_string());
    dom::toggle(&top, "on", p > 0.0);
    let t = ((y - 34.0) / 14.0).clamp(0.0, 1.0);
    let _ = ts.set_property("opacity", &t.to_string());
    let _ = ts.set_property("transform", &format!("translateY({}px)", (1.0 - t) * 5.0));
    let hs = h1.style();
    let _ = hs.set_property("opacity", &(1.0 - y / 38.0).clamp(0.0, 1.0).to_string());
    let _ = hs.set_property("transform", &if y < 0.0 { format!("scale({})", 1.0 + (-y / 350.0).min(0.14)) } else { format!("translateY({}px)", -0.85 * y.min(56.0)) });
}
fn scroll_to_top(name: &str) {
    if let Some(s) = screen(name) {
        let o = web_sys::ScrollToOptions::new();
        o.set_top(0.0);
        o.set_behavior(if dom::reduce_motion() { web_sys::ScrollBehavior::Auto } else { web_sys::ScrollBehavior::Smooth });
        s.scroll_to_with_scroll_to_options(&o);
    }
}
/// 관리·공지 화면처럼 탭 바에 없는 화면으로 가요
pub(crate) fn go_screen(name: &str) {
    show_screen(name);
    if name == "manage" {
        crate::manage::refresh();
    }
    if name == "notices" {
        crate::notices::refresh();
    }
    set_title(name);
    if let Some(s) = screen(name) {
        update_chrome(&s);
    }
    invoke(&backdrop(), "setActive", &[JsValue::from_str(name)]);
    invoke(&tabbar(), "requestRender", &[]);
}

/* ---------- 알림, 확인 창, 시트 ---------- */

pub(crate) fn toast(t: &str) {
    let Some(el) = by_id("toast") else { return };
    el.set_text_content(Some(t));
    let _ = el.class_list().add_1("on");
    win().clear_timeout_with_handle(TOAST_T.get());
    // 안 된 이야기는 읽을 시간이 더 필요해요
    let bad = t.contains("못") || t.contains("않") || t.contains("모자라") || t.contains("부족");
    let cb = Closure::once_into_js(move || {
        let _ = el.class_list().remove_1("on");
    });
    TOAST_T.set(win().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), if bad { 4200 } else { 2200 }).unwrap_or(0));
    biz_log(t, bad);
}
/// 사업 탭에서 알림은 화면에 남겨 둬요 (알림은 금방 사라져서 되고 있는지 알 수 없었어요). 최근 3개
fn biz_log(t: &str, bad: bool) {
    if !dom::screen_active("business") {
        return;
    }
    let Some(box_) = by_id("biz-log") else { return };
    let d = js_sys::Date::new_0();
    let item = format!(r#"<li class="{}"><time>{:02}:{:02}</time><span>{}</span></li>"#, if bad { "bad" } else { "ok" }, d.get_hours(), d.get_minutes(), dom::esc(t));
    let old = box_.inner_html();
    let mut items: Vec<String> = old.split("</li>").filter(|x| x.contains("<li")).map(|x| format!("{x}</li>")).take(2).collect();
    items.insert(0, item);
    box_.set_inner_html(&items.concat());
    box_.set_hidden(false);
}
/// 확인 창: 고르면 true (cancel이 비어 있으면 확인 버튼만 보여요)
pub(crate) fn ask_promise(title: &str, body: &str, ok: &str, cancel: &str) -> Promise {
    let (title, body, ok, cancel) = (title.to_string(), body.to_string(), ok.to_string(), cancel.to_string());
    Promise::new(&mut |resolve, _| {
        let (Some(alert), Some(okb), Some(cb)) = (by_id("alert"), by_id("alert-ok"), by_id("alert-cancel")) else {
            let _ = resolve.call1(&JsValue::NULL, &false.into());
            return;
        };
        dom::set_text("alert-title", &title);
        dom::set_text("alert-body", &body);
        okb.set_text_content(Some(&ok));
        cb.set_text_content(Some(&cancel));
        let _ = cb.style().set_property("display", if cancel.is_empty() { "none" } else { "" });
        alert.set_hidden(false);
        let a2 = alert.clone();
        leptos::prelude::request_animation_frame(move || {
            let _ = a2.class_list().add_1("is-open");
        });
        let done = {
            let (alert, okb, cb) = (alert.clone(), okb.clone(), cb.clone());
            std::rc::Rc::new(move |v: bool| {
                let _ = alert.class_list().remove_1("is-open");
                okb.set_onclick(None);
                cb.set_onclick(None);
                let _ = resolve.call1(&JsValue::NULL, &v.into());
            })
        };
        let d1 = done.clone();
        okb.set_onclick(Some(&Closure::once_into_js(move || d1(true)).unchecked_into()));
        let d2 = done.clone();
        cb.set_onclick(Some(&Closure::once_into_js(move || d2(false)).unchecked_into()));
    })
}
pub(crate) async fn ask(title: &str, body: &str, ok: &str, cancel: &str) -> bool {
    JsFuture::from(ask_promise(title, body, ok, cancel)).await.map(|v| v.is_truthy()).unwrap_or(false)
}
/// 아래에서 올라오는 시트 (on=false면 모든 시트를 닫아요). el이 없으면 프로필 편집 시트
pub(crate) fn sheet_open(on: bool, el: Option<web_sys::Element>) {
    if on {
        let el = el.or_else(|| by_id("sheet").map(Into::into));
        if let Some(e) = el {
            let _ = e.class_list().add_1("is-open");
        }
    } else {
        for s in qsa(&dom::doc().document_element().expect("html"), ".sheet") {
            let _ = s.class_list().remove_1("is-open");
        }
    }
    if let Some(d) = by_id("dim") {
        dom::toggle(&d, "is-open", on);
    }
    dom::toggle(&app(), "sheet-open", on);
}
pub(crate) fn sheet_by_id(on: bool, id: &str) {
    sheet_open(on, by_id(id).map(Into::into));
}

/* ---------- 내 설정 저장: 프로필 글과 대화 배경만, 바뀐 뒤 3초 모아서 한 번에 ---------- */

pub(crate) fn save_settings() {
    if !cloud::signed_in() {
        return;
    }
    win().clear_timeout_with_handle(SAVE_T.get());
    let cb = Closure::once_into_js(|| {
        let walls = js_sys::JSON::parse(&dom::store_get("walls", "{}")).ok().filter(|v| v.is_object()).unwrap_or_else(|| Object::new().into());
        let clean = Object::new();
        for (n, e) in Object::entries(walls.unchecked_ref()).iter().enumerate() {
            if n >= 150 {
                break;
            }
            let pair = js_sys::Array::from(&e);
            if pair.get(1).as_string().is_some_and(|w| WALLS.contains(&w.as_str())) {
                let _ = Reflect::set(&clean, &pair.get(0), &pair.get(1));
            }
        }
        let profile = dom::obj(&[("name", JsValue::from_str(&dom::cut(&dom::store_get("profile-name", "내 계정"), 30))), ("bio", JsValue::from_str(&dom::cut(&dom::store_get("profile-bio", ""), 120)))]);
        let _ = cloud::call_now("saveSettings", &[dom::obj(&[("profile", profile), ("walls", clean.into())])]);
    });
    SAVE_T.set(win().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 3000).unwrap_or(0));
}

/* ---------- 다른 화면에 넘기는 도움 묶음 ---------- */

fn f0(f: impl Fn() -> JsValue + 'static) -> JsValue {
    Closure::<dyn Fn() -> JsValue>::new(f).into_js_value()
}
fn f1(f: impl Fn(JsValue) -> JsValue + 'static) -> JsValue {
    Closure::<dyn Fn(JsValue) -> JsValue>::new(f).into_js_value()
}
fn f2(f: impl Fn(JsValue, JsValue) -> JsValue + 'static) -> JsValue {
    Closure::<dyn Fn(JsValue, JsValue) -> JsValue>::new(f).into_js_value()
}
fn f3(f: impl Fn(JsValue, JsValue, JsValue) -> JsValue + 'static) -> JsValue {
    Closure::<dyn Fn(JsValue, JsValue, JsValue) -> JsValue>::new(f).into_js_value()
}
fn s(v: &JsValue) -> String {
    v.as_string().unwrap_or_default()
}
fn make_ctx() -> Ctx {
    let o = Object::new();
    let set = |k: &str, v: JsValue| {
        let _ = Reflect::set(&o, &JsValue::from_str(k), &v);
    };
    set("cloud", f0(|| global("cloud")));
    set("toast", f1(|m| {
        toast(&s(&m));
        JsValue::UNDEFINED
    }));
    set("ask", f1(|o| {
        let g = |k| s(&cloud::get(&o, k));
        let ok = g("ok");
        ask_promise(&g("title"), &g("body"), if ok.is_empty() { "확인" } else { &ok }, &g("cancel")).into()
    }));
    set("sheetOpen", f2(|on, el| {
        sheet_open(on.is_truthy(), el.dyn_into::<web_sys::Element>().ok());
        JsValue::UNDEFINED
    }));
    set("company", f0(company::current));
    set("mark", f3(|n, l, c| JsValue::from_str(&dom::avatar_html(&s(&n), &s(&l), &{ let c = s(&c); if c.is_empty() { "avatar contact".into() } else { c } }))));
    set("safeImg", f2(|v, max| JsValue::from_str(safe_img(&s(&v), max.as_f64().unwrap_or(140000.0) as usize))));
    set("walletOp", f1(|f| f.dyn_into::<Function>().map(|f| crate::money::wallet_op(f).into()).unwrap_or(JsValue::UNDEFINED)));
    set("settle", f0(|| crate::money::claim().into()));
    set("log", f1(|e| {
        crate::money::log(&e);
        JsValue::UNDEFINED
    }));
    set("patchCompany", f1(|p| {
        company::patch(&p);
        JsValue::UNDEFINED
    }));
    set("saveSettings", f0(|| {
        save_settings();
        JsValue::UNDEFINED
    }));
    set("showLogin", f1(|on| {
        login::show_login(on.is_truthy());
        JsValue::UNDEFINED
    }));
    set("pkv", f3(|p, mini, co| JsValue::from_str(&crate::biz::pkv(&p, mini.is_truthy(), &co))));
    let vit = Object::new();
    let _ = Reflect::set(&vit, &JsValue::from_str("get"), &f0(crate::vitals::get));
    let _ = Reflect::set(&vit, &JsValue::from_str("set"), &f1(|v| {
        crate::vitals::set(v);
        JsValue::UNDEFINED
    }));
    set("vitals", vit.into());
    CTX_OBJ.with(|c| *c.borrow_mut() = o.clone().into());
    Ctx(o.into())
}

/* ---------- 탭 바와 뒤 화면 사진 ---------- */

fn setup_tabbar(first: &str) {
    let Some(nav) = nav() else { return };
    let params = web_sys::UrlSearchParams::new_with_str(&win().location().search().unwrap_or_default()).ok();
    let p = |k: &str| params.as_ref().and_then(|x| x.get(k)).unwrap_or_default();
    let glass_off = p("glass") == "off";
    let gl_off = p("gl") == "off";
    let scr: Vec<(String, web_sys::HtmlElement)> = screens().into_iter().map(|el| (dom::data(&el, "screen"), el)).collect();
    // 탭 바 뒤 화면을 사진으로 찍어 두는 곳과, 그 사진으로 막대 유리를 그리는 탭 바 (둘 다 러스트)
    let bd = if glass_off || gl_off {
        JsValue::NULL
    } else {
        let busy: Function = f0(|| JsValue::from_bool(invoke(&tabbar(), "busy", &[]).is_truthy())).unchecked_into();
        let upd: Function = f2(|name, shot| {
            let t = tabbar();
            if !t.is_null() {
                invoke(&t, "setShot", &[name, shot]);
            }
            JsValue::UNDEFINED
        })
        .unchecked_into();
        crate::backdrop::create(app(), scr, busy, upd)
    };
    BACKDROP.with(|b| *b.borrow_mut() = bd.clone());
    crate::quickmenu::init(&nav);
    let tb = crate::tabbar::create(nav.clone(), bd.clone(), app(), !gl_off);
    TABBAR.with(|t| *t.borrow_mut() = tb.clone());
    set_global("tabbar", &tb);
    set_global("backdrop", &bd);
    let tab_of = |e: &web_sys::Event| -> String {
        let d = Reflect::get(e, &JsValue::from_str("detail")).unwrap_or(JsValue::UNDEFINED);
        cloud::get(&cloud::get(&d, "tab"), "dataset").dyn_into::<Object>().ok().map(|ds| s(&cloud::get(&ds, "tab"))).unwrap_or_default()
    };
    dom::on(&nav, "tabchange", move |e| {
        let name = tab_of(&e);
        show_screen(&name);
        crate::chat::leave(&name);
        set_title(&name);
        if let Some(sc) = screen(&name) {
            update_chrome(&sc);
        }
        match name.as_str() {
            "money" => crate::money::render(),
            "ranking" => ranking::load(false),
            "work" => crate::work::refresh(false),
            "shop" => crate::shop::refresh(),
            "inventory" => crate::inventory::refresh(),
            "business" => crate::biz::refresh(), // 상품 재고와 금고를 새로 읽어요
            "untitled3" => crate::sns::open(),
            _ => {}
        }
        invoke(&backdrop(), "setActive", &[JsValue::from_str(&name)]);
        invoke(&tabbar(), "requestRender", &[]);
    });
    // 이미 선택된 탭을 다시 누르면 맨 위로
    dom::on(&nav, "tabreselect", move |e| {
        let name = tab_of(&e);
        scroll_to_top(&name);
        if name == "ranking" {
            ranking::load(true);
        }
        let cur = app().get_attribute("data-screen").unwrap_or_default();
        if name == "account" && (cur == "manage" || cur == "notices") {
            go_screen("account");
        }
    });
    if let Ok(Some(mq)) = win().match_media("(prefers-color-scheme: dark)") {
        dom::on(&mq, "change", |_| {
            apply_theme();
            let bd = backdrop();
            if bd.is_null() {
                return;
            }
            invoke(&tabbar(), "dropShots", &[]); // 색이 틀린 낡은 사진은 버리고
            invoke(&bd, "invalidateAll", &[]); // 새로 찍어요
            invoke(&bd, "ensure", &[cloud::get(&bd, "active"), true.into()]);
        });
    }
    if !bd.is_null() {
        invoke(&bd, "setActive", &[JsValue::from_str(first)]);
        // 나머지 화면도 미리 찍어 둬요 (탭을 옮길 때 바로 유리가 나오도록)
        for el in screens() {
            let n = dom::data(&el, "screen");
            if n != first {
                invoke(&bd, "ensure", &[JsValue::from_str(&n)]);
            }
        }
    }
    if let Some(sc) = screen(first) {
        update_chrome(&sc);
    }
}

/* ---------- 로그인한 계정이면 서버에서 지갑, 대화, 설정을 불러와요 ---------- */

pub(crate) fn boot(cloud_v: JsValue) {
    set_global("__booted", &true.into());
    BOOTED.set(true);
    login::render_auth(&cloud_v);
    let user = cloud::get(&cloud_v, "user");
    if cloud_v.is_null() || cloud_v.is_undefined() || user.is_null() || user.is_undefined() || STARTED.get() {
        crate::chat::render();
        crate::money::render();
        return;
    }
    STARTED.set(true);
    let name = s(&cloud::get(&user, "name"));
    if !name.is_empty() && dom::store_get("profile-name", "내 계정") == "내 계정" {
        dom::store_set("profile-name", &name);
        account::apply_profile();
    }
    leptos::task::spawn_local(async move {
        if let Err(e) = cloud::call("ensureAccount", &[]).await {
            cloud::warn(&format!("계정 정보를 채우지 못했어요: {}", e.code));
            toast("계정 정보를 만들지 못했어요. 새로고침해 주세요");
        }
        crate::money::start_wallet();
        crate::notices::check();
        company::load();
        crate::vitals::load();
        crate::chat::start();
        match cloud::call("loadSettings", &[]).await {
            Ok(data) => {
                let p = cloud::get(&data, "profile");
                if p.is_truthy() {
                    let n = cloud::get(&p, "name").as_string().filter(|x| !x.is_empty()).unwrap_or_else(|| "내 계정".into());
                    dom::store_set("profile-name", &dom::cut(&n, 30));
                    dom::store_set("profile-bio", &dom::cut(&s(&cloud::get(&p, "bio")), 120));
                    account::apply_profile();
                }
                let w = cloud::get(&data, "walls");
                if w.is_object() {
                    let clean = Object::new();
                    for e in Object::entries(w.unchecked_ref()).iter() {
                        let pair = js_sys::Array::from(&e);
                        if pair.get(1).as_string().is_some_and(|x| WALLS.contains(&x.as_str())) {
                            let _ = Reflect::set(&clean, &pair.get(0), &pair.get(1));
                        }
                    }
                    if let Ok(t) = js_sys::JSON::stringify(&clean) {
                        dom::store_set("walls", &String::from(t));
                    }
                }
            }
            Err(e) => cloud::warn(&format!("저장해 둔 설정을 불러오지 못했어요: {}", e.code)),
        }
        // 내 프로필 사진이 서버에만 있으면 가져와요
        let me_uid = s(&cloud::get(&global("cloud"), "uid"));
        let peek = || cloud::call_now("peek", &[JsValue::from_str(&me_uid)]).unwrap_or(JsValue::UNDEFINED);
        let me = peek();
        if me.is_truthy() {
            let ph = s(&cloud::get(&me, "photo"));
            if !ph.is_empty() && dom::store_get("profile-photo", "").is_empty() {
                dom::store_set("profile-photo", &ph);
                account::apply_profile();
            }
        } else {
            let arr = js_sys::Array::of1(&JsValue::from_str(&me_uid));
            if cloud::call("fetchProfiles", &[arr.into()]).await.is_ok() {
                let p = peek();
                let ph = s(&cloud::get(&p, "photo"));
                if !ph.is_empty() && dom::store_get("profile-photo", "").is_empty() {
                    dom::store_set("profile-photo", &ph);
                    account::apply_profile();
                }
            }
        }
    });
}
pub(crate) fn restart() {
    STARTED.set(false);
}
/// firebase.js(모듈)가 연결을 마칠 때까지 기다려요. 연결하지 못하면 null
pub(crate) async fn cloud_ready() -> JsValue {
    loop {
        let r = global("cloudReady");
        if let Ok(p) = r.clone().dyn_into::<Promise>() {
            return JsFuture::from(p).await.unwrap_or(JsValue::NULL);
        }
        if !r.is_undefined() && !r.is_null() {
            return r;
        }
        dom::sleep(30).await;
    }
}

/* ---------- 시작 ---------- */

pub fn start() {
    crate::fire::start();
    crate::glass::start();
    apply_theme();
    // 내용이 없는 자리 표시 줄
    for bx in qsa(&dom::doc().document_element().expect("html"), ".skeletons") {
        let rows = win().get_computed_style(&bx).ok().flatten().and_then(|cs| cs.get_property_value("--rows").ok()).and_then(|v| v.trim().parse::<usize>().ok()).filter(|n| *n > 0).unwrap_or(6);
        bx.set_inner_html(&r#"<div class="skeleton-row"><i></i><span><b></b><b></b></span></div>"#.repeat(rows));
    }
    let first = nav().and_then(|n| qs(&n, "[aria-current]")).map(|b| dom::data(&b, "tab")).filter(|t| !t.is_empty()).unwrap_or_else(|| "messages".into());
    show_screen(&first);
    set_title(&first);
    for el in screens() {
        let e2 = el.clone();
        dom::on(&el, "scroll", move |_| update_chrome(&e2));
    }
    // 관리 화면 (owner 전용 버튼), 공지 화면
    for (id, to) in [("open-manage", "manage"), ("manage-back", "account"), ("open-notices", "notices"), ("notices-back", "account")] {
        if let Some(b) = by_id(id) {
            dom::on(&b, "click", move |_| go_screen(to));
        }
    }
    // 서버 데이터베이스가 꺼져 있으면(관리자 설정 문제) 화면 위에 이유를 알려줘요
    let show_down = || {
        dom::set_hidden("cloud-down", false);
        crate::money::render();
        crate::chat::render();
    };
    dom::on(&win(), "cloud-down", move |_| show_down());
    if global("cloudDown").is_truthy() {
        show_down();
    }
    if let Some(cd) = by_id("cloud-down") {
        let c2 = cd.clone();
        dom::on(&cd, "click", move |_| c2.set_hidden(true));
    }
    if let Some(d) = by_id("dim") {
        dom::on(&d, "click", |_| sheet_open(false, None));
    }
    // 다른 화면들 (모두 러스트)
    let c = make_ctx();
    crate::biz::init(c.clone());
    crate::chat::init(c.clone());
    crate::money::init(c.clone());
    crate::work::init(c.clone());
    crate::wage::init();
    crate::inventory::init(c.clone());
    crate::manage::init(c.clone());
    crate::notices::init();
    crate::shop::init(c.clone());
    crate::vitals::init();
    company::init();
    ranking::init();
    login::init();
    account::init();
    setup_tabbar(&first);
    dom::on(&dom::doc(), "click", |e| {
        if dom::closest(&e, ".to-account").is_some() {
            if let Some(b) = nav().and_then(|n| qs(&n, "[data-tab=\"account\"]")) {
                b.click();
            }
        }
    });
    // 연결은 느린 폰에서 몇 초 걸릴 수 있어요. 끝나면 바로 반영하고, 8초가 지나도 안 끝나면 일단 지금 화면으로 보여요
    leptos::task::spawn_local(async {
        let c = cloud_ready().await;
        boot(c);
    });
    leptos::prelude::set_timeout(
        || {
            if !BOOTED.get() {
                boot(JsValue::NULL);
            }
        },
        std::time::Duration::from_millis(8000),
    );
}
