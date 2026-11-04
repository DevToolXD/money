//! 결제 연출: 카드가 반짝이며 빙글 돌아 나타나서(변신하듯), 리더기에 "쓰윽" 긁히고, 결과에 따라 초록 체크나 빨간 표시가 떠요.
//! 실제 결제(서버)는 연출과 동시에 진행하고, 둘 다 끝나야 결과를 보여줘요.

use std::future::Future;
use wasm_bindgen::JsCast;

use crate::dom;

const CHECK: &str = r#"<path d="M5 12.5l4.5 4.5L19 7.5"/>"#;
const CROSS: &str = r#"<path d="M7 7l10 10M17 7L7 17"/>"#;

fn svg(d: &str) -> String {
    format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{d}</svg>"#)
}

/// 카드를 긁으면서 job을 해요. job의 결과를 그대로 돌려줘요
pub async fn swipe<T, E>(amount: &str, job: impl Future<Output = Result<T, E>>) -> Result<T, E> {
    let el = show(amount);
    let anim = dom::sleep(if dom::reduce_motion() { 200 } else { 1650 });
    let (r, _) = futures_join(job, anim).await;
    if let Some(el) = el {
        let _ = el.class_list().add_1(if r.is_ok() { "ok" } else { "no" });
        dom::sleep(if r.is_ok() { 650 } else { 900 }).await;
        let _ = el.class_list().add_1("bye");
        dom::sleep(260).await;
        el.remove();
    }
    r
}

/// 두 일을 함께 기다려요
pub(crate) async fn futures_join<A: Future, B: Future>(a: A, b: B) -> (A::Output, B::Output) {
    use std::cell::RefCell;
    use std::rc::Rc;
    let ra: Rc<RefCell<Option<A::Output>>> = Rc::new(RefCell::new(None));
    let rb: Rc<RefCell<Option<B::Output>>> = Rc::new(RefCell::new(None));
    let (ra2, rb2) = (ra.clone(), rb.clone());
    let fa = async move { *ra2.borrow_mut() = Some(a.await) };
    let fb = async move { *rb2.borrow_mut() = Some(b.await) };
    Join { a: Box::pin(fa), b: Box::pin(fb), da: false, db: false }.await;
    let a = ra.borrow_mut().take().expect("a");
    let b = rb.borrow_mut().take().expect("b");
    (a, b)
}
struct Join<'a> {
    a: std::pin::Pin<Box<dyn Future<Output = ()> + 'a>>,
    b: std::pin::Pin<Box<dyn Future<Output = ()> + 'a>>,
    da: bool,
    db: bool,
}
impl Future for Join<'_> {
    type Output = ();
    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        if !self.da && self.a.as_mut().poll(cx).is_ready() {
            self.da = true;
        }
        if !self.db && self.b.as_mut().poll(cx).is_ready() {
            self.db = true;
        }
        if self.da && self.db {
            std::task::Poll::Ready(())
        } else {
            std::task::Poll::Pending
        }
    }
}

fn show(amount: &str) -> Option<web_sys::HtmlElement> {
    let app = dom::app_el()?;
    let el: web_sys::HtmlElement = dom::doc().create_element("div").ok()?.dyn_into().ok()?;
    el.set_class_name("pay");
    let _ = el.set_attribute("role", "status");
    let _ = el.set_attribute("aria-label", "결제 중");
    let sparks: String = (0..10).map(|i| format!(r#"<i class="pay-spark" style="--a:{}deg;--d:{}ms"></i>"#, i * 36, i * 40)).collect();
    el.set_inner_html(&format!(
        r#"<div class="pay-stage"><div class="pay-ring" aria-hidden="true"></div>{sparks}<div class="pay-card"><span class="pay-chip"></span><span class="pay-brand">MONEY SIM</span><span class="pay-num">•••• •••• •••• 2026</span><span class="pay-amt">{}</span></div><div class="pay-reader"><span class="pay-slot"></span><span class="pay-light"></span><span class="pay-res ok">{}</span><span class="pay-res no">{}</span></div></div><p class="pay-t">결제하는 중이에요</p>"#,
        dom::esc(amount),
        svg(CHECK),
        svg(CROSS)
    ));
    let _ = app.append_child(&el);
    let _ = el.offset_width();
    let _ = el.class_list().add_1("go");
    dom::vibrate(10);
    let e2 = el.clone();
    // 긁히는 순간 짧게 떨려요
    wasm_bindgen_futures::spawn_local(async move {
        dom::sleep(1050).await;
        if e2.is_connected() {
            dom::vibrate(18);
        }
    });
    Some(el)
}
