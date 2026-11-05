//! 직원 채용: 사람만 골라요 (인공지능 직원은 아직 막아 두었어요).
//! + − 로 정한 숫자가 곧 모집 인원이에요. 맞추기를 누르면 그 숫자로 맞춰져요 (숫자에는 제한이 없어요)

use std::cell::{Cell, RefCell};

use goods::catalog::{icon, svg_i};
use leptos::mount::mount_to;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::prelude::*;

use super::{co_str, company, ctx};
use crate::dom::{self, by_id};

#[derive(Clone, Copy)]
struct Hire {
    n: RwSignal<f64>,
    open: RwSignal<f64>,
    staff: RwSignal<f64>,
    hiring: RwSignal<bool>,
}
thread_local! {
    static H: Cell<Option<Hire>> = const { Cell::new(None) };
    static FOR: RefCell<String> = const { RefCell::new(String::new()) };
}

pub fn init() {
    let Some(root) = by_id("biz-hire") else { return };
    let h = Hire { n: RwSignal::new(0.0), open: RwSignal::new(0.0), staff: RwSignal::new(0.0), hiring: RwSignal::new(false) };
    H.set(Some(h));
    root.set_inner_html("");
    mount_to(root, move || view! { <HireCard h/> }).forget();
}

/// 회사 숫자(모집 중, 직원)를 다시 그려요. 회사를 바꾸면 지금 모집 인원부터 보여요
pub fn paint() {
    let (Some(h), Some(co)) = (H.get(), company()) else { return };
    let open = cloud::get(&co, "open").as_f64().unwrap_or(0.0);
    let id = co_str(&co, "id");
    if FOR.with(|f| *f.borrow() != id) {
        FOR.with(|f| *f.borrow_mut() = id);
        h.n.set(open);
    }
    h.n.update(|n| *n = n.max(0.0));
    h.open.set(open);
    h.staff.set(cloud::get(&co, "staff").as_f64().unwrap_or(0.0));
}

#[component]
fn HireCard(h: Hire) -> impl IntoView {
    let num = NodeRef::<leptos::html::Output>::new();
    let step = move |d: f64| {
        h.n.update(|n| *n = (*n + d).max(0.0));
        if let Some(el) = num.get_untracked() {
            dom::repop(&el);
        }
    };
    let post = move |_| {
        let Some(co) = company() else { return };
        if h.hiring.get_untracked() {
            return;
        }
        let n = h.n.get_untracked();
        h.hiring.set(true);
        spawn_local(async move {
            let c = ctx();
            match cloud::call("setOpenings", &[JsValue::from_str(&co_str(&co, "id")), JsValue::from(n)]).await {
                Ok(_) => {
                    let _ = c.call("patchCompany", &[dom::obj(&[("open", JsValue::from(n))])]);
                    c.toast(&format!("모집 인원을 {}명으로 맞췄어요", n as i64));
                }
                Err(e) => c.toast(&format!("모집 인원을 맞추지 못했어요 · {}", cloud::why(&e.code))),
            }
            h.hiring.set(false);
            paint();
        });
    };
    view! {
        <p class="hire-stat" id="hire-stat">{move || format!("모집 중 {}명 · 직원 {}명", h.open.get() as i64, h.staff.get() as i64)}</p>
        <div class="gseg hire-type" role="radiogroup" aria-label="직원 종류" style="--n:2;--i:0">
            <button type="button" role="radio" aria-checked="true">"사람"</button>
            <button type="button" role="radio" aria-checked="false" disabled>"인공지능"<span>"준비 중"</span></button>
        </div>
        <div class="hire-row">
            <p class="hire-label">"채용할 직원 수"</p>
            <div class="stepper" role="group" aria-label="채용할 직원 수">
                <button class="gg hire-btn" id="hire-minus" type="button" aria-label="한 명 줄이기" disabled=move || { h.n.get() <= 0.0 } inner_html=svg_i(icon("minus")) on:click=move |_| step(-1.0)></button>
                <output class="hire-num" id="hire-num" aria-live="polite" node_ref=num>{move || (h.n.get() as i64).to_string()}</output>
                <button class="gg hire-btn" id="hire-plus" type="button" aria-label="한 명 늘리기" inner_html=svg_i(icon("plus")) on:click=move |_| step(1.0)></button>
            </div>
        </div>
        <p class="hire-note" id="hire-note">"숫자에는 제한이 없어요. 정한 숫자가 그대로 모집 인원이 돼요"</p>
        <button class="btn primary" id="hire-go" type="button" class:busy=move || h.hiring.get() disabled=move || { h.hiring.get() || h.n.get() == h.open.get() } on:click=post>
            {move || format!("모집 인원 {}명으로 맞추기", h.n.get() as i64)}
        </button>
    }
}
