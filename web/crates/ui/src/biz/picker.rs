//! 팔 상품 정하기 (전체 화면). 식품 → 물·음식·에너지 식품 → … → 모양·뚜껑·내용물·색 → 이름과 가격 → 팔기.
//! 칸에서 고른 것은 확인 버튼을 눌러야 다음으로 넘어가요. 예전 JS와 같은 HTML을 그려서 CSS가 그대로 맞아요.

use std::cell::RefCell;
use std::time::Duration;

use goods::catalog::{self, color_of, cost_lines, icon, leaf_of, name_of, step_info, svg_i, Node, Pick, StepKind, ICON_COL, ROOT};
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::{co_str, company, ctx, pkv_pick};
use crate::dom::{self, anim, anim_always, esc, on_finish, qs, qsa};

const SPRING: &str = "cubic-bezier(0.2, 0.9, 0.25, 1.06)";

#[derive(Clone, Copy)]
enum Entry {
    Root,
    List(&'static Node),
    Soon(&'static Node),
    Step(&'static str),
    Summary,
}

#[derive(Clone)]
struct PickS {
    p: Pick,
    name: Option<String>,
    price: Option<i64>,
}

#[derive(Clone)]
struct Els {
    pk: web_sys::HtmlElement,
    stage: web_sys::HtmlElement,
    prev: web_sys::HtmlElement,
    title: web_sys::HtmlElement,
    crumb: web_sys::HtmlElement,
    back: web_sys::HtmlElement,
    foot: web_sys::HtmlElement,
    ok: web_sys::HtmlButtonElement,
}

#[derive(Default)]
struct St {
    hist: Vec<Entry>,
    pick: Option<PickS>,
    sel: Option<u32>,
    busy: bool,
    last_prev: String,
    /// 처음 업종을 정할 때: 맨 위 칸이 "업종을 정하세요"가 되고, 고르면 회사 업종이 정해져요
    kind_mode: bool,
}
/// 업종 칸: 식품·컴퓨터·인공지능·원자력 발전
const KINDS: [(&str, &str, &str); 4] = [("food", "식품", "food"), ("pc", "컴퓨터", "pc"), ("ai", "인공지능", "ai"), ("nuclear", "전기", "nuclear")];

thread_local! {
    static ELS: RefCell<Option<Els>> = const { RefCell::new(None) };
    static ST: RefCell<St> = RefCell::new(St::default());
}
fn els() -> Option<Els> {
    ELS.with(|e| e.borrow().clone())
}
fn st<R>(f: impl FnOnce(&mut St) -> R) -> R {
    ST.with(|s| f(&mut s.borrow_mut()))
}
fn top() -> Option<Entry> {
    st(|s| s.hist.last().copied())
}
pub fn busy() -> bool {
    st(|s| s.busy)
}
fn leaf_now() -> &'static catalog::Leaf {
    leaf_of(&st(|s| s.pick.as_ref().map(|p| p.p.item.clone()).unwrap_or_default()))
}
/// 센트 정수를 입력칸 글자로 (50 → "0.5")
fn fmt_cents(c: i64) -> String {
    goods::shapes::num((c as f64 / 100.0 * 100.0).round() / 100.0)
}

pub fn init() {
    let doc = dom::doc();
    let Some(app) = dom::app_el() else { return };
    let Some(pk) = doc.create_element("section").ok().and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()) else { return };
    pk.set_class_name("pk");
    pk.set_hidden(true);
    let _ = pk.set_attribute("data-volatile", "");
    let _ = pk.set_attribute("aria-label", "팔 상품 정하기");
    pk.set_inner_html(&format!(
        r#"<header class="pk-top"><button class="pk-glass" type="button" data-back aria-label="닫기">{}</button><div class="pk-title"><b></b><span></span></div><span class="pk-spacer" aria-hidden="true"></span></header>
      <div class="pk-prev" aria-hidden="true"></div>
      <div class="pk-stage"></div>
      <div class="pk-foot" hidden><button class="pk-ok" type="button" disabled>확인</button></div>"#,
        svg_i(icon("back"))
    ));
    let _ = app.append_child(&pk);
    let (Some(stage), Some(prev), Some(title), Some(crumb), Some(back), Some(foot), Some(ok)) =
        (qs(&pk, ".pk-stage"), qs(&pk, ".pk-prev"), qs(&pk, ".pk-title b"), qs(&pk, ".pk-title span"), qs(&pk, "[data-back]"), qs(&pk, ".pk-foot"), qs(&pk, ".pk-ok").and_then(|b| b.dyn_into::<web_sys::HtmlButtonElement>().ok()))
    else {
        return;
    };
    dom::on(&back, "click", |_| {
        if !busy() {
            pop();
        }
    });
    dom::on(&ok, "click", |_| confirm_step());
    dom::on(&stage, "click", on_stage);
    dom::on(&stage, "input", on_input);
    dom::on(&stage, "focusout", on_blur);
    ELS.with(|e| *e.borrow_mut() = Some(Els { pk, stage, prev, title, crumb, back, foot, ok }));
}

/* ---------- 제목 ---------- */

fn title_of(e: Entry) -> &'static str {
    match e {
        Entry::Root if st(|s| s.kind_mode) => "업종을 정하세요",
        Entry::Root => "팔 상품 정하기",
        Entry::List(n) | Entry::Soon(n) => n.label,
        Entry::Step(k) => step_info(k, leaf_now()).title,
        Entry::Summary => "이름과 가격 정하기",
    }
}
fn crumb_of() -> String {
    let (hist, item) = st(|s| (s.hist.clone(), s.pick.as_ref().map(|p| p.p.item.clone())));
    let mut out: Vec<&str> = Vec::new();
    for e in hist {
        match e {
            Entry::List(n) | Entry::Soon(n) => out.push(n.label),
            Entry::Step(k) => {
                if let Some(item) = &item {
                    let l = leaf_of(item);
                    if l.flow.first() == Some(&k) {
                        out.push(l.label);
                    }
                }
            }
            _ => {}
        }
    }
    out.join(" › ")
}

/* ---------- 화면 조각 ---------- */

fn row_html(n: &Node, i: usize) -> String {
    format!(
        r#"<button class="pk-row" type="button" data-node="{}" style="--i:{i}"><span class="pk-ic" style="--c:{}">{}</span><span class="pk-main"><b>{}</b><span>{}</span></span><span class="pk-chev">{}</span></button>"#,
        n.id,
        ICON_COL[i % ICON_COL.len()],
        svg_i(icon(n.icon)),
        esc(n.label),
        esc(n.hint),
        svg_i(icon("chev"))
    )
}
fn step_html(key: &str) -> String {
    let leaf = leaf_now();
    let stp = step_info(key, leaf);
    let sel = st(|s| s.sel);
    let on = |k: u32| if sel == Some(k) { " is-on" } else { "" };
    match stp.kind {
        StepKind::Shape => {
            let tiles: String = (0..50u32)
                .map(|k| {
                    let kind = stp.kinds.get((k / 5) as usize).copied().unwrap_or("");
                    format!(r#"<button class="pk-tile{}" type="button" data-v="{k}" style="--i:{k}">{}</button>"#, on(k), catalog::thumb_html(stp.family, kind, (k % 5) as usize))
                })
                .collect();
            format!(r#"<div class="pk-grid">{tiles}</div>"#)
        }
        StepKind::Color => {
            let sw: String = (0..50u32).map(|k| format!(r#"<button class="pk-sw{}" type="button" data-v="{k}" style="--i:{k};--c:{}" aria-label="색 {}"></button>"#, on(k), color_of(k), k + 1)).collect();
            format!(r#"<div class="pk-grid pk-swgrid">{sw}</div>"#)
        }
        StepKind::Options => {
            let rows: String = leaf
                .contents
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    format!(
                        r#"<button class="pk-row{}" type="button" data-v="{i}" style="--i:{i}"><span class="pk-ic" style="--c:{}">{}</span><span class="pk-main"><b>{}</b></span><span class="pk-chev">{}</span></button>"#,
                        on(i as u32),
                        ICON_COL[i % ICON_COL.len()],
                        svg_i(icon("dot")),
                        esc(c),
                        svg_i(icon("chev"))
                    )
                })
                .collect();
            format!(r#"<div class="pk-list">{rows}</div>"#)
        }
    }
}
fn summary_html() -> String {
    let Some(mut ps) = st(|s| s.pick.clone()) else { return String::new() };
    let (lines, total) = cost_lines(&ps.p);
    let sug = (total * 2).max(1);
    if ps.price.is_none() {
        ps.price = Some(sug);
    }
    if ps.name.is_none() {
        ps.name = Some(name_of(&ps.p));
    }
    st(|s| s.pick = Some(ps.clone()));
    let rows: String = lines.iter().map(|(n, v)| format!(r#"<div class="pk-cost-row"><span>{n}</span><b>{} 토큰</b></div>"#, cloud::tok(*v as f64))).collect();
    let steps: String = [-100i64, -10, 10, 100]
        .iter()
        .map(|d| format!(r#"<button class="pk-step" type="button" data-step="{d}">{}{}</button>"#, if *d > 0 { "+" } else { "−" }, cloud::tok(d.abs() as f64)))
        .collect();
    format!(
        r#"<div class="pk-sum">
      <p class="pk-sum-sub">{}</p>
      <div class="pk-card"><input id="pk-name" class="pk-name" maxlength="30" autocomplete="off" placeholder="상품 이름" value="{}" aria-label="상품 이름"></div>
      <div class="pk-card">
        <p class="pk-lab">원가 계산</p>
        {rows}
        <div class="pk-cost-row pk-cost-total"><span>원가 합계</span><b>{} 토큰</b></div>
      </div>
      <div class="pk-card">
        <input id="pk-input" class="pk-input" inputmode="decimal" autocomplete="off" value="{}" aria-label="팔 가격">
        <div class="pk-steps">{steps}</div>
        <button class="pk-chip" type="button" data-suggest="{sug}">추천 {} 토큰 (원가의 2배)</button>
        <p class="pk-profit" id="pk-profit"></p>
      </div>
      <p class="pk-err" id="pk-err" role="alert"></p>
      <button class="pk-sell" id="pk-sell" type="button">이 가격으로 팔기</button>
    </div>"#,
        esc(&crumb_of()),
        esc(ps.name.as_deref().unwrap_or("")),
        cloud::tok(total as f64),
        fmt_cents(ps.price.unwrap_or(1)),
        cloud::tok(sug as f64)
    )
}
fn view_html(e: Entry) -> String {
    match e {
        Entry::Root if st(|s| s.kind_mode) => {
            let orbs: String = KINDS
                .iter()
                .enumerate()
                .map(|(i, (_, label, ic))| format!(r#"<div class="pk-orbwrap"><button class="pk-orb" type="button" data-orb="{i}" aria-label="{label}">{}</button><span class="pk-orb-l">{label}</span></div>"#, svg_i(icon(ic))))
                .collect();
            format!(r#"<div class="pk-root pk-kinds">{orbs}</div>"#)
        }
        Entry::Root => {
            let orbs: String = ROOT
                .iter()
                .enumerate()
                .map(|(i, n)| format!(r#"<div class="pk-orbwrap"><button class="pk-orb" type="button" data-orb="{i}" aria-label="{}">{}</button><span class="pk-orb-l">{}</span></div>"#, n.label, svg_i(icon(n.icon)), n.label))
                .collect();
            format!(r#"<div class="pk-root">{orbs}</div>"#)
        }
        Entry::List(n) => format!(r#"<div class="pk-list">{}</div>"#, n.children.iter().enumerate().map(|(i, c)| row_html(c, i)).collect::<String>()),
        Entry::Soon(n) => format!(r#"<div class="pk-soon"><div class="pk-soon-orb">{}</div><p class="pk-soon-t">{}</p><p class="pk-soon-s">아직 준비 중이에요</p></div>"#, svg_i(icon(n.icon)), n.label),
        Entry::Step(k) => step_html(k),
        Entry::Summary => summary_html(),
    }
}

/// 원가와 이익을 다시 써요 (입력칸은 건드리지 않아요)
fn paint_profit() {
    let Some(e) = els() else { return };
    let Some(el) = qs(&e.pk, "#pk-profit") else { return };
    let Some(ps) = st(|s| s.pick.clone()) else { return };
    let (_, total) = cost_lines(&ps.p);
    let pr = ps.price.unwrap_or(0);
    let gain = pr - total;
    dom::toggle(&el, "loss", pr >= 1 && gain < 0);
    let t = if pr < 1 {
        "가격을 정해 주세요".to_string()
    } else if gain > 0 {
        format!("한 개에 {} 토큰 남아요 (이익률 {}%)", cloud::tok(gain as f64), ((gain as f64 / pr as f64) * 100.0 + 0.5).floor() as i64)
    } else if gain == 0 {
        "원가와 같아요. 남는 돈이 없어요".into()
    } else {
        format!("원가보다 {} 토큰 낮아요", cloud::tok(-gain as f64))
    };
    el.set_text_content(Some(&t));
}

/* ---------- 그림 칸 (위쪽 미리보기) ---------- */

/// 칸에서 고른 것은 확인 전이라도 미리 보여줘요
fn pick_preview() -> Option<Pick> {
    let (t, sel, ps) = st(|s| (s.hist.last().copied(), s.sel, s.pick.clone()));
    let mut p = ps?.p;
    if let (Some(Entry::Step(k)), Some(v)) = (t, sel) {
        p.set(step_info(k, leaf_of(&p.item)).field, Some(v));
    }
    Some(p)
}
fn set_prev(html: String, big: bool, on: bool) {
    let Some(e) = els() else { return };
    dom::toggle(&e.prev, "on", on);
    dom::toggle(&e.prev, "pk-big", big);
    if st(|s| s.last_prev == html) {
        return;
    }
    st(|s| s.last_prev = html.clone());
    let old = qsa(&e.prev, ".pk-pvbox");
    if !html.is_empty() {
        if let Ok(bx) = dom::doc().create_element("div") {
            bx.set_class_name("pk-pvbox");
            bx.set_inner_html(&html);
            let _ = e.prev.append_child(&bx);
            anim(&bx, r#"[{"opacity":0,"transform":"scale(.9) translateY(10px)"},{"opacity":1,"transform":"none"}]"#, &format!(r#"{{"duration":440,"easing":"{SPRING}","fill":"backwards"}}"#));
        }
    }
    for o in old {
        let a = anim_always(&o, r#"[{"opacity":1,"transform":"none"},{"opacity":0,"transform":"scale(1.03)"}]"#, r#"{"duration":300,"fill":"forwards"}"#);
        on_finish(a, move || o.remove());
    }
}
fn render_prev(e: Entry) {
    match e {
        Entry::Step(_) => set_prev(pick_preview().map(|p| pkv_pick(&p, false, &JsValue::UNDEFINED)).unwrap_or_default(), false, true),
        Entry::Summary => set_prev(st(|s| s.pick.clone()).map(|p| pkv_pick(&p.p, false, &JsValue::UNDEFINED)).unwrap_or_default(), true, true),
        _ => set_prev(String::new(), false, false),
    }
}

/* ---------- 화면 넘기기 ---------- */

fn make_view(html: &str) -> Option<web_sys::HtmlElement> {
    let el: web_sys::HtmlElement = dom::doc().create_element("div").ok()?.dyn_into().ok()?;
    el.set_class_name("pk-view");
    el.set_inner_html(html);
    Some(el)
}
fn swap(el: web_sys::HtmlElement, dir: f64) {
    let Some(e) = els() else { return };
    let old = qsa(&e.stage, ".pk-view");
    let _ = e.stage.append_child(&el);
    if dom::reduce_motion() {
        old.iter().for_each(|o| o.remove());
        return;
    }
    anim(&el, &format!(r#"[{{"opacity":0,"transform":"translate3d({}px,0,0)"}},{{"opacity":1,"transform":"translate3d(0,0,0)"}}]"#, dir * 46.0), &format!(r#"{{"duration":460,"easing":"{SPRING}","fill":"backwards"}}"#));
    for o in old {
        let _ = o.style().set_property("pointer-events", "none");
        let a = anim_always(&o, &format!(r#"[{{"opacity":1,"transform":"none"}},{{"opacity":0,"transform":"translate3d({}px,0,0)"}}]"#, -dir * 32.0), r#"{"duration":320,"easing":"ease-in","fill":"forwards"}"#);
        on_finish(a, move || o.remove());
    }
}
fn sync_top() {
    let (Some(e), Some(t)) = (els(), top()) else { return };
    let n = st(|s| s.hist.len());
    e.title.set_text_content(Some(title_of(t)));
    e.crumb.set_text_content(Some(&if n > 1 { crumb_of() } else { String::new() }));
    let _ = e.back.set_attribute("aria-label", if n > 1 { "뒤로" } else { "닫기" });
    anim(&e.title, r#"[{"opacity":0.2,"transform":"translateY(4px)"},{"opacity":1,"transform":"none"}]"#, &format!(r#"{{"duration":320,"easing":"{SPRING}","fill":"backwards"}}"#));
}
/// 확인 버튼은 고르는 칸에서만 보여요. 고르기 전에는 누를 수 없어요
fn paint_foot() {
    let Some(e) = els() else { return };
    e.foot.set_hidden(!matches!(top(), Some(Entry::Step(_))));
    e.ok.set_disabled(st(|s| s.sel.is_none()));
}
fn show(en: Entry, dir: f64) {
    if let Entry::Step(k) = en {
        // 이미 정한 칸으로 돌아오면 그 값을 골라 둬요
        st(|s| {
            let v = s.pick.as_ref().and_then(|p| p.p.get(step_info(k, leaf_of(&p.p.item)).field));
            s.sel = v;
        });
    }
    if let Some(v) = make_view(&view_html(en)) {
        swap(v, dir);
    }
    sync_top();
    render_prev(en);
    paint_foot();
    if matches!(en, Entry::Summary) {
        request_animation_frame(paint_profit);
    }
}
fn push(en: Entry) {
    st(|s| s.hist.push(en));
    show(en, 1.0);
}
fn pop() {
    if st(|s| s.hist.len()) <= 1 {
        return close();
    }
    st(|s| s.hist.pop());
    if let Some(t) = top() {
        show(t, -1.0);
    }
}
fn advance() {
    let Some(Entry::Step(k)) = top() else { return };
    let flow = leaf_now().flow;
    let i = flow.iter().position(|x| *x == k).unwrap_or(0);
    push(if i + 1 < flow.len() { Entry::Step(flow[i + 1]) } else { Entry::Summary });
}

/* ---------- 고르기 ---------- */

/// 식품·컴퓨터·인공지능 동그라미: 누른 동그라미가 위쪽 제목 자리로 모여요
fn pick_root(i: usize, orb: web_sys::HtmlElement) {
    if busy() {
        return;
    }
    let Some(e) = els() else { return };
    st(|s| s.busy = true);
    let kind_mode = st(|s| s.kind_mode);
    let node = &ROOT[i.min(ROOT.len() - 1)];
    if let Some(t) = qs(&e.pk, ".pk-title") {
        let t = t.get_bounding_client_rect();
        let r = orb.get_bounding_client_rect();
        let dx = t.left() + t.width() / 2.0 - (r.left() + r.width() / 2.0);
        let dy = t.top() + t.height() / 2.0 - (r.top() + r.height() / 2.0) - 6.0;
        anim(&orb, &format!(r#"[{{"transform":"translate3d(0,0,0) scale(1)"}},{{"transform":"translate3d({dx}px,{dy}px,0) scale(.5)"}}]"#), &format!(r#"{{"duration":480,"easing":"{SPRING}"}}"#));
    }
    let wrap = orb.parent_element();
    for w in qsa(&e.stage, ".pk-orbwrap") {
        if Some(&*w) != wrap.as_ref() {
            anim(&w, r#"[{"opacity":1,"transform":"none"},{"opacity":0,"transform":"scale(.7)"}]"#, r#"{"duration":300}"#);
        }
    }
    if let Some(l) = wrap.as_ref().and_then(|w| qs(w, ".pk-orb-l")) {
        anim(&l, r#"[{"opacity":1},{"opacity":0}]"#, r#"{"duration":200}"#);
    }
    if kind_mode {
        let (kind, label, _) = KINDS[i.min(KINDS.len() - 1)];
        spawn_local(async move {
            let cid = company().map(|c| co_str(&c, "id")).unwrap_or_default();
            let ok = cloud::call("setCompanyKind", &[JsValue::from_str(&cid), JsValue::from_str(kind)]).await.is_ok();
            st(|s| {
                s.busy = false;
                s.kind_mode = false;
            });
            if !ok {
                ctx().toast("업종을 정하지 못했어요. 잠시 뒤 다시 해 주세요");
                return close();
            }
            crate::shell::company::patch(&dom::obj(&[("kind", JsValue::from_str(kind))]));
            crate::shell::company::render();
            ctx().toast(&format!("업종을 {label}(으)로 정했어요"));
            crate::work::refresh(true);
            super::refresh_now().await;
            // 식품 회사는 바로 팔 상품을 이어서 골라요. 다른 업종은 사업 탭의 연구실에서 해요
            match catalog::node("food").filter(|_| kind == "food") {
                Some(n) => {
                    if let Some(e) = els() {
                        e.stage.set_inner_html("");
                    }
                    st(|s| s.hist = vec![Entry::List(n)]);
                    if let (Some(e), Some(v)) = (els(), make_view(&view_html(Entry::List(n)))) {
                        let _ = e.stage.append_child(&v);
                    }
                    sync_top();
                    paint_foot();
                }
                None => close(),
            }
        });
        return;
    }
    set_timeout(
        move || {
            st(|s| s.busy = false);
            push(if node.soon { Entry::Soon(node) } else { Entry::List(node) });
        },
        Duration::from_millis(340),
    );
}
fn choose_node(id: &str) {
    let Some(n) = catalog::node(id) else { return };
    if !n.children.is_empty() {
        return push(Entry::List(n));
    }
    if let Some(item) = n.leaf {
        st(|s| s.pick = Some(PickS { p: Pick { item: item.into(), ..Pick::default() }, name: None, price: None }));
        push(Entry::Step(leaf_of(item).flow[0]));
    }
}
/// 칸에서 하나를 고르면 미리 보여줘요. 확인을 눌러야 다음으로 넘어가요
fn choose(v: u32, el: web_sys::HtmlElement) {
    if busy() || st(|s| s.pick.is_none()) {
        return;
    }
    let Some(e) = els() else { return };
    st(|s| s.sel = Some(v));
    for s in qsa(&e.stage, "[data-v]") {
        dom::toggle(&s, "is-on", s == el);
    }
    anim(&el, r#"[{"transform":"scale(1)"},{"transform":"scale(.92)"},{"transform":"scale(1.05)"},{"transform":"scale(1)"}]"#, r#"{"duration":420,"easing":"ease-out"}"#);
    dom::vibrate(6);
    set_prev(pick_preview().map(|p| pkv_pick(&p, false, &JsValue::UNDEFINED)).unwrap_or_default(), false, true);
    paint_foot();
}
fn confirm_step() {
    let Some(Entry::Step(k)) = top() else { return };
    if busy() {
        return;
    }
    let ok = st(|s| {
        let Some(v) = s.sel else { return false };
        let Some(ps) = s.pick.as_mut() else { return false };
        let f = step_info(k, leaf_of(&ps.p.item)).field;
        ps.p.set(f, Some(v));
        s.sel = None;
        true
    });
    if ok {
        advance();
    }
}

/* ---------- 가격 ---------- */

fn set_price(cents: f64) {
    let v = cents.round().clamp(1.0, 99999999.0) as i64;
    st(|s| {
        if let Some(p) = s.pick.as_mut() {
            p.price = Some(v);
        }
    });
    if let Some(inp) = els().and_then(|e| e.pk.query_selector("#pk-input").ok().flatten()).and_then(|i| i.dyn_into::<web_sys::HtmlInputElement>().ok()) {
        inp.set_value(&fmt_cents(v));
    }
    paint_profit();
}

/* ---------- 팔기 ---------- */

fn burst() {
    let Some(e) = els() else { return };
    let Ok(d) = dom::doc().create_element("div") else { return };
    d.set_class_name("pk-done");
    d.set_inner_html(&format!("<i>{}</i>", svg_i(icon("check"))));
    let _ = e.pk.append_child(&d);
    if let Some(i) = d.first_element_child() {
        anim(&i, r#"[{"transform":"scale(.4)","opacity":0},{"transform":"scale(1.08)","opacity":1,"offset":0.6},{"transform":"scale(1)","opacity":1}]"#, &format!(r#"{{"duration":620,"easing":"{SPRING}"}}"#));
    }
}
fn sell() {
    let Some(e) = els() else { return };
    let Some(ps) = st(|s| s.pick.clone()) else { return };
    if busy() {
        return;
    }
    let (Some(err), Some(btn)) = (qs(&e.pk, "#pk-err"), qs(&e.pk, "#pk-sell")) else { return };
    let name = super::clean_name(ps.name.as_deref().unwrap_or(""));
    if name.is_empty() || name.chars().count() > 30 || name.contains(['<', '>']) {
        err.set_text_content(Some("이름은 1~30자로, < > 없이 적어 주세요"));
        return;
    }
    if !(ps.price.unwrap_or(0) >= 1) {
        err.set_text_content(Some("가격을 0.01 토큰 이상으로 정해 주세요"));
        return;
    }
    let Some(co) = company() else {
        err.set_text_content(Some("먼저 창업해야 팔 수 있어요"));
        return;
    };
    st(|s| s.busy = true);
    err.set_text_content(Some(""));
    let p = &ps.p;
    let leaf = leaf_of(&p.item);
    let (_, total) = cost_lines(p);
    let num = |v: Option<u32>| v.map(|v| JsValue::from(v as f64)).unwrap_or(JsValue::NULL);
    let mut fields: Vec<(&str, JsValue)> = vec![
        ("cat", "food".into()),
        ("group", leaf.group.into()),
        ("item", JsValue::from_str(&p.item)),
        ("name", JsValue::from_str(&name)),
        ("shape", num(p.shape)),
        ("color", p.color.map(|c| JsValue::from(c as f64)).unwrap_or(JsValue::from(-1))),
        ("costCents", JsValue::from(total as f64)),
        ("priceCents", JsValue::from(ps.price.unwrap_or(1) as f64)),
        ("paused", false.into()),
        ("active", true.into()),
    ];
    if leaf.family == "bottle" {
        fields.push(("cap", num(p.cap)));
        fields.push(("capColor", num(p.cap_color)));
    }
    if !leaf.contents.is_empty() && p.content.is_some() {
        fields.push(("content", num(p.content)));
    }
    let obj = dom::obj(&fields);
    dom::toggle(&btn, "busy", true);
    let cid = co_str(&co, "id");
    spawn_local(async move {
        match cloud::call("createProduct", &[JsValue::from_str(&cid), obj]).await {
            Ok(_) => {
                burst();
                super::refresh_now().await;
                ctx().toast(&format!("{name}을(를) 팔기 시작했어요"));
                set_timeout(close, Duration::from_millis(900));
            }
            Err(e) => {
                err.set_text_content(Some(if e.code == "permission-denied" { "올리지 못했어요. 회사가 있는지 확인해 주세요" } else { "올리지 못했어요. 잠시 뒤 다시 해 주세요" }));
                dom::toggle(&btn, "busy", false);
            }
        }
        st(|s| s.busy = false);
    });
}

/* ---------- 열기, 닫기 ---------- */

/// 처음 업종을 정할 때 (창업한 뒤에 열려요)
pub fn open_kind() {
    open_as(true);
}
pub fn open() {
    open_as(false);
}
fn open_as(kind_mode: bool) {
    if company().is_none() {
        return;
    }
    let Some(e) = els() else { return };
    // 업종이 정해진 회사는 그 업종 칸부터 열어요 (식품 회사면 식품 안의 것만)
    let kind = company().map(|c| co_str(&c, "kind")).unwrap_or_default();
    let first = if kind_mode {
        Entry::Root
    } else {
        match catalog::node(if kind.is_empty() { "food" } else { &kind }) {
            Some(n) if !n.soon => Entry::List(n),
            _ => Entry::Root,
        }
    };
    st(|s| {
        s.kind_mode = kind_mode;
        s.hist = vec![first];
        s.pick = None;
        s.sel = None;
        s.last_prev.clear();
    });
    e.stage.set_inner_html("");
    set_prev(String::new(), false, false);
    if let Some(v) = make_view(&view_html(first)) {
        let _ = e.stage.append_child(&v);
    }
    sync_top();
    paint_foot();
    e.pk.set_hidden(false);
    anim(&e.pk, r#"[{"transform":"translate3d(0,100%,0)"},{"transform":"none"}]"#, &format!(r#"{{"duration":520,"easing":"{SPRING}"}}"#));
    for (i, w) in qsa(&e.stage, ".pk-orbwrap").iter().enumerate() {
        anim(w, r#"[{"opacity":0,"transform":"translate3d(0,26px,0) scale(.9)"},{"opacity":1,"transform":"none"}]"#, &format!(r#"{{"duration":560,"delay":{},"easing":"{SPRING}","fill":"backwards"}}"#, 160 + i * 80));
    }
}
fn close() {
    st(|s| s.busy = false);
    let Some(e) = els() else { return };
    let done = {
        let e = e.clone();
        move || {
            e.pk.set_hidden(true);
            e.stage.set_inner_html("");
            e.prev.set_inner_html("");
            let _ = e.prev.class_list().remove_2("on", "pk-big");
            st(|s| {
                s.last_prev.clear();
                s.hist.clear();
                s.pick = None;
                s.sel = None;
            });
            for d in qsa(&e.pk, ".pk-done") {
                d.remove();
            }
        }
    };
    if dom::reduce_motion() || e.pk.hidden() {
        return done();
    }
    let a = anim_always(&e.pk, r#"[{"transform":"translate3d(0,0,0)"},{"transform":"translate3d(0,100%,0)"}]"#, r#"{"duration":420,"easing":"cubic-bezier(.4,0,.2,1)","fill":"forwards"}"#);
    let a2 = a.clone();
    on_finish(a, move || {
        if let Some(a) = a2 {
            dom::cancel(&a);
        }
        done();
    });
}

/* ---------- 이벤트 ---------- */

fn on_stage(ev: web_sys::Event) {
    if busy() {
        return;
    }
    if let Some(orb) = dom::closest(&ev, ".pk-orb") {
        return pick_root(dom::data(&orb, "orb").parse().unwrap_or(0), orb);
    }
    if let Some(n) = dom::closest(&ev, "[data-node]") {
        return choose_node(&dom::data(&n, "node"));
    }
    if let Some(v) = dom::closest(&ev, "[data-v]") {
        if matches!(top(), Some(Entry::Step(_))) {
            return choose(dom::data(&v, "v").parse().unwrap_or(0), v);
        }
    }
    if let Some(s) = dom::closest(&ev, "[data-step]") {
        let d: f64 = dom::data(&s, "step").parse().unwrap_or(0.0);
        return set_price(st(|s| s.pick.as_ref().and_then(|p| p.price).unwrap_or(0)) as f64 + d);
    }
    if let Some(s) = dom::closest(&ev, "[data-suggest]") {
        return set_price(dom::data(&s, "suggest").parse().unwrap_or(1.0));
    }
    if dom::closest(&ev, "#pk-sell").is_some() {
        sell();
    }
}
fn target_input(ev: &web_sys::Event) -> Option<web_sys::HtmlInputElement> {
    ev.target()?.dyn_into().ok()
}
fn on_input(ev: web_sys::Event) {
    let Some(t) = target_input(&ev) else { return };
    if st(|s| s.pick.is_none()) {
        return;
    }
    if t.id() == "pk-name" {
        st(|s| s.pick.as_mut().unwrap().name = Some(t.value()));
    }
    if t.id() == "pk-input" {
        let v = dom::js_number(&t.value().replacen(',', ".", 1));
        if v.is_finite() && v >= 0.0 {
            st(|s| s.pick.as_mut().unwrap().price = Some((v * 100.0).round().min(99999999.0) as i64));
        }
        paint_profit();
    }
}
fn on_blur(ev: web_sys::Event) {
    let Some(t) = target_input(&ev) else { return };
    if t.id() != "pk-input" || st(|s| s.pick.is_none()) {
        return;
    }
    let v = st(|s| {
        let p = s.pick.as_mut().unwrap();
        let v = p.price.filter(|v| *v != 0).unwrap_or(1).max(1);
        p.price = Some(v);
        v
    });
    t.set_value(&fmt_cents(v));
    paint_profit();
}
