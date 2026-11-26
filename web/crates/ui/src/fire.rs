//! 파이어베이스 연결 (예전 app/firebase.src.js의 우리 코드를 그대로 옮겼어요). 구글 라이브러리 함수는 window.fb에 있어요.
//! - 로그인은 아이디·비밀번호(안 보이는 이메일로 바꿔서) 하나뿐이고 익명 로그인은 쓰지 않아요.
//! - 서버 규칙(firestore.rules)이 진짜 지킴이고, 여기서는 규칙이 받아 주는 모양으로만 써요.
//! - 읽기·쓰기를 아끼려고: 메시지는 최근 40개만 듣고, 사람 정보는 30분 캐시하고, 지갑 이자는 10분에 한 번만 정산해요.
//! 설정값이 비어 있거나 연결에 실패하면 window.cloud는 null이에요. 화면 코드와 시험은 예전처럼 window.cloud를 불러요.

use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{future_to_promise, JsFuture};

type R = Result<JsValue, JsValue>;
type Fut = Pin<Box<dyn Future<Output = R>>>;

const PROFILE_TTL: f64 = 30.0 * 60.0 * 1000.0;
const HI: &str = "\u{f8ff}";

/* ---------- JS 도움 ---------- */

fn s(v: &str) -> JsValue {
    JsValue::from_str(v)
}
fn n(v: f64) -> JsValue {
    JsValue::from_f64(v)
}
fn get(o: &JsValue, k: &str) -> JsValue {
    if o.is_null() || o.is_undefined() {
        return JsValue::UNDEFINED;
    }
    js_sys::Reflect::get(o, &s(k)).unwrap_or(JsValue::UNDEFINED)
}
fn set(o: &JsValue, k: &str, v: &JsValue) {
    let _ = js_sys::Reflect::set(o, &s(k), v);
}
fn num(v: &JsValue) -> f64 {
    v.as_f64().unwrap_or(f64::NAN)
}
fn is_int(v: &JsValue) -> bool {
    js_sys::Number::is_integer(v)
}
fn st(v: &JsValue) -> String {
    v.as_string().unwrap_or_default()
}
/// JS의 String(v)
fn jstr(v: &JsValue) -> String {
    if let Some(x) = v.as_string() {
        return x;
    }
    get(&js_sys::global(), "String").dyn_into::<js_sys::Function>().ok().and_then(|f| f.call1(&JsValue::NULL, v).ok()).and_then(|x| x.as_string()).unwrap_or_default()
}
fn o(pairs: &[(&str, JsValue)]) -> JsValue {
    crate::dom::obj(pairs)
}
/// {...a, ...pairs}
fn ext(a: &JsValue, pairs: &[(&str, JsValue)]) -> JsValue {
    let base = js_sys::Object::new();
    if a.is_object() {
        js_sys::Object::assign(&base, a.unchecked_ref());
    }
    for (k, v) in pairs {
        set(&base, k, v);
    }
    base.into()
}
fn arr(v: &[JsValue]) -> JsValue {
    v.iter().cloned().collect::<js_sys::Array>().into()
}
fn fbo() -> JsValue {
    get(&js_sys::global(), "fb")
}
/// window.fb의 라이브러리 함수 부르기
fn f(name: &str, args: &[JsValue]) -> R {
    let func: js_sys::Function = get(&fbo(), name).dyn_into()?;
    func.apply(&JsValue::UNDEFINED, &args.iter().cloned().collect())
}
/// obj.name(...args)
fn m(obj: &JsValue, name: &str, args: &[JsValue]) -> R {
    let func: js_sys::Function = get(obj, name).dyn_into()?;
    func.apply(obj, &args.iter().cloned().collect())
}
/// 약속(Promise)이면 기다려요
async fn aw(v: R) -> R {
    let v = v?;
    match v.clone().dyn_into::<js_sys::Promise>() {
        Ok(p) => JsFuture::from(p).await,
        Err(_) => Ok(v),
    }
}
fn now() -> f64 {
    js_sys::Date::now()
}
fn ts() -> JsValue {
    f("serverTimestamp", &[]).unwrap_or(JsValue::NULL)
}
fn inc(v: f64) -> JsValue {
    f("increment", &[n(v)]).unwrap_or(JsValue::NULL)
}
fn from_millis(v: f64) -> JsValue {
    m(&get(&fbo(), "Timestamp"), "fromMillis", &[n(v)]).unwrap_or(JsValue::NULL)
}
fn ms(t: &JsValue) -> f64 {
    if t.is_object() && get(t, "toMillis").is_function() {
        return m(t, "toMillis", &[]).map(|v| num(&v)).unwrap_or(0.0);
    }
    let v = js_sys::Number::from(t.clone()).value_of();
    if v.is_nan() || v == 0.0 {
        0.0
    } else {
        v
    }
}
fn exists(snap: &JsValue) -> bool {
    m(snap, "exists", &[]).map(|v| v.is_truthy()).unwrap_or(false)
}
fn data(snap: &JsValue) -> JsValue {
    m(snap, "data", &[]).unwrap_or(JsValue::UNDEFINED)
}
fn data_est(snap: &JsValue) -> JsValue {
    m(snap, "data", &[o(&[("serverTimestamps", s("estimate"))])]).unwrap_or(JsValue::UNDEFINED)
}
fn docs(snap: &JsValue) -> Vec<JsValue> {
    get(snap, "docs").dyn_into::<js_sys::Array>().map(|a| a.to_vec()).unwrap_or_default()
}
fn fail(code: &str, msg: &str) -> JsValue {
    let e = js_sys::Error::new(msg);
    set(&e, "code", &s(code));
    e.into()
}
fn ko(v: f64) -> String {
    m(&n(v), "toLocaleString", &[s("ko-KR")]).map(|x| st(&x)).unwrap_or_default()
}

/* ---------- 오류 ---------- */

fn messages(code: &str) -> Option<&'static str> {
    Some(match code {
        "auth/invalid-email" => "아이디가 올바르지 않아요",
        "auth/missing-password" => "비밀번호를 입력해 주세요",
        "auth/weak-password" => "비밀번호가 너무 약해요",
        "auth/email-already-in-use" => "이미 있는 아이디예요. 다른 아이디를 써 주세요",
        "auth/invalid-credential" | "auth/wrong-password" | "auth/user-not-found" => "아이디 또는 비밀번호가 맞지 않아요",
        "auth/too-many-requests" => "시도가 너무 많아요. 잠시 뒤 다시 해 주세요",
        "auth/operation-not-allowed" => "이 로그인 방법이 아직 켜지지 않았어요",
        "auth/network-request-failed" => "인터넷 연결을 확인해 주세요",
        "auth/invalid-verification-code" => "인증번호가 맞지 않아요",
        "phone-used" => "이미 이 휴대폰 번호로 가입한 계정이 있어요. 그 계정으로 로그인해 주세요",
        "no-phone" => "휴대폰 인증번호를 먼저 받아 주세요",
        "no-code" => "문자로 받은 인증번호를 입력해 주세요",
        "bad-phone" => "휴대폰 번호를 확인해 주세요 (예: 010-1234-5678)",
        "auth/code-expired" => "인증번호가 만료됐어요. 다시 받아 주세요",
        "auth/invalid-phone-number" => "휴대폰 번호를 확인해 주세요",
        "auth/quota-exceeded" => "오늘 보낼 수 있는 문자를 다 썼어요. 내일 다시 해 주세요",
        "auth/captcha-check-failed" => "자동 가입 방지 확인에 실패했어요. 다시 해 주세요",
        "auth/credential-already-in-use" => "이미 다른 계정에 쓰인 정보예요",
        "auth/provider-already-linked" => "이미 가입한 계정이에요",
        _ => return None,
    })
}
// 프로젝트에서 Firestore(데이터베이스)가 아직 꺼져 있거나 만들어지지 않았을 때 서버가 보내는 오류예요
fn db_down(err: &JsValue) -> bool {
    let msg = get(err, "message");
    let t = if msg.is_undefined() || msg.is_null() || !msg.is_truthy() { String::new() } else { jstr(&msg) }.to_lowercase();
    ["has not been used", "is disabled", "service_disabled", "database (default) does not exist"].iter().any(|k| t.contains(k))
}
fn report(err: &JsValue) {
    let g = js_sys::global();
    if db_down(err) && !get(&g, "cloudDown").is_truthy() {
        set(&g, "cloudDown", &true.into());
        if let Ok(ev) = web_sys::CustomEvent::new("cloud-down") {
            let _ = web_sys::window().map(|w| w.dispatch_event(&ev));
        }
    }
}
fn rethrow<T>(r: Result<T, JsValue>) -> Result<T, JsValue> {
    r.map_err(|e| {
        report(&e);
        e
    })
}
fn explain(err: &JsValue) -> String {
    if db_down(err) {
        return "서버 데이터베이스(Firestore)가 꺼져 있어요. 관리자가 Firebase에서 켜야 해요".into();
    }
    messages(&st(&get(err, "code"))).unwrap_or("처리하지 못했어요. 잠시 뒤 다시 해 주세요").into()
}
fn to_email(id: &str) -> String {
    format!("{}@money-sim.app", id.trim().to_lowercase())
}

/* ---------- 상태 ---------- */

#[derive(Default)]
struct Rank {
    sent: Option<f64>, // null이면 아직 몰라요
    at: f64,
    me: JsValue,
    top: JsValue,
    top_at: f64,
}
struct St {
    auth: JsValue,
    db: JsValue,
    mem: HashMap<String, JsValue>,
    list_cursor: JsValue,
    names: HashMap<String, String>,
    rank: Rank,
    ensured: String,
}
thread_local! {
    static ST: RefCell<Option<St>> = const { RefCell::new(None) };
}
fn auth() -> JsValue {
    ST.with(|x| x.borrow().as_ref().map(|s| s.auth.clone()).unwrap_or(JsValue::NULL))
}
fn db() -> JsValue {
    ST.with(|x| x.borrow().as_ref().map(|s| s.db.clone()).unwrap_or(JsValue::NULL))
}
fn with<T>(fun: impl FnOnce(&mut St) -> T) -> T {
    ST.with(|x| fun(x.borrow_mut().as_mut().expect("cloud")))
}
fn current() -> JsValue {
    get(&auth(), "currentUser")
}
fn me() -> Result<JsValue, JsValue> {
    let u = current();
    if u.is_null() || u.is_undefined() {
        return Err(js_sys::Error::new("로그인이 필요해요").into());
    }
    Ok(u)
}
fn uid_of() -> Result<String, JsValue> {
    Ok(st(&get(&me()?, "uid")))
}
fn my_id() -> String {
    let e = st(&get(&current(), "email"));
    e.strip_suffix("@money-sim.app").map(String::from).unwrap_or(e)
}
fn d(path: &[&str]) -> JsValue {
    let mut a = vec![db()];
    a.extend(path.iter().map(|p| s(p)));
    f("doc", &a).unwrap_or(JsValue::NULL)
}
fn c(path: &[&str]) -> JsValue {
    let mut a = vec![db()];
    a.extend(path.iter().map(|p| s(p)));
    f("collection", &a).unwrap_or(JsValue::NULL)
}
fn q(base: JsValue, cons: Vec<JsValue>) -> JsValue {
    let mut a = vec![base];
    a.extend(cons);
    f("query", &a).unwrap_or(JsValue::NULL)
}
fn w(field: JsValue, op: &str, v: JsValue) -> JsValue {
    f("where", &[field, s(op), v]).unwrap_or(JsValue::NULL)
}
fn lim(k: f64) -> JsValue {
    f("limit", &[n(k)]).unwrap_or(JsValue::NULL)
}
fn order(field: &str, dir: Option<&str>) -> JsValue {
    match dir {
        Some(dd) => f("orderBy", &[s(field), s(dd)]),
        None => f("orderBy", &[s(field)]),
    }
    .unwrap_or(JsValue::NULL)
}
fn wallet_ref() -> Result<JsValue, JsValue> {
    Ok(d(&["wallets", &uid_of()?]))
}
async fn get_doc(r: JsValue) -> R {
    aw(f("getDoc", &[r])).await
}
async fn get_docs(qq: JsValue) -> R {
    aw(f("getDocs", &[qq])).await
}
async fn set_doc(r: JsValue, v: JsValue) -> R {
    aw(f("setDoc", &[r, v])).await
}
async fn set_merge(r: JsValue, v: JsValue) -> R {
    aw(f("setDoc", &[r, v, o(&[("merge", true.into())])])).await
}
async fn update_doc(r: JsValue, v: JsValue) -> R {
    aw(f("updateDoc", &[r, v])).await
}
fn batch() -> JsValue {
    f("writeBatch", &[db()]).unwrap_or(JsValue::NULL)
}
async fn commit(b: &JsValue) -> R {
    aw(m(b, "commit", &[])).await
}
fn new_id(col: JsValue) -> JsValue {
    f("doc", &[col]).unwrap_or(JsValue::NULL)
}
/// 한 묶음(트랜잭션). body는 다시 불릴 수 있어요
async fn run_tx(body: impl Fn(JsValue) -> Fut + 'static) -> R {
    let cb = Closure::<dyn FnMut(JsValue) -> js_sys::Promise>::new(move |tx: JsValue| future_to_promise(body(tx)));
    let r = aw(f("runTransaction", &[db(), cb.as_ref().clone()])).await;
    drop(cb);
    r
}
async fn tx_get(tx: &JsValue, r: &JsValue) -> R {
    aw(m(tx, "get", &[r.clone()])).await
}

/* ---------- 사람 정보 캐시 (메모리 + 이 기기) ---------- */
fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}
fn read_cache(uid: &str) -> JsValue {
    if let Some(hit) = with(|x| x.mem.get(uid).cloned()) {
        return hit;
    }
    if let Some(raw) = storage().and_then(|s| s.get_item(&format!("pf:{uid}")).ok().flatten()) {
        if let Ok(v) = js_sys::JSON::parse(&raw) {
            if !v.is_null() && !v.is_undefined() {
                with(|x| x.mem.insert(uid.to_string(), v.clone()));
                return v;
            }
        }
    }
    JsValue::NULL
}
fn write_cache(uid: &str, dd: &JsValue) {
    let v = o(&[("d", dd.clone()), ("t", n(now()))]);
    with(|x| x.mem.insert(uid.to_string(), v.clone()));
    if let (Some(s_), Ok(j)) = (storage(), js_sys::JSON::stringify(&v)) {
        let _ = s_.set_item(&format!("pf:{uid}"), &String::from(j));
    }
}

/* ---------- 공지 본 기록 (기기) ---------- */
fn seen_key(uid: &str) -> String {
    format!("pv-noticeSeen-{uid}")
}
fn seen_here(uid: &str) -> Vec<String> {
    let raw = storage().and_then(|s| s.get_item(&seen_key(uid)).ok().flatten()).unwrap_or_else(|| "[]".into());
    let mut out: Vec<String> = Vec::new();
    if let Ok(a) = js_sys::JSON::parse(&raw).and_then(|v| v.dyn_into::<js_sys::Array>().map_err(|e| e)) {
        for v in a.iter() {
            let k = jstr(&v);
            if !out.contains(&k) {
                out.push(k);
            }
        }
    }
    out
}
fn remember_seen(uid: &str, ids: &[String]) {
    let mut all = seen_here(uid);
    for id in ids {
        if !all.contains(id) {
            all.push(id.clone());
        }
    }
    let start = all.len().saturating_sub(60);
    let a: js_sys::Array = all[start..].iter().map(|x| s(x)).collect();
    if let (Some(s_), Ok(j)) = (storage(), js_sys::JSON::stringify(&a)) {
        let _ = s_.set_item(&seen_key(uid), &String::from(j));
    }
}
fn notice_of(snap: &JsValue) -> JsValue {
    let dd = data_est(snap);
    let or = |k: &str| {
        let v = get(&dd, k);
        if v.is_truthy() {
            v
        } else {
            s("")
        }
    };
    let opts = get(&dd, "options");
    o(&[
        ("id", get(snap, "id")),
        ("title", or("title")),
        ("body", or("body")),
        ("by", or("by")),
        ("options", if js_sys::Array::is_array(&opts) { opts } else { JsValue::NULL }),
        ("at", n(ms(&get(&dd, "createdAt")))),
    ])
}

/* ---------- 모양 바꾸기 ---------- */

// 회사 문서를 화면에서 쓰는 모양으로 바꿔요. 예전 회사는 종류·모집·직원 값이 없어서 기본값(식품, 0명)으로 봐요
fn nn(v: JsValue) -> JsValue {
    if v.is_undefined() || v.is_null() {
        JsValue::NULL
    } else {
        v
    }
}
// 연구실 숫자 기본값
// 문서에 쓸 모양: 기본값인 칸(등급 1, 성능 100, 공장 비움, 판매 기록 없음)은 적지 않아요
fn sorted(a: &str, b: &str) -> [String; 2] {
    if a < b {
        [a.into(), b.into()]
    } else {
        [b.into(), a.into()]
    }
}
fn inv_ref(cid: &str, pid: &str) -> Result<JsValue, JsValue> {
    Ok(d(&["inventory", &uid_of()?, "items", &format!("{cid}_{pid}")]))
}
// 인벤토리에 넣을 모양: 상품의 그림과 이름을 함께 적어요
fn inv_data(p: &JsValue, cid: &str, pid: &str, k: f64, co_name: &str) -> JsValue {
    let name: String = jstr(&get(p, "name")).chars().take(30).collect();
    let x = o(&[("cid", s(cid)), ("pid", s(pid)), ("group", get(p, "group")), ("name", s(&name)), ("item", get(p, "item")), ("shape", get(p, "shape")), ("color", get(p, "color")), ("n", n(k))]);
    if !co_name.is_empty() {
        set(&x, "coName", &s(&co_name.chars().take(20).collect::<String>()));
    }
    for key in ["cap", "capColor", "content"] {
        let v = get(p, key);
        if is_int(&v) {
            set(&x, key, &v);
        }
    }
    x
}
fn chat_part(id: &JsValue, dd: &JsValue) -> JsValue {
    let group = get(dd, "group") == JsValue::TRUE;
    let owner = get(dd, "owner");
    let or_s = |k: &str| {
        let v = get(dd, k);
        if v.is_truthy() {
            v
        } else {
            s("")
        }
    };
    let reads = js_sys::Object::new();
    let rv = get(dd, "reads");
    if rv.is_truthy() {
        for e in js_sys::Object::entries(rv.unchecked_ref()).iter() {
            let e: js_sys::Array = e.unchecked_into();
            set(&reads, &jstr(&e.get(0)), &n(ms(&e.get(1))));
        }
    }
    let name = if group {
        let v = get(dd, "name");
        s(&if v.is_truthy() { jstr(&v) } else { "단톡방".into() })
    } else {
        s("")
    };
    o(&[
        ("id", id.clone()),
        ("members", get(dd, "members")),
        ("group", group.into()),
        ("name", name),
        ("owner", if owner.is_string() { owner } else { s("") }),
        ("last", or_s("last")),
        ("lastFrom", or_s("lastFrom")),
        ("lastAt", n(ms(&get(dd, "lastAt")))),
        ("reads", reads.into()),
    ])
}
fn msg_part(snap: &JsValue) -> JsValue {
    let dd = data_est(snap);
    let x = js_sys::Object::new();
    set(&x, "id", &get(snap, "id"));
    js_sys::Object::assign(&x, dd.unchecked_ref());
    set(&x, "at", &n(ms(&get(&dd, "at"))));
    x.into()
}

/* ---------- 계정 ---------- */

async fn make_wallet() -> R {
    set_doc(wallet_ref()?, o(&[("bal", n(0.0)), ("rate", n(0.0)), ("t", ts()), ("wt", ts()), ("op", o(&[("kind", s("init"))]))])).await
}
/// 가입할 때 지갑을 만들면서, 초대 링크로 왔으면 같은 묶음에서 초대 기록을 남겨요 (초대한 사람이 없거나 이상하면 지갑만 만들어요)
async fn make_wallet_with(ref_: &str) -> R {
    let uid = uid_of()?;
    let ok_ref = !ref_.is_empty() && ref_ != uid && ref_.len() <= 128 && ref_.chars().all(|c| c.is_ascii_alphanumeric());
    if ok_ref && get_doc(d(&["profiles", ref_])).await.map(|sn| exists(&sn)).unwrap_or(false) {
        let b = batch();
        m(&b, "set", &[wallet_ref()?, o(&[("bal", n(0.0)), ("rate", n(0.0)), ("t", ts()), ("wt", ts()), ("op", o(&[("kind", s("init"))]))])])?;
        m(&b, "set", &[d(&["invites", &uid]), o(&[("by", s(ref_)), ("at", ts()), ("claimed", false.into())])])?;
        if commit(&b).await.is_ok() {
            return Ok(JsValue::UNDEFINED);
        }
    }
    make_wallet().await
}
/* ----- 피드백 ----- */
async fn feedback_send(kind: String, text: String) -> R {
    let text: String = text.trim().chars().take(1000).collect();
    let kind = if ["bug", "idea", "etc"].contains(&kind.as_str()) { kind } else { "etc".into() };
    rethrow(set_doc(new_id(c(&["feedback"])), o(&[("by", s(&uid_of()?)), ("kind", s(&kind)), ("text", s(&text)), ("at", ts())])).await)
}
async fn feedback_list() -> R {
    let snap = rethrow(get_docs(q(c(&["feedback"]), vec![order("at", Some("desc")), lim(50.0)])).await)?;
    Ok(docs(&snap).iter().map(|dd| { let v = data(dd); o(&[("id", get(dd, "id")), ("by", get(&v, "by")), ("kind", get(&v, "kind")), ("text", get(&v, "text")), ("at", n(ms(&get(&v, "at"))))]) }).collect::<js_sys::Array>().into())
}
/* ----- 친구 초대 ----- */
async fn invites_list() -> R {
    let snap = rethrow(get_docs(q(c(&["invites"]), vec![w(s("by"), "==", s(&uid_of()?)), lim(100.0)])).await)?;
    Ok(docs(&snap).iter().map(|dd| { let v = data(dd); o(&[("uid", get(dd, "id")), ("at", n(ms(&get(&v, "at")))), ("claimed", (get(&v, "claimed") == JsValue::TRUE).into())]) }).collect::<js_sys::Array>().into())
}
async fn invite_claim(r: String) -> R {
    let b = batch();
    m(&b, "update", &[d(&["invites", &r]), o(&[("claimed", true.into())])])?;
    m(&b, "update", &[wallet_ref()?, o(&[("bal", inc(50.0)), ("wt", ts()), ("op", o(&[("kind", s("invite")), ("ref", s(&r)), ("amt", n(50.0))]))])])?;
    rethrow(commit(&b).await)?;
    Ok(n(50.0))
}
/* ----- 베타 종료 (관리자 한 번만): 모든 데이터를 지워요 ----- */
async fn beta_state() -> R {
    let sn = get_doc(d(&["meta", "beta"])).await?;
    Ok(if exists(&sn) { get(&data(&sn), "state") } else { s("") })
}
async fn beta_end() -> R {
    let st0 = st(&beta_state().await?);
    if st0 == "done" {
        return Err(fail("done", "베타 종료는 이미 한 번 했어요"));
    }
    if st0.is_empty() {
        rethrow(set_doc(d(&["meta", "beta"]), o(&[("state", s("wiping")), ("at", ts())])).await)?;
    }
    let mut total = 0.0;
    // 하위 묶음부터 지우고, 위 문서를 지워요
    // 규칙(firestore.rules)에 있는 모든 컬렉션이 들어가야 해요. 새 컬렉션을 만들면 여기에도 넣어요 (전화번호 인증 기록 phones·verified·otp·otpday 포함)
    let groups = ["messages", "c", "lab", "products", "items", "votes", "entries"];
    let tops = [
        "posts", "calls", "chats", "invites", "jobs", "companies", "mining", "vitals", "inventory", "ranks", "noticeReads", "notices", "adminLog", "adminAuth", "bag", "contests", "feedback", "game", "gamePlayers", "lands", "plans",
        "postGifts", "presence", "trades", "otp", "otpday", "phones", "verified", "owned", "pulls", "users", "wallets", "profiles",
    ];
    for (is_group, names) in [(true, &groups[..]), (false, &tops[..])] {
        for name in names {
            // 켜져 있는 다른 화면이 다시 만드는 문서가 있어도 끝없이 돌지 않게 몇 번까지만 돌아요
            for _round in 0..40 {
                let base = if is_group { f("collectionGroup", &[db(), s(name)])? } else { c(&[name]) };
                let snap = rethrow(get_docs(q(base, vec![lim(300.0)])).await)?;
                let ds = docs(&snap);
                if ds.is_empty() {
                    break;
                }
                let b = batch();
                for dd in ds.iter() {
                    m(&b, "delete", &[get(dd, "ref")])?;
                }
                rethrow(commit(&b).await)?;
                total += ds.len() as f64;
            }
        }
    }
    let _ = rethrow(aw(f("deleteDoc", &[d(&["meta", "noticeIndex"])])).await);
    rethrow(set_doc(d(&["meta", "beta"]), o(&[("state", s("done")), ("at", ts())])).await)?;
    Ok(n(total))
}

/// wrap: 오류를 한국어 문장으로 바꿔요 (code는 그대로)
fn wrapped(e: JsValue) -> JsValue {
    report(&e);
    let err = js_sys::Error::new(&explain(&e));
    set(&err, "code", &get(&e, "code"));
    err.into()
}
thread_local! {
    /// 휴대폰 인증: reCAPTCHA(보이지 않음)와 문자를 보낸 뒤의 확인 객체
    static VERIFIER: RefCell<JsValue> = const { RefCell::new(JsValue::UNDEFINED) };
    static CONFIRM: RefCell<JsValue> = const { RefCell::new(JsValue::UNDEFINED) };
    static PHONE: RefCell<String> = const { RefCell::new(String::new()) };
}
/// "010-1234-5678" → "+821012345678" (이미 +로 시작하면 그대로)
fn e164(p: &str) -> Option<String> {
    let d: String = p.chars().filter(|c| c.is_ascii_digit() || *c == '+').collect();
    if let Some(r) = d.strip_prefix('+') {
        return (r.len() >= 8 && r.len() <= 15 && r.chars().all(|c| c.is_ascii_digit())).then(|| format!("+{r}"));
    }
    let r = d.strip_prefix('0')?;
    (r.len() >= 9 && r.len() <= 10 && r.starts_with('1')).then(|| format!("+82{r}"))
}
/// 인증번호 보내기 (가입 화면). 같은 번호로 이미 가입했는지는 확인할 때 알려 줘요
async fn phone_send(phone: String) -> R {
    let Some(num) = e164(&phone) else { return Err(fail("bad-phone", "휴대폰 번호를 확인해 주세요 (예: 010-1234-5678)")) };
    let emu = get(&fbo(), "emu").is_truthy();
    if emu {
        set(&get(&auth(), "settings"), "appVerificationDisabledForTesting", &true.into());
    }
    let v = VERIFIER.with(|x| x.borrow().clone());
    let v = if v.is_undefined() {
        let ctor: js_sys::Function = get(&fbo(), "RecaptchaVerifier").dyn_into()?;
        let nv = js_sys::Reflect::construct(&ctor, &arr(&[auth(), s("phone-captcha"), o(&[("size", s("invisible"))])]).into())?;
        VERIFIER.with(|x| *x.borrow_mut() = nv.clone());
        nv
    } else {
        v
    };
    let conf = aw(f("signInWithPhoneNumber", &[auth(), s(&num), v])).await.map_err(|e| {
        // 실패하면 reCAPTCHA를 새로 만들어요
        VERIFIER.with(|x| *x.borrow_mut() = JsValue::UNDEFINED);
        wrapped(e)
    })?;
    CONFIRM.with(|x| *x.borrow_mut() = conf);
    PHONE.with(|x| *x.borrow_mut() = num.clone());
    Ok(s(&num))
}
/// 아이디·비밀번호로만 가입해요 (초대 링크로 왔으면 초대 기록도 남겨요)
async fn sign_up_plain(id: String, pw: String, name: String, photo: String, ref_: String) -> R {
    let r: R = async {
        let cred = aw(f("createUserWithEmailAndPassword", &[auth(), s(&to_email(&id)), s(&pw)])).await?;
        let user = get(&cred, "user");
        let rest: R = async {
            aw(f("updateProfile", &[user.clone(), o(&[("displayName", s(&name))])])).await?;
            let ph = if photo.encode_utf16().count() <= 40000 { photo.clone() } else { String::new() };
            set_doc(d(&["profiles", &st(&get(&user, "uid"))]), o(&[("id", s(&id.trim().to_lowercase())), ("name", s(&name)), ("photo", s(&ph))])).await?;
            make_wallet_with(&ref_).await
        }
        .await;
        if let Err(e) = rest {
            let _ = aw(f("deleteUser", &[user])).await;
            return Err(e);
        }
        Ok(JsValue::UNDEFINED)
    }
    .await;
    r.map_err(wrapped)
}
#[allow(dead_code)]
/// 시험(에뮬레이터)에서만: 가짜 번호로 문자를 보내고 에뮬레이터에서 인증번호를 읽어요
async fn emu_code() -> Result<String, JsValue> {
    let num = format!("+1555{:07}", (js_sys::Math::random() * 9_999_999.0) as u64);
    phone_send(num.clone()).await?;
    let pid = st(&get(&get(&fbo(), "config"), "projectId"));
    let resp = JsFuture::from(web_sys::window().ok_or("no window")?.fetch_with_str(&format!("http://127.0.0.1:9099/emulator/v1/projects/{pid}/verificationCodes"))).await?;
    let j = JsFuture::from(m(&resp, "json", &[])?.dyn_into::<js_sys::Promise>()?).await?;
    let list = js_sys::Array::from(&get(&j, "verificationCodes"));
    let code = list.iter().filter(|c| st(&get(c, "phoneNumber")) == num).last().map(|c| st(&get(&c, "code"))).unwrap_or_default();
    Ok(code)
}
/// 가입: 휴대폰 인증번호로 먼저 로그인하고(번호 하나에 계정 하나), 그 계정에 아이디·비밀번호를 붙인 뒤 아이디로 다시 로그인해요
async fn sign_up(id: String, pw: String, name: String, photo: String, code: String, ref_: String) -> R {
    // 지금은 휴대폰 인증 없이 가입해요 (중복 가입은 앱이 기기에서 막아요). 인증번호가 오면 휴대폰 인증 방식으로 가입해요
    if code.trim().is_empty() {
        return sign_up_plain(id, pw, name, photo, ref_).await;
    }
    let r: R = async {
        let code = code.trim().to_string();
        let conf = CONFIRM.with(|x| x.borrow().clone());
        if conf.is_undefined() {
            return Err(fail("no-phone", "휴대폰 인증번호를 먼저 받아 주세요"));
        }
        if code.is_empty() {
            return Err(fail("no-code", "문자로 받은 인증번호를 입력해 주세요"));
        }
        let pc = aw(m(&conf, "confirm", &[s(&code)])).await?;
        let user = get(&pc, "user");
        let uid = st(&get(&user, "uid"));
        // 이 번호로 이미 가입한 계정이 있으면 막아요
        let has_pw = js_sys::Array::from(&get(&user, "providerData")).iter().any(|p| st(&get(&p, "providerId")) == "password");
        let has_profile = get_doc(d(&["profiles", &uid])).await.map(|sn| exists(&sn)).unwrap_or(false);
        if has_pw || has_profile {
            let _ = aw(f("signOut", &[auth()])).await;
            return Err(fail("phone-used", "이미 이 휴대폰 번호로 가입한 계정이 있어요. 그 계정으로 로그인해 주세요"));
        }
        let rest: R = async {
            let ep: JsValue = get(&fbo(), "EmailAuthProvider");
            let cred = m(&ep, "credential", &[s(&to_email(&id)), s(&pw)])?;
            aw(f("linkWithCredential", &[user.clone(), cred])).await?;
            aw(f("updateProfile", &[user.clone(), o(&[("displayName", s(&name))])])).await?;
            // 아이디·비밀번호로 다시 로그인해야 서버가 아이디 로그인으로 알아봐요 (휴대폰 번호도 함께 들어 있어요)
            aw(f("signInWithEmailAndPassword", &[auth(), s(&to_email(&id)), s(&pw)])).await?;
            let ph = if photo.encode_utf16().count() <= 40000 { photo.clone() } else { String::new() };
            set_doc(d(&["profiles", &uid]), o(&[("id", s(&id.trim().to_lowercase())), ("name", s(&name)), ("photo", s(&ph))])).await?;
            make_wallet_with(&ref_).await
        }
        .await;
        if let Err(e) = rest {
            // 반쪽짜리 계정이 남지 않게 지워요 (번호도 다시 쓸 수 있게)
            let _ = aw(f("deleteUser", &[get(&auth(), "currentUser")])).await;
            let _ = aw(f("deleteUser", &[user])).await;
            return Err(e);
        }
        CONFIRM.with(|x| *x.borrow_mut() = JsValue::UNDEFINED);
        Ok(JsValue::UNDEFINED)
    }
    .await;
    r.map_err(wrapped)
}
fn ok_name(nm: &str) -> bool {
    let c: Vec<char> = nm.chars().collect();
    let hangul = (2..=6).contains(&c.len()) && c.iter().all(|ch| ('가'..='힣').contains(ch));
    let alpha = |ch: &char| ch.is_ascii_alphabetic();
    let latin = c.len() >= 3 && c.len() <= 40 && alpha(&c[0]) && alpha(&c[c.len() - 1]) && c[1..c.len() - 1].iter().all(|ch| alpha(ch) || *ch == ' ');
    hangul || latin
}
async fn ensure_account() -> R {
    let uid = uid_of()?;
    if with(|x| x.ensured == uid) {
        return Ok(JsValue::UNDEFINED); // 한 번 열 때마다 한 번만 확인해요
    }
    let r = d(&["profiles", &uid]);
    let ex = match get_doc(r.clone()).await {
        Ok(sn) => exists(&sn),
        Err(e) => {
            report(&e);
            false
        }
    };
    if !ex {
        // 옛 방식으로 만든 계정은 프로필이 없어요. 이름이 있으면 쓰고, 없으면 아이디를 이름으로 써요
        let mut name = st(&get(&current(), "displayName"));
        if !ok_name(&name) {
            name = match get_doc(d(&["users", &uid])).await {
                Ok(sn) => {
                    let v = get(&get(&data(&sn), "profile"), "name");
                    if v.is_truthy() {
                        jstr(&v)
                    } else {
                        String::new()
                    }
                }
                Err(_) => String::new(),
            };
        }
        let mid = my_id();
        set_doc(r, o(&[("id", s(&mid)), ("name", s(if ok_name(&name) { &name } else { &mid })), ("photo", s(""))])).await?;
    }
    with(|x| x.ensured = uid);
    Ok(JsValue::UNDEFINED)
}

/* ---------- 사람 ---------- */

async fn fetch_profiles(uids: JsValue) -> R {
    let list: Vec<String> = uids.dyn_into::<js_sys::Array>().map(|a| a.iter().map(|v| jstr(&v)).collect()).unwrap_or_default();
    let mut need: Vec<String> = Vec::new();
    for u in list {
        if need.contains(&u) {
            continue;
        }
        let cc = read_cache(&u);
        if cc.is_null() || now() - num(&get(&cc, "t")) > PROFILE_TTL {
            need.push(u);
        }
    }
    let futs: Vec<_> = need
        .iter()
        .take(30)
        .map(|u| {
            let u = u.clone();
            async move {
                if let Ok(sn) = get_doc(d(&["profiles", &u])).await {
                    if exists(&sn) {
                        write_cache(&u, &data(&sn));
                    }
                }
            }
        })
        .collect();
    join_all(futs).await;
    Ok(n(need.len() as f64))
}
/// 여러 약속을 함께 기다려요 (Promise.all처럼 한꺼번에 보내요)
async fn join_all<F: Future + 'static>(futs: Vec<F>) -> Vec<F::Output>
where
    F::Output: 'static,
{
    let cells: Vec<Rc<RefCell<Option<F::Output>>>> = futs.iter().map(|_| Rc::new(RefCell::new(None))).collect();
    let ps: js_sys::Array = futs
        .into_iter()
        .zip(cells.iter().cloned())
        .map(|(fu, cell)| {
            JsValue::from(future_to_promise(async move {
                *cell.borrow_mut() = Some(fu.await);
                Ok(JsValue::UNDEFINED)
            }))
        })
        .collect();
    let _ = JsFuture::from(js_sys::Promise::all(&ps)).await;
    cells.into_iter().map(|c| c.borrow_mut().take().expect("done")).collect()
}
fn user_row(dd: &JsValue) -> JsValue {
    let x = o(&[("uid", get(dd, "id"))]);
    let v = data(dd);
    if v.is_object() {
        js_sys::Object::assign(x.unchecked_ref::<js_sys::Object>(), v.unchecked_ref::<js_sys::Object>());
    }
    x
}
async fn search_users(t: String) -> R {
    let t = t.trim().to_string();
    if t.is_empty() {
        return Ok(js_sys::Array::new().into());
    }
    let snap = get_docs(q(c(&["profiles"]), vec![w(s("name"), ">=", s(&t)), w(s("name"), "<=", s(&format!("{t}{HI}"))), lim(10.0)])).await?;
    let me_ = uid_of()?;
    let out = js_sys::Array::new();
    for dd in docs(&snap) {
        let id = st(&get(&dd, "id"));
        if id == me_ {
            continue;
        }
        write_cache(&id, &data(&dd));
        out.push(&user_row(&dd));
    }
    Ok(out.into())
}
async fn list_users(reset: bool) -> R {
    if reset {
        with(|x| x.list_cursor = JsValue::NULL);
    }
    let cur = with(|x| x.list_cursor.clone());
    let mut cons = vec![order("name", None)];
    if !cur.is_null() {
        cons.push(f("startAfter", &[cur])?);
    }
    cons.push(lim(20.0));
    let snap = rethrow(get_docs(q(c(&["profiles"]), cons)).await)?;
    let ds = docs(&snap);
    if let Some(last) = ds.last() {
        with(|x| x.list_cursor = last.clone());
    }
    let me_ = uid_of()?;
    let list = js_sys::Array::new();
    for dd in &ds {
        let id = st(&get(dd, "id"));
        if id == me_ {
            continue;
        }
        write_cache(&id, &data(dd));
        list.push(&user_row(dd));
    }
    Ok(o(&[("list", list.into()), ("more", (ds.len() >= 20).into())]))
}
// 아이디 또는 이름(똑같이)으로 찾아요
async fn find_uid(id: &str) -> Result<String, JsValue> {
    let text = id.trim().to_string();
    let mut sn = rethrow(get_docs(q(c(&["profiles"]), vec![w(s("id"), "==", s(&text.to_lowercase())), lim(1.0)])).await)?;
    if docs(&sn).is_empty() {
        sn = rethrow(get_docs(q(c(&["profiles"]), vec![w(s("name"), "==", s(&text)), lim(1.0)])).await)?;
    }
    match docs(&sn).first() {
        Some(x) => Ok(st(&get(x, "id"))),
        None => Err(fail("nouser", "그런 아이디가 없어요")),
    }
}
// 자동 검색: 아이디 앞부분 또는 이름 앞부분으로 최대 8명 (두 번 읽어요)
async fn search_profiles(text: String) -> R {
    let t = text.trim().to_string();
    if t.is_empty() {
        return Ok(js_sys::Array::new().into());
    }
    let lo = t.to_lowercase();
    let a = get_docs(q(c(&["profiles"]), vec![w(s("id"), ">=", s(&lo)), w(s("id"), "<", s(&format!("{lo}{HI}"))), lim(8.0)]));
    let b = get_docs(q(c(&["profiles"]), vec![w(s("name"), ">=", s(&t)), w(s("name"), "<", s(&format!("{t}{HI}"))), lim(8.0)]));
    let rs = join_all(vec![Box::pin(a) as Fut, Box::pin(b) as Fut]).await;
    let mut all = Vec::new();
    for r in rs {
        all.extend(docs(&rethrow(r)?));
    }
    let mut seen: Vec<String> = Vec::new();
    let out = js_sys::Array::new();
    for dd in all {
        let id = st(&get(&dd, "id"));
        if seen.contains(&id) {
            continue;
        }
        seen.push(id.clone());
        let v = data(&dd);
        let pid = get(&v, "id");
        let pname = get(&v, "name");
        out.push(&o(&[
            ("uid", s(&id)),
            ("id", if pid.is_truthy() { pid.clone() } else { s("") }),
            ("name", if pname.is_truthy() { pname } else if pid.is_truthy() { pid } else { s("") }),
        ]));
        if out.length() >= 8 {
            break;
        }
    }
    Ok(out.into())
}
// 인증토큰 3개: 앱이 만들어 서버에 올려요 (5분 뒤에는 쓸 수 없어요)
async fn admin_issue() -> R {
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyz0123456789".chars().collect();
    let crypto = get(&js_sys::global(), "crypto");
    let one = || {
        let r = js_sys::Uint32Array::new_with_length(12);
        let _ = m(&crypto, "getRandomValues", &[r.clone().into()]);
        r.to_vec().iter().map(|x| chars[(*x as usize) % chars.len()]).collect::<String>()
    };
    let tk = arr(&[s(&one()), s(&one()), s(&one())]);
    rethrow(set_doc(d(&["adminAuth", &uid_of()?]), o(&[("tk", tk.clone()), ("exp", n(now() + 290000.0))])).await)?;
    Ok(tk)
}

/* ---------- 랭킹 ---------- */

async fn sync_rank(bal: JsValue) -> R {
    let uid = uid_of()?;
    let b = num(&bal);
    let (sent, at) = with(|x| (x.rank.sent, x.rank.at));
    if sent == Some(b) {
        return Ok(s("same"));
    }
    if now() - at < 21000.0 {
        return Ok(s("soon")); // 방금 썼으면 쉬어요
    }
    let r = d(&["ranks", &uid]);
    if sent.is_none() {
        let cur = get_doc(r.clone()).await?;
        let v = if exists(&cur) { num(&get(&data(&cur), "bal")) } else { -1.0 };
        with(|x| x.rank.sent = Some(v));
        if v == b {
            return Ok(s("same"));
        }
    }
    if !with(|x| x.rank.me.is_truthy()) {
        let p = data(&get_doc(d(&["profiles", &uid])).await?);
        with(|x| x.rank.me = p);
    }
    let meo = with(|x| x.rank.me.clone());
    set_doc(r, o(&[("id", get(&meo, "id")), ("name", get(&meo, "name")), ("bal", bal), ("at", ts())])).await?;
    with(|x| {
        x.rank.sent = Some(b);
        x.rank.at = now();
    });
    Ok(s("sent"))
}
fn rank_row(dd: &JsValue) -> JsValue {
    let v = data(dd);
    o(&[("uid", get(dd, "id")), ("id", get(&v, "id")), ("name", get(&v, "name")), ("bal", get(&v, "bal"))])
}
async fn load_ranks(force: bool) -> R {
    let (top, top_at) = with(|x| (x.rank.top.clone(), x.rank.top_at));
    if !force && top.is_truthy() && now() - top_at < 45000.0 {
        return Ok(top);
    }
    let snap = rethrow(get_docs(q(c(&["ranks"]), vec![order("bal", Some("desc")), lim(50.0)])).await)?;
    let out: js_sys::Array = docs(&snap).iter().map(rank_row).collect();
    with(|x| {
        x.rank.top = out.clone().into();
        x.rank.top_at = now();
    });
    Ok(out.into())
}
async fn count_of(qq: JsValue) -> R {
    let r = aw(f("getCountFromServer", &[qq])).await?;
    Ok(get(&data(&r), "count"))
}

/* ---------- 회사 ---------- */


/* ----- 직원 관리 ----- */

/* ---------- 연구실·공장 ---------- */

fn lab_ref(cid: &str) -> JsValue {
    d(&["companies", cid, "lab", "main"])
}
// 연구실 숫자 한 묶음: 읽고, fun이 새 숫자를 주면 저장해요 (없으면 아무 일도 안 해요)
// 공장·일하기 전에 연구실 문서가 없으면 기본값으로 먼저 만들어요
// 정산 (사장): 지금까지 만든 개수만큼 재료값이 금고에서 나가고 재고가 늘어요. 서버 시각을 쓰니 3초 덜 세요
// 일하기 (직원·사장): 일한 만큼 에너지가 내려가고 회사 식품 성능이 올라요. 배고픔·목마름이 0이어도 일할 수 있어요

/* ---------- 상점·인벤토리 ---------- */

async fn shop_buy(cid: String, pid: String, k: JsValue, co_name: String) -> R {
    rethrow(
        run_tx(move |tx| {
            let (cid, pid, k, co_name) = (cid.clone(), pid.clone(), k.clone(), co_name.clone());
            Box::pin(async move {
                let pref = d(&["companies", &cid, "products", &pid]);
                let lref = lab_ref(&cid);
                let wref = wallet_ref()?;
                let iref = inv_ref(&cid, &pid)?;
                let ps = tx_get(&tx, &pref).await?;
                let ls = tx_get(&tx, &lref).await?;
                let ws = tx_get(&tx, &wref).await?;
                let is = tx_get(&tx, &iref).await?;
                let kn = num(&k);
                if !(is_int(&k) && kn >= 1.0 && kn <= 100.0) {
                    return Err(fail("bad", "수량을 확인해 주세요"));
                }
                let p = if exists(&ps) { data(&ps) } else { JsValue::NULL };
                if !p.is_truthy() || get(&p, "active") != JsValue::TRUE || get(&p, "paused") == JsValue::TRUE || !is_int(&get(&p, "priceCents")) || !exists(&ls) {
                    return Err(fail("unsellable", "지금은 살 수 없는 상품이에요"));
                }
                let stock = if is_int(&get(&p, "stock")) { num(&get(&p, "stock")) } else { 0.0 };
                if stock < kn {
                    return Err(fail("stock", "재고가 부족해요"));
                }
                let tokens = (kn * num(&get(&p, "priceCents")) / 100.0).ceil();
                let bal = num(&get(&data(&ws), "bal"));
                if bal < tokens {
                    return Err(fail("insufficient", "지갑 잔액이 부족해요"));
                }
                let lc = get(&data(&ls), "cash");
                let cash = if is_int(&lc) { num(&lc) } else { 0.0 };
                m(&tx, "update", &[pref, o(&[("stock", n(stock - kn))])])?;
                m(&tx, "update", &[lref, o(&[("cash", n(cash + tokens * 100.0))])])?;
                m(&tx, "update", &[wref, o(&[("bal", n(bal - tokens)), ("wt", ts()), ("op", o(&[("kind", s("shop")), ("cid", s(&cid)), ("pid", s(&pid)), ("n", k.clone()), ("tokens", n(tokens))]))])])?;
                if exists(&is) {
                    let had = get(&data(&is), "n");
                    m(&tx, "update", &[iref, o(&[("n", n(if had.is_truthy() { num(&had) } else { 0.0 } + kn))])])?;
                } else {
                    m(&tx, "set", &[iref, inv_data(&p, &cid, &pid, kn, &co_name)])?;
                }
                Ok(n(tokens))
            })
        })
        .await,
    )
}

/* ---------- 관리·공지 ---------- */

async fn admin_grant(id: String, amt: JsValue) -> R {
    let uid = find_uid(&id).await?;
    let tk = admin_issue().await?;
    let b = batch();
    m(&b, "update", &[d(&["wallets", &uid]), o(&[("bal", inc(num(&amt))), ("wt", ts()), ("op", o(&[("kind", s("grant")), ("amt", amt), ("tk", tk)]))])])?;
    m(&b, "delete", &[d(&["adminAuth", &uid_of()?])])?;
    rethrow(commit(&b).await)?;
    Ok(s(&uid))
}
fn trim_js(v: &JsValue) -> String {
    if v.is_undefined() || v.is_null() {
        String::new()
    } else {
        jstr(v).trim().to_string()
    }
}
fn u16len(t: &str) -> usize {
    t.encode_utf16().count()
}
fn u16take(t: &str, k: usize) -> String {
    String::from_utf16_lossy(&t.encode_utf16().take(k).collect::<Vec<u16>>())
}
async fn admin_notice(arg: JsValue) -> R {
    let t = trim_js(&get(&arg, "title"));
    let text = trim_js(&get(&arg, "body"));
    let ov = get(&arg, "options");
    let opts: Vec<String> = if js_sys::Array::is_array(&ov) {
        ov.unchecked_into::<js_sys::Array>().iter().map(|x| u16take(jstr(&x).trim(), 40)).filter(|x| !x.is_empty()).take(6).collect()
    } else {
        Vec::new()
    };
    if t.is_empty() || u16len(&t) > 40 || text.is_empty() || u16len(&text) > 2000 {
        return Err(fail("invalid", "제목과 내용을 확인해 주세요"));
    }
    if opts.len() == 1 {
        return Err(fail("invalid", "투표 항목은 2개 이상 적어 주세요"));
    }
    let id = st(&get(&new_id(c(&["notices"])), "id"));
    let idx = rethrow(get_doc(d(&["meta", "noticeIndex"])).await)?;
    let old = get(&data(&idx), "ids");
    let ids = js_sys::Array::of1(&s(&id));
    if let Ok(a) = old.dyn_into::<js_sys::Array>() {
        for x in a.iter() {
            ids.push(&x);
        }
    }
    let ids = ids.slice(0, 100);
    let tk = admin_issue().await?;
    let b = batch();
    let note = o(&[("title", s(&t)), ("body", s(&text)), ("by", s(&uid_of()?)), ("createdAt", ts()), ("tk", tk)]);
    if opts.len() >= 2 {
        set(&note, "options", &opts.iter().map(|x| s(x)).collect::<js_sys::Array>());
    }
    m(&b, "set", &[d(&["notices", &id]), note])?;
    m(&b, "set", &[d(&["meta", "noticeIndex"]), o(&[("ids", ids.into())])])?;
    m(&b, "delete", &[d(&["adminAuth", &uid_of()?])])?;
    rethrow(commit(&b).await)?;
    Ok(s(&id))
}
async fn notice_ids() -> R {
    let sn = rethrow(get_doc(d(&["meta", "noticeIndex"])).await)?;
    let ids = get(&data(&sn), "ids");
    Ok(if js_sys::Array::is_array(&ids) { ids } else { js_sys::Array::new().into() })
}
async fn notice_list(ids: JsValue) -> R {
    let a: js_sys::Array = ids.dyn_into().unwrap_or_default();
    let futs: Vec<_> = a
        .slice(0, 40)
        .iter()
        .map(|id| {
            let id = jstr(&id);
            async move {
                match get_doc(d(&["notices", &id])).await {
                    Ok(sn) if exists(&sn) => Some(notice_of(&sn)),
                    _ => None,
                }
            }
        })
        .collect();
    Ok(join_all(futs).await.into_iter().flatten().collect::<js_sys::Array>().into())
}
async fn mark_notice_seen(id: String) -> R {
    let uid = uid_of()?;
    let seen = js_sys::Object::new();
    set(&seen, &id, &true.into());
    rethrow(set_merge(d(&["noticeReads", &uid]), o(&[("seen", seen.into())])).await)?;
    remember_seen(&uid, &[id]);
    Ok(JsValue::UNDEFINED)
}
// 아직 안 본 공지 중 가장 오래된 것 하나 (없으면 null)
async fn unseen_notice() -> R {
    let uid = uid_of()?;
    let ids: Vec<String> = notice_ids().await?.unchecked_into::<js_sys::Array>().iter().map(|x| jstr(&x)).collect();
    let here = seen_here(&uid);
    if ids.is_empty() || ids.iter().all(|x| here.contains(x)) {
        return Ok(JsValue::NULL);
    }
    let reads = get_doc(d(&["noticeReads", &uid])).await.ok();
    let seen = reads.as_ref().map(|r| get(&data(r), "seen")).filter(|v| !v.is_undefined() && !v.is_null()).unwrap_or_else(|| js_sys::Object::new().into());
    let keys: Vec<String> = js_sys::Object::keys(seen.unchecked_ref::<js_sys::Object>()).iter().map(|x| jstr(&x)).filter(|k| get(&seen, k) == JsValue::TRUE).collect();
    remember_seen(&uid, &keys);
    let created = st(&get(&get(&current(), "metadata"), "creationTime"));
    let since = js_sys::Date::parse(&created);
    let since = if since.is_nan() { 0.0 } else { since };
    for id in ids.iter().rev() {
        if get(&seen, id) == JsValue::TRUE || here.contains(id) {
            continue;
        }
        let Ok(sn) = get_doc(d(&["notices", id])).await else { continue };
        if !exists(&sn) {
            continue;
        }
        let nt = notice_of(&sn);
        let at = num(&get(&nt, "at"));
        if st(&get(&nt, "by")) == uid || (at != 0.0 && at < since) {
            let _ = mark_notice_seen(id.clone()).await;
            continue;
        }
        return Ok(nt);
    }
    Ok(JsValue::NULL)
}
async fn notice_tally(nt: JsValue) -> R {
    let id = jstr(&get(&nt, "id"));
    let opts: js_sys::Array = get(&nt, "options").dyn_into().unwrap_or_default();
    let futs: Vec<_> = (0..opts.length())
        .map(|i| {
            let id = id.clone();
            async move { count_of(q(c(&["notices", &id, "votes"]), vec![w(s("choice"), "==", n(i as f64))])).await }
        })
        .collect();
    let out = js_sys::Array::new();
    for r in join_all(futs).await {
        out.push(&r?);
    }
    Ok(out.into())
}
async fn admin_delete(id: String) -> R {
    let uid = find_uid(&id).await?;
    let tk = admin_issue().await?;
    let b = batch();
    m(&b, "set", &[new_id(c(&["adminLog"])), o(&[("tk", tk), ("kind", s("delete")), ("target", s(&uid)), ("at", ts())])])?;
    for col in ["profiles", "users", "wallets", "vitals"] {
        m(&b, "delete", &[d(&[col, &uid])])?;
    }
    m(&b, "delete", &[d(&["adminAuth", &uid_of()?])])?;
    rethrow(commit(&b).await)?;
    Ok(s(&uid))
}

/* ---------- 상태·사진·지갑 ---------- */

async fn set_name(name: String) -> R {
    let uid = uid_of()?;
    update_doc(d(&["profiles", &uid]), o(&[("name", s(&name))])).await?;
    let cc = read_cache(&uid);
    let base = get(&cc, "d");
    if base.is_truthy() {
        write_cache(&uid, &ext(&base, &[("name", s(&name))]));
    }
    // 랭킹에도 새 이름이 올라가게 다음 순위 갱신 때 다시 써요
    with(|x| {
        x.rank.sent = Some(-1.0);
        x.rank.me = JsValue::NULL;
        x.rank.at = 0.0;
        x.rank.top_at = 0.0;
    });
    Ok(JsValue::UNDEFINED)
}
async fn set_blue(uid: String, on: bool) -> R {
    update_doc(d(&["profiles", &uid]), o(&[("blue", on.into())])).await?;
    let cc = read_cache(&uid);
    let base = get(&cc, "d");
    if base.is_truthy() {
        write_cache(&uid, &ext(&base, &[("blue", on.into())]));
    }
    Ok(JsValue::UNDEFINED)
}
async fn set_photo(photo: JsValue) -> R {
    let uid = uid_of()?;
    update_doc(d(&["profiles", &uid]), o(&[("photo", photo.clone())])).await?;
    let cc = read_cache(&uid);
    let base = get(&cc, "d");
    let base = if base.is_truthy() {
        base
    } else {
        let dn = get(&current(), "displayName");
        o(&[("id", s(&my_id())), ("name", if dn.is_truthy() { dn } else { s("") })])
    };
    write_cache(&uid, &ext(&base, &[("photo", photo)]));
    Ok(JsValue::UNDEFINED)
}
fn watch_wallet(cb: js_sys::Function) -> R {
    let cb2 = cb.clone();
    let next = Closure::<dyn FnMut(JsValue)>::new(move |sn: JsValue| {
        if !exists(&sn) {
            wasm_bindgen_futures::spawn_local(async {
                let _ = make_wallet().await;
            });
            return;
        }
        let dd = data_est(&sn);
        let _ = cb.call1(&JsValue::NULL, &o(&[("bal", get(&dd, "bal")), ("rate", get(&dd, "rate")), ("t", n(ms(&get(&dd, "t")))), ("wt", n(ms(&get(&dd, "wt"))))]));
    })
    .into_js_value();
    let err = Closure::<dyn FnMut(JsValue)>::new(move |e: JsValue| {
        report(&e);
        let _ = cb2.call1(&JsValue::NULL, &JsValue::NULL);
    })
    .into_js_value();
    f("onSnapshot", &[wallet_ref()?, next, err])
}
fn report_fn() -> JsValue {
    Closure::<dyn FnMut(JsValue)>::new(|e: JsValue| report(&e)).into_js_value()
}

/* ---------- 대화 ---------- */

async fn open_chat(other: String) -> R {
    let uid = uid_of()?;
    let mem = sorted(&uid, &other);
    let cid = mem.join("_");
    let r = set_doc(
        d(&["chats", &cid]),
        o(&[("members", arr(&[s(&mem[0]), s(&mem[1])])), ("last", s("")), ("lastFrom", s("")), ("lastAt", from_millis(0.0)), ("reads", js_sys::Object::new().into())]),
    )
    .await;
    if r.is_err() {
        // 이미 있는 대화면 만들기가 막혀요. 읽을 수 있으면 있는 거예요
        get_doc(d(&["chats", &cid])).await?;
    }
    Ok(s(&cid))
}
fn uniq(list: Vec<String>, k: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for x in list {
        if !out.contains(&x) {
            out.push(x);
        }
    }
    out.truncate(k);
    out
}
fn strs(v: &JsValue) -> Vec<String> {
    v.clone().dyn_into::<js_sys::Array>().map(|a| a.iter().map(|x| jstr(&x)).collect()).unwrap_or_default()
}
async fn create_group(name: JsValue, uids: JsValue) -> R {
    let uid = uid_of()?;
    let mut all = vec![uid.clone()];
    all.extend(strs(&uids));
    let members = uniq(all, 20);
    if members.len() < 2 {
        return Err(fail("bad", "두 명 이상 골라 주세요"));
    }
    let cid = format!("g_{}", st(&get(&new_id(c(&["chats"])), "id")));
    let nm = u16take(jstr(&name).trim(), 20);
    rethrow(
        set_doc(
            d(&["chats", &cid]),
            o(&[
                ("members", members.iter().map(|x| s(x)).collect::<js_sys::Array>().into()),
                ("group", true.into()),
                ("name", s(&nm)),
                ("owner", s(&uid)),
                ("last", s("")),
                ("lastFrom", s("")),
                ("lastAt", from_millis(0.0)),
                ("reads", js_sys::Object::new().into()),
            ]),
        )
        .await,
    )?;
    Ok(s(&cid))
}
async fn invite_members(cid: String, uids: JsValue) -> R {
    let r = d(&["chats", &cid]);
    let sn = rethrow(get_doc(r.clone()).await)?;
    let old = strs(&get(&data(&sn), "members"));
    let mut all = old.clone();
    all.extend(strs(&uids));
    let members = uniq(all, 20);
    let a: JsValue = members.iter().map(|x| s(x)).collect::<js_sys::Array>().into();
    if members.len() == old.len() {
        return Ok(a);
    }
    rethrow(update_doc(r, o(&[("members", a.clone())])).await)?;
    Ok(a)
}
async fn name_of(uid: String) -> R {
    if let Some(v) = with(|x| x.names.get(&uid).cloned()) {
        return Ok(s(&v));
    }
    let nm = match get_doc(d(&["profiles", &uid])).await {
        Ok(sn) if exists(&sn) => {
            let v = data(&sn);
            let x = get(&v, "name");
            let y = get(&v, "id");
            if x.is_truthy() {
                jstr(&x)
            } else if y.is_truthy() {
                jstr(&y)
            } else {
                String::new()
            }
        }
        _ => String::new(),
    };
    with(|x| x.names.insert(uid, nm.clone()));
    Ok(s(&nm))
}
fn watch_chats(cb: js_sys::Function) -> R {
    let next = Closure::<dyn FnMut(JsValue)>::new(move |snap: JsValue| {
        let list: js_sys::Array = docs(&snap).iter().map(|dd| chat_part(&get(dd, "id"), &data_est(dd))).collect();
        let _ = cb.call1(&JsValue::NULL, &list);
    })
    .into_js_value();
    f("onSnapshot", &[q(c(&["chats"]), vec![w(s("members"), "array-contains", s(&uid_of()?)), lim(50.0)]), next, report_fn()])
}
fn watch_messages(cid: String, cb: js_sys::Function, k: JsValue) -> R {
    let k = if k.is_undefined() { 40.0 } else { num(&k) };
    let next = Closure::<dyn FnMut(JsValue)>::new(move |snap: JsValue| {
        let ds = docs(&snap);
        let list: js_sys::Array = ds.iter().rev().map(msg_part).collect();
        let _ = cb.call2(&JsValue::NULL, &list, &((ds.len() as f64) >= k).into());
    })
    .into_js_value();
    f("onSnapshot", &[q(c(&["chats", &cid, "messages"]), vec![order("at", Some("desc")), lim(k)]), next, report_fn()])
}
async fn load_older(cid: String, before: JsValue, k: JsValue) -> R {
    let k = if k.is_undefined() { 30.0 } else { num(&k) };
    let snap = get_docs(q(c(&["chats", &cid, "messages"]), vec![order("at", Some("desc")), f("startAfter", &[from_millis(num(&before))])?, lim(k)])).await?;
    let ds = docs(&snap);
    Ok(o(&[("list", ds.iter().rev().map(msg_part).collect::<js_sys::Array>().into()), ("more", ((ds.len() as f64) >= k).into())]))
}
// 글·사진·그림·요청·투표: 메시지와 대화 목록 갱신을 한 묶음으로 써요
async fn send(cid: String, to: JsValue, payload: JsValue, preview: JsValue, gift: Option<f64>) -> R {
    let uid = uid_of()?;
    let mref = new_id(c(&["chats", &cid, "messages"]));
    let b = batch();
    m(&b, "set", &[mref.clone(), ext(&payload, &[("from", s(&uid)), ("to", to), ("at", ts())])])?;
    m(&b, "update", &[d(&["chats", &cid]), o(&[("last", s(&u16take(&jstr(&preview), 200))), ("lastAt", ts()), ("lastFrom", s(&uid))])])?;
    if let Some(amount) = gift {
        m(&b, "update", &[wallet_ref()?, o(&[("bal", inc(-amount)), ("wt", ts()), ("op", o(&[("kind", s("gift")), ("cid", s(&cid)), ("mid", get(&mref, "id"))]))])])?;
    }
    commit(&b).await?;
    Ok(get(&mref, "id"))
}
async fn claim_gift(cid: String, mid: String, amount: JsValue) -> R {
    let b = batch();
    m(&b, "update", &[d(&["chats", &cid, "messages", &mid]), o(&[("done", true.into())])])?;
    m(&b, "update", &[wallet_ref()?, o(&[("bal", inc(num(&amount))), ("wt", ts()), ("op", o(&[("kind", s("claim")), ("cid", s(&cid)), ("mid", s(&mid))]))])])?;
    commit(&b).await?;
    Ok(JsValue::UNDEFINED)
}
/* ---------- 접속 중 표시 ---------- */
async fn presence_beat() -> R {
    set_doc(d(&["presence", &uid_of()?]), o(&[("at", ts())])).await?;
    Ok(JsValue::UNDEFINED)
}
/// 최근 3분 안에 들어온 사람들: [uid, 시각(ms)] 목록
async fn presence_list() -> R {
    let snap = get_docs(q(c(&["presence"]), vec![w(s("at"), ">=", from_millis(now() - 180_000.0)), lim(200.0)])).await?;
    let out = js_sys::Array::new();
    for dd in docs(&snap) {
        out.push(&o(&[("uid", get(&dd, "id")), ("at", n(ms(&get(&data(&dd), "at"))))]));
    }
    Ok(out.into())
}

/* ---------- 공모전 ---------- */
async fn contest_list() -> R {
    let snap = rethrow(get_docs(q(c(&["contests"]), vec![order("at", Some("desc")), lim(20.0)])).await)?;
    Ok(docs(&snap)
        .iter()
        .map(|dd| {
            let x = data(dd);
            o(&[("id", get(dd, "id")), ("title", get(&x, "title")), ("open", (get(&x, "open") == JsValue::TRUE).into()), ("n", get(&x, "n")), ("at", n(ms(&get(&x, "at"))))])
        })
        .collect::<js_sys::Array>()
        .into())
}
/// 사진들 (표 많은 순, 100개)
async fn contest_entries(cid: String) -> R {
    let snap = rethrow(get_docs(q(c(&["contests", &cid, "entries"]), vec![order("votes", Some("desc")), lim(100.0)])).await)?;
    Ok(docs(&snap)
        .iter()
        .map(|dd| {
            let x = data(dd);
            o(&[("by", get(dd, "id")), ("img", get(&x, "img")), ("votes", get(&x, "votes")), ("at", n(ms(&get(&x, "at"))))])
        })
        .collect::<js_sys::Array>()
        .into())
}
/// 사진 올리기: 내 자리(내 uid)에 하나, 참가 수 +1을 같은 묶음으로
async fn contest_enter(cid: String, img_: String) -> R {
    let uid = uid_of()?;
    let b = batch();
    m(&b, "set", &[d(&["contests", &cid, "entries", &uid]), o(&[("by", s(&uid)), ("img", s(&img_)), ("at", ts()), ("votes", n(0.0))])])?;
    m(&b, "update", &[d(&["contests", &cid]), o(&[("n", inc(1.0))])])?;
    rethrow(commit(&b).await)?;
    Ok(JsValue::UNDEFINED)
}
/// 투표·바꾸기·취소 (old: 지금 내 표, 같은 사진을 다시 누르면 취소)
async fn contest_vote(cid: String, entry: String, old: String) -> R {
    let me_ = uid_of()?;
    let vr = d(&["contests", &cid, "votes", &me_]);
    let b = batch();
    if old == entry {
        m(&b, "delete", &[vr])?;
        m(&b, "update", &[d(&["contests", &cid, "entries", &entry]), o(&[("votes", inc(-1.0))])])?;
    } else {
        m(&b, "set", &[vr, o(&[("entry", s(&entry)), ("at", ts())])])?;
        m(&b, "update", &[d(&["contests", &cid, "entries", &entry]), o(&[("votes", inc(1.0))])])?;
        if !old.is_empty() {
            m(&b, "update", &[d(&["contests", &cid, "entries", &old]), o(&[("votes", inc(-1.0))])])?;
        }
    }
    rethrow(commit(&b).await)?;
    Ok(JsValue::UNDEFINED)
}

/* ---------- 상점의 장신구 ---------- */
/// 내 가방: 상점에서 산 장신구들
async fn deco_bag() -> R {
    let sn = rethrow(get_doc(d(&["bag", &uid_of()?])).await)?;
    Ok(if exists(&sn) { get(&data(&sn), "items") } else { js_sys::Array::new().into() })
}
/// 장신구 사기: 지갑에서 값이 빠지고 같은 묶음에서 가방에 붙어요 (items: 지금 가방)
async fn deco_buy(k: String, price: f64, items: JsValue) -> R {
    let uid = uid_of()?;
    let all: js_sys::Array = items.dyn_into::<js_sys::Array>().unwrap_or_default();
    all.push(&s(&k));
    let b = batch();
    m(&b, "set", &[d(&["bag", &uid]), o(&[("items", all.into())])])?;
    m(&b, "update", &[wallet_ref()?, o(&[("bal", inc(-price)), ("wt", ts()), ("op", o(&[("kind", s("deco")), ("k", s(&k))]))])])?;
    rethrow(commit(&b).await)?;
    Ok(JsValue::UNDEFINED)
}
/// 전화번호 인증 서버(Zavu 함수)에 보낼 로그인 토큰
async fn id_token() -> R {
    let u = current();
    if u.is_null() || u.is_undefined() {
        return Err(fail("not-signed-in", "로그인이 필요해요"));
    }
    aw(m(&u, "getIdToken", &[])).await
}
/// 이 계정이 전화번호 인증을 끝냈나 (verified/{uid} 문서가 있나)
async fn phone_verified() -> R {
    let sn = rethrow(get_doc(d(&["verified", &uid_of()?])).await)?;
    Ok(exists(&sn).into())
}
/// 도박장(슬롯): 건 돈(bet)만큼 빠졌다가 배율만큼 돌아와요. 지갑은 net(= 돌려받는 돈 - 건 돈)만큼 바뀌어요.
/// 서버는 이 모양만 받아요 (100의 배수, 잃으면 -건 돈, 1.5배면 +절반, 10배면 +9배: firestore.rules의 'bet')
async fn casino_bet(bet: f64, net: f64) -> R {
    let b = batch();
    m(&b, "update", &[wallet_ref()?, o(&[("bal", inc(net)), ("wt", ts()), ("op", o(&[("kind", s("bet")), ("amt", bet.into())]))])])?;
    rethrow(commit(&b).await)?;
    Ok(JsValue::UNDEFINED)
}
/// 장신구 끼기: 낀 장신구 번호 목록 그대로 (gear, 빈 목록이면 모두 빼기). 내 프로필 캐시도 바로 고쳐서 내 화면에 바로 보여요
async fn deco_set(list: JsValue) -> R {
    let uid = uid_of()?;
    rethrow(update_doc(d(&["profiles", &uid]), o(&[("gear", list.clone())])).await)?;
    let cc = read_cache(&uid);
    let dd = get(&cc, "d");
    if dd.is_object() {
        set(&dd, "gear", &list);
        write_cache(&uid, &dd);
    }
    Ok(JsValue::UNDEFINED)
}

/* ---------- 릴스 선물 ---------- */
/// 글쓴이에게 멸공을 보내요: 선물 문서와 내 지갑을 같은 묶음으로 (규칙이 금액을 맞춰 봐요)
async fn reel_gift(post: String, to: String, amount: f64) -> R {
    let uid = uid_of()?;
    let g = new_id(c(&["postGifts"]));
    let b = batch();
    m(&b, "set", &[g.clone(), o(&[("from", s(&uid)), ("to", s(&to)), ("post", s(&post)), ("amount", n(amount)), ("done", false.into()), ("at", ts())])])?;
    m(&b, "update", &[wallet_ref()?, o(&[("bal", inc(-amount)), ("wt", ts()), ("op", o(&[("kind", s("rgift")), ("gid", get(&g, "id"))]))])])?;
    rethrow(commit(&b).await)?;
    Ok(get(&g, "id"))
}
/// 나에게 온 릴스 선물 (최근 30개, 새것부터)
async fn reel_gifts() -> R {
    let snap = rethrow(get_docs(q(c(&["postGifts"]), vec![w(s("to"), "==", s(&uid_of()?)), lim(30.0)])).await)?;
    let mut v: Vec<JsValue> = docs(&snap)
        .iter()
        .map(|dd| {
            let x = data(dd);
            o(&[("id", get(dd, "id")), ("from", get(&x, "from")), ("post", get(&x, "post")), ("amount", get(&x, "amount")), ("done", (get(&x, "done") == JsValue::TRUE).into()), ("at", n(ms(&get(&x, "at"))))])
        })
        .collect();
    v.sort_by(|a, b| num(&get(b, "at")).partial_cmp(&num(&get(a, "at"))).unwrap_or(std::cmp::Ordering::Equal));
    Ok(v.into_iter().collect::<js_sys::Array>().into())
}
async fn reel_gift_claim(gid: String) -> R {
    let gr = d(&["postGifts", &gid]);
    let g = rethrow(get_doc(gr.clone()).await)?;
    let amount = num(&get(&data(&g), "amount"));
    let b = batch();
    m(&b, "update", &[gr, o(&[("done", true.into())])])?;
    m(&b, "update", &[wallet_ref()?, o(&[("bal", inc(amount)), ("wt", ts()), ("op", o(&[("kind", s("rclaim")), ("gid", s(&gid))]))])])?;
    rethrow(commit(&b).await)?;
    Ok(n(amount))
}

/* ---------- 게임 (독소전쟁 대항전) ---------- */
/// 전체 점수 한 문서만 들어요 (r: 소련, b: 독일)
fn game_watch(cb: js_sys::Function) -> R {
    let next = Closure::<dyn FnMut(JsValue)>::new(move |sn: JsValue| {
        let v = if exists(&sn) { data(&sn) } else { o(&[("r", n(0.0)), ("b", n(0.0))]) };
        let _ = cb.call1(&JsValue::NULL, &v);
    })
    .into_js_value();
    f("onSnapshot", &[d(&["game", "main"]), next, report_fn()])
}
async fn game_me() -> R {
    let sn = get_doc(d(&["gamePlayers", &uid_of()?])).await?;
    Ok(if exists(&sn) { data(&sn) } else { JsValue::NULL })
}
async fn game_join(team: String, name: String) -> R {
    let g = get_doc(d(&["game", "main"])).await?;
    if !exists(&g) {
        let _ = set_doc(d(&["game", "main"]), o(&[("r", n(0.0)), ("b", n(0.0))])).await;
    }
    rethrow(set_doc(d(&["gamePlayers", &uid_of()?]), o(&[("team", s(&team)), ("name", s(&name)), ("clicks", n(0.0)), ("at", ts())])).await)?;
    Ok(JsValue::UNDEFINED)
}
/// 모아 둔 클릭을 한 번에 올려요 (내 점수와 팀 점수가 같은 묶음에서 같은 만큼)
async fn game_flush(team: String, k: f64) -> R {
    let b = batch();
    m(&b, "update", &[d(&["gamePlayers", &uid_of()?]), o(&[("clicks", inc(k)), ("at", ts())])])?;
    m(&b, "update", &[d(&["game", "main"]), o(&[(team.as_str(), inc(k))])])?;
    rethrow(commit(&b).await)?;
    Ok(JsValue::UNDEFINED)
}
async fn game_top() -> R {
    let snap = rethrow(get_docs(q(c(&["gamePlayers"]), vec![order("clicks", Some("desc")), lim(20.0)])).await)?;
    Ok(docs(&snap).iter().map(|dd| { let v = data(dd); o(&[("uid", get(dd, "id")), ("team", get(&v, "team")), ("name", get(&v, "name")), ("clicks", get(&v, "clicks"))]) }).collect::<js_sys::Array>().into())
}

/* ---------- 거래 (물물교환·사고팔기) ---------- */
fn trade_row(dd: &JsValue) -> JsValue {
    let v = data(dd);
    o(&[("id", get(dd, "id")), ("by", get(&v, "by")), ("title", get(&v, "title")), ("want", get(&v, "want")), ("price", get(&v, "price")), ("text", get(&v, "text")), ("img", get(&v, "img")), ("done", get(&v, "done")), ("at", n(ms(&get(&v, "at"))))])
}
/// 최근 거래 글 40개 (서버를 아끼려고 탭을 열 때 한 번 읽어요)
async fn trades_list() -> R {
    let snap = rethrow(get_docs(q(c(&["trades"]), vec![order("at", Some("desc")), lim(40.0)])).await)?;
    Ok(docs(&snap).iter().map(trade_row).collect::<js_sys::Array>().into())
}
async fn trade_create(v: JsValue) -> R {
    let r = new_id(c(&["trades"]));
    rethrow(set_doc(r.clone(), o(&[("by", s(&uid_of()?)), ("title", get(&v, "title")), ("want", get(&v, "want")), ("price", n(num(&get(&v, "price")).floor().max(0.0))), ("text", get(&v, "text")), ("img", get(&v, "img")), ("done", false.into()), ("at", ts())])).await)?;
    Ok(get(&r, "id"))
}
async fn trade_done(id: String, on: bool) -> R {
    rethrow(update_doc(d(&["trades", &id]), o(&[("done", on.into())])).await)?;
    Ok(JsValue::UNDEFINED)
}
async fn trade_delete(id: String) -> R {
    let b = batch();
    m(&b, "delete", &[d(&["trades", &id])])?;
    rethrow(commit(&b).await)?;
    Ok(JsValue::UNDEFINED)
}

/* ---------- 놀기 (약속) ---------- */
fn plan_row(dd: &JsValue) -> JsValue {
    let v = data(dd);
    o(&[("id", get(dd, "id")), ("by", get(&v, "by")), ("title", get(&v, "title")), ("place", get(&v, "place")), ("when", get(&v, "when")), ("text", get(&v, "text")), ("vis", get(&v, "vis")), ("invited", get(&v, "invited")), ("going", get(&v, "going")), ("at", n(ms(&get(&v, "at"))))])
}
/// 모두에게 보이는 약속 + 나를 부른 약속 (각 60개까지). 순서는 화면에서 정해요
async fn plans_list() -> R {
    let me_ = uid_of()?;
    let a = get_docs(q(c(&["plans"]), vec![w(s("vis"), "==", s("all")), lim(60.0)])).await?;
    let b = get_docs(q(c(&["plans"]), vec![w(s("invited"), "array-contains", s(&me_)), lim(60.0)])).await?;
    let out = js_sys::Array::new();
    let mut seen: Vec<String> = Vec::new();
    for dd in docs(&a).into_iter().chain(docs(&b)) {
        let id = st(&get(&dd, "id"));
        if !seen.contains(&id) {
            seen.push(id);
            out.push(&plan_row(&dd));
        }
    }
    Ok(out.into())
}
async fn plan_create(v: JsValue) -> R {
    let me_ = uid_of()?;
    let r = new_id(c(&["plans"]));
    let inv: js_sys::Array = get(&v, "invited").dyn_into().unwrap_or_default();
    let vis = if inv.length() > 0 { "pick" } else { "all" };
    if vis == "pick" && !inv.includes(&s(&me_), 0) {
        inv.push(&s(&me_));
    }
    rethrow(set_doc(r.clone(), o(&[("by", s(&me_)), ("title", get(&v, "title")), ("place", get(&v, "place")), ("when", n(num(&get(&v, "when")).floor())), ("text", get(&v, "text")), ("vis", s(vis)), ("invited", inv.into()), ("going", js_sys::Array::of1(&s(&me_)).into()), ("at", ts())])).await)?;
    Ok(get(&r, "id"))
}
/// 참여(on=true)·취소: 지금 명단을 읽고 나만 넣거나 빼요 (한 묶음 안에서)
async fn plan_join(id: String, on: bool) -> R {
    let me_ = uid_of()?;
    rethrow(
        run_tx(move |tx| {
            let (id, me_) = (id.clone(), me_.clone());
            Box::pin(async move {
                let r = d(&["plans", &id]);
                let sn = tx_get(&tx, &r).await?;
                if !exists(&sn) {
                    return Err(fail("gone", "없어진 약속이에요"));
                }
                let going: js_sys::Array = get(&data(&sn), "going").dyn_into().unwrap_or_default();
                let list: js_sys::Array = going.iter().filter(|u| st(u) != me_).collect();
                if on {
                    list.push(&s(&me_));
                }
                m(&tx, "update", &[r, o(&[("going", list.clone().into())])])?;
                Ok(list.into())
            })
        })
        .await,
    )
}
async fn plan_delete(id: String) -> R {
    let b = batch();
    m(&b, "delete", &[d(&["plans", &id])])?;
    rethrow(commit(&b).await)?;
    Ok(JsValue::UNDEFINED)
}

/* ---------- 땅 (현실 지도의 육각형 칸) ---------- */
async fn cancel_gift(cid: String, mid: String) -> R {
    let mr = d(&["chats", &cid, "messages", &mid]);
    let ms = get_doc(mr.clone()).await?;
    let amount = num(&get(&data(&ms), "amount"));
    let b = batch();
    m(&b, "update", &[mr, o(&[("done", true.into()), ("cancel", true.into())])])?;
    m(&b, "update", &[wallet_ref()?, o(&[("bal", inc(amount)), ("wt", ts()), ("op", o(&[("kind", s("ungift")), ("cid", s(&cid)), ("mid", s(&mid))]))])])?;
    commit(&b).await?;
    Ok(JsValue::UNDEFINED)
}
async fn pay_request(cid: String, to: JsValue, req: String, amount: JsValue) -> R {
    let uid = uid_of()?;
    let a = num(&amount);
    let gref = new_id(c(&["chats", &cid, "messages"]));
    let b = batch();
    m(&b, "update", &[d(&["chats", &cid, "messages", &req]), o(&[("done", true.into())])])?;
    m(&b, "set", &[gref.clone(), o(&[("type", s("gift")), ("amount", amount.clone()), ("done", false.into()), ("from", s(&uid)), ("to", to), ("at", ts())])])?;
    m(&b, "update", &[d(&["chats", &cid]), o(&[("last", s(&format!("멸공 {} 선물", ko(a)))), ("lastAt", ts()), ("lastFrom", s(&uid))])])?;
    m(&b, "update", &[wallet_ref()?, o(&[("bal", inc(-a)), ("wt", ts()), ("op", o(&[("kind", s("gift")), ("cid", s(&cid)), ("mid", get(&gref, "id"))]))])])?;
    commit(&b).await?;
    Ok(JsValue::UNDEFINED)
}

/* ---------- 통화 (연결 정보만 주고받아요) ---------- */

// 나에게 걸려 오는 전화 (울리는 것만)

/* ---------- SNS (무제 탭) ---------- */

fn post_row(dd: &JsValue) -> JsValue {
    let v = data_est(dd);
    let count = |k: &str| {
        let m_ = get(&v, k);
        if m_.is_object() { js_sys::Object::keys(m_.unchecked_ref::<js_sys::Object>()).length() as f64 } else { 0.0 }
    };
    let me_ = st(&get(&current(), "uid"));
    let mine = |k: &str| get(&get(&v, k), &me_) == JsValue::TRUE;
    o(&[
        ("id", get(dd, "id")),
        ("by", get(&v, "by")),
        ("text", get(&v, "text")),
        ("img", nn(get(&v, "img"))),
        ("gif", nn(get(&v, "gif"))),
        ("at", n(ms(&get(&v, "at")))),
        ("parent", get(&v, "parent")),
        ("likes", n(count("likes"))),
        ("reposts", n(count("reposts"))),
        ("rc", { let r = get(&v, "rc"); if is_int(&r) { r } else { n(0.0) } }),
        ("topic", { let t = get(&v, "topic"); if t.is_string() { t } else { s("") } }),
        ("first", (get(&v, "first") == JsValue::TRUE).into()),
        ("sh", { let x = get(&v, "sh"); if is_int(&x) { x } else { n(0.0) } }),
        ("last", { let x = get(&v, "last"); if x.is_string() { x } else { s("") } }),
        ("liked", mine("likes").into()),
        ("reposted", mine("reposts").into()),
    ])
}
/// 새 글 (parent가 있으면 그 글의 댓글이에요)
async fn post_create(text: String, img_: String, parent: String, topic: String, gif: String) -> R {
    // 처음 쓰는 글이면 "첫 스레드" 표시를 붙여요
    let me_ = uid_of()?;
    let mine = get_docs(q(c(&["posts"]), vec![w(s("by"), "==", s(&me_)), lim(1.0)])).await.map(|sn| docs(&sn).len()).unwrap_or(1);
    let r = new_id(c(&["posts"]));
    let id = get(&r, "id");
    let data_ = o(&[("by", s(&uid_of()?)), ("text", s(&text)), ("at", ts()), ("likes", js_sys::Object::new().into()), ("reposts", js_sys::Object::new().into()), ("rc", n(0.0)), ("parent", s(&parent))]);
    if !img_.is_empty() {
        set(&data_, "img", &s(&img_));
    }
    if !gif.is_empty() {
        set(&data_, "gif", &s(&gif));
    }
    let topic: String = topic.trim().chars().filter(|c| *c != '<' && *c != '>').take(20).collect();
    if !topic.is_empty() && parent.is_empty() {
        set(&data_, "topic", &s(&topic));
    }
    if mine == 0 && parent.is_empty() {
        set(&data_, "first", &true.into());
    }
    let b = batch();
    m(&b, "set", &[r, data_])?;
    if !parent.is_empty() {
        m(&b, "update", &[d(&["posts", &parent]), o(&[("rc", inc(1.0)), ("last", id.clone())])])?;
    }
    rethrow(commit(&b).await)?;
    Ok(id)
}
/// 홈 피드: 최근 40개만 들어요 (댓글은 빼고 보여줘요)
fn posts_watch(cb: js_sys::Function) -> R {
    let next = Closure::<dyn FnMut(JsValue)>::new(move |snap: JsValue| {
        let list: js_sys::Array = docs(&snap).iter().map(post_row).filter(|p| st(&get(p, "parent")).is_empty()).collect();
        let _ = cb.call1(&JsValue::NULL, &list);
    })
    .into_js_value();
    f("onSnapshot", &[q(c(&["posts"]), vec![order("at", Some("desc")), lim(40.0)]), next, report_fn()])
}
/// 한 글의 댓글 (오래된 것부터)
async fn post_replies(id: String) -> R {
    let snap = rethrow(get_docs(q(c(&["posts"]), vec![w(s("parent"), "==", s(&id)), lim(100.0)])).await)?;
    let mut list: Vec<JsValue> = docs(&snap).iter().map(post_row).collect();
    list.sort_by(|a, b| num(&get(a, "at")).partial_cmp(&num(&get(b, "at"))).unwrap_or(std::cmp::Ordering::Equal));
    Ok(list.into_iter().collect::<js_sys::Array>().into())
}
async fn post_get(id: String) -> R {
    let sn = rethrow(get_doc(d(&["posts", &id])).await)?;
    Ok(if exists(&sn) { post_row(&sn) } else { JsValue::NULL })
}
/// 글 지우기: 답글이면 같은 묶음에서 원래 글의 댓글 수도 1 줄여요
async fn post_delete(id: String) -> R {
    let sn = rethrow(get_doc(d(&["posts", &id])).await)?;
    let parent = st(&get(&data(&sn), "parent"));
    let b = batch();
    m(&b, "delete", &[d(&["posts", &id])])?;
    if !parent.is_empty() {
        let ps = get_doc(d(&["posts", &parent])).await?;
        if exists(&ps) && num(&get(&data(&ps), "rc")) >= 1.0 {
            m(&b, "update", &[d(&["posts", &parent]), o(&[("rc", inc(-1.0)), ("last", s(&id))])])?;
        }
    }
    rethrow(commit(&b).await)?;
    Ok(s(&parent))
}
/// 좋아요·리포스트 켜고 끄기 (내 표시만)
async fn post_mark(id: String, field: &str, on: bool) -> R {
    let k = format!("{field}.{}", uid_of()?);
    let x = js_sys::Object::new();
    set(&x, &k, &if on { JsValue::TRUE } else { f("deleteField", &[])? });
    rethrow(update_doc(d(&["posts", &id]), x.into()).await)
}

/* ---------- window.cloud 만들기 ---------- */

type Args = [JsValue; 5];
fn method(api: &js_sys::Object, name: &str, fun: impl Fn(Args) -> Fut + 'static) {
    let c = Closure::<dyn FnMut(JsValue, JsValue, JsValue, JsValue, JsValue) -> JsValue>::new(move |a, b, c, d_, e| JsValue::from(future_to_promise(fun([a, b, c, d_, e]))));
    set(api, name, &c.into_js_value());
}
/// 약속이 아니라 바로 값을 돌려주는 것 (peek, watch…)
fn method_now(api: &js_sys::Object, name: &str, fun: impl Fn(Args) -> R + 'static) {
    let c = Closure::<dyn FnMut(JsValue, JsValue, JsValue, JsValue, JsValue) -> Result<JsValue, JsValue>>::new(move |a, b, c, d_, e| fun([a, b, c, d_, e]));
    set(api, name, &c.into_js_value());
}
fn getter(api: &js_sys::Object, name: &str, fun: impl Fn() -> JsValue + 'static) {
    let g = Closure::<dyn FnMut() -> JsValue>::new(fun).into_js_value();
    let desc = o(&[("get", g), ("enumerable", true.into())]);
    let _ = js_sys::Object::define_property(api, &s(name), desc.unchecked_ref());
}
macro_rules! am {
    ($api:expr, $name:expr, |$a:ident| $body:expr) => {
        method(&$api, $name, |$a: Args| Box::pin(async move { $body }))
    };
}
fn sa(v: &JsValue) -> String {
    jstr(v)
}
fn opt_s(v: &JsValue) -> String {
    if v.is_undefined() {
        String::new()
    } else {
        jstr(v)
    }
}

fn build() -> js_sys::Object {
    let api = js_sys::Object::new();
    getter(&api, "uid", || s(&st(&get(&current(), "uid"))));
    getter(&api, "emu", || get(&fbo(), "emu").is_truthy().into());
    // 계정을 만든 시각 (밀리초, 모르면 0). 공지는 이 뒤에 올라온 것만 보여줘요
    getter(&api, "createdAt", || {
        let t = js_sys::Date::parse(&st(&get(&get(&current(), "metadata"), "creationTime")));
        n(if t.is_nan() { 0.0 } else { t })
    });
    getter(&api, "user", || {
        let u = current();
        if u.is_null() || u.is_undefined() {
            return JsValue::NULL;
        }
        let dn = get(&u, "displayName");
        o(&[("id", s(&my_id())), ("name", if dn.is_truthy() { dn } else { s("") })])
    });
    /* ----- 계정 ----- */
    // 인자는 5개까지예요: 아이디, 비밀번호, 이름, 사진, 초대한 사람 번호 (휴대폰 인증번호는 지금 쓰지 않아요)
    am!(api, "signUp", |a| sign_up(sa(&a[0]), sa(&a[1]), sa(&a[2]), opt_s(&a[3]), String::new(), opt_s(&a[4])).await);
    am!(api, "invitesList", |_a| invites_list().await);
    am!(api, "feedbackSend", |a| feedback_send(sa(&a[0]), sa(&a[1])).await);
    am!(api, "feedbackList", |_a| feedback_list().await);
    am!(api, "feedbackDelete", |a| rethrow(aw(f("deleteDoc", &[d(&["feedback", &sa(&a[0])])])).await));
    am!(api, "inviteClaim", |a| invite_claim(sa(&a[0])).await);
    am!(api, "betaState", |_a| beta_state().await);
    am!(api, "betaEnd", |_a| beta_end().await);
    am!(api, "phoneSend", |a| phone_send(sa(&a[0])).await);
    am!(api, "signIn", |a| aw(f("signInWithEmailAndPassword", &[auth(), s(&to_email(&sa(&a[0]))), a[1].clone()])).await.map_err(wrapped));
    am!(api, "signOut", |_a| aw(f("signOut", &[auth()])).await);
    am!(api, "ensureAccount", |_a| ensure_account().await);
    /* ----- 내 설정 ----- */
    am!(api, "loadSettings", |_a| {
        let sn = rethrow(get_doc(d(&["users", &uid_of()?])).await)?;
        Ok(if exists(&sn) { data(&sn) } else { JsValue::NULL })
    });
    am!(api, "saveSettings", |a| set_merge(d(&["users", &uid_of()?]), ext(&a[0], &[("updatedAt", n(now()))])).await);
    /* ----- 사람 ----- */
    method_now(&api, "peek", |a| {
        let cc = read_cache(&sa(&a[0]));
        let v = get(&cc, "d");
        Ok(if v.is_truthy() { v } else { JsValue::NULL })
    });
    am!(api, "setName", |a| set_name(sa(&a[0])).await);
    am!(api, "setBlue", |a| set_blue(sa(&a[0]), a[1].is_truthy()).await);
    am!(api, "fetchProfiles", |a| fetch_profiles(a[0].clone()).await);
    am!(api, "searchUsers", |a| search_users(sa(&a[0])).await);
    am!(api, "listUsers", |a| list_users(a[0].is_truthy()).await);
    /* ----- 랭킹 ----- */
    am!(api, "syncRank", |a| sync_rank(a[0].clone()).await);
    am!(api, "loadRanks", |a| load_ranks(a[0].is_truthy()).await);
    am!(api, "countAbove", |a| count_of(q(c(&["ranks"]), vec![w(s("bal"), ">", a[0].clone()), lim(50.0)])).await);
    /* ----- 회사 ----- */
    /* ----- 연구실·공장 ----- */
    /* ----- 상점·인벤토리 ----- */
    am!(api, "shopBuy", |a| shop_buy(sa(&a[0]), sa(&a[1]), a[2].clone(), opt_s(&a[3])).await);
    /* ----- 관리·공지 ----- */
    am!(api, "searchProfiles", |a| search_profiles(sa(&a[0])).await);
    am!(api, "adminGrant", |a| admin_grant(sa(&a[0]), a[1].clone()).await);
    am!(api, "adminNotice", |a| admin_notice(a[0].clone()).await);
    am!(api, "noticeIds", |_a| notice_ids().await);
    am!(api, "noticeList", |a| notice_list(a[0].clone()).await);
    am!(api, "unseenNotice", |_a| unseen_notice().await);
    am!(api, "markNoticeSeen", |a| mark_notice_seen(sa(&a[0])).await);
    am!(api, "voteNotice", |a| {
        rethrow(set_doc(d(&["notices", &sa(&a[0]), "votes", &uid_of()?]), o(&[("choice", n(js_sys::Number::from(a[1].clone()).value_of())), ("at", ts())])).await)?;
        Ok(JsValue::UNDEFINED)
    });
    am!(api, "noticeTally", |a| notice_tally(a[0].clone()).await);
    am!(api, "adminDelete", |a| admin_delete(sa(&a[0])).await);
    /* ----- 상태·지갑 ----- */
    am!(api, "setPhoto", |a| set_photo(a[0].clone()).await);
    method_now(&api, "watchWallet", |a| watch_wallet(a[0].clone().dyn_into()?));
    am!(api, "claimInterest", |a| update_doc(wallet_ref()?, o(&[("bal", inc(num(&a[0]))), ("t", from_millis(num(&a[1]))), ("wt", ts()), ("op", o(&[("kind", s("interest"))]))])).await);
    am!(api, "buyProduct", |a| update_doc(wallet_ref()?, o(&[("rate", a[0].clone()), ("bal", inc(-num(&a[1]))), ("t", ts()), ("wt", ts()), ("op", o(&[("kind", s("buy"))]))])).await);
    /* ----- 대화 ----- */
    am!(api, "openChat", |a| open_chat(sa(&a[0])).await);
    am!(api, "createGroup", |a| create_group(a[0].clone(), a[1].clone()).await);
    am!(api, "inviteMembers", |a| invite_members(sa(&a[0]), a[1].clone()).await);
    // 단톡방 이름이나 사진 바꾸기: 둘째 값은 {name} 또는 {photo}
    am!(api, "setGroup", |a| rethrow(update_doc(d(&["chats", &sa(&a[0])]), a[1].clone()).await));
    am!(api, "nameOf", |a| name_of(sa(&a[0])).await);
    method_now(&api, "watchChats", |a| watch_chats(a[0].clone().dyn_into()?));
    method_now(&api, "watchMessages", |a| watch_messages(sa(&a[0]), a[1].clone().dyn_into()?, a[2].clone()));
    am!(api, "loadOlder", |a| load_older(sa(&a[0]), a[1].clone(), a[2].clone()).await);
    am!(api, "send", |a| send(sa(&a[0]), a[1].clone(), a[2].clone(), a[3].clone(), None).await);
    am!(api, "sendGift", |a| {
        let amt = num(&a[2]);
        send(sa(&a[0]), a[1].clone(), o(&[("type", s("gift")), ("amount", a[2].clone()), ("done", false.into())]), s(&format!("멸공 {} 선물", ko(amt))), Some(amt)).await
    });
    am!(api, "presenceBeat", |_a| presence_beat().await);
    am!(api, "presenceList", |_a| presence_list().await);
    method_now(&api, "gameWatch", |a| game_watch(a[0].clone().dyn_into().map_err(|_| JsValue::from_str("cb"))?));
    am!(api, "gameMe", |_a| game_me().await);
    am!(api, "gameJoin", |a| game_join(sa(&a[0]), sa(&a[1])).await);
    am!(api, "gameFlush", |a| game_flush(sa(&a[0]), num(&a[1])).await);
    am!(api, "gameTop", |_a| game_top().await);
    am!(api, "tradesList", |_a| trades_list().await);
    am!(api, "tradeCreate", |a| trade_create(a[0].clone()).await);
    am!(api, "tradeDone", |a| trade_done(sa(&a[0]), a[1].is_truthy()).await);
    am!(api, "tradeDelete", |a| trade_delete(sa(&a[0])).await);
    am!(api, "plansList", |_a| plans_list().await);
    am!(api, "planCreate", |a| plan_create(a[0].clone()).await);
    am!(api, "planJoin", |a| plan_join(sa(&a[0]), a[1].is_truthy()).await);
    am!(api, "planDelete", |a| plan_delete(sa(&a[0])).await);
    am!(api, "cancelGift", |a| cancel_gift(sa(&a[0]), sa(&a[1])).await);
    am!(api, "claimGift", |a| claim_gift(sa(&a[0]), sa(&a[1]), a[2].clone()).await);
    am!(api, "payRequest", |a| pay_request(sa(&a[0]), a[1].clone(), sa(&a[2]), a[3].clone()).await);
    am!(api, "vote", |a| {
        let k = format!("votes.{}", uid_of()?);
        let x = js_sys::Object::new();
        set(&x, &k, &a[2]);
        update_doc(d(&["chats", &sa(&a[0]), "messages", &sa(&a[1])]), x.into()).await
    });
    /* ----- 통화 ----- */
    /* ----- SNS ----- */
    am!(api, "postCreate", |a| post_create(opt_s(&a[0]), opt_s(&a[1]), opt_s(&a[2]), opt_s(&a[3]), opt_s(&a[4])).await);
    am!(api, "reelGift", |a| reel_gift(sa(&a[0]), sa(&a[1]), num(&a[2])).await);
    am!(api, "reelGifts", |_a| reel_gifts().await);
    am!(api, "decoBag", |_a| deco_bag().await);
    am!(api, "decoBuy", |a| deco_buy(sa(&a[0]), num(&a[1]), a[2].clone()).await);
    am!(api, "idToken", |_a| id_token().await);
    am!(api, "phoneVerified", |_a| phone_verified().await);
    am!(api, "casinoBet", |a| casino_bet(num(&a[0]), num(&a[1])).await);
    am!(api, "contestList", |_a| contest_list().await);
    am!(api, "contestCreate", |a| rethrow(set_doc(new_id(c(&["contests"])), o(&[("title", s(&sa(&a[0]))), ("open", true.into()), ("at", ts()), ("n", n(0.0))])).await));
    am!(api, "contestClose", |a| rethrow(update_doc(d(&["contests", &sa(&a[0])]), o(&[("open", false.into()), ("end", ts())])).await));
    am!(api, "contestEntries", |a| contest_entries(sa(&a[0])).await);
    am!(api, "contestMyVote", |a| {
        let sn = get_doc(d(&["contests", &sa(&a[0]), "votes", &uid_of()?])).await?;
        Ok(if exists(&sn) { get(&data(&sn), "entry") } else { s("") })
    });
    am!(api, "contestEnter", |a| contest_enter(sa(&a[0]), sa(&a[1])).await);
    am!(api, "contestVote", |a| contest_vote(sa(&a[0]), sa(&a[1]), opt_s(&a[2])).await);
    am!(api, "decoSet", |a| deco_set(a[0].clone()).await);
    am!(api, "giphyGet", |_a| {
        let sn = get_doc(d(&["meta", "giphy"])).await?;
        Ok(if exists(&sn) { get(&data(&sn), "key") } else { s("") })
    });
    am!(api, "giphySet", |a| rethrow(set_doc(d(&["meta", "giphy"]), o(&[("key", s(&sa(&a[0])))])).await));
    am!(api, "reelGiftClaim", |a| reel_gift_claim(sa(&a[0])).await);
    am!(api, "postShare", |a| rethrow(update_doc(d(&["posts", &sa(&a[0])]), o(&[("sh", inc(1.0))])).await));
    // 팔로우: 내 설정 문서의 follows (사람 번호 → true)
    method_now(&api, "postsWatch", |a| posts_watch(a[0].clone().dyn_into()?));
    am!(api, "postReplies", |a| post_replies(sa(&a[0])).await);
    am!(api, "postGet", |a| post_get(sa(&a[0])).await);
    am!(api, "postLike", |a| post_mark(sa(&a[0]), "likes", a[1].is_truthy()).await);
    am!(api, "postDelete", |a| post_delete(sa(&a[0])).await);
    am!(api, "markRead", |a| {
        let k = format!("reads.{}", uid_of()?);
        let x = js_sys::Object::new();
        set(&x, &k, &ts());
        Ok(update_doc(d(&["chats", &sa(&a[0])]), x.into()).await.unwrap_or(JsValue::UNDEFINED))
    });
    api
}

async fn connect() -> R {
    let g = js_sys::global();
    // firebase.js(모듈)가 라이브러리를 넘겨 줄 때까지 기다려요 (모듈은 이 WASM보다 먼저 돌아요)
    for _ in 0..200 {
        if fbo().is_object() {
            break;
        }
        crate::dom::sleep(25).await;
    }
    let cfg = get(&fbo(), "config");
    if !get(&cfg, "apiKey").is_truthy() || !get(&cfg, "projectId").is_truthy() {
        set(&g, "cloudError", &s("설정값이 비어 있어요"));
        return Ok(JsValue::NULL);
    }
    let r: R = async {
        let app = f("initializeApp", &[cfg])?;
        let auth_ = f("initializeAuth", &[app.clone(), o(&[("persistence", arr(&[get(&fbo(), "indexedDBLocalPersistence"), get(&fbo(), "browserLocalPersistence")]))])])?;
        set(&auth_, "languageCode", &s("ko"));
        // 느린 망이나 프록시에서도 끊기지 않게 자동으로 긴 연결 방식을 골라요. 오프라인 저장은 쓰지 않아요(메모리만)
        let db_ = f("initializeFirestore", &[app, o(&[("localCache", f("memoryLocalCache", &[])?), ("experimentalAutoDetectLongPolling", true.into())])])?;
        // 시험용 묶음에서만 켜져요
        if get(&fbo(), "emu").is_truthy() {
            f("connectAuthEmulator", &[auth_.clone(), s("http://127.0.0.1:9099"), o(&[("disableWarnings", true.into())])])?;
            f("connectFirestoreEmulator", &[db_.clone(), s("127.0.0.1"), n(8081.0)])?;
        }
        ST.with(|x| {
            *x.borrow_mut() = Some(St { auth: auth_.clone(), db: db_, mem: HashMap::new(), list_cursor: JsValue::NULL, names: HashMap::new(), rank: Rank::default(), ensured: String::new() })
        });
        // 로그인 상태가 정해질 때까지 한 번만 기다려요
        let p = js_sys::Promise::new(&mut |resolve, _| {
            let off: Rc<RefCell<JsValue>> = Rc::new(RefCell::new(JsValue::UNDEFINED));
            let off2 = off.clone();
            let cb = Closure::<dyn FnMut(JsValue)>::new(move |_| {
                if let Ok(fun) = off2.borrow().clone().dyn_into::<js_sys::Function>() {
                    let _ = fun.call0(&JsValue::NULL);
                }
                let _ = resolve.call0(&JsValue::NULL);
            })
            .into_js_value();
            if let Ok(v) = f("onAuthStateChanged", &[auth_.clone(), cb]) {
                *off.borrow_mut() = v;
            }
        });
        JsFuture::from(p).await?;
        let api = build();
        set(&g, "cloud", &api);
        Ok(api.into())
    }
    .await;
    match r {
        Ok(v) => Ok(v),
        Err(err) => {
            web_sys::console::warn_2(&s("파이어베이스에 연결하지 못했어요:"), &err);
            let code = get(&err, "code");
            let msg = if code.is_truthy() {
                format!("{} ({})", explain(&err), jstr(&code))
            } else {
                let mm = get(&err, "message");
                format!("연결하지 못했어요 ({})", if mm.is_truthy() { jstr(&mm) } else { jstr(&err) })
            };
            set(&g, "cloudError", &s(&msg));
            Ok(JsValue::NULL)
        }
    }
}

/// window.cloudReady를 만들어요 (연결을 마치면 window.cloud, 실패하면 null)
pub fn start() {
    let g = js_sys::global();
    if !get(&g, "cloudReady").is_undefined() {
        return;
    }
    set(&g, "cloudReady", &future_to_promise(connect()));
}
