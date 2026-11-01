//! 파이어베이스 다리 (`window.cloud`)를 러스트 함수로 감싸요.
//!
//! 파이어베이스는 JS 묶음(`app/firebase.js`, 원본은 `app/firebase.src.js`)이 맡아요.
//! 실시간 듣기와 트랜잭션을 그대로 쓰기 위해서예요 (docs/rust-migration-plan.md 3.2).
//! 여기서는 그 함수들을 이름으로 불러서, 돌아온 값을 러스트 구조체로 바꿔요.

use js_sys::{Array, Function, Object, Promise, Reflect};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

/// 다리에서 온 오류. `code`는 파이어베이스 오류 코드나 다리가 정한 코드예요 (예: `permission-denied`, `busy`).
#[derive(Debug, Clone, PartialEq)]
pub struct CloudError {
    pub code: String,
    pub message: String,
}

impl CloudError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into() }
    }

    fn from_js(v: JsValue) -> Self {
        let field = |k: &str| Reflect::get(&v, &JsValue::from_str(k)).ok().and_then(|x| x.as_string());
        Self {
            code: field("code").unwrap_or_default(),
            message: field("message").or_else(|| v.as_string()).unwrap_or_else(|| "처리하지 못했어요".into()),
        }
    }
}

impl std::fmt::Display for CloudError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

/// 지금의 `window.cloud` (아직 연결 전이면 없어요)
pub fn cloud() -> Option<Object> {
    let c = Reflect::get(&js_sys::global(), &JsValue::from_str("cloud")).ok()?;
    if c.is_undefined() || c.is_null() {
        None
    } else {
        c.dyn_into().ok()
    }
}

/// 로그인했는지
pub fn signed_in() -> bool {
    cloud()
        .and_then(|c| Reflect::get(&c, &JsValue::from_str("user")).ok())
        .map(|u| u.is_truthy())
        .unwrap_or(false)
}

/// `window.cloud.<name>(...args)`를 부르고, 약속(Promise)이면 끝날 때까지 기다려요
async fn call(name: &str, args: &[JsValue]) -> Result<JsValue, CloudError> {
    let c = cloud().ok_or_else(|| CloudError::new("nocloud", "서버에 연결하지 못했어요"))?;
    let f: Function = Reflect::get(&c, &JsValue::from_str(name))
        .map_err(CloudError::from_js)?
        .dyn_into()
        .map_err(|_| CloudError::new("nofn", format!("{name} 함수가 없어요")))?;
    let list = Array::new();
    for a in args {
        list.push(a);
    }
    let out = f.apply(&c, &list).map_err(CloudError::from_js)?;
    match out.dyn_into::<Promise>() {
        Ok(p) => JsFuture::from(p).await.map_err(CloudError::from_js),
        Err(v) => Ok(v),
    }
}

fn shape<T: DeserializeOwned>(v: JsValue) -> Result<T, CloudError> {
    serde_wasm_bindgen::from_value(v).map_err(|e| CloudError::new("shape", e.to_string()))
}

/* ---------- 공지 ---------- */

/// 공지 한 개 (`firebase.src.js`의 `noticeOf`와 같은 모양)
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Notice {
    pub id: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub by: String,
    /// 투표 항목 (투표가 없으면 없어요)
    #[serde(default)]
    pub options: Option<Vec<String>>,
    /// 올린 시각 (밀리초)
    #[serde(default)]
    pub at: f64,
}

impl Notice {
    /// 투표 항목 (2개 이상일 때만 투표예요)
    pub fn poll(&self) -> Option<&[String]> {
        self.options.as_deref().filter(|o| o.len() >= 2)
    }
}

/// 공지 아이디 목록 (최신이 앞)
pub async fn notice_ids() -> Result<Vec<String>, CloudError> {
    shape(call("noticeIds", &[]).await?)
}

/// 아이디 목록의 공지들 (앞의 40개만 읽어요)
pub async fn notice_list(ids: &[String]) -> Result<Vec<Notice>, CloudError> {
    let arr: Array = ids.iter().map(|s| JsValue::from_str(s)).collect();
    shape(call("noticeList", &[arr.into()]).await?)
}

/// 아직 안 본 공지 중 가장 오래된 것 하나
pub async fn unseen_notice() -> Result<Option<Notice>, CloudError> {
    let v = call("unseenNotice", &[]).await?;
    if v.is_null() || v.is_undefined() {
        Ok(None)
    } else {
        shape(v).map(Some)
    }
}

/// 본 것으로 적어요 (다시 띄우지 않아요)
pub async fn mark_notice_seen(id: &str) -> Result<(), CloudError> {
    call("markNoticeSeen", &[JsValue::from_str(id)]).await.map(|_| ())
}

/// 투표 (한 번만)
pub async fn vote_notice(id: &str, choice: u32) -> Result<(), CloudError> {
    call("voteNotice", &[JsValue::from_str(id), JsValue::from(choice)]).await.map(|_| ())
}

/* ---------- 도움 ---------- */

/// "2026년 11월 1일" (시각이 없으면 빈 글자)
pub fn date_ko(ms: f64) -> String {
    if ms <= 0.0 {
        return String::new();
    }
    let d = js_sys::Date::new(&JsValue::from_f64(ms));
    format!("{}년 {}월 {}일", d.get_full_year(), d.get_month() + 1, d.get_date())
}

/// 콘솔에 경고를 남겨요
pub fn warn(msg: &str) {
    web_sys::console::warn_1(&JsValue::from_str(msg));
}
