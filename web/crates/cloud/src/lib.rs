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
/// `window.cloud.<name>(...args)`를 부르고, 돌아온 값(약속이면 약속 그대로)을 줘요. JS 쪽 줄 세우기(walletOp)에 넘길 때 써요
pub fn call_now(name: &str, args: &[JsValue]) -> Result<JsValue, CloudError> {
    let c = cloud().ok_or_else(|| CloudError::new("nocloud", "서버에 연결하지 못했어요"))?;
    let f: Function = Reflect::get(&c, &JsValue::from_str(name))
        .map_err(CloudError::from_js)?
        .dyn_into()
        .map_err(|_| CloudError::new("nofn", format!("{name} 함수가 없어요")))?;
    let list = Array::new();
    for a in args {
        list.push(a);
    }
    f.apply(&c, &list).map_err(CloudError::from_js)
}

/// JS 약속이면 기다리고, 아니면 그대로
pub async fn settle(v: JsValue) -> Result<JsValue, CloudError> {
    match v.dyn_into::<Promise>() {
        Ok(p) => JsFuture::from(p).await.map_err(CloudError::from_js),
        Err(v) => Ok(v),
    }
}

pub async fn call(name: &str, args: &[JsValue]) -> Result<JsValue, CloudError> {
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


/* ---------- 공지 ---------- */

/// 공지 한 개 (`firebase.src.js`의 `noticeOf`와 같은 모양)
pub fn shape<T: DeserializeOwned>(v: JsValue) -> Result<T, CloudError> {
    serde_wasm_bindgen::from_value(v).map_err(|e| CloudError::new("shape", e.to_string()))
}

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

/// 오너: 투표 집계 (항목마다 몇 명)
pub async fn notice_tally(n: &Notice) -> Result<Vec<u32>, CloudError> {
    let o = js_sys::Object::new();
    let _ = Reflect::set(&o, &JsValue::from_str("id"), &JsValue::from_str(&n.id));
    let opts: Array = n.options.clone().unwrap_or_default().iter().map(|s| JsValue::from_str(s)).collect();
    let _ = Reflect::set(&o, &JsValue::from_str("options"), &opts);
    shape(call("noticeTally", &[o.into()]).await?)
}

/* ---------- 관리 (오너) ---------- */

/// 자동 검색 결과 한 명
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ProfileHit {
    pub uid: String,
    pub id: String,
    pub name: String,
}


/// 멸공 지급 (아이디나 이름)
pub async fn admin_grant(id: &str, amt: u32) -> Result<(), CloudError> {
    call("adminGrant", &[JsValue::from_str(id), JsValue::from(amt)]).await.map(|_| ())
}

/// 공지 올리기 (투표 항목은 비우면 투표가 없어요)
pub async fn admin_notice(title: &str, body: &str, options: &[String]) -> Result<(), CloudError> {
    let o = js_sys::Object::new();
    let _ = Reflect::set(&o, &JsValue::from_str("title"), &JsValue::from_str(title));
    let _ = Reflect::set(&o, &JsValue::from_str("body"), &JsValue::from_str(body));
    let opts: Array = options.iter().map(|s| JsValue::from_str(s)).collect();
    let _ = Reflect::set(&o, &JsValue::from_str("options"), &opts);
    call("adminNotice", &[o.into()]).await.map(|_| ())
}

/// 계정 데이터 지우기
pub async fn admin_delete(id: &str) -> Result<(), CloudError> {
    call("adminDelete", &[JsValue::from_str(id)]).await.map(|_| ())
}

/* ---------- 회사·일 ---------- */

/// 회사 한 곳 (`firebase.src.js`의 companyOf와 같은 모양)
#[derive(Debug, Clone)]
pub struct Company {
    pub id: String,
    pub name: String,
    pub logo: String,
    pub kind: String,
    pub owner: String,
    pub staff: f64,
    pub open: f64,
    pub at: f64,
}

impl Company {
    pub fn from_js(v: &JsValue) -> Self {
        let s = |k: &str| get(v, k).as_string().unwrap_or_default();
        let n = |k: &str| get(v, k).as_f64().unwrap_or(0.0);
        Company { id: s("id"), name: s("name"), logo: s("logo"), kind: s("kind"), owner: s("owner"), staff: n("staff"), open: n("open"), at: n("at") }
    }
}















/* ---------- 상점 ---------- */

/// 상점 진열 상품 한 개. `raw`(상품)와 `co_raw`(파는 회사)는 그림을 그리는 JS 함수에 넘겨요
#[derive(Debug, Clone)]
pub struct ShopItem {
    pub id: String,
    pub cid: String,
    pub name: String,
    pub co_name: String,
    pub price_cents: f64,
    pub stock: i64,
    pub raw: JsValue,
    pub co_raw: JsValue,
}


/// 상점의 물건(컴퓨터)·자원(전기) 칸: 회사가 내놓은 단가와 재고
#[derive(Debug, Clone, PartialEq)]
pub struct Offer {
    pub co_name: String,
    pub price: f64,
    pub elec: f64,
}


/* ---------- 인벤토리 ---------- */

/// 인벤토리 한 칸. `raw`는 다리에서 온 그대로의 값이에요 (상품 그림을 그리는 JS 함수에 넘겨요)
#[derive(Debug, Clone)]
pub struct InvItem {
    pub id: String,
    pub cid: String,
    pub pid: String,
    pub group: String,
    pub name: String,
    pub co_name: String,
    pub n: i64,
    pub raw: JsValue,
}





/* ---------- 도움 ---------- */

/// 객체의 값 하나 (없으면 undefined)
pub fn get(obj: &JsValue, key: &str) -> JsValue {
    Reflect::get(obj, &JsValue::from_str(key)).unwrap_or(JsValue::UNDEFINED)
}

/// "2026년 11월 1일" (시각이 없으면 빈 글자)
pub fn date_ko(ms: f64) -> String {
    if ms <= 0.0 {
        return String::new();
    }
    let d = js_sys::Date::new(&JsValue::from_f64(ms));
    format!("{}년 {}월 {}일", d.get_full_year(), d.get_month() + 1, d.get_date())
}

/// "11. 1. 오후 06:30" (월·일·시·분, 한국 표기)
pub fn when_ko(ms: f64) -> String {
    if ms <= 0.0 {
        return String::new();
    }
    let d = js_sys::Date::new(&JsValue::from_f64(ms));
    let o = js_sys::Object::new();
    for (k, v) in [("month", "numeric"), ("day", "numeric"), ("hour", "2-digit"), ("minute", "2-digit")] {
        let _ = Reflect::set(&o, &JsValue::from_str(k), &JsValue::from_str(v));
    }
    d.to_locale_string("ko-KR", &o).into()
}

/// 숫자를 한국식으로 (천 단위 쉼표, 소수는 최대 max_frac자리, 끝의 0은 지워요). 예전 JS의 toLocaleString("ko-KR")과 같아요
pub fn fmt_num(v: f64, max_frac: u32) -> String {
    let p = 10f64.powi(max_frac as i32);
    let r = (v * p).round() / p;
    let neg = r < 0.0;
    let r = r.abs();
    let int = r.trunc() as u64;
    let mut frac = format!("{:.*}", max_frac as usize, r - r.trunc());
    frac = frac.trim_start_matches('0').trim_end_matches('0').trim_end_matches('.').to_string();
    let digits = int.to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if !frac.is_empty() {
        out.push_str(&frac);
    }
    if neg && (int > 0 || !frac.is_empty()) {
        out.insert(0, '-');
    }
    out
}


/// 콘솔에 경고를 남겨요
pub fn warn(msg: &str) {
    web_sys::console::warn_1(&JsValue::from_str(msg));
}

/// 오류 코드를 사람이 알 수 있는 이유로 바꿔요 (왜 안 됐는지 화면에 보여 주려고)
pub fn why(code: &str) -> &'static str {
    match code {
        "permission-denied" => "지금은 할 수 없는 일이에요",
        "unavailable" | "network-request-failed" | "walletop" => "인터넷 연결을 확인해 주세요",
        "deadline-exceeded" | "aborted" | "busy" => "서버가 바빠요. 다시 눌러 주세요",
        "resource-exhausted" => "조금 천천히 눌러 주세요",
        "failed-precondition" => "지금 상태에서는 할 수 없어요",
        "not-found" => "찾는 정보가 없어요",
        "insufficient" => "멸공이 모자라요",
        "unauthenticated" => "다시 로그인해 주세요",
        _ => "잠시 뒤 다시 해 주세요",
    }
}
