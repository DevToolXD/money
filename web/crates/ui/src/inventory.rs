//! 인벤토리: 상점에서 사거나 사장이 창고에서 꺼낸 음식. 골라서 먹기를 누르면 한 개가 줄고 상태가 올라가요.
//! 예전 `app/inv.js`와 같은 class를 써요 (화면 시험 tests/e2e/inv-ui.js).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use cloud::InvItem;
use leptos::mount::mount_to;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

use crate::ctx::Ctx;
use crate::parts::{note, note_retry, skeleton};

#[derive(Clone)]
enum State {
    Out,
    Loading,
    Ready(Vec<InvItem>),
    Failed,
}

#[derive(Clone, Copy)]
struct Sig {
    state: RwSignal<State, LocalStorage>,
    selected: RwSignal<Option<String>>,
    gains: RwSignal<HashMap<String, f64>>,
    /// 회사 번호 → 회사 (마크를 그리려고 한 번씩 읽어요)
    cos: RwSignal<HashMap<String, wasm_bindgen::JsValue>, LocalStorage>,
    busy: RwSignal<bool>,
}

thread_local! {
    static CTX: RefCell<Ctx> = RefCell::new(Ctx::default());
    static SIG: Cell<Option<Sig>> = const { Cell::new(None) };
    static SEQ: Cell<u32> = const { Cell::new(0) };
}

fn ctx() -> Ctx {
    CTX.with(|c| c.borrow().clone())
}

/// 음식 종류별로 오르는 상태 이름 (오르는 양은 회사 식품 성능으로 정해져요)
fn effect(group: &str) -> Option<&'static str> {
    match group {
        "water" => Some("목마름"),
        "snack" | "meal" => Some("배고픔"),
        "energy" => Some("에너지"),
        _ => None,
    }
}

fn fmt_gain(v: f64) -> String {
    format!("{v}")
}

pub fn init(c: Ctx) {
    CTX.with(|x| *x.borrow_mut() = c);
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return };
    let Some(root) = doc.get_element_by_id("inv-root") else { return };
    let Ok(root) = root.dyn_into::<web_sys::HtmlElement>() else { return };
    root.set_inner_html("");
    mount_to(root, || {
        let sig = Sig {
            state: RwSignal::new_local(State::Loading),
            selected: RwSignal::new(None),
            gains: RwSignal::new(HashMap::new()),
            cos: RwSignal::new_local(HashMap::new()),
            busy: RwSignal::new(false),
        };
        SIG.set(Some(sig));
        view! { <Inventory sig/> }
    })
    .forget();
}

/// 서버에서 다시 읽어요
pub fn refresh() {
    let Some(sig) = SIG.get() else { return };
    let my = SEQ.get() + 1;
    SEQ.set(my);
    if !cloud::signed_in() {
        sig.state.set(State::Out);
        return;
    }
    spawn_local(async move {
        let got = cloud::list_inventory().await;
        if SEQ.get() != my {
            return;
        }
        match got {
            Ok(mut list) => {
                list.sort_by(|a, b| a.name.cmp(&b.name));
                if let Some(s) = sig.selected.get_untracked() {
                    if !list.iter().any(|x| x.id == s) {
                        sig.selected.set(None);
                    }
                }
                // 회사 마크: 아직 모르는 회사만 읽어요
                let need: Vec<String> = {
                    let mut v: Vec<String> = list.iter().map(|i| i.cid.clone()).filter(|c| !sig.cos.with_untracked(|m| m.contains_key(c))).collect();
                    v.sort();
                    v.dedup();
                    v
                };
                for cid in need {
                    spawn_local(async move {
                        if let Ok(co) = cloud::call("getCompany", &[wasm_bindgen::JsValue::from_str(&cid)]).await {
                            if !co.is_null() {
                                sig.cos.update(|m| {
                                    m.insert(cid, co);
                                });
                            }
                        }
                    });
                }
                sig.state.set(State::Ready(list));
            }
            Err(e) => {
                cloud::warn(&format!("인벤토리를 불러오지 못했어요: {e}"));
                sig.state.set(State::Failed);
            }
        }
    });
}

fn pick(sig: Sig, it: &InvItem) {
    let same = sig.selected.get_untracked().as_deref() == Some(it.id.as_str());
    sig.selected.set(if same { None } else { Some(it.id.clone()) });
    if !same && !sig.gains.with_untracked(|g| g.contains_key(&it.cid)) {
        let cid = it.cid.clone();
        spawn_local(async move {
            if let Ok(v) = cloud::food_gain(&cid).await {
                sig.gains.update(|g| {
                    g.insert(cid, v);
                });
            }
        });
    }
}

/// 음식 그림 (회사 마크까지). 회사를 아직 못 읽었으면 이름 글자로 그려요
fn art_of(sig: Sig, it: &InvItem) -> String {
    match sig.cos.with(|m| m.get(&it.cid).cloned()) {
        Some(co) => ctx().pkv_with(&it.raw, false, &co),
        None => ctx().pkv(&it.raw, false, &it.co_name),
    }
}

fn eat(sig: Sig, it: InvItem) {
    let c = ctx();
    let Some(label) = effect(&it.group) else { return c.toast("먹을 수 없는 음식이에요") };
    if sig.busy.get_untracked() {
        return;
    }
    let vit = c.call("vitals.get", &[]).unwrap_or(wasm_bindgen::JsValue::UNDEFINED);
    if vit.is_null() || vit.is_undefined() {
        return c.toast("상태를 아직 불러오지 못했어요");
    }
    sig.busy.set(true);
    spawn_local(async move {
        // 먹는 연출과 서버 기록을 함께 해요 (물은 꿀꺽 마시고, 음식은 세 번 베어 물어요)
        let stage = eat_show(&art_of(sig, &it), it.group == "water");
        let (r, _) = crate::pay::futures_join(cloud::eat_from_inventory(&it.cid, &it.pid, &it.group, &vit), crate::dom::sleep(if crate::dom::reduce_motion() { 150 } else { 1500 })).await;
        if let Some(st) = &stage {
            if let Ok(v) = &r {
                let gained = cloud::get(v, "gained").as_f64().unwrap_or(0.0);
                if let Some(t) = st.query_selector(".eat-gain").ok().flatten() {
                    t.set_text_content(Some(&format!("{} +{}", label, fmt_gain(gained))));
                }
                let _ = st.class_list().add_1("done");
                crate::dom::sleep(750).await;
            }
            let _ = st.class_list().add_1("bye");
            crate::dom::sleep(250).await;
            st.remove();
        }
        match r {
            Ok(v) => {
                let _ = c.call("vitals.set", &[v.clone()]);
                let gained = cloud::get(&v, "gained").as_f64().unwrap_or(0.0);
                sig.gains.update(|g| {
                    g.insert(it.cid.clone(), gained);
                });
                c.toast(&format!("{} 한 개를 먹었어요 · {} +{}", it.name, label, fmt_gain(gained)));
            }
            Err(e) => c.toast(if e.code == "permission-denied" { "지금은 먹을 수 없어요. 잠시 뒤에 다시 해 주세요" } else if e.message.is_empty() { "먹지 못했어요. 잠시 뒤에 다시 해 주세요" } else { &e.message }),
        }
        sig.busy.set(false);
        refresh(); // 개수를 서버에서 다시 읽어요
    });
}

/// 먹는 연출 판: 음식이 크게 떠오르고, 베어 문 자국이 생기며 부스러기가 튀어요 (마실 것은 기울며 줄어들어요)
fn eat_show(art: &str, drink: bool) -> Option<web_sys::HtmlElement> {
    let app = crate::dom::app_el()?;
    let el: web_sys::HtmlElement = crate::dom::doc().create_element("div").ok()?.dyn_into().ok()?;
    el.set_class_name(if drink { "eat drink" } else { "eat" });
    let _ = el.set_attribute("aria-hidden", "true");
    let crumbs: String = (0..9).map(|i| format!(r#"<i class="eat-crumb" style="--a:{}deg;--d:{}ms"></i>"#, i * 40 - 160, (i % 3) * 420 + 300)).collect();
    el.set_inner_html(&format!(r#"<div class="eat-stage"><div class="eat-food">{art}</div>{crumbs}</div><p class="eat-gain"></p>"#));
    let _ = app.append_child(&el);
    let _ = el.offset_width();
    let _ = el.class_list().add_1("go");
    for (i, ms) in [380, 800, 1220].into_iter().enumerate() {
        let e2 = el.clone();
        spawn_local(async move {
            crate::dom::sleep(ms).await;
            if e2.is_connected() {
                let _ = e2.class_list().add_1(&format!("b{}", i + 1));
                crate::dom::vibrate(12);
            }
        });
    }
    Some(el)
}

#[component]
fn Inventory(sig: Sig) -> impl IntoView {
    move || match sig.state.get() {
        State::Loading => skeleton(4),
        State::Out => note("로그인이 필요해요", "로그인하면 볼 수 있어요", "상점에서 사거나 사장님이 창고에서 꺼낸 음식이 여기에 들어가요."),
        State::Failed => note_retry("잠깐만요", "인벤토리를 불러오지 못했어요", "인터넷을 확인하고 다시 해 주세요.", "inv-retry", refresh),
        State::Ready(list) if list.is_empty() => view! { <p class="inv-empty-text">"아무것도 없어요"</p> }.into_any(),
        State::Ready(list) => {
            let detail_list = list.clone();
            view! {
                {move || {
                    let sel = sig.selected.get()?;
                    let it = detail_list.iter().find(|x| x.id == sel)?.clone();
                    Some(view! { <Detail sig it/> })
                }}
                <h2 class="section-title">"내 음식"</h2>
                <div class="shop-grid">
                    {list.into_iter().enumerate().map(|(i, it)| view! { <Card sig it i/> }).collect_view()}
                </div>
            }
            .into_any()
        }
    }
}

#[component]
fn Card(sig: Sig, it: InvItem, i: usize) -> impl IntoView {
    let it_art = it.clone();
    let art = move || art_of(sig, &it_art);
    let id = it.id.clone();
    let on = move || sig.selected.with(|s| s.as_deref() == Some(id.as_str()));
    let key = it.id.clone();
    let name = it.name.clone();
    let co = it.co_name.clone();
    let n = it.n;
    view! {
        <article class="card shop-card inv-card" class:is-on=on style=format!("--i:{i}") data-key=key on:click=move |_| pick(sig, &it)>
            <div class="shop-art" inner_html=art></div>
            <div class="shop-body">
                <b class="shop-name">{name}</b>
                <span class="shop-co">{co}</span>
                <span class="shop-stock">{format!("{n}개")}</span>
            </div>
        </article>
    }
}

#[component]
fn Detail(sig: Sig, it: InvItem) -> impl IntoView {
    let it_art = it.clone();
    let art = move || art_of(sig, &it_art);
    let label = effect(&it.group);
    let cid = it.cid.clone();
    let effect_text = move || match label {
        Some(l) => format!("{l} +{}", sig.gains.with(|g| g.get(&cid).map(|v| fmt_gain(*v)).unwrap_or_else(|| "…".into()))),
        None => "먹을 수 없어요".into(),
    };
    let item = it.clone();
    view! {
        <div class="card inv-detail">
            <div class="shop-art" inner_html=art></div>
            <div class="inv-info">
                <b class="shop-name">{it.name.clone()}</b>
                <span class="shop-co">{it.co_name.clone()}</span>
                <span class="shop-stock">{format!("{}개 가지고 있어요", it.n)}</span>
                <span class="inv-effect">{effect_text}</span>
                <button type="button" class="btn primary inv-eat" class:busy=move || sig.busy.get() data-eat="1" disabled=label.is_none() on:click=move |_| eat(sig, item.clone())>
                    "먹기"
                </button>
            </div>
        </div>
    }
}
