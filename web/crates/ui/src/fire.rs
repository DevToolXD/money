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
const KINDS: [&str; 4] = ["food", "pc", "ai", "nuclear"];
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
fn company_of(id: &str, dd: &JsValue) -> JsValue {
    let or_s = |k: &str| {
        let v = get(dd, k);
        if v.is_truthy() {
            v
        } else {
            s("")
        }
    };
    let kind = get(dd, "kind");
    let int_or0 = |k: &str| {
        let v = get(dd, k);
        if is_int(&v) {
            v
        } else {
            n(0.0)
        }
    };
    let owner = get(dd, "owner");
    let products = get(dd, "products");
    o(&[
        ("id", s(id)),
        ("name", or_s("name")),
        ("logo", or_s("logo")),
        ("kind", if kind.as_string().is_some_and(|k| KINDS.contains(&k.as_str())) { kind } else { s("food") }),
        ("open", int_or0("open")),
        ("staff", int_or0("staff")),
        ("owner", if owner.is_string() { owner } else { s(id) }), // 예전 회사는 문서 번호가 사장이에요
        ("products", if js_sys::Array::is_array(&products) { products } else { js_sys::Array::new().into() }),
        ("at", n(ms(&get(dd, "at")))),
        ("kindSet", (get(dd, "kindSet") == JsValue::TRUE).into()),
    ])
}
fn int3(v: &JsValue, keys: &[&str]) -> bool {
    v.is_truthy() && keys.iter().all(|k| is_int(&get(v, k)))
}
fn nn(v: JsValue) -> JsValue {
    if v.is_undefined() || v.is_null() {
        JsValue::NULL
    } else {
        v
    }
}
// 연구실 숫자 기본값
fn lab_of(dd: &JsValue) -> JsValue {
    let ni = |k: &str, dflt: f64| {
        let v = get(dd, k);
        if is_int(&v) {
            v
        } else {
            n(dflt)
        }
    };
    let g = get(dd, "gen");
    let r = get(dd, "rel");
    let pcm = get(dd, "pcM");
    let trn = get(dd, "trn");
    o(&[
        ("nperf", ni("nperf", 100.0)),
        ("elec", ni("elec", 0.0)),
        ("gen", if int3(&g, &["amt", "secs"]) && get(&g, "started").is_truthy() { o(&[("amt", get(&g, "amt")), ("secs", get(&g, "secs")), ("started", get(&g, "started"))]) } else { JsValue::NULL }),
        ("cap", ni("cap", 0.0)),
        ("spd", ni("spd", 0.0)),
        ("pow", ni("pow", 0.0)),
        ("rel", if int3(&r, &["cap", "spd", "pow"]) { o(&[("cap", get(&r, "cap")), ("spd", get(&r, "spd")), ("pow", get(&r, "pow"))]) } else { JsValue::NULL }),
        ("aiData", ni("aiData", 0.0)),
        ("aiScore", ni("aiScore", -1.0)),
        ("cash", ni("cash", 0.0)),
        ("price", ni("price", 0.0)),
        ("sold", nn(get(dd, "sold"))),
        ("bought", nn(get(dd, "bought"))),
        ("pcN", ni("pcN", 0.0)),
        ("pcM", if int3(&pcm, &["cap", "spd", "pow"]) { o(&[("cap", get(&pcm, "cap")), ("spd", get(&pcm, "spd")), ("pow", get(&pcm, "pow"))]) } else { JsValue::NULL }),
        ("aiH", ni("aiH", 0.0)),
        ("aiDev", ni("aiDev", 0.0)),
        (
            "trn",
            if int3(&trn, &["d", "secs", "score"]) && get(&trn, "started").is_truthy() {
                o(&[("d", get(&trn, "d")), ("secs", get(&trn, "secs")), ("score", get(&trn, "score")), ("started", get(&trn, "started"))])
            } else {
                JsValue::NULL
            },
        ),
        ("food", nn(get(dd, "food"))),
        ("fperf", ni("fperf", 100.0)),
        ("plv", ni("plv", 1.0)),
        ("prod", nn(get(dd, "prod"))),
    ])
}
// 문서에 쓸 모양: 기본값인 칸(등급 1, 성능 100, 공장 비움, 판매 기록 없음)은 적지 않아요
fn lab_write(l: &JsValue) -> JsValue {
    let x = ext(l, &[]);
    let del = |k: &str| {
        let _ = js_sys::Reflect::delete_property(x.unchecked_ref::<js_sys::Object>(), &s(k));
    };
    if num(&get(&x, "plv")) == 1.0 {
        del("plv");
    }
    if num(&get(&x, "fperf")) == 100.0 {
        del("fperf");
    }
    if get(&x, "prod").is_null() {
        del("prod");
    }
    if get(&x, "food").is_null() {
        del("food");
    }
    x
}
fn lab_data(snap: &JsValue) -> JsValue {
    lab_of(&if exists(snap) { data(snap) } else { JsValue::NULL })
}
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
/// wrap: 오류를 한국어 문장으로 바꿔요 (code는 그대로)
fn wrapped(e: JsValue) -> JsValue {
    report(&e);
    let err = js_sys::Error::new(&explain(&e));
    set(&err, "code", &get(&e, "code"));
    err.into()
}
async fn sign_up(id: String, pw: String, name: String, photo: String) -> R {
    let r: R = async {
        let cred = aw(f("createUserWithEmailAndPassword", &[auth(), s(&to_email(&id)), s(&pw)])).await?;
        let user = get(&cred, "user");
        let rest: R = async {
            aw(f("updateProfile", &[user.clone(), o(&[("displayName", s(&name))])])).await?;
            let ph = if photo.encode_utf16().count() <= 40000 { photo.clone() } else { String::new() };
            set_doc(d(&["profiles", &st(&get(&user, "uid"))]), o(&[("id", s(&id.trim().to_lowercase())), ("name", s(&name)), ("photo", s(&ph))])).await?;
            make_wallet().await
        }
        .await;
        if let Err(e) = rest {
            // 반쪽짜리 계정이 남지 않게 지워요
            let _ = aw(f("deleteUser", &[user])).await;
            return Err(e);
        }
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
        // 옛 방식으로 만든 계정은 프로필이 없어요. 실명이 있으면 쓰고, 없으면 아이디를 이름으로 써요
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
async fn load_ranks_of(uids: JsValue) -> R {
    let a: js_sys::Array = uids.dyn_into().unwrap_or_default();
    let out = js_sys::Array::new();
    let mut i = 0;
    while i < a.length() && i < 30 {
        let part = a.slice(i, i + 10);
        let snap = get_docs(q(c(&["ranks"]), vec![w(f("documentId", &[])?, "in", part.into()), lim(10.0)])).await?;
        for dd in docs(&snap) {
            out.push(&rank_row(&dd));
        }
        i += 10;
    }
    Ok(out.into())
}

/* ---------- 회사 ---------- */

async fn list_my_companies() -> R {
    let uid = uid_of()?;
    let a = get_docs(q(c(&["companies"]), vec![w(s("owner"), "==", s(&uid)), lim(50.0)]));
    let b = get_doc(d(&["companies", &uid]));
    let rs = join_all(vec![Box::pin(a) as Fut, Box::pin(b) as Fut]).await;
    let mut it = rs.into_iter();
    let snap = rethrow(it.next().unwrap())?;
    let legacy = rethrow(it.next().unwrap())?;
    let mut out: Vec<JsValue> = docs(&snap).iter().map(|dd| company_of(&st(&get(dd, "id")), &data_est(dd))).collect();
    if exists(&legacy) && get(&data(&legacy), "owner").is_undefined() {
        out.push(company_of(&uid, &data_est(&legacy)));
    }
    out.sort_by(|x, y| num(&get(x, "at")).partial_cmp(&num(&get(y, "at"))).unwrap_or(std::cmp::Ordering::Equal));
    Ok(out.into_iter().collect::<js_sys::Array>().into())
}
async fn get_company(cid: String) -> R {
    let sn = rethrow(get_doc(d(&["companies", &cid])).await)?;
    Ok(if exists(&sn) { company_of(&cid, &data(&sn)) } else { JsValue::NULL })
}
async fn list_companies(qs: JsValue) -> R {
    let raw = if qs.is_undefined() { String::new() } else { jstr(&qs) };
    let term: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    // JS: trim 뒤 공백 줄이기, 20자까지
    let term = u16take(&term, 20);
    let r = c(&["companies"]);
    let snap = rethrow(
        if term.is_empty() {
            get_docs(q(r, vec![order("at", Some("desc")), lim(50.0)])).await
        } else {
            get_docs(q(r, vec![order("name", None), w(s("name"), ">=", s(&term)), w(s("name"), "<=", s(&format!("{term}{HI}"))), lim(20.0)])).await
        },
    )?;
    Ok(docs(&snap).iter().map(|dd| company_of(&st(&get(dd, "id")), &data_est(dd))).collect::<js_sys::Array>().into())
}

async fn load_my_job() -> R {
    let sn = rethrow(get_doc(d(&["jobs", &uid_of()?])).await)?;
    if !exists(&sn) {
        return Ok(JsValue::NULL);
    }
    let dd = data_est(&sn);
    Ok(o(&[("cid", get(&dd, "cid")), ("kind", get(&dd, "kind")), ("at", n(ms(&get(&dd, "at"))))]))
}
async fn apply_job(cid: String, kind: JsValue) -> R {
    let b = batch();
    m(&b, "set", &[d(&["jobs", &uid_of()?]), o(&[("cid", s(&cid)), ("kind", kind), ("at", ts())])])?;
    m(&b, "update", &[d(&["companies", &cid]), o(&[("open", inc(-1.0)), ("staff", inc(1.0))])])?;
    rethrow(commit(&b).await)?;
    Ok(JsValue::UNDEFINED)
}
async fn quit_job(cid: String) -> R {
    let b = batch();
    m(&b, "delete", &[d(&["jobs", &uid_of()?])])?;
    m(&b, "update", &[d(&["companies", &cid]), o(&[("staff", inc(-1.0))])])?;
    rethrow(commit(&b).await)?;
    Ok(JsValue::UNDEFINED)
}
async fn found_company(name: JsValue, logo: JsValue, kind: JsValue) -> R {
    // 업종을 함께 주면 바로 정해져요. 안 주면(화면에서 창업할 때) 식품으로 두고, 이어서 업종을 정해요
    let chosen = !kind.is_undefined();
    let kind = if kind.is_undefined() { s("food") } else { kind };
    let r = new_id(c(&["companies"]));
    let id = get(&r, "id");
    let b = batch();
    let co = o(&[("name", name), ("logo", logo), ("kind", kind), ("owner", s(&uid_of()?)), ("products", js_sys::Array::new().into()), ("open", n(0.0)), ("staff", n(0.0)), ("at", ts())]);
    if chosen {
        set(&co, "kindSet", &true.into());
    }
    m(&b, "set", &[r, co])?;
    m(&b, "update", &[wallet_ref()?, o(&[("bal", inc(-10.0)), ("wt", ts()), ("op", o(&[("kind", s("found")), ("cid", id.clone())]))])])?;
    commit(&b).await?;
    Ok(id)
}

/* ---------- 연구실·공장 ---------- */

fn lab_ref(cid: &str) -> JsValue {
    d(&["companies", cid, "lab", "main"])
}
async fn lab_get(cid: String) -> R {
    let sn = rethrow(get_doc(lab_ref(&cid)).await)?;
    Ok(lab_data(&sn))
}
// 연구실 숫자 한 묶음: 읽고, fun이 새 숫자를 주면 저장해요 (없으면 아무 일도 안 해요)
async fn lab_tx(cid: String, fun: Rc<dyn Fn(JsValue) -> Option<JsValue>>) -> R {
    rethrow(
        run_tx(move |tx| {
            let cid = cid.clone();
            let fun = fun.clone();
            Box::pin(async move {
                let r = lab_ref(&cid);
                let sn = tx_get(&tx, &r).await?;
                let Some(next) = fun(lab_data(&sn)) else { return Ok(JsValue::NULL) };
                let clean = lab_write(&lab_of(&next));
                m(&tx, "set", &[r, clean.clone()])?;
                Ok(clean)
            })
        })
        .await,
    )
}
fn started_ms(v: &JsValue) -> f64 {
    m(&get(v, "started"), "toMillis", &[]).map(|x| num(&x)).unwrap_or(f64::NAN)
}
async fn lab_gen_start(cid: String, amt: JsValue) -> R {
    let why = Rc::new(RefCell::new("ok"));
    let wy = why.clone();
    let r = lab_tx(
        cid,
        Rc::new(move |l: JsValue| {
            let nperf = num(&get(&l, "nperf"));
            let a = num(&amt);
            if !(is_int(&amt) && a >= 1.0 && a <= 100000f64.min(nperf * 10.0)) {
                *wy.borrow_mut() = "max";
                return None;
            }
            if get(&l, "gen").is_truthy() {
                *wy.borrow_mut() = "busy";
                return None;
            }
            let secs = (a * 100.0 / nperf).ceil();
            if num(&get(&l, "cash")) < secs * 100.0 {
                *wy.borrow_mut() = "poor";
                return None;
            }
            Some(ext(&l, &[("cash", n(num(&get(&l, "cash")) - secs * 100.0)), ("gen", o(&[("amt", amt.clone()), ("secs", n(secs)), ("started", ts())]))]))
        }),
    )
    .await?;
    Ok(s(if r.is_truthy() { "ok" } else { *why.borrow() }))
}
async fn lab_gen_claim(cid: String) -> R {
    let got = Rc::new(RefCell::new(0.0));
    let g2 = got.clone();
    let r = lab_tx(
        cid,
        Rc::new(move |l: JsValue| {
            let g = get(&l, "gen");
            if !g.is_truthy() || now() < started_ms(&g) + num(&get(&g, "secs")) * 1000.0 {
                return None;
            }
            *g2.borrow_mut() = num(&get(&g, "amt"));
            Some(ext(&l, &[("elec", n(num(&get(&l, "elec")) + num(&get(&g, "amt")))), ("gen", JsValue::NULL)]))
        }),
    )
    .await?;
    Ok(n(if r.is_truthy() { *got.borrow() } else { 0.0 }))
}
async fn lab_train_start(cid: String, dv: JsValue) -> R {
    let why = Rc::new(RefCell::new("ok"));
    let wy = why.clone();
    let r = lab_tx(
        cid,
        Rc::new(move |l: JsValue| {
            if get(&l, "trn").is_truthy() {
                *wy.borrow_mut() = "busy";
                return None;
            }
            let pcm = get(&l, "pcM");
            if !(num(&get(&l, "pcN")) >= 1.0 && pcm.is_truthy() && num(&get(&pcm, "cap")) >= 1.0 && num(&get(&pcm, "spd")) >= 1.0) {
                *wy.borrow_mut() = "nocomp";
                return None;
            }
            let dd = num(&dv);
            if !(is_int(&dv) && dd >= 1.0 && dd <= num(&get(&l, "aiData"))) {
                *wy.borrow_mut() = "nodata";
                return None;
            }
            let (cap, spd, pow) = (num(&get(&pcm, "cap")), num(&get(&pcm, "spd")), num(&get(&pcm, "pow")));
            let batches = (dd / cap).ceil();
            let used = batches * pow;
            if num(&get(&l, "elec")) < used {
                *wy.borrow_mut() = "poor";
                return None;
            }
            let secs = (batches / spd).ceil();
            let score = (dd * (cap + spd) / 100.0).floor();
            Some(ext(&l, &[("elec", n(num(&get(&l, "elec")) - used)), ("trn", o(&[("d", dv.clone()), ("secs", n(secs)), ("score", n(score)), ("started", ts())]))]))
        }),
    )
    .await?;
    Ok(s(if r.is_truthy() { "ok" } else { *why.borrow() }))
}
async fn lab_train_claim(cid: String) -> R {
    let got = Rc::new(RefCell::new(-1.0));
    let g2 = got.clone();
    let r = lab_tx(
        cid,
        Rc::new(move |l: JsValue| {
            let t = get(&l, "trn");
            if !t.is_truthy() || now() < started_ms(&t) + num(&get(&t, "secs")) * 1000.0 {
                return None;
            }
            *g2.borrow_mut() = num(&get(&t, "score"));
            Some(ext(&l, &[("aiDev", get(&t, "score")), ("trn", JsValue::NULL)]))
        }),
    )
    .await?;
    Ok(n(if r.is_truthy() { *got.borrow() } else { -1.0 }))
}
async fn lab_cash(cid: String, tokens: JsValue, dir: JsValue) -> R {
    rethrow(
        run_tx(move |tx| {
            let (cid, tokens, dir) = (cid.clone(), tokens.clone(), dir.clone());
            Box::pin(async move {
                let wref = wallet_ref()?;
                let lref = lab_ref(&cid);
                let ws = tx_get(&tx, &wref).await?;
                let ls = tx_get(&tx, &lref).await?;
                let bal = num(&get(&data(&ws), "bal"));
                let l = lab_data(&ls);
                let (tk, dr) = (num(&tokens), num(&dir));
                if dr > 0.0 && bal < tk {
                    return Err(fail("insufficient", "잔액이 부족해요"));
                }
                if dr < 0.0 && num(&get(&l, "cash")) < tk * 100.0 {
                    return Err(fail("insufficient", "금고가 부족해요"));
                }
                m(&tx, "update", &[wref, o(&[("bal", n(bal - dr * tk)), ("wt", ts()), ("op", o(&[("kind", s(if dr > 0.0 { "deposit" } else { "withdraw" })), ("cid", s(&cid)), ("amt", tokens.clone())]))])])?;
                let cash = num(&get(&l, "cash")) + dr * tk * 100.0;
                m(&tx, "set", &[lref, lab_write(&lab_of(&ext(&l, &[("cash", n(cash))])))])?;
                Ok(n(cash))
            })
        })
        .await,
    )
}
async fn lab_buy(buyer: String, seller: String, kind: String, k: JsValue) -> R {
    rethrow(
        run_tx(move |tx| {
            let (buyer, seller, kind, k) = (buyer.clone(), seller.clone(), kind.clone(), k.clone());
            Box::pin(async move {
                let bref = lab_ref(&buyer);
                let sref = lab_ref(&seller);
                let bs = tx_get(&tx, &bref).await?;
                let ss = tx_get(&tx, &sref).await?;
                let b = lab_data(&bs);
                let sl = lab_data(&ss);
                let kn = num(&k);
                if !(is_int(&k) && kn >= 1.0) {
                    return Err(fail("bad", "수량을 확인해 주세요"));
                }
                let total = kn * num(&get(&sl, "price"));
                if num(&get(&b, "cash")) < total {
                    return Err(fail("insufficient", "금고가 부족해요"));
                }
                if kind == "e" && num(&get(&sl, "elec")) < kn {
                    return Err(fail("stock", "재고가 부족해요"));
                }
                if kind == "p" && !get(&sl, "rel").is_truthy() {
                    return Err(fail("unreleased", "아직 출시하지 않은 컴퓨터예요"));
                }
                if kind == "h" && num(&get(&sl, "aiScore")) < 0.0 {
                    return Err(fail("unreleased", "아직 출시하지 않은 인공지능이에요"));
                }
                let at = ts();
                let sold = o(&[("to", s(&buyer)), ("kind", s(&kind)), ("n", k.clone()), ("total", n(total)), ("at", at.clone())]);
                let bought = o(&[("from", s(&seller)), ("kind", s(&kind)), ("n", k.clone()), ("total", n(total)), ("at", at)]);
                let s_next = ext(&sl, &[("cash", n(num(&get(&sl, "cash")) + total)), ("sold", sold.clone())]);
                let b_next = ext(&b, &[("cash", n(num(&get(&b, "cash")) - total)), ("bought", bought.clone())]);
                if kind == "e" {
                    set(&s_next, "elec", &n(num(&get(&sl, "elec")) - kn));
                    set(&b_next, "elec", &n(num(&get(&b, "elec")) + kn));
                }
                if kind == "p" {
                    let rel = get(&sl, "rel");
                    let mm = o(&[("cap", get(&rel, "cap")), ("spd", get(&rel, "spd")), ("pow", get(&rel, "pow"))]);
                    for key in ["cap", "spd", "pow"] {
                        set(&sold, key, &get(&mm, key));
                        set(&bought, key, &get(&mm, key));
                    }
                    let pm = get(&b, "pcM");
                    let same = pm.is_truthy() && ["cap", "spd", "pow"].iter().all(|key| num(&get(&pm, key)) == num(&get(&mm, key)));
                    set(&b_next, "pcM", &mm);
                    set(&b_next, "pcN", &n(if same { num(&get(&b, "pcN")) } else { 0.0 } + kn)); // 같은 모델이면 더하고, 다른 모델이면 새 모델로 바꿔요
                }
                if kind == "h" {
                    set(&b_next, "aiH", &n(num(&get(&b, "aiH")) + kn));
                }
                m(&tx, "set", &[sref, lab_write(&lab_of(&s_next))])?;
                m(&tx, "set", &[bref, lab_write(&lab_of(&b_next))])?;
                Ok(n(total))
            })
        })
        .await,
    )
}
// 공장·일하기 전에 연구실 문서가 없으면 기본값으로 먼저 만들어요
async fn ensure_lab(cid: String) -> R {
    let lref = lab_ref(&cid);
    let sn = rethrow(get_doc(lref.clone()).await)?;
    if !exists(&sn) {
        let _ = set_doc(lref, lab_write(&lab_of(&JsValue::NULL))).await;
    }
    Ok(JsValue::UNDEFINED)
}
async fn lab_start_factory(cid: String, pid: String, k: JsValue) -> R {
    ensure_lab(cid.clone()).await?;
    rethrow(
        run_tx(move |tx| {
            let (cid, pid, k) = (cid.clone(), pid.clone(), k.clone());
            Box::pin(async move {
                let lref = lab_ref(&cid);
                let pref = d(&["companies", &cid, "products", &pid]);
                let ls = tx_get(&tx, &lref).await?;
                let ps = tx_get(&tx, &pref).await?;
                if !exists(&ls) {
                    return Err(fail("nolab", "공장을 아직 만들지 않았어요"));
                }
                let p = if exists(&ps) { data(&ps) } else { JsValue::NULL };
                if !p.is_truthy() || get(&p, "active") != JsValue::TRUE {
                    return Err(fail("unsellable", "지금은 만들 수 없는 상품이에요"));
                }
                let kn = num(&k);
                if !(is_int(&k) && kn >= 1.0 && kn <= 100000.0) {
                    return Err(fail("bad", "개수를 확인해 주세요"));
                }
                let l = lab_of(&data(&ls));
                if get(&l, "prod").is_truthy() {
                    return Err(fail("busy", "공장이 이미 돌고 있어요. 다 만들 때까지 기다려 주세요"));
                }
                m(&tx, "update", &[lref, o(&[("prod", o(&[("pid", s(&pid)), ("left", k.clone()), ("at", ts())]))])])?;
                Ok(k)
            })
        })
        .await,
    )
}
// 정산 (사장): 지금까지 만든 개수만큼 재료값이 금고에서 나가고 재고가 늘어요. 서버 시각을 쓰니 3초 덜 세요
async fn lab_settle_factory(cid: String) -> R {
    rethrow(
        run_tx(move |tx| {
            let cid = cid.clone();
            Box::pin(async move {
                let zero = |left: f64| o(&[("units", n(0.0)), ("left", n(left))]);
                let lref = lab_ref(&cid);
                let ls = tx_get(&tx, &lref).await?;
                if !exists(&ls) {
                    return Ok(zero(0.0));
                }
                let l = lab_of(&data(&ls));
                let prod = get(&l, "prod");
                if !prod.is_truthy() {
                    return Ok(zero(0.0));
                }
                let pid = jstr(&get(&prod, "pid"));
                let pref = d(&["companies", &cid, "products", &pid]);
                let ps = tx_get(&tx, &pref).await?;
                let p = if exists(&ps) { data(&ps) } else { JsValue::NULL };
                if !p.is_truthy() {
                    return Err(fail("unsellable", "상품이 없어요"));
                }
                let each = if is_int(&get(&p, "costCents")) { num(&get(&p, "costCents")) } else { 0.0 };
                let pat = m(&get(&prod, "at"), "toMillis", &[]).map(|x| num(&x))?;
                let plv = num(&get(&l, "plv"));
                let pleft = num(&get(&prod, "left"));
                let elapsed = (now() - 3000.0 - pat).max(0.0);
                // 60초가 한 번 지날 때마다 공장 대수만큼 한꺼번에 나와요 (공장이 늘어도 빨라지지 않아요)
                let mut u = pleft.min((elapsed / 60000.0).floor() * plv);
                if each > 0.0 {
                    u = u.min((num(&get(&l, "cash")) / each).floor());
                }
                if u < 1.0 {
                    return Ok(zero(pleft));
                }
                let cash = num(&get(&l, "cash")) - u * each;
                let stock = if is_int(&get(&p, "stock")) { num(&get(&p, "stock")) } else { 0.0 };
                let left = pleft - u;
                if left == 0.0 {
                    m(&tx, "update", &[lref, o(&[("cash", n(cash)), ("prod", f("deleteField", &[])?)])])?;
                } else {
                    m(&tx, "update", &[lref, o(&[("cash", n(cash)), ("prod", o(&[("pid", s(&pid)), ("left", n(left)), ("at", from_millis(pat + (u / plv).ceil() * 60000.0))]))])])?;
                }
                m(&tx, "update", &[pref, o(&[("stock", n(stock + u))])])?;
                Ok(o(&[("units", n(u)), ("left", n(left))]))
            })
        })
        .await,
    )
}
async fn lab_buy_factory(cid: String) -> R {
    ensure_lab(cid.clone()).await?;
    rethrow(
        run_tx(move |tx| {
            let cid = cid.clone();
            Box::pin(async move {
                let lref = lab_ref(&cid);
                let ls = tx_get(&tx, &lref).await?;
                if !exists(&ls) {
                    return Err(fail("nolab", "공장을 아직 만들지 않았어요"));
                }
                let l = lab_of(&data(&ls));
                if get(&l, "prod").is_truthy() {
                    return Err(fail("busy", "공장이 돌고 있어요. 다 만든 뒤에 더 살 수 있어요"));
                }
                let plv = num(&get(&l, "plv"));
                if plv >= 1000.0 {
                    return Err(fail("max", "공장을 더 살 수 없어요"));
                }
                if num(&get(&l, "cash")) < 50000.0 {
                    return Err(fail("poor", "회사 금고에 500 토큰이 부족해요. 먼저 금고에 넣어 주세요"));
                }
                m(&tx, "update", &[lref, o(&[("cash", n(num(&get(&l, "cash")) - 50000.0)), ("plv", n(plv + 1.0))])])?;
                Ok(n(plv + 1.0))
            })
        })
        .await,
    )
}
const FULL_H: f64 = 86400000.0;
const FULL_T: f64 = 64800000.0;
const FULL_E: f64 = 2592000000.0;
// 일하기 (직원·사장): 일한 만큼 에너지가 내려가고 회사 식품 성능이 올라요. 배고픔·목마름이 0이어도 일할 수 있어요
async fn staff_work(cid: String, k: JsValue) -> R {
    let k = if k.is_undefined() { n(1.0) } else { k };
    ensure_lab(cid.clone()).await?;
    rethrow(
        run_tx(move |tx| {
            let (cid, k) = (cid.clone(), k.clone());
            Box::pin(async move {
                let vref = d(&["vitals", &uid_of()?]);
                let lref = lab_ref(&cid);
                let vs = tx_get(&tx, &vref).await?;
                let ls = tx_get(&tx, &lref).await?;
                if !exists(&vs) {
                    return Err(fail("novit", "상태를 아직 만들지 않았어요"));
                }
                if !exists(&ls) {
                    return Err(fail("nolab", "회사를 아직 정하지 않았어요"));
                }
                let v = data(&vs);
                let t_now = now();
                let tt = (t_now - num(&get(&v, "at"))).max(0.0);
                let eff = tt; // 베타: 에너지가 0이어도 두 배로 닳지 않아요 (서버 규칙과 같아요)
                let h = (num(&get(&v, "h")) - eff * 100.0 / FULL_H).max(0.0);
                let t = (num(&get(&v, "t")) - eff * 100.0 / FULL_T).max(0.0);
                let e = num(&get(&v, "e")) - tt * 100.0 / FULL_E;
                let kn = num(&k);
                if e < kn {
                    return Err(fail("tired", "에너지가 모자라요. 먹고 쉬어 주세요"));
                }
                let next = o(&[("h", n(h)), ("t", n(t)), ("e", n(e - kn)), ("at", n(t_now)), ("wk", k.clone())]);
                m(&tx, "update", &[vref, next.clone()])?;
                m(&tx, "update", &[lref, o(&[("fperf", n(num(&get(&lab_of(&data(&ls)), "fperf")) + kn))])])?;
                let out = ext(&v, &[]);
                js_sys::Object::assign(out.unchecked_ref::<js_sys::Object>(), next.unchecked_ref::<js_sys::Object>());
                Ok(out)
            })
        })
        .await,
    )
}
async fn lab_food_make(cid: String, pid: String, k: JsValue) -> R {
    lab_start_factory(cid.clone(), pid, k).await?;
    loop {
        crate::dom::sleep(2000).await;
        let r = lab_settle_factory(cid.clone()).await?;
        if num(&get(&r, "left")) == 0.0 {
            return Ok(JsValue::UNDEFINED);
        }
    }
}

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
async fn take_out(cid: String, p: JsValue, k: JsValue, co_name: String) -> R {
    let pid = jstr(&get(&p, "id"));
    let iref = inv_ref(&cid, &pid)?;
    let is = rethrow(get_doc(iref.clone()).await)?;
    let kn = num(&k);
    let b = batch();
    m(&b, "update", &[d(&["companies", &cid, "products", &pid]), o(&[("stock", inc(-kn))])])?;
    if exists(&is) {
        m(&b, "update", &[iref, o(&[("n", inc(kn))])])?;
    } else {
        m(&b, "set", &[iref, inv_data(&p, &cid, &pid, kn, &co_name)])?;
    }
    rethrow(commit(&b).await)?;
    Ok(JsValue::UNDEFINED)
}
async fn list_inventory() -> R {
    let snap = rethrow(get_docs(q(c(&["inventory", &uid_of()?, "items"]), vec![w(s("n"), ">", n(0.0)), lim(100.0)])).await)?;
    Ok(docs(&snap).iter().map(|dd| ext(&data_est(dd), &[("id", get(dd, "id"))])).collect::<js_sys::Array>().into())
}
async fn food_gain(cid: String) -> R {
    let l = rethrow(get_doc(lab_ref(&cid)).await)?;
    let fp = if exists(&l) { get(&data(&l), "fperf") } else { JsValue::UNDEFINED };
    let perf = if exists(&l) && is_int(&fp) { num(&fp) } else { 100.0 };
    Ok(n(20.0 * perf / 100.0))
}
async fn eat_from_inventory(cid: String, pid: String, group: String, v: JsValue) -> R {
    let qv = num(&food_gain(cid.clone()).await?);
    let stat = match group.as_str() {
        "water" => "t",
        "snack" | "meal" => "h",
        "energy" => "e",
        _ => return Err(fail("bad", "먹을 수 없는 음식이에요")),
    };
    let t_now = now();
    let tt = (t_now - num(&get(&v, "at"))).max(0.0);
    let eff = tt; // 베타: 두 배로 닳지 않아요
    let left = [("h", (num(&get(&v, "h")) - eff * 100.0 / FULL_H).max(0.0)), ("t", (num(&get(&v, "t")) - eff * 100.0 / FULL_T).max(0.0)), ("e", (num(&get(&v, "e")) - tt * 100.0 / FULL_E).max(0.0))];
    let next: Vec<(&str, f64)> = left.iter().map(|(k, x)| (*k, if *k == stat { 100f64.min(x + qv) } else { *x })).collect();
    let (h, t, e) = (next[0].1, next[1].1, next[2].1);
    let eat = o(&[("cid", s(&cid)), ("pid", s(&pid))]);
    let b = batch();
    m(&b, "update", &[inv_ref(&cid, &pid)?, o(&[("n", inc(-1.0))])])?;
    m(&b, "update", &[d(&["vitals", &uid_of()?]), o(&[("h", n(h)), ("t", n(t)), ("e", n(e)), ("at", n(t_now)), ("eat", eat.clone()), ("wk", n(0.0))])])?;
    rethrow(commit(&b).await)?;
    Ok(o(&[("h", n(h)), ("t", n(t)), ("e", n(e)), ("at", n(t_now)), ("deaths", get(&v, "deaths")), ("eat", eat), ("gained", n(qv)), ("stat", s(stat))]))
}
async fn list_offers(kind: String) -> R {
    let all = list_companies(s("")).await?;
    let cos: Vec<JsValue> = all.dyn_into::<js_sys::Array>().map(|a| a.iter().filter(|cc| st(&get(cc, "kind")) == kind).take(20).collect()).unwrap_or_default();
    let labs = join_all(cos.iter().map(|cc| lab_get(st(&get(cc, "id")))).collect()).await;
    let out = js_sys::Array::new();
    for (cc, l) in cos.into_iter().zip(labs) {
        let l = l.unwrap_or(JsValue::NULL);
        let ok = l.is_truthy() && num(&get(&l, "price")) > 0.0 && if kind == "nuclear" { num(&get(&l, "elec")) > 0.0 } else { get(&l, "rel").is_truthy() };
        if ok {
            out.push(&o(&[("co", cc), ("L", l)]));
        }
    }
    Ok(out.into())
}
async fn list_shop() -> R {
    let companies = rethrow(list_companies(JsValue::UNDEFINED).await)?;
    let out = js_sys::Array::new();
    for co in companies.dyn_into::<js_sys::Array>().unwrap_or_default().iter() {
        let cid = st(&get(&co, "id"));
        let snap = rethrow(get_docs(q(c(&["companies", &cid, "products"]), vec![w(s("active"), "==", true.into()), lim(50.0)])).await)?;
        for dd in docs(&snap) {
            let v = data_est(&dd);
            let stock = get(&v, "stock");
            if get(&v, "paused") == JsValue::TRUE || !is_int(&get(&v, "priceCents")) || !(is_int(&stock) && num(&stock) > 0.0) {
                continue;
            }
            let or = |k: &str, dflt: &str| {
                let x = get(&co, k);
                if x.is_truthy() {
                    x
                } else {
                    s(dflt)
                }
            };
            out.push(&ext(
                &v,
                &[
                    ("id", get(&dd, "id")),
                    ("cid", s(&cid)),
                    ("stock", stock.clone()),
                    ("priceCents", get(&v, "priceCents")),
                    ("at", n(ms(&get(&v, "at")))),
                    ("co", o(&[("id", s(&cid)), ("name", or("name", "회사")), ("logo", or("logo", "")), ("kind", or("kind", "food"))])),
                ],
            ));
        }
    }
    Ok(out.into())
}
fn js_round(v: f64) -> f64 {
    (v + 0.5).floor()
}
async fn list_products(cid: String) -> R {
    let snap = rethrow(get_docs(q(c(&["companies", &cid, "products"]), vec![w(s("active"), "==", true.into()), lim(50.0)])).await)?;
    Ok(docs(&snap)
        .iter()
        .map(|dd| {
            let v = data_est(dd);
            // 예전 상품은 토큰 단위(cost·price)로 적혀 있어서 센트로 바꿔 보여줘요
            let cents = |ck: &str, tk: &str| {
                let x = get(&v, ck);
                if is_int(&x) {
                    x
                } else {
                    let t = get(&v, tk);
                    n(js_round(if t.is_truthy() { num(&t) } else { 0.0 } * 100.0))
                }
            };
            let stock = get(&v, "stock");
            ext(
                &v,
                &[
                    ("id", get(dd, "id")),
                    ("at", n(ms(&get(&v, "at")))),
                    ("costCents", cents("costCents", "cost")),
                    ("priceCents", cents("priceCents", "price")),
                    ("paused", (get(&v, "paused") == JsValue::TRUE).into()),
                    ("stock", if is_int(&stock) { stock } else { n(0.0) }),
                ],
            )
        })
        .collect::<js_sys::Array>()
        .into())
}
async fn create_product(cid: String, p: JsValue) -> R {
    let r = new_id(c(&["companies", &cid, "products"]));
    let all = ext(&p, &[("owner", s(&uid_of()?)), ("active", true.into()), ("at", ts())]);
    let clean = js_sys::Object::new();
    for e in js_sys::Object::entries(all.unchecked_ref()).iter() {
        let e: js_sys::Array = e.unchecked_into();
        if !e.get(1).is_undefined() {
            let _ = js_sys::Reflect::set(&clean, &e.get(0), &e.get(1));
        }
    }
    rethrow(set_doc(r.clone(), clean.into()).await)?;
    Ok(get(&r, "id"))
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

async fn load_vitals() -> R {
    let r = d(&["vitals", &uid_of()?]);
    let sn = rethrow(get_doc(r.clone()).await)?;
    if exists(&sn) {
        return Ok(data(&sn));
    }
    let v = o(&[("h", n(100.0)), ("t", n(100.0)), ("e", n(100.0)), ("at", n(now())), ("deaths", n(0.0))]);
    rethrow(set_doc(r, v.clone()).await)?;
    Ok(v)
}
async fn die(deaths: JsValue) -> R {
    let v = o(&[("h", n(10.0)), ("t", n(10.0)), ("e", n(10.0)), ("at", n(now() + num(&deaths) * 3600000.0)), ("deaths", deaths), ("wk", n(0.0))]);
    rethrow(update_doc(d(&["vitals", &uid_of()?]), v.clone()).await)?;
    Ok(v)
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
async fn pay_request(cid: String, to: JsValue, req: String, amount: JsValue) -> R {
    let uid = uid_of()?;
    let a = num(&amount);
    let gref = new_id(c(&["chats", &cid, "messages"]));
    let b = batch();
    m(&b, "update", &[d(&["chats", &cid, "messages", &req]), o(&[("done", true.into())])])?;
    m(&b, "set", &[gref.clone(), o(&[("type", s("gift")), ("amount", amount.clone()), ("done", false.into()), ("from", s(&uid)), ("to", to), ("at", ts())])])?;
    m(&b, "update", &[d(&["chats", &cid]), o(&[("last", s(&format!("토큰 {} 선물", ko(a)))), ("lastAt", ts()), ("lastFrom", s(&uid))])])?;
    m(&b, "update", &[wallet_ref()?, o(&[("bal", inc(-a)), ("wt", ts()), ("op", o(&[("kind", s("gift")), ("cid", s(&cid)), ("mid", get(&gref, "id"))]))])])?;
    commit(&b).await?;
    Ok(JsValue::UNDEFINED)
}

/* ---------- 통화 (연결 정보만 주고받아요) ---------- */

async fn call_start(to: String, chat: String, offer: JsValue) -> R {
    let r = new_id(c(&["calls"]));
    rethrow(set_doc(r.clone(), o(&[("from", s(&uid_of()?)), ("to", s(&to)), ("chat", s(&chat)), ("offer", offer), ("state", s("ringing")), ("at", ts())])).await)?;
    Ok(get(&r, "id"))
}
fn call_watch(id: String, cb: js_sys::Function) -> R {
    let next = Closure::<dyn FnMut(JsValue)>::new(move |sn: JsValue| {
        let v = if exists(&sn) { data_est(&sn) } else { JsValue::NULL };
        let _ = cb.call1(&JsValue::NULL, &v);
    })
    .into_js_value();
    f("onSnapshot", &[d(&["calls", &id]), next, report_fn()])
}
// 나에게 걸려 오는 전화 (울리는 것만)
fn call_incoming(cb: js_sys::Function) -> R {
    let next = Closure::<dyn FnMut(JsValue)>::new(move |snap: JsValue| {
        let list: js_sys::Array = docs(&snap)
            .iter()
            .map(|dd| {
                let v = data_est(dd);
                ext(&v, &[("id", get(dd, "id")), ("at", n(ms(&get(&v, "at"))))])
            })
            .collect();
        let _ = cb.call1(&JsValue::NULL, &list);
    })
    .into_js_value();
    f("onSnapshot", &[q(c(&["calls"]), vec![w(s("to"), "==", s(&uid_of()?)), w(s("state"), "==", s("ringing")), lim(5.0)]), next, report_fn()])
}
fn call_cands(id: String, cb: js_sys::Function) -> R {
    let me_ = uid_of()?;
    let next = Closure::<dyn FnMut(JsValue)>::new(move |snap: JsValue| {
        let Ok(ch) = m(&snap, "docChanges", &[]).and_then(|v| v.dyn_into::<js_sys::Array>().map_err(|e| e)) else { return };
        for x in ch.iter() {
            if st(&get(&x, "type")) != "added" {
                continue;
            }
            let v = data(&get(&x, "doc"));
            if st(&get(&v, "by")) == me_ {
                continue;
            }
            let _ = cb.call1(&JsValue::NULL, &get(&v, "c"));
        }
    })
    .into_js_value();
    f("onSnapshot", &[c(&["calls", &id, "c"]), next, report_fn()])
}

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
        ("at", n(ms(&get(&v, "at")))),
        ("parent", get(&v, "parent")),
        ("likes", n(count("likes"))),
        ("reposts", n(count("reposts"))),
        ("rc", { let r = get(&v, "rc"); if is_int(&r) { r } else { n(0.0) } }),
        ("topic", { let t = get(&v, "topic"); if t.is_string() { t } else { s("") } }),
        ("first", (get(&v, "first") == JsValue::TRUE).into()),
        ("sh", { let x = get(&v, "sh"); if is_int(&x) { x } else { n(0.0) } }),
        ("liked", mine("likes").into()),
        ("reposted", mine("reposts").into()),
    ])
}
/// 새 글 (parent가 있으면 그 글의 댓글이에요)
async fn post_create(text: String, img_: String, parent: String, topic: String) -> R {
    // 처음 쓰는 글이면 "첫 스레드" 표시를 붙여요
    let me_ = uid_of()?;
    let mine = get_docs(q(c(&["posts"]), vec![w(s("by"), "==", s(&me_)), lim(1.0)])).await.map(|sn| docs(&sn).len()).unwrap_or(1);
    let r = new_id(c(&["posts"]));
    let id = get(&r, "id");
    let data_ = o(&[("by", s(&uid_of()?)), ("text", s(&text)), ("at", ts()), ("likes", js_sys::Object::new().into()), ("reposts", js_sys::Object::new().into()), ("rc", n(0.0)), ("parent", s(&parent))]);
    if !img_.is_empty() {
        set(&data_, "img", &s(&img_));
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
    getter(&api, "user", || {
        let u = current();
        if u.is_null() || u.is_undefined() {
            return JsValue::NULL;
        }
        let dn = get(&u, "displayName");
        o(&[("id", s(&my_id())), ("name", if dn.is_truthy() { dn } else { s("") })])
    });
    /* ----- 계정 ----- */
    am!(api, "signUp", |a| sign_up(sa(&a[0]), sa(&a[1]), sa(&a[2]), opt_s(&a[3])).await);
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
    am!(api, "fetchProfiles", |a| fetch_profiles(a[0].clone()).await);
    am!(api, "searchUsers", |a| search_users(sa(&a[0])).await);
    am!(api, "listUsers", |a| list_users(a[0].is_truthy()).await);
    /* ----- 랭킹 ----- */
    am!(api, "syncRank", |a| sync_rank(a[0].clone()).await);
    am!(api, "loadRanks", |a| load_ranks(a[0].is_truthy()).await);
    am!(api, "countAbove", |a| count_of(q(c(&["ranks"]), vec![w(s("bal"), ">", a[0].clone()), lim(50.0)])).await);
    am!(api, "loadRanksOf", |a| load_ranks_of(a[0].clone()).await);
    /* ----- 회사 ----- */
    am!(api, "listMyCompanies", |_a| list_my_companies().await);
    am!(api, "getCompany", |a| get_company(sa(&a[0])).await);
    am!(api, "listCompanies", |a| list_companies(a[0].clone()).await);
    am!(api, "setOpenings", |a| rethrow(update_doc(d(&["companies", &sa(&a[0])]), o(&[("open", a[1].clone())])).await));
    am!(api, "setCompanyKind", |a| rethrow(update_doc(d(&["companies", &sa(&a[0])]), o(&[("kind", a[1].clone()), ("kindSet", true.into())])).await));
    am!(api, "loadMyJob", |_a| load_my_job().await);
    am!(api, "applyJob", |a| apply_job(sa(&a[0]), a[1].clone()).await);
    am!(api, "quitJob", |a| quit_job(sa(&a[0])).await);
    am!(api, "foundCompany", |a| found_company(a[0].clone(), a[1].clone(), a[2].clone()).await);
    /* ----- 연구실·공장 ----- */
    am!(api, "labGet", |a| lab_get(sa(&a[0])).await);
    am!(api, "labChange", |a| {
        let fun: js_sys::Function = a[1].clone().dyn_into()?;
        lab_tx(
            sa(&a[0]),
            Rc::new(move |l| {
                let v = fun.call1(&JsValue::NULL, &l).unwrap_or(JsValue::NULL);
                if v.is_truthy() {
                    Some(v)
                } else {
                    None
                }
            }),
        )
        .await
    });
    am!(api, "labGenStart", |a| lab_gen_start(sa(&a[0]), a[1].clone()).await);
    am!(api, "labCash", |a| lab_cash(sa(&a[0]), a[1].clone(), a[2].clone()).await);
    am!(api, "labBuy", |a| lab_buy(sa(&a[0]), sa(&a[1]), sa(&a[2]), a[3].clone()).await);
    am!(api, "labTrainStart", |a| lab_train_start(sa(&a[0]), a[1].clone()).await);
    am!(api, "labTrainClaim", |a| lab_train_claim(sa(&a[0])).await);
    am!(api, "ensureLab", |a| ensure_lab(sa(&a[0])).await);
    am!(api, "labStartFactory", |a| lab_start_factory(sa(&a[0]), sa(&a[1]), a[2].clone()).await);
    am!(api, "labSettleFactory", |a| lab_settle_factory(sa(&a[0])).await);
    am!(api, "labBuyFactory", |a| lab_buy_factory(sa(&a[0])).await);
    am!(api, "labGenClaim", |a| lab_gen_claim(sa(&a[0])).await);
    am!(api, "staffWork", |a| staff_work(sa(&a[0]), a[1].clone()).await);
    am!(api, "labFoodMake", |a| lab_food_make(sa(&a[0]), sa(&a[1]), a[2].clone()).await);
    /* ----- 상점·인벤토리 ----- */
    am!(api, "shopBuy", |a| shop_buy(sa(&a[0]), sa(&a[1]), a[2].clone(), opt_s(&a[3])).await);
    am!(api, "takeOut", |a| take_out(sa(&a[0]), a[1].clone(), a[2].clone(), opt_s(&a[3])).await);
    am!(api, "listInventory", |_a| list_inventory().await);
    am!(api, "foodGain", |a| food_gain(sa(&a[0])).await);
    am!(api, "eatFromInventory", |a| eat_from_inventory(sa(&a[0]), sa(&a[1]), sa(&a[2]), a[3].clone()).await);
    /* ----- 관리·공지 ----- */
    am!(api, "searchProfiles", |a| search_profiles(sa(&a[0])).await);
    am!(api, "listOffers", |a| list_offers(sa(&a[0])).await);
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
    am!(api, "listShop", |_a| list_shop().await);
    am!(api, "listProducts", |a| list_products(if a[0].is_undefined() { uid_of()? } else { sa(&a[0]) }).await);
    am!(api, "createProduct", |a| create_product(sa(&a[0]), a[1].clone()).await);
    am!(api, "renameProduct", |a| rethrow(update_doc(d(&["companies", &sa(&a[0]), "products", &sa(&a[1])]), o(&[("name", a[2].clone())])).await));
    am!(api, "setProductPaused", |a| rethrow(update_doc(d(&["companies", &sa(&a[0]), "products", &sa(&a[1])]), o(&[("paused", (a[2] == JsValue::TRUE).into())])).await));
    am!(api, "endProduct", |a| rethrow(update_doc(d(&["companies", &sa(&a[0]), "products", &sa(&a[1])]), o(&[("active", false.into())])).await));
    /* ----- 상태·지갑 ----- */
    am!(api, "loadVitals", |_a| load_vitals().await);
    am!(api, "die", |a| die(a[0].clone()).await);
    am!(api, "setPhoto", |a| set_photo(a[0].clone()).await);
    method_now(&api, "watchWallet", |a| watch_wallet(a[0].clone().dyn_into()?));
    am!(api, "claimInterest", |a| update_doc(wallet_ref()?, o(&[("bal", inc(num(&a[0]))), ("t", from_millis(num(&a[1]))), ("wt", ts()), ("op", o(&[("kind", s("interest"))]))])).await);
    am!(api, "buyProduct", |a| update_doc(wallet_ref()?, o(&[("rate", a[0].clone()), ("bal", inc(-num(&a[1]))), ("t", ts()), ("wt", ts()), ("op", o(&[("kind", s("buy"))]))])).await);
    /* ----- 대화 ----- */
    am!(api, "openChat", |a| open_chat(sa(&a[0])).await);
    am!(api, "createGroup", |a| create_group(a[0].clone(), a[1].clone()).await);
    am!(api, "inviteMembers", |a| invite_members(sa(&a[0]), a[1].clone()).await);
    am!(api, "nameOf", |a| name_of(sa(&a[0])).await);
    method_now(&api, "watchChats", |a| watch_chats(a[0].clone().dyn_into()?));
    method_now(&api, "watchMessages", |a| watch_messages(sa(&a[0]), a[1].clone().dyn_into()?, a[2].clone()));
    am!(api, "loadOlder", |a| load_older(sa(&a[0]), a[1].clone(), a[2].clone()).await);
    am!(api, "send", |a| send(sa(&a[0]), a[1].clone(), a[2].clone(), a[3].clone(), None).await);
    am!(api, "sendGift", |a| {
        let amt = num(&a[2]);
        send(sa(&a[0]), a[1].clone(), o(&[("type", s("gift")), ("amount", a[2].clone()), ("done", false.into())]), s(&format!("토큰 {} 선물", ko(amt))), Some(amt)).await
    });
    am!(api, "claimGift", |a| claim_gift(sa(&a[0]), sa(&a[1]), a[2].clone()).await);
    am!(api, "payRequest", |a| pay_request(sa(&a[0]), a[1].clone(), sa(&a[2]), a[3].clone()).await);
    am!(api, "vote", |a| {
        let k = format!("votes.{}", uid_of()?);
        let x = js_sys::Object::new();
        set(&x, &k, &a[2]);
        update_doc(d(&["chats", &sa(&a[0]), "messages", &sa(&a[1])]), x.into()).await
    });
    /* ----- 통화 ----- */
    am!(api, "callStart", |a| call_start(sa(&a[0]), sa(&a[1]), a[2].clone()).await);
    am!(api, "callAnswer", |a| update_doc(d(&["calls", &sa(&a[0])]), o(&[("answer", a[1].clone()), ("state", s("active"))])).await);
    am!(api, "callEnd", |a| Ok(update_doc(d(&["calls", &sa(&a[0])]), o(&[("state", s("ended"))])).await.unwrap_or(JsValue::UNDEFINED)));
    am!(api, "callCand", |a| set_doc(new_id(c(&["calls", &sa(&a[0]), "c"])), o(&[("by", s(&uid_of()?)), ("c", a[1].clone())])).await);
    method_now(&api, "callWatch", |a| call_watch(sa(&a[0]), a[1].clone().dyn_into()?));
    method_now(&api, "callIncoming", |a| call_incoming(a[0].clone().dyn_into()?));
    method_now(&api, "callCands", |a| call_cands(sa(&a[0]), a[1].clone().dyn_into()?));
    /* ----- SNS ----- */
    am!(api, "postCreate", |a| post_create(opt_s(&a[0]), opt_s(&a[1]), opt_s(&a[2]), opt_s(&a[3])).await);
    am!(api, "postShare", |a| rethrow(update_doc(d(&["posts", &sa(&a[0])]), o(&[("sh", inc(1.0))])).await));
    // 팔로우: 내 설정 문서의 follows (사람 번호 → true)
    am!(api, "myFollows", |_a| {
        let sn = get_doc(d(&["users", &uid_of()?])).await?;
        let f_ = get(&data(&sn), "follows");
        Ok(if f_.is_object() { js_sys::Object::keys(f_.unchecked_ref::<js_sys::Object>()).into() } else { js_sys::Array::new().into() })
    });
    am!(api, "follow", |a| {
        let x = js_sys::Object::new();
        set(&x, &sa(&a[0]), &if a[1].is_truthy() { JsValue::TRUE } else { f("deleteField", &[])? });
        rethrow(set_merge(d(&["users", &uid_of()?]), o(&[("follows", x.into())])).await)
    });
    method_now(&api, "postsWatch", |a| posts_watch(a[0].clone().dyn_into()?));
    am!(api, "postReplies", |a| post_replies(sa(&a[0])).await);
    am!(api, "postGet", |a| post_get(sa(&a[0])).await);
    am!(api, "postLike", |a| post_mark(sa(&a[0]), "likes", a[1].is_truthy()).await);
    am!(api, "postRepost", |a| post_mark(sa(&a[0]), "reposts", a[1].is_truthy()).await);
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
