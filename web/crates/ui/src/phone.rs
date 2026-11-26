//! 회원가입 전화번호 인증: 한 사람이 계정 하나만 쓰도록, 새로 만든 계정은 휴대폰 번호를 한 번 확인해야 앱을 쓸 수 있어요.
//! 문자는 Zavu 서버 함수(server/phone-verify)가 보내고 확인해요. 앱은 그 주소에 로그인 토큰과 번호·인증번호만 보내요.
//! 서버가 확인되면 Firestore에 verified/{uid}를 적어요 (firestore.rules: 인증 전용 계정만 쓸 수 있어요).
//! - 이 기능을 넣기 전에 만든 계정(CUTOFF 이전)과 시험(에뮬레이터)은 묻지 않아요.

use std::cell::Cell;

use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};

use crate::dom::{self, by_id};

const URL: &str = "https://fn.zavu.dev/k5704ejjg0e7gsp2zahmhrwm258jz7kg/phone-verify";
/// 2026-11-26 12:30 UTC: 이 뒤에 만든 계정부터 인증해요
const CUTOFF: f64 = 1_795_696_200_000.0;
const RESEND_S: i32 = 60;

thread_local! {
    static BOUND: Cell<bool> = const { Cell::new(false) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
    static WAIT: Cell<i32> = const { Cell::new(0) };
}

fn why(code: &str) -> &'static str {
    match code {
        "bad_phone" => "010으로 시작하는 휴대폰 번호를 넣어 주세요",
        "phone_used" => "이 번호로 이미 인증한 계정이 있어요. 번호 하나에 계정 하나만 만들 수 있어요",
        "too_soon" => "인증번호는 1분에 한 번 받을 수 있어요. 잠시 뒤 다시 받아 주세요",
        "user_limit" => "오늘은 인증번호를 더 받을 수 없어요. 내일 다시 해 주세요",
        "daily_limit" => "오늘 보낼 수 있는 인증 문자를 다 썼어요. 내일 다시 해 주세요",
        "sms_failed" => "문자를 보내지 못했어요. 번호를 확인하고 다시 해 주세요",
        "no_code" => "인증번호를 먼저 받아 주세요",
        "expired" => "인증번호 시간(5분)이 지났어요. 다시 받아 주세요",
        "too_many" => "여러 번 틀렸어요. 인증번호를 다시 받아 주세요",
        "wrong_code" => "인증번호가 맞지 않아요",
        "not_signed_in" => "로그인이 풀렸어요. 다시 로그인해 주세요",
        _ => "잠시 뒤 다시 해 주세요",
    }
}

/// 숫자만 남기고 010-1234-5678 모양으로
fn pretty(raw: &str) -> String {
    let d: String = raw.chars().filter(|c| c.is_ascii_digit()).take(11).collect();
    match d.len() {
        0..=3 => d,
        4..=7 => format!("{}-{}", &d[..3], &d[3..]),
        _ => format!("{}-{}-{}", &d[..3], &d[3..7], &d[7..]),
    }
}
fn input(id: &str) -> String {
    by_id(id).and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok()).map(|i| i.value()).unwrap_or_default()
}
fn set_input(id: &str, v: &str) {
    if let Some(i) = by_id(id).and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok()) {
        i.set_value(v);
    }
}
fn msg(t: &str, good: bool) {
    dom::set_text("pg-msg", t);
    if let Some(m) = by_id("pg-msg") {
        dom::toggle(&m, "good", good);
    }
}
fn disable(id: &str, on: bool) {
    if let Some(b) = by_id(id) {
        if on {
            let _ = b.set_attribute("disabled", "");
        } else {
            let _ = b.remove_attribute("disabled");
        }
    }
}

/// 서버 함수에 보내요. 글자(text/plain)로 보내서 브라우저가 미리 묻는 요청(CORS 사전 요청) 없이 바로 가요
async fn post(op: &str, phone: &str, code: &str) -> Result<(), String> {
    let token = cloud::call("idToken", &[]).await.map_err(|_| "not_signed_in".to_string())?;
    let body = js_sys::JSON::stringify(&dom::obj(&[("op", op.into()), ("idToken", token), ("phone", phone.into()), ("code", code.into())])).map(String::from).unwrap_or_default();
    let w = web_sys::window().ok_or("server")?;
    let fetch: js_sys::Function = js_sys::Reflect::get(&w, &"fetch".into()).ok().and_then(|f| f.dyn_into().ok()).ok_or("server")?;
    let init = dom::obj(&[("method", "POST".into()), ("headers", dom::obj(&[("content-type", "text/plain;charset=UTF-8".into())])), ("body", body.into())]);
    let p: js_sys::Promise = fetch.call2(&w, &URL.into(), &init).ok().and_then(|p| p.dyn_into().ok()).ok_or("server")?;
    let resp = JsFuture::from(p).await.map_err(|_| "network".to_string())?;
    let jp: js_sys::Promise = js_sys::Reflect::get(&resp, &"json".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()).and_then(|f| f.call0(&resp).ok()).and_then(|p| p.dyn_into().ok()).ok_or("server")?;
    let j = JsFuture::from(jp).await.map_err(|_| "server".to_string())?;
    if cloud::get(&j, "ok").is_truthy() {
        Ok(())
    } else {
        Err(cloud::get(&j, "code").as_string().unwrap_or_else(|| "server".into()))
    }
}

fn tick() {
    spawn_local(async {
        while WAIT.get() > 0 {
            dom::set_text("pg-resend", &format!("인증번호 다시 받기 ({}초)", WAIT.get()));
            disable("pg-resend", true);
            dom::sleep(1000).await;
            WAIT.set(WAIT.get() - 1);
        }
        dom::set_text("pg-resend", "인증번호 다시 받기");
        disable("pg-resend", false);
    });
}

async fn send() {
    if BUSY.replace(true) {
        return;
    }
    let phone = input("pg-phone");
    disable("pg-send", true);
    msg("인증번호를 보내는 중이에요", true);
    let r = post("send", &phone, "").await;
    BUSY.set(false);
    disable("pg-send", false);
    match r {
        Ok(()) => {
            msg("문자로 인증번호 6자리를 보냈어요. 5분 안에 넣어 주세요", true);
            dom::set_hidden("pg-code-row", false);
            dom::set_hidden("pg-check", false);
            dom::set_hidden("pg-resend", false);
            dom::set_hidden("pg-send", true);
            WAIT.set(RESEND_S);
            tick();
            if let Some(c) = by_id("pg-code") {
                let _ = c.focus();
            }
        }
        Err(c) => msg(why(&c), false),
    }
}

async fn check_code() {
    if BUSY.replace(true) {
        return;
    }
    let (phone, code) = (input("pg-phone"), input("pg-code"));
    if code.trim().len() != 6 {
        BUSY.set(false);
        return msg("인증번호 6자리를 넣어 주세요", false);
    }
    disable("pg-check", true);
    let r = post("check", &phone, code.trim()).await;
    BUSY.set(false);
    disable("pg-check", false);
    match r {
        Ok(()) => {
            msg("인증했어요", true);
            dom::vibrate(14);
            dom::sleep(500).await;
            hide();
            crate::shell::toast("전화번호 인증을 마쳤어요");
        }
        Err(c) => msg(why(&c), false),
    }
}

fn bind() {
    if BOUND.replace(true) {
        return;
    }
    if let Some(i) = by_id("pg-phone") {
        dom::on(&i, "input", |_| {
            let v = pretty(&input("pg-phone"));
            set_input("pg-phone", &v);
        });
    }
    if let Some(b) = by_id("pg-send") {
        dom::on(&b, "click", |_| spawn_local(send()));
    }
    if let Some(b) = by_id("pg-resend") {
        dom::on(&b, "click", |_| spawn_local(send()));
    }
    if let Some(b) = by_id("pg-check") {
        dom::on(&b, "click", |_| spawn_local(check_code()));
    }
    if let Some(i) = by_id("pg-code") {
        dom::on(&i, "input", |_| {
            let v: String = input("pg-code").chars().filter(|c| c.is_ascii_digit()).take(6).collect();
            set_input("pg-code", &v);
            if v.len() == 6 {
                spawn_local(check_code());
            }
        });
    }
    if let Some(b) = by_id("pg-logout") {
        dom::on(&b, "click", |_| {
            spawn_local(async {
                hide();
                crate::shell::logout().await;
            })
        });
    }
}

fn show() {
    bind();
    dom::set_hidden("phone-gate", false);
    dom::set_hidden("pg-code-row", true);
    dom::set_hidden("pg-check", true);
    dom::set_hidden("pg-resend", true);
    dom::set_hidden("pg-send", false);
    msg("", false);
}
fn hide() {
    dom::set_hidden("phone-gate", true);
}

/// 로그인한 뒤 한 번: 새 계정인데 아직 인증 안 했으면 인증 화면을 띄워요
pub async fn check() {
    let Some(c) = cloud::cloud() else { return };
    if cloud::get(&c, "emu").is_truthy() {
        return;
    }
    let created = cloud::get(&c, "createdAt").as_f64().unwrap_or(0.0);
    if created < CUTOFF {
        return;
    }
    match cloud::call("phoneVerified", &[]).await {
        Ok(v) if v.is_truthy() => {}
        Ok(_) => show(),
        Err(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::pretty;
    #[test]
    fn formats_numbers() {
        assert_eq!(pretty("01012345678"), "010-1234-5678");
        assert_eq!(pretty("010-12"), "010-12");
        assert_eq!(pretty("010"), "010");
        assert_eq!(pretty("0101234567899"), "010-1234-5678");
    }
}
