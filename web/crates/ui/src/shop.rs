//! 상점: 재고가 있는 음식을 여러 회사에서 모아 보여주고, 골라서 사요. 산 돈은 파는 회사 금고로 들어가요.
//! 식품 · 물건 · 자원 칸이 있어요 (인공지능은 팔지 않아요). 예전 `app/shop.js`와 같은 class예요.

use std::cell::{Cell, RefCell};
use std::time::Duration;

use cloud::{fmt_num, tok, Offer, ShopItem};
use leptos::mount::mount_to;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::{JsCast, JsValue};

use crate::ctx::Ctx;
use crate::parts::{note, note_retry, skeleton};

const MAX_N: i64 = 100;
const CATS: [(&str, &str); 3] = [("food", "식품"), ("acc", "장신구"), ("trade", "거래")];

#[derive(Clone)]
enum State {
    Out,
    Loading,
    Food(Vec<ShopItem>),
    Offers(Vec<(bool, Offer)>), // (컴퓨터인지, 내놓은 것)
    Soon,
    Failed,
}

#[derive(Clone, Copy)]
struct Sig {
    state: RwSignal<State, LocalStorage>,
    cat: RwSignal<&'static str>,
    dir: RwSignal<i32>,
    phase: RwSignal<&'static str>,
    busy: RwSignal<bool>,
}

thread_local! {
    static CTX: RefCell<Ctx> = RefCell::new(Ctx::default());
    static SIG: Cell<Option<Sig>> = const { Cell::new(None) };
    static SEQ: Cell<u32> = const { Cell::new(0) };
    static SWAP: Cell<u32> = const { Cell::new(0) };
}

fn ctx() -> Ctx {
    CTX.with(|c| c.borrow().clone())
}

fn cat_index(k: &str) -> usize {
    CATS.iter().position(|(c, _)| *c == k).unwrap_or(0)
}

/// 살 때 내는 토큰: 센트는 올림해서 토큰으로 내요
fn cost_of(cents: f64, n: i64) -> i64 {
    ((cents * n as f64) / 100.0).ceil() as i64
}

pub fn init(c: Ctx) {
    CTX.with(|x| *x.borrow_mut() = c);
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return };
    let Some(root) = doc.get_element_by_id("shop-root") else { return };
    let Ok(root) = root.dyn_into::<web_sys::HtmlElement>() else { return };
    root.set_inner_html("");
    mount_to(root, || {
        let sig = Sig {
            state: RwSignal::new_local(State::Loading),
            cat: RwSignal::new("food"),
            dir: RwSignal::new(1),
            phase: RwSignal::new(""),
            busy: RwSignal::new(false),
        };
        SIG.set(Some(sig));
        view! { <Shop sig/> }
    })
    .forget();
}

/// 지금 칸의 목록을 서버에서 다시 읽어요. 끝나면 true
async fn load(sig: Sig) -> bool {
    let my = SEQ.get() + 1;
    SEQ.set(my);
    if !cloud::signed_in() {
        sig.state.set(State::Out);
        return true;
    }
    let cat = sig.cat.get_untracked();
    let next = if cat == "food" {
        cloud::list_shop().await.map(|mut l| {
            l.sort_by(|a, b| a.co_name.cmp(&b.co_name).then(a.name.cmp(&b.name)));
            State::Food(l)
        })
    } else if cat == "acc" {
        Ok(State::Soon)
    } else {
        // 거래: 회사들이 내놓은 컴퓨터와 전기를 한데 모아요
        async {
            let mut all: Vec<(bool, Offer)> = cloud::list_offers("pc").await?.into_iter().map(|o| (true, o)).collect();
            all.extend(cloud::list_offers("nuclear").await?.into_iter().map(|o| (false, o)));
            all.sort_by(|a, b| a.1.co_name.cmp(&b.1.co_name));
            Ok(State::Offers(all))
        }
        .await
    };
    if SEQ.get() != my {
        return false;
    }
    sig.state.set(next.unwrap_or_else(|e| {
        cloud::warn(&format!("상점을 불러오지 못했어요: {e}"));
        State::Failed
    }));
    true
}

pub fn refresh() {
    if let Some(sig) = SIG.get() {
        spawn_local(async move {
            load(sig).await;
        });
    }
}

/// 칸을 바꿀 때: 칸(알약)은 그대로, 아래 내용만 살짝 치웠다가 새 내용을 밀어 올려요
fn set_cat(sig: Sig, next: &'static str) {
    if sig.cat.get_untracked() == next {
        return;
    }
    let my = SWAP.get() + 1;
    SWAP.set(my);
    sig.dir.set(if cat_index(next) > cat_index(sig.cat.get_untracked()) { 1 } else { -1 });
    sig.cat.set(next);
    sig.phase.set("pane-out");
    set_timeout(
        move || {
            spawn_local(async move {
                if SWAP.get() != my {
                    return;
                }
                load(sig).await;
                if SWAP.get() == my {
                    sig.phase.set("pane-in");
                }
            });
        },
        Duration::from_millis(170),
    );
}

fn buy(sig: Sig, p: ShopItem, n: i64) {
    let c = ctx();
    if !cloud::signed_in() {
        return c.toast("로그인하면 살 수 있어요");
    }
    if sig.busy.get_untracked() {
        return;
    }
    sig.busy.set(true);
    spawn_local(async move {
        let (cid, id, co) = (p.cid.clone(), p.id.clone(), p.co_name.clone());
        // 카드를 리더기에 긁는 연출과 결제를 함께 해요
        let amount = format!("{} 토큰", fmt_num(cost_of(p.price_cents, n) as f64, 0));
        let got = crate::pay::swipe(
            &amount,
            c.wallet_op(move || {
                cloud::call_now("shopBuy", &[JsValue::from_str(&cid), JsValue::from_str(&id), JsValue::from(n as f64), JsValue::from_str(&co)])
                    .unwrap_or_else(|e| js_sys::Promise::reject(&JsValue::from_str(&e.message)).into())
            }),
        )
        .await;
        match got {
            Ok(paid) => {
                c.toast(&format!("{} {}개를 샀어요. 인벤토리에 넣었어요", p.name, n));
                let entry = js_sys::Object::new();
                for (k, v) in [
                    ("t", JsValue::from_f64(js_sys::Date::now())),
                    ("name", JsValue::from_str(&format!("{} {}개", p.name, n))),
                    ("cat", JsValue::from_str("상점")),
                    ("icon", JsValue::from_str("store")),
                    ("amt", JsValue::from_f64(-paid.as_f64().unwrap_or(0.0))),
                ] {
                    let _ = js_sys::Reflect::set(&entry, &JsValue::from_str(k), &v);
                }
                c.log(&entry);
            }
            Err(e) => c.toast(if e.code == "permission-denied" { "지금은 살 수 없어요. 잠시 뒤에 다시 해 주세요" } else if e.message.is_empty() { "사지 못했어요. 잠시 뒤에 다시 해 주세요" } else { &e.message }),
        }
        sig.busy.set(false);
        load(sig).await; // 재고와 목록을 서버에서 다시 읽어요
    });
}

#[component]
fn Shop(sig: Sig) -> impl IntoView {
    view! {
        <div class="shop-chips">
            <div class="gseg shop-cats" role="radiogroup" aria-label="상점 칸" style=move || format!("--n:{};--i:{}", CATS.len(), cat_index(sig.cat.get()))>
                {CATS.iter().map(|(k, t)| {
                    let k: &'static str = k;
                    view! {
                        <button type="button" role="radio" aria-checked=move || (sig.cat.get() == k).to_string() data-cat=k on:click=move |_| set_cat(sig, k)>
                            {*t}
                        </button>
                    }
                }).collect_view()}
            </div>
        </div>
        <div class=move || format!("shop-pane {}", sig.phase.get()) style=move || format!("--dir:{}", sig.dir.get())>
            {move || match sig.state.get() {
                State::Loading => skeleton(4),
                State::Out => note("로그인이 필요해요", "로그인하면 살 수 있어요", "식품 회사가 만든 음식을 여기서 살 수 있어요. 산 돈은 파는 회사의 금고로 들어가요."),
                State::Failed => note_retry("잠깐만요", "상점을 불러오지 못했어요", "인터넷을 확인하고 다시 해 주세요.", "shop-retry", refresh),
                State::Soon => note("장신구", "곧 들어와요", "반지·목걸이 같은 장신구를 여기서 사고팔 수 있게 준비하고 있어요."),
                State::Offers(list) => {
                    if list.is_empty() {
                        note(
                            "지금 거래할 물건이 없어요",
                            "회사가 시장에 내놓으면 여기에 나와요",
                            "지금은 인공지능 회사가 사업 탭의 시장에서 사요. 사람이 직접 사는 기능은 아직 없어요.",
                        )
                    } else {
                        view! {
                            <h2 class="section-title">"회사가 내놓은 것"</h2>
                            <p class="lab-sub shop-note-line">"지금은 인공지능 회사가 사업 탭의 시장에서 사요. 사람이 직접 사는 기능은 아직 없어요."</p>
                            <div class="shop-grid">
                                {list.into_iter().enumerate().map(|(i, (pc, o))| view! {
                                    <article class="card shop-card offer-card" style=format!("--i:{i}")>
                                        <div class="shop-body">
                                            <b class="shop-name">{if pc { "컴퓨터" } else { "전기" }}</b>
                                            <span class="shop-co">{if o.co_name.is_empty() { "회사".to_string() } else { o.co_name.clone() }}</span>
                                            <span class="shop-stock">{format!("1개 {} 토큰 · {}", tok(o.price), if pc { "컴퓨터".to_string() } else { format!("전기 {}", fmt_num(o.elec, 0)) })}</span>
                                        </div>
                                    </article>
                                }).collect_view()}
                            </div>
                        }
                        .into_any()
                    }
                }
                State::Food(list) if list.is_empty() => view! {
                    <div class="card shop-note">
                        <div class="art mint"></div>
                        <div class="art-text">
                            <p class="eyebrow">"아직 진열된 음식이 없어요"</p>
                            <h2 class="card-title">"음식이 만들어지면 여기에 나와요"</h2>
                            <p class="card-body">"식품 회사의 사장님이나 직원이 음식을 만들면 상점에 진열돼요."</p>
                        </div>
                    </div>
                }
                .into_any(),
                State::Food(list) => view! {
                    <h2 class="section-title">"지금 살 수 있어요"</h2>
                    <div class="shop-grid">
                        {list.into_iter().enumerate().map(|(i, p)| view! { <Card sig p i/> }).collect_view()}
                    </div>
                }
                .into_any(),
            }}
        </div>
    }
}

fn icon(d: &'static str) -> impl IntoView {
    view! {
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d=d/>
        </svg>
    }
}

#[component]
fn Card(sig: Sig, p: ShopItem, i: usize) -> impl IntoView {
    let art = ctx().pkv_with(&p.raw, false, &p.co_raw);
    let most = p.stock.clamp(1, MAX_N);
    let n = RwSignal::new(1i64);
    let cents = p.price_cents;
    let key = format!("{}/{}", p.cid, p.id);
    let item = p.clone();
    view! {
        <article class="card shop-card" style=format!("--i:{i}") data-key=key>
            <div class="shop-art" inner_html=art></div>
            <div class="shop-body">
                <b class="shop-name">{p.name.clone()}</b>
                <span class="shop-co">{p.co_name.clone()}</span>
                <span class="shop-price">{format!("1개 {} 토큰", tok(cents))}</span>
                <span class="shop-stock">{format!("재고 {}개", p.stock)}</span>
                <div class="shop-step" role="group" aria-label="살 개수">
                    <button type="button" class="shop-round" data-d="-1" aria-label="한 개 줄이기" disabled=move || { n.get() <= 1 } on:click=move |_| n.update(|v| *v = (*v - 1).max(1))>
                        {icon("M5 12h14")}
                    </button>
                    <output class="shop-n" aria-live="polite">{move || n.get()}</output>
                    <button type="button" class="shop-round" data-d="1" aria-label="한 개 늘리기" disabled=move || { n.get() >= most } on:click=move |_| n.update(|v| *v = (*v + 1).min(most))>
                        {icon("M12 5v14M5 12h14")}
                    </button>
                </div>
                <button type="button" class="btn primary shop-buy" data-buy="1" class:busy=move || sig.busy.get() on:click=move |_| buy(sig, item.clone(), n.get_untracked())>
                    {move || format!("{} 토큰에 사기", fmt_num(cost_of(cents, n.get()) as f64, 0))}
                </button>
            </div>
        </article>
    }
}
