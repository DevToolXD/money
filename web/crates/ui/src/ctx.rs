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
