//! 예전 JS 모듈이 init(ctx)로 받던 도움 묶음(ctx)을 감싸요.
//! ctx는 app.js가 만들어요: toast, pkv(상품 그림), vitals(상태) 같은 함수가 들어 있어요.

use js_sys::{Array, Function, Reflect};
use wasm_bindgen::{JsCast, JsValue};

#[derive(Clone, Default)]
pub struct Ctx(pub JsValue);

impl Ctx {
    /// `ctx.a.b(...)`처럼 점으로 이어진 이름의 함수를 불러요 (없거나 실패하면 Err)
    pub fn call(&self, path: &str, args: &[JsValue]) -> Result<JsValue, JsValue> {
        let mut this = self.0.clone();
        let mut parts = path.split('.').peekable();
        while let Some(p) = parts.next() {
            let next = Reflect::get(&this, &JsValue::from_str(p))?;
            if parts.peek().is_none() {
                let f: Function = next.dyn_into()?;
                let list = Array::new();
                for a in args {
                    list.push(a);
                }
                return f.apply(&this, &list);
            }
            this = next;
        }
        Err(JsValue::from_str("빈 이름"))
    }

    /// 화면 아래 알림
    pub fn toast(&self, msg: &str) {
        let _ = self.call("toast", &[JsValue::from_str(msg)]);
    }

    /// 상품 그림 (JS의 Business.pkv가 만드는 SVG 글자). 실패하면 빈 글자
    pub fn pkv(&self, raw: &JsValue, mini: bool, co_name: &str) -> String {
        let co = js_sys::Object::new();
        let _ = Reflect::set(&co, &JsValue::from_str("name"), &JsValue::from_str(co_name));
        let _ = Reflect::set(&co, &JsValue::from_str("logo"), &JsValue::from_str(""));
        self.call("pkv", &[raw.clone(), JsValue::from_bool(mini), co.into()]).ok().and_then(|v| v.as_string()).unwrap_or_default()
    }
}

impl Ctx {
    /// 상품 그림 (회사 값을 그대로 넘겨요: 로고가 가운데에 들어가요)
    pub fn pkv_with(&self, raw: &JsValue, mini: bool, co: &JsValue) -> String {
        self.call("pkv", &[raw.clone(), JsValue::from_bool(mini), co.clone()]).ok().and_then(|v| v.as_string()).unwrap_or_default()
    }

    /// 지갑 쓰기 줄 세우기: 서버 규칙이 2초에 한 번만 받아서, app.js가 순서대로 보내요.
    /// job은 JS 약속을 돌려주는 함수예요 (예: cloud::call_now("shopBuy", ...))
    pub async fn wallet_op(&self, job: impl FnOnce() -> JsValue + 'static) -> Result<JsValue, cloud::CloudError> {
        let f = wasm_bindgen::closure::Closure::once_into_js(job);
        let p = self.call("walletOp", &[f]).map_err(|e| cloud::CloudError { code: "walletop".into(), message: format!("{e:?}") })?;
        cloud::settle(p).await
    }

    /// 돈 탭 기록에 한 줄 남겨요 (있을 때만)
    pub fn log(&self, entry: &JsValue) {
        let _ = self.call("log", &[entry.clone()]);
    }
}

impl Ctx {
    /// 지금 고른 내 회사 (undefined: 아직 읽는 중, null: 회사 없음)
    pub fn company(&self) -> JsValue {
        self.call("company", &[]).unwrap_or(JsValue::UNDEFINED)
    }

    /// 회사 마크 (아바타 HTML 글자)
    pub fn mark(&self, name: &str, logo: &str, cls: &str) -> String {
        self.call("mark", &[JsValue::from_str(name), JsValue::from_str(logo), JsValue::from_str(cls)]).ok().and_then(|v| v.as_string()).unwrap_or_default()
    }

    /// 확인 창 (예/아니요). 고르면 true
    pub async fn ask(&self, title: &str, body: &str, ok: &str, cancel: &str) -> bool {
        let o = js_sys::Object::new();
        for (k, v) in [("title", title), ("body", body), ("ok", ok), ("cancel", cancel)] {
            let _ = Reflect::set(&o, &JsValue::from_str(k), &JsValue::from_str(v));
        }
        match self.call("ask", &[o.into()]) {
            Ok(p) => cloud::settle(p).await.map(|v| v.is_truthy()).unwrap_or(false),
            Err(_) => false,
        }
    }
}

/// 화면 상태(배고픔·목마름·에너지)를 app.js에 알려요 (window.__vitals.set)
pub fn set_vitals(v: &JsValue) {
    let vit = Reflect::get(&js_sys::global(), &JsValue::from_str("__vitals")).unwrap_or(JsValue::UNDEFINED);
    if let Ok(f) = Reflect::get(&vit, &JsValue::from_str("set")).and_then(|f| f.dyn_into::<Function>()) {
        let _ = f.call1(&vit, v);
    }
}
