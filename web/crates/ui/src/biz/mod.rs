//! 사업 탭 (예전 app/biz.js): 판매 목록, 더보기(이름·멈추기·종료), 창고에서 꺼내기, 직원 채용, 연구실, 팔 상품 정하기, 블록 옮기기.
//! 사업 탭의 틀(제목, 회사 바꾸기, 창업)은 아직 app.js에 있어서, 같은 id의 칸에 붙어요.

mod drag;
mod hire;
mod lab;
pub(crate) mod picker;

use std::cell::{Cell, RefCell};

use goods::catalog::{self, Pick};
use leptos::mount::mount_to;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::ctx::Ctx;
use crate::dom::{self, by_id, esc};

/// 판매 목록의 상품 하나
#[derive(Clone, PartialEq)]
pub(crate) struct Prod {
    id: String,
    name: String,
    price_cents: f64,
    cost_cents: f64,
    stock: f64,
    paused: bool,
    at: f64,
    raw: JsValue,
}
impl Prod {
    fn from_js(v: &JsValue) -> Prod {
        let g = |k| cloud::get(v, k);
        Prod {
            id: g("id").as_string().unwrap_or_default(),
            name: g("name").as_string().unwrap_or_default(),
            price_cents: g("priceCents").as_f64().unwrap_or(0.0),
            cost_cents: g("costCents").as_f64().unwrap_or(0.0),
            stock: g("stock").as_f64().unwrap_or(0.0),
            paused: g("paused").is_truthy(),
            at: g("at").as_f64().unwrap_or(0.0),
            raw: v.clone(),
        }
    }
}

thread_local! {
    static ASKED_KIND: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static CTX: RefCell<Ctx> = RefCell::new(Ctx::default());
    static PRODUCTS: Cell<Option<RwSignal<Vec<Prod>, LocalStorage>>> = const { Cell::new(None) };
    static TAKING: Cell<bool> = const { Cell::new(false) };
}
pub(crate) fn ctx() -> Ctx {
    CTX.with(|c| c.borrow().clone())
}
fn products() -> Option<RwSignal<Vec<Prod>, LocalStorage>> {
    PRODUCTS.get()
}
/// 지금 고른 내 회사 (없으면 None)
pub(crate) fn company() -> Option<JsValue> {
    let co = ctx().company();
    if co.is_null() || co.is_undefined() {
        None
    } else {
        Some(co)
    }
}
pub(crate) fn co_str(co: &JsValue, k: &str) -> String {
    cloud::get(co, k).as_string().unwrap_or_default()
}
/// 내가 사장인 회사인지 (창고에서 꺼내는 것은 사장만 해요)
fn mine_co() -> bool {
    let Some(co) = company() else { return false };
    let uid = cloud::cloud().map(|c| cloud::get(&c, "uid")).and_then(|u| u.as_string()).unwrap_or_default();
    !uid.is_empty() && co_str(&co, "owner") == uid
}

/* ---------- 상품 그림 ---------- */

/// 상품 값(서버 그대로)을 그림 값으로
pub fn pick_of(raw: &JsValue) -> Pick {
    let n = |k| {
        let v = cloud::get(raw, k).as_f64()?;
        (v >= 0.0).then_some(v as u32)
    };
    Pick {
        item: cloud::get(raw, "item").as_string().unwrap_or_else(|| "water".into()),
        shape: n("shape"),
        cap: n("cap"),
        cap_color: n("capColor"),
        color: n("color"),
        content: n("content"),
    }
}
/// 상품 그림 (가운데 로고는 co 회사, 없으면 지금 고른 내 회사)
pub fn pkv(raw: &JsValue, mini: bool, co: &JsValue) -> String {
    pkv_pick(&pick_of(raw), mini, co)
}
pub(crate) fn pkv_pick(p: &Pick, mini: bool, co: &JsValue) -> String {
    let co = if co.is_undefined() { ctx().company() } else { co.clone() };
    let has_co = !(co.is_null() || co.is_undefined());
    let img = if has_co {
        ctx().call("safeImg", &[cloud::get(&co, "logo"), JsValue::from(40000)]).ok().and_then(|v| v.as_string()).unwrap_or_default()
    } else {
        String::new()
    };
    let inner = if img.is_empty() {
        let name = if has_co { co_str(&co, "name") } else { String::new() };
        let first: String = if name.is_empty() { "사".into() } else { name.chars().next().map(String::from).unwrap_or_default() };
        format!("<b>{}</b>", esc(&first))
    } else {
        format!("<i style=\"background-image:url('{img}')\"></i>")
    };
    catalog::pkv_html(p, mini, &inner, !img.is_empty())
}

/* ---------- 시작 ---------- */

pub fn init(c: Ctx) {
    CTX.with(|x| *x.borrow_mut() = c);
    let list = RwSignal::new_local(Vec::<Prod>::new());
    PRODUCTS.set(Some(list));
    if let Some(ul) = by_id("biz-list") {
        ul.set_inner_html("");
        mount_to(ul, move || view! { <ProdList list/> }).forget();
    }
    if let Some(ul) = by_id("dash-list") {
        ul.set_inner_html("");
        mount_to(ul, move || view! { <DashList list/> }).forget();
    }
    if let Some(b) = by_id("biz-pick") {
        dom::on(&b, "click", |_| picker::open());
    }
    picker::init();
    hire::init();
    lab::init();
    drag::init();
}

/// 판매 목록과 연구실을 다시 읽어요
pub fn refresh() {
    spawn_local(refresh_now());
}
pub(crate) async fn refresh_now() {
    let Some(list) = products() else { return };
    hire::paint();
    let co = company();
    // 상품을 파는 업종: 식품·컴퓨터·인공지능 (원자력은 연구실에서 전기를 팔아요)
    let food = co.as_ref().map(|c| co_str(c, "kind")).map(|k| matches!(k.as_str(), "" | "food" | "pc" | "ai")).unwrap_or(true);
    dom::set_hidden("biz-prod-title", !food);
    if let Some(card) = by_id("biz-list").and_then(|l| l.closest(".card").ok().flatten()).and_then(|c| c.dyn_into::<web_sys::HtmlElement>().ok()) {
        card.set_hidden(!food);
    }
    dom::set_hidden("biz-lab", false);
    // 원자력(전기) 회사는 팔 상품이 없어요: 상품 칸을 통째로 숨겨요
    dom::set_hidden("biz-empty", !food);
    // 업종을 아직 정하지 않은 내 회사는 먼저 업종부터 정해요 (회사마다 한 번)
    if let Some(c) = co.as_ref() {
        let id = co_str(c, "id");
        if mine_co() && cloud::get(c, "kindSet") == JsValue::FALSE && ASKED_KIND.with(|a| !a.borrow().contains(&id)) {
            ASKED_KIND.with(|a| a.borrow_mut().push(id));
            picker::open_kind();
        }
    }
    lab::paint();
    let Some(co) = co.filter(|_| food && cloud::signed_in()) else {
        list.set(Vec::new());
        paint();
        return;
    };
    let cid = co_str(&co, "id");
    match cloud::list_products(&cid).await {
        Ok(raw) => {
            let mut ps: Vec<Prod> = raw.iter().map(Prod::from_js).collect();
            ps.sort_by(|a, b| b.at.partial_cmp(&a.at).unwrap_or(std::cmp::Ordering::Equal));
            list.set(ps);
            paint();
        }
        Err(e) => cloud::warn(&format!("상품을 불러오지 못했어요: {}", e.code)),
    }
}

/// 사업 탭 칸들을 목록에 맞춰 보이거나 숨겨요 (목록 자체는 신호로 그려져요)
fn paint() {
    let Some(list) = products() else { return };
    let n = list.with_untracked(|l| l.len());
    let sells = company().map(|c| matches!(co_str(&c, "kind").as_str(), "" | "food" | "pc" | "ai")).unwrap_or(true);
    dom::set_hidden("biz-empty", n > 0 || !sells);
    let show = mine_co() && n > 0;
    dom::set_hidden("biz-dash-card", !show);
    dom::set_hidden("biz-dash-title", !show);
    // 식품 공장 (사장): 사업 탭에서 돌리고 모아요 (factory.rs)
    if let Some(fac) = by_id("biz-factory") {
        let co = company();
        let show = mine_co() && co.as_ref().map(|c| matches!(co_str(c, "kind").as_str(), "" | "food" | "pc" | "ai")).unwrap_or(false);
        fac.set_hidden(!show);
        if let (true, Some(co)) = (show, co) {
            crate::factory::factory_into(fac, co_str(&co, "id"));
        }
    }
}

/// 사업 탭을 열 때: 연구실 숫자를 다시 읽어요 (상품 목록은 그대로 두어요)
pub fn lab_refresh() {
    if company().is_some() {
        lab::paint();
    }
}

/* ---------- 판매 목록 ---------- */

#[component]
fn ProdList(list: RwSignal<Vec<Prod>, LocalStorage>) -> impl IntoView {
    let dots = catalog::svg_i(catalog::icon("dots"));
    view! {
        <For each=move || list.get().into_iter().enumerate() key=|(i, p)| (*i, p.id.clone(), p.name.clone(), p.paused) let:item>
            {
                let (i, p) = item;
                let thumb = pkv(&p.raw, true, &JsValue::UNDEFINED);
                let id = p.id.clone();
                view! {
                    <li class="biz-item" data-id=p.id.clone() style=format!("--i:{i}")>
                        <span class="biz-thumb" inner_html=thumb></span>
                        <span class="biz-main">
                            <b>{p.name.clone()}</b>
                            <span>{format!("판매 {} 토큰", cloud::tok(p.price_cents))}</span>
                            <span>{format!("원가 {} 토큰", cloud::tok(p.cost_cents))}</span>
                            {p.paused.then(|| view! { <em class="biz-tag">"판매 멈춤"</em> })}
                        </span>
                        <button class="biz-more" type="button" data-more=p.id.clone() aria-label="더보기" inner_html=dots.clone() on:click=move |_| on_more(id.clone())></button>
                    </li>
                }
            }
        </For>
    }
}

/* ---------- 창고 · 재고 (사장): 파는 상품마다 재고를 보여주고, 꺼낼 개수를 적어 꺼내요 ---------- */

#[component]
fn DashList(list: RwSignal<Vec<Prod>, LocalStorage>) -> impl IntoView {
    view! {
        <For each=move || list.get().into_iter().enumerate() key=|(i, p)| (*i, p.id.clone(), p.name.clone(), p.stock as i64) let:item>
            {
                let (i, p) = item;
                let co = ctx().company();
                let thumb = pkv(&p.raw, true, &co);
                let q = NodeRef::<leptos::html::Input>::new();
                let empty = p.stock < 1.0;
                let pp = p.clone();
                let take = move |ev: web_sys::MouseEvent| {
                    let btn: Option<web_sys::HtmlElement> = ev.current_target().and_then(|t| t.dyn_into().ok());
                    let n = q.get_untracked().map(|i| i.value()).unwrap_or_default().chars().filter(|c| c.is_ascii_digit()).collect::<String>();
                    let n = n.parse::<f64>().unwrap_or(0.0);
                    if let Some(btn) = btn {
                        take_out(pp.clone(), btn, n);
                    }
                };
                view! {
                    <li class="dash-item" style=format!("--i:{i}")>
                        <span class="dash-thumb" inner_html=thumb></span>
                        <span class="dash-main">
                            <b>{p.name.clone()}</b>
                            <span class="dash-stock">"재고 "<strong>{fmt_int(p.stock)}</strong>"개"</span>
                        </span>
                        <span class="dash-take">
                            <input class="dash-q" type="text" inputmode="numeric" value="1" maxlength="6" aria-label="꺼낼 개수" data-q=p.id.clone() node_ref=q/>
                            <button class="btn dash-go" type="button" data-take=p.id.clone() disabled=empty on:click=take>"꺼내기"</button>
                        </span>
                    </li>
                }
            }
        </For>
    }
}
fn fmt_int(v: f64) -> String {
    shapes_num(v)
}
fn shapes_num(v: f64) -> String {
    goods::shapes::num(v)
}

/// 꺼내기 (사장): 창고에서 정한 개수만큼 꺼내 내 인벤토리에 넣어요. 먹는 것은 인벤토리에서 해요
fn take_out(p: Prod, btn: web_sys::HtmlElement, n: f64) {
    let Some(co) = company() else { return };
    if TAKING.get() {
        return;
    }
    let c = ctx();
    if !(n >= 1.0) || n > p.stock {
        return c.toast(if n >= 1.0 { "창고에 그만큼 없어요" } else { "꺼낼 개수를 적어 주세요" });
    }
    TAKING.set(true);
    dom::toggle(&btn, "busy", true);
    spawn_local(async move {
        let r = cloud::call("takeOut", &[JsValue::from_str(&co_str(&co, "id")), p.raw.clone(), JsValue::from(n), JsValue::from_str(&co_str(&co, "name"))]).await;
        match r {
            Ok(_) => {
                c.toast(&format!("{} {}개를 인벤토리에 넣었어요", p.name, n as i64));
                refresh_now().await;
            }
            Err(e) => c.toast(if e.code == "permission-denied" {
                "지금은 꺼낼 수 없어요. 재고를 확인해 주세요"
            } else if !e.message.is_empty() {
                &e.message
            } else {
                "꺼내지 못했어요. 잠시 뒤 다시 해 주세요"
            }),
        }
        TAKING.set(false);
        dom::toggle(&btn, "busy", false);
    });
}

/* ---------- 더보기: 이름 바꾸기, 판매 멈추기(다시 시작), 판매 종료하기. 멈추기와 종료는 두 번 물어봐요 ---------- */

fn on_more(id: String) {
    let Some(list) = products() else { return };
    let Some(p) = list.with_untracked(|l| l.iter().find(|x| x.id == id).cloned()) else { return };
    if picker::busy() {
        return;
    }
    spawn_local(async move {
        let choice = sheet(
            &p.name,
            &[("rename", "이름 바꾸기", ""), ("pause", if p.paused { "판매 다시 시작하기" } else { "판매 멈추기" }, ""), ("end", "판매 종료하기", "danger")],
        )
        .await;
        let c = ctx();
        let Some(co) = company() else { return };
        let cid = co_str(&co, "id");
        let r: Result<(), cloud::CloudError> = async {
            match choice.as_deref() {
                Some("rename") => {
                    let Some(name) = ask_name(&p.name).await else { return Ok(()) };
                    if name == p.name {
                        return Ok(()); // 그대로면 서버에 보내지 않아요
                    }
                    cloud::call("renameProduct", &[JsValue::from_str(&cid), JsValue::from_str(&id), JsValue::from_str(&name)]).await?;
                    list.update(|l| l.iter_mut().filter(|x| x.id == id).for_each(|x| x.name = name.clone()));
                    paint();
                    c.toast("이름을 바꿨어요");
                }
                Some("pause") => {
                    let resume = p.paused;
                    let a = c
                        .ask(
                            if resume { "판매를 다시 시작할까요?" } else { "판매를 멈출까요?" },
                            if resume { "다시 시작하면 이 상품이 다시 팔려요." } else { "멈추면 이 상품이 팔리지 않아요. 나중에 다시 시작할 수 있어요." },
                            if resume { "다시 시작" } else { "멈추기" },
                            "취소",
                        )
                        .await;
                    if !a {
                        return Ok(());
                    }
                    if !resume && !c.ask("정말 하시겠습니까?", "판매를 멈춰요.", "네, 멈출게요", "아니요").await {
                        return Ok(());
                    }
                    cloud::call("setProductPaused", &[JsValue::from_str(&cid), JsValue::from_str(&id), JsValue::from_bool(!resume)]).await?;
                    list.update(|l| l.iter_mut().filter(|x| x.id == id).for_each(|x| x.paused = !resume));
                    paint();
                    c.toast(if resume { "판매를 다시 시작했어요" } else { "판매를 멈췄어요" });
                }
                Some("end") => {
                    if !c.ask(&format!("\"{}\" 판매를 끝낼까요?", p.name), "끝내면 이 상품은 다시 팔 수 없어요.", "판매 종료", "취소").await {
                        return Ok(());
                    }
                    if !c.ask("정말 하시겠습니까?", "판매를 끝내면 되돌릴 수 없어요.", "네, 끝낼게요", "아니요").await {
                        return Ok(());
                    }
                    cloud::call("endProduct", &[JsValue::from_str(&cid), JsValue::from_str(&id)]).await?;
                    let id2 = id.clone();
                    set_timeout(
                        move || {
                            list.update(|l| l.retain(|x| x.id != id2));
                            paint();
                        },
                        std::time::Duration::from_millis(240),
                    );
                    c.toast(&format!("{} 판매를 끝냈어요", p.name));
                }
                _ => {}
            }
            Ok(())
        }
        .await;
        if r.is_err() {
            c.toast("처리하지 못했어요. 잠시 뒤 다시 해 주세요");
        }
    });
}

/// 아래에서 올라오는 선택 목록 (아이폰 액션 시트). (id, 글자, 색) 가운데 고른 id, 취소면 None
async fn sheet(title: &str, actions: &[(&str, &str, &str)]) -> Option<String> {
    let doc = dom::doc();
    let app = dom::app_el()?;
    let dim: web_sys::HtmlElement = doc.create_element("div").ok()?.dyn_into().ok()?;
    dim.set_class_name("bz-dim");
    let bx: web_sys::HtmlElement = doc.create_element("div").ok()?.dyn_into().ok()?;
    bx.set_class_name("bz-sheet");
    let acts: String = actions
        .iter()
        .map(|(id, label, tone)| format!(r#"<button type="button" class="bz-act{}" data-id="{id}">{}</button>"#, if tone.is_empty() { String::new() } else { format!(" {tone}") }, esc(label)))
        .collect();
    bx.set_inner_html(&format!(r#"<p class="bz-sheet-t">{}</p><div class="bz-sheet-g">{acts}</div><button type="button" class="bz-act bz-cancel" data-id="">취소</button>"#, esc(title)));
    let (dim2, bx2) = (dim.clone(), bx.clone());
    let got = js_sys::Promise::new(&mut |resolve, _| {
        let (dim2, bx2) = (dim2.clone(), bx2.clone());
        let (dim3, bx3) = (dim2.clone(), bx2.clone());
        let done = std::rc::Rc::new(move |id: String| {
            let (dim2, bx2) = (&dim3, &bx3);
            let _ = dim2.class_list().remove_1("on");
            let _ = bx2.class_list().remove_1("on");
            let (d, b) = (dim2.clone(), bx2.clone());
            set_timeout(
                move || {
                    d.remove();
                    b.remove();
                },
                std::time::Duration::from_millis(300),
            );
            let _ = resolve.call1(&JsValue::NULL, &JsValue::from_str(&id));
        });
        let d1 = done.clone();
        dom::on(&bx2, "click", move |e| {
            if let Some(b) = dom::closest(&e, "[data-id]") {
                d1(dom::data(&b, "id"));
            }
        });
        let d2 = done.clone();
        dom::on(&dim2, "click", move |_| d2(String::new()));
    });
    let _ = app.append_with_node_2(&dim, &bx);
    let _ = bx.offset_height(); // 시작 자리를 먼저 잡아야 올라오는 움직임이 나와요
    let _ = dim.class_list().add_1("on");
    let _ = bx.class_list().add_1("on");
    let v = wasm_bindgen_futures::JsFuture::from(got).await.ok()?.as_string()?;
    (!v.is_empty()).then_some(v)
}

/// 이름 바꾸기: 입력 카드 (1~30자, < > 없이). 취소면 None
async fn ask_name(current: &str) -> Option<String> {
    let doc = dom::doc();
    let app = dom::app_el()?;
    let dim: web_sys::HtmlElement = doc.create_element("div").ok()?.dyn_into().ok()?;
    dim.set_class_name("bz-dim");
    let card: web_sys::HtmlElement = doc.create_element("div").ok()?.dyn_into().ok()?;
    card.set_class_name("bz-card");
    card.set_inner_html(&format!(
        r#"<p class="bz-card-t">이름 바꾸기</p><input class="bz-input" maxlength="30" autocomplete="off" value="{}" aria-label="새 이름"><p class="bz-err"></p><div class="bz-card-b"><button type="button" class="bz-btn" data-k="no">취소</button><button type="button" class="bz-btn primary" data-k="ok">저장</button></div>"#,
        esc(current)
    ));
    let input: web_sys::HtmlInputElement = card.query_selector(".bz-input").ok()??.dyn_into().ok()?;
    let err = dom::qs(&card, ".bz-err")?;
    let (dim2, card2) = (dim.clone(), card.clone());
    let got = js_sys::Promise::new(&mut |resolve, _| {
        let (dim2, card2) = (dim2.clone(), card2.clone());
        let (dim3, card3) = (dim2.clone(), card2.clone());
        let done = std::rc::Rc::new(move |v: JsValue| {
            let (dim2, card2) = (&dim3, &card3);
            let _ = dim2.class_list().remove_1("on");
            let _ = card2.class_list().remove_1("on");
            let (d, b) = (dim2.clone(), card2.clone());
            set_timeout(
                move || {
                    d.remove();
                    b.remove();
                },
                std::time::Duration::from_millis(300),
            );
            let _ = resolve.call1(&JsValue::NULL, &v);
        });
        let d1 = done.clone();
        let (input, err) = (input.clone(), err.clone());
        dom::on(&card2, "click", move |e| {
            let Some(b) = dom::closest(&e, "[data-k]") else { return };
            if dom::data(&b, "k") == "no" {
                return d1(JsValue::NULL);
            }
            let name = clean_name(&input.value());
            if name.is_empty() || name.chars().count() > 30 || name.contains(['<', '>']) {
                err.set_text_content(Some("이름은 1~30자로, < > 없이 적어 주세요"));
                return;
            }
            d1(JsValue::from_str(&name));
        });
        let d2 = done.clone();
        dom::on(&dim2, "click", move |_| d2(JsValue::NULL));
    });
    let _ = app.append_with_node_2(&dim, &card);
    let _ = card.offset_height();
    let _ = dim.class_list().add_1("on");
    let _ = card.class_list().add_1("on");
    let _ = input.focus();
    wasm_bindgen_futures::JsFuture::from(got).await.ok()?.as_string()
}
/// 앞뒤 빈칸을 지우고, 빈칸 여러 개는 하나로
pub(crate) fn clean_name(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
