//! 로그인·가입 화면, 상태바 색, 로그인했는지에 따라 화면 나누기, 로그아웃

use std::cell::{Cell, RefCell};

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use super::{ask, boot, cloud_ready, global, restart, screen};
use crate::dom::{self, by_id, safe_img};

/// 로그인 사진 다섯 장의 위쪽 평균 색 (사진이 바뀌면 상태바 색도 따라가요)
const LOGIN_COLOURS: [&str; 5] = ["#3e4250", "#2e3949", "#4e5047", "#997880", "#9b725c"];

thread_local! {
    static SIGNUP: Cell<bool> = const { Cell::new(false) };
    static WARNED: Cell<bool> = const { Cell::new(false) };
    static LOGIN_ON: Cell<bool> = const { Cell::new(false) };
    static SLIDE: Cell<usize> = const { Cell::new(0) };
    static SLIDE_T: Cell<i32> = const { Cell::new(0) };
    static HIDE_T: Cell<i32> = const { Cell::new(0) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
    static SWAP_FN: RefCell<Option<Closure<dyn FnMut()>>> = const { RefCell::new(None) };
}
fn win() -> web_sys::Window {
    web_sys::window().expect("window")
}
fn input(id: &str) -> Option<web_sys::HtmlInputElement> {
    by_id(id)?.dyn_into().ok()
}
fn slides() -> Vec<web_sys::HtmlElement> {
    dom::qsa(&dom::doc().document_element().expect("html"), "#slides i")
}

/// 상태바 색은 지금 보이는 맨 위 색을 따라가요: 로그인(어두움) > 대화(흰색·민트) > 화면(라벤더 회색)
pub fn paint_status() {
    let Some(meta) = dom::doc().query_selector(r#"meta[name="theme-color"][media="(prefers-color-scheme: light)"]"#).ok().flatten() else { return };
    let colour = if LOGIN_ON.get() {
        LOGIN_COLOURS.get(SLIDE.get()).copied().unwrap_or("#2a2a2e")
    } else if dom::app_el().is_some_and(|a| a.class_list().contains("in-thread")) {
        if by_id("thread").and_then(|t| t.get_attribute("data-wall")).as_deref() == Some("mint") {
            "#eaf6ee"
        } else {
            "#ffffff"
        }
    } else {
        "#f2f2f7"
    };
    let _ = meta.set_attribute("content", colour);
    // 상태바 뒤쪽 띠는 페이지 바탕색이 보여요. 그래서 바탕색도 같은 색으로 맞춰요 (theme-color를 따르지 않는 기기에서도 보이게)
    if let Some(r) = dom::doc().document_element().and_then(|r| r.dyn_into::<web_sys::HtmlElement>().ok()) {
        let _ = r.style().set_property("background-color", colour);
    }
    if let Some(b) = dom::doc().body() {
        let _ = b.style().set_property("background-color", colour);
    }
}
fn show_slide(n: usize) {
    for (i, s) in slides().iter().enumerate() {
        dom::toggle(s, "on", i == n);
    }
    paint_status();
}

fn set_mode(on: bool, animate: bool) {
    let Some(login) = by_id("login") else { return };
    let apply = move || {
        SIGNUP.set(on);
        dom::set_text("login-title", if on { "계정 만들기" } else { "로그인" });
        dom::set_text("login-sub", if on { "아이디와 비밀번호만 정하면 바로 시작할 수 있어요." } else { "아이디와 비밀번호로 대화와 프로필을 어디서나 이어서 쓸 수 있어요." });
        dom::set_text("login-submit", if on { "계정 만들기" } else { "로그인" });
        if let Some(sw) = by_id("login-switch") {
            sw.set_inner_html(if on { "이미 계정이 있나요? <b>로그인</b>" } else { "계정이 없나요? <b>계정 만들기</b>" });
        }
        if let Some(pw) = input("login-pw") {
            let _ = pw.set_attribute("autocomplete", if on { "new-password" } else { "current-password" });
        }
        dom::set_text("login-error", "");
        if let Some(p2) = input("login-pw2") {
            p2.set_value("");
        }
    };
    // 글자는 흐려졌다가 바뀌고, 비밀번호 확인 칸은 동시에 펼쳐지거나 접혀요
    dom::toggle(&login, "signup", on);
    if !animate || dom::reduce_motion() {
        return apply();
    }
    let _ = login.class_list().add_1("swapping");
    leptos::prelude::set_timeout(
        move || {
            apply();
            let _ = login.class_list().remove_1("swapping");
        },
        std::time::Duration::from_millis(200),
    );
}

pub fn show_login(on: bool) {
    let Some(login) = by_id("login") else { return };
    LOGIN_ON.set(on);
    paint_status();
    win().clear_interval_with_handle(SLIDE_T.get());
    if on {
        show_slide(SLIDE.get());
        SWAP_FN.with(|f| {
            let mut f = f.borrow_mut();
            let c = f.get_or_insert_with(|| {
                Closure::<dyn FnMut()>::new(|| {
                    let n = slides().len().max(1);
                    SLIDE.set((SLIDE.get() + 1) % n);
                    show_slide(SLIDE.get());
                })
            });
            SLIDE_T.set(win().set_interval_with_callback_and_timeout_and_arguments_0(c.as_ref().unchecked_ref(), 5000).unwrap_or(0));
        });
    }
    win().clear_timeout_with_handle(HIDE_T.get());
    login.set_hidden(false);
    let l2 = login.clone();
    leptos::prelude::request_animation_frame(move || dom::toggle(&l2, "is-open", on));
    // 닫은 뒤에는 화면에서 완전히 빼서 뒤에서 아무것도 그리지 않게 해요
    if !on {
        let cb = Closure::once_into_js(move || {
            login.set_hidden(true);
            for s in slides() {
                let _ = s.class_list().remove_1("on");
            }
        });
        HIDE_T.set(win().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 700).unwrap_or(0));
    } else {
        set_mode(false, false);
        WARNED.set(false);
        if let Some(n) = input("login-name") {
            n.set_value("");
        }
    }
}

fn set_busy(on: bool) {
    BUSY.set(on);
    if let Some(b) = by_id("login-submit") {
        dom::toggle(&b, "busy", on);
        let _ = b.set_attribute("aria-busy", &on.to_string());
        if !on {
            let _ = b.blur();
        }
    }
}
/// 로그인·가입을 해요. 비밀번호를 계속 틀리면 잠깐 막아요 (5번 틀리면 30초, 그다음부터 두 배씩, 최대 10분)
async fn run(job: impl FnOnce(JsValue) -> Result<JsValue, cloud::CloudError>, is_login: bool) {
    dom::set_text("login-error", "");
    let lock = js_sys::JSON::parse(&dom::store_get("login-lock", "{}")).unwrap_or(JsValue::NULL);
    let until = cloud::get(&lock, "until").as_f64().unwrap_or(0.0);
    let now = js_sys::Date::now();
    if is_login && until > now {
        return dom::set_text("login-error", &format!("시도가 너무 많아요. {}초 뒤에 다시 해 주세요", ((until - now) / 1000.0).ceil()));
    }
    if BUSY.get() {
        return;
    }
    set_busy(true);
    let c = cloud_ready().await; // 연결이 아직 끝나지 않았을 수 있어요
    if c.is_null() || c.is_undefined() {
        let why = global("cloudError").as_string().unwrap_or_else(|| "알 수 없는 오류".into());
        dom::set_text("login-error", &format!("서버에 연결하지 못했어요: {why}"));
        set_busy(false);
        return;
    }
    // 서버가 답이 없어도 버튼이 영원히 돌지 않게 25초에서 끊어요
    let late = js_sys::Promise::new(&mut |_, rej| {
        let cb = Closure::once_into_js(move || {
            let _ = rej.call1(&JsValue::NULL, &js_sys::Error::new("서버 응답이 너무 늦어요. 잠시 뒤 다시 해 주세요"));
        });
        let _ = win().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 25000);
    });
    let r = match job(c.clone()) {
        Ok(p) => JsFuture::from(js_sys::Promise::race(&js_sys::Array::of2(&p, &late))).await,
        Err(e) => Err(js_sys::Error::new(&e.message).into()),
    };
    match r {
        Ok(_) => {
            dom::store_set("login-lock", "{}");
            // 새로고침 없이 바로 들어가요: 가입하면 이미 로그인된 상태라 화면만 로그인한 계정으로 바꿔요
            restart();
            render_auth(&c);
            boot(c); // 지갑·대화는 뒤에서 불러와요 (기다리지 않아요)
            show_login(false);
            set_busy(false);
            for id in ["login-pw", "login-pw2"] {
                if let Some(i) = input(id) {
                    i.set_value("");
                }
            }
        }
        Err(e) => {
            dom::set_text("login-error", &cloud::get(&e, "message").as_string().unwrap_or_default());
            set_busy(false);
            if is_login {
                let n = cloud::get(&lock, "n").as_f64().unwrap_or(0.0) + 1.0;
                let until = if n >= 5.0 { js_sys::Date::now() + (30.0 * 2f64.powf(n - 5.0)).min(600.0) * 1000.0 } else { 0.0 };
                dom::store_set("login-lock", &format!(r#"{{"n":{n},"until":{until}}}"#));
            }
        }
    }
}
fn valid_id(id: &str) -> bool {
    (3..=20).contains(&id.chars().count()) && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}
/// 실명: 한글 2~6자 또는 영문 이름
fn valid_name(n: &str) -> bool {
    let cs: Vec<char> = n.chars().collect();
    let ko = (2..=6).contains(&cs.len()) && cs.iter().all(|c| ('가'..='힣').contains(c));
    let en = (3..=40).contains(&cs.len())
        && cs.first().is_some_and(|c| c.is_ascii_alphabetic())
        && cs.last().is_some_and(|c| c.is_ascii_alphabetic())
        && cs.iter().all(|c| c.is_ascii_alphabetic() || *c == ' ');
    ko || en
}
async fn submit() {
    if let Some(b) = by_id("login-submit") {
        let _ = b.blur(); // 눌린 모양이 남지 않게 해요
    }
    if BUSY.get() {
        return;
    }
    let id = dom::input_value("login-email").trim().to_string();
    let pw = dom::input_value("login-pw");
    let signup = SIGNUP.get();
    if id.is_empty() || pw.is_empty() {
        return dom::set_text("login-error", "아이디와 비밀번호를 입력해 주세요");
    }
    if !valid_id(&id) {
        return dom::set_text("login-error", "아이디는 영문, 숫자, _ 로 3~20자예요");
    }
    let plen = pw.encode_utf16().count();
    if signup && !(6..=64).contains(&plen) {
        return dom::set_text("login-error", "비밀번호는 6자 이상으로 써 주세요");
    }
    if signup && pw != dom::input_value("login-pw2") {
        return dom::set_text("login-error", "비밀번호가 서로 달라요");
    }
    if !signup {
        return run(move |_| cloud::call_now("signIn", &[JsValue::from_str(&id), JsValue::from_str(&pw)]), true).await;
    }
    let name = crate::biz::clean_name(&dom::input_value("login-name"));
    if !valid_name(&name) {
        return dom::set_text("login-error", "실명을 입력해 주세요 (한글 2~6자 또는 영문 이름)");
    }
    let sure = ask(&format!("\"{name}\" 님이 실명이 맞나요?"), "실명이 아니면 이용이 제한될 수 있어요. 가입한 뒤에는 이름을 바꾸기 어려워요.", "실명이에요", "다시 입력").await;
    if !sure {
        if let Some(n) = by_id("login-name") {
            let _ = n.focus();
        }
        return;
    }
    let photo = safe_img(&dom::store_get("profile-photo", ""), 40000).to_string();
    run(move |_| cloud::call_now("signUp", &[JsValue::from_str(&id), JsValue::from_str(&pw), JsValue::from_str(&name), JsValue::from_str(&photo)]), false).await;
}

/// 로그인했는지에 따라 화면을 나눠요. 로그인한 적이 있으면 연결이 끝나기 전에도 로그인된 화면을 바로 보여줘요 (깜빡임 없이)
pub fn render_auth(c: &JsValue) {
    let has_cloud = !(c.is_null() || c.is_undefined());
    let hint = if has_cloud { JsValue::NULL } else { js_sys::JSON::parse(&dom::store_get("auth-hint", "null")).unwrap_or(JsValue::NULL) };
    let user = cloud::get(c, "user");
    let signed = if has_cloud { !(user.is_null() || user.is_undefined()) } else { !(hint.is_null() || hint.is_undefined()) };
    let id = if has_cloud { cloud::get(&user, "id").as_string().unwrap_or_default() } else { cloud::get(&hint, "id").as_string().unwrap_or_default() };
    if has_cloud {
        dom::store_set("auth-hint", &if signed { format!(r#"{{"id":{}}}"#, js_sys::JSON::stringify(&JsValue::from_str(&id)).map(String::from).unwrap_or_default()) } else { "null".into() });
    }
    for name in ["account", "money"] {
        if let Some(s) = screen(name) {
            dom::toggle(&s, "guest", !signed);
            dom::toggle(&s, "member", signed);
        }
    }
    if let Some(a) = dom::app_el() {
        dom::toggle(&a, "guest-account", !signed);
    }
    for name in ["ranking", "business", "work"] {
        if let Some(s) = screen(name) {
            dom::toggle(&s, "guest", !signed);
        }
    }
    dom::set_hidden("auth-card", signed);
    dom::set_hidden("auth-row", !signed);
    dom::set_text("profile-id", &if signed { format!("@{id}") } else { String::new() });
    dom::set_hidden("open-manage", !(signed && id.to_lowercase() == "owner")); // 관리 버튼은 아이디가 owner인 사람에게만 보여요
    if signed {
        if dom::store_get("joined", "").is_empty() {
            dom::store_set("joined", &js_sys::Date::now().to_string());
        }
        let joined = dom::js_number(&dom::store_get("joined", &js_sys::Date::now().to_string()));
        dom::set_text("stat-days", &((js_sys::Date::now() - joined) / 864e5).ceil().max(1.0).to_string());
    }
}

async fn logout() {
    if let Some(c) = cloud::cloud() {
        if let Ok(p) = cloud::call_now("signOut", &[]) {
            let _ = cloud::settle(p).await;
        }
        let _ = c;
    }
    // 같이 쓰는 기기에 내 정보가 남지 않게 지워요
    if let Some(s) = win().local_storage().ok().flatten() {
        let keys: Vec<String> = (0..s.length().unwrap_or(0)).filter_map(|i| s.key(i).ok().flatten()).collect();
        const KEEP_OUT: [&str; 11] = ["auth-hint", "bank-tx", "prefs", "walls", "profile-name", "profile-bio", "profile-photo", "money-order", "acct-ok", "joined", "login-lock"];
        for k in keys {
            if k.starts_with("pf:") || KEEP_OUT.contains(&k.as_str()) {
                let _ = s.remove_item(&k);
            }
        }
    }
    let _ = win().location().reload();
}

pub fn init() {
    // 이름 칸을 처음 누르면 실명 안내를 한 번 보여줘요
    if let Some(n) = by_id("login-name") {
        let n2 = n.clone();
        dom::on(&n, "focus", move |_| {
            if !SIGNUP.get() || WARNED.get() {
                return;
            }
            WARNED.set(true);
            let _ = n2.blur();
            let n3 = n2.clone();
            leptos::task::spawn_local(async move {
                ask("실명으로 가입해 주세요", "이름은 무조건 본인의 실명이어야 해요. 별명이나 다른 사람의 이름으로는 가입할 수 없어요.", "알겠어요", "").await;
                let _ = n3.focus();
            });
        });
    }
    for b in dom::qsa(&dom::doc().document_element().expect("html"), "#open-login, .js-open-login") {
        dom::on(&b, "click", |_| show_login(true));
    }
    if let Some(b) = by_id("login-close") {
        dom::on(&b, "click", |_| show_login(false));
    }
    if let Some(b) = by_id("login-switch") {
        dom::on(&b, "click", |_| set_mode(!SIGNUP.get(), true));
    }
    if let Some(f) = by_id("login-form") {
        dom::on(&f, "submit", |e| e.prevent_default());
    }
    if let Some(b) = by_id("login-submit") {
        dom::on(&b, "click", |_| leptos::task::spawn_local(submit()));
    }
    if let Some(b) = by_id("logout") {
        dom::on(&b, "click", |_| leptos::task::spawn_local(logout()));
    }
    let f = Closure::<dyn Fn()>::new(paint_status).into_js_value();
    let _ = Reflect::set(&js_sys::global(), &JsValue::from_str("__paintStatus"), &f);
    render_auth(&JsValue::NULL);
    leptos::prelude::set_timeout(paint_status, std::time::Duration::from_millis(0)); // 처음 켰을 때도 맞춰요
}
