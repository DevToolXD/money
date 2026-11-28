//! 앱 설치 안내: 폰 브라우저로 들어오면 앱으로 설치하라고 권해요 (이미 앱이거나 홈 화면 앱이면 아무것도 안 해요).
//! - 안드로이드: 사이트에 올려 둔 설치 파일(capitalism.apk)을 받게 해요. 들어올 때마다 안내 창이 뜨고, 아래에는 작은 회색 안내가 계속 있어요.
//! - 아이폰: 사파리에서 "홈 화면에 추가"로 웹앱을 설치하는 방법을 안내해요 (같은 모양).
//! 앱(APK)은 브라우저 이름표 끝에 "JabonApp"을 붙여서 사이트가 알아봐요 (android/src/.../MainActivity.java).

use js_sys::Reflect;
use wasm_bindgen::JsValue;

use crate::dom::{self, by_id};

const APK: &str = "capitalism.apk";

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Android,
    Ios,
}

fn get(o: &JsValue, k: &str) -> JsValue {
    Reflect::get(o, &JsValue::from_str(k)).unwrap_or(JsValue::UNDEFINED)
}

/// 이 기기가 어떤 안내를 받아야 하나 (이미 앱이면 None)
fn detect() -> Option<Kind> {
    let w = web_sys::window()?;
    let nav = get(&w, "navigator");
    let ua = get(&nav, "userAgent").as_string().unwrap_or_default();
    if ua.contains("JabonApp") {
        return None;
    }
    // 홈 화면 앱(아이폰 navigator.standalone, 그 밖의 display-mode)으로 열었으면 이미 설치한 거예요
    if get(&nav, "standalone").is_truthy() {
        return None;
    }
    if let Ok(Some(m)) = w.match_media("(display-mode: standalone)") {
        if m.matches() {
            return None;
        }
    }
    if ua.contains("Android") {
        return Some(Kind::Android);
    }
    let touch = get(&nav, "maxTouchPoints").as_f64().unwrap_or(0.0) > 1.0;
    if ua.contains("iPhone") || ua.contains("iPad") || ua.contains("iPod") || (ua.contains("Macintosh") && touch) {
        return Some(Kind::Ios);
    }
    None
}

const SHARE: &str = r#"<svg class="inst-ic" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 15V4"/><path d="M8.5 7.5 12 4l3.5 3.5"/><path d="M6 11H5.5A1.5 1.5 0 0 0 4 12.5v6A1.5 1.5 0 0 0 5.5 20h13a1.5 1.5 0 0 0 1.5-1.5v-6a1.5 1.5 0 0 0-1.5-1.5H18"/></svg>"#;
const PLUS: &str = r#"<svg class="inst-ic" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="4" y="4" width="16" height="16" rx="3.5"/><path d="M12 8.5v7M8.5 12h7"/></svg>"#;
const DOWN: &str = r#"<svg class="inst-ic" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 4v11"/><path d="M8 11.5l4 4 4-4"/><path d="M5 19.5h14"/></svg>"#;

fn body() -> Option<web_sys::HtmlElement> {
    dom::doc().body()
}

fn add(html: &str) -> Option<web_sys::HtmlElement> {
    let b = body()?;
    let host = dom::doc().create_element("div").ok()?;
    host.set_inner_html(html);
    let el: web_sys::HtmlElement = host.first_element_child()?.dyn_into_html()?;
    let _ = b.append_child(&el);
    Some(el)
}

trait IntoHtml {
    fn dyn_into_html(self) -> Option<web_sys::HtmlElement>;
}
impl IntoHtml for web_sys::Element {
    fn dyn_into_html(self) -> Option<web_sys::HtmlElement> {
        wasm_bindgen::JsCast::dyn_into(self).ok()
    }
}

/// 아래에 계속 붙어 있는 작은 회색 안내
fn bar(k: Kind) {
    let html = match k {
        Kind::Android => format!(r#"<a class="inst-bar" id="inst-bar" href="{APK}" download="{APK}">{DOWN}<span>앱으로 설치하면 더 빨라요 · APK 설치</span></a>"#),
        Kind::Ios => format!(r#"<button class="inst-bar" id="inst-bar" type="button">{SHARE}<span>앱으로 설치하면 더 빨라요 · 홈 화면에 추가</span></button>"#),
    };
    let Some(el) = add(&html) else { return };
    if k == Kind::Ios {
        dom::on_tap(&el, sheet_ios);
    }
}

fn close() {
    if let Some(s) = by_id("inst-sheet") {
        s.remove();
    }
}

fn open_sheet(inner: &str) {
    close();
    let html = format!(r#"<div class="inst-sheet" id="inst-sheet" role="dialog" aria-modal="true" aria-label="앱 설치"><div class="inst-dim" data-close="1"></div><div class="inst-card">{inner}</div></div>"#);
    let Some(el) = add(&html) else { return };
    dom::on(&el, "click", |e| {
        if dom::closest(&e, "[data-close]").is_some() {
            close();
        }
    });
}

fn sheet_android() {
    open_sheet(&format!(
        r#"<span class="inst-badge">{DOWN}</span><h2>앱으로 설치하세요</h2><p>설치 파일(APK)을 받아 열면 홈 화면에서 바로 켜져요. 받은 뒤 "출처를 알 수 없는 앱" 허용이 나오면 허용해 주세요.</p><a class="btn primary inst-go" href="{APK}" download="{APK}" data-close="1">APK 설치</a><button class="inst-later" type="button" data-close="1">나중에</button>"#
    ));
}

fn sheet_ios() {
    open_sheet(&format!(
        r#"<span class="inst-badge">{SHARE}</span><h2>앱으로 설치하세요</h2><p>사파리에서 홈 화면에 추가하면 앱처럼 열려요.</p><ol class="inst-steps"><li>{SHARE}<span>아래 가운데 <b>공유</b> 버튼을 눌러요</span></li><li>{PLUS}<span><b>홈 화면에 추가</b>를 골라요</span></li><li><b class="inst-n">3</b><span>오른쪽 위 <b>추가</b>를 누르면 끝이에요</span></li></ol><button class="btn primary inst-go" type="button" data-close="1">알겠어요</button>"#
    ));
}

/// 앱을 열 때 한 번: 아래 안내를 붙이고, 잠시 뒤 안내 창을 띄워요 (들어올 때마다)
pub fn init() {
    let Some(k) = detect() else { return };
    bar(k);
    wasm_bindgen_futures::spawn_local(async move {
        dom::sleep(2200).await;
        match k {
            Kind::Android => sheet_android(),
            Kind::Ios => sheet_ios(),
        }
    });
}
