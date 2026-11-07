//! 튜토리얼: 화면 위에 어두운 막을 깔고, 눌러 볼 곳만 밝게 뚫어서 "여기를 눌러 보세요"를 하나씩 보여 줘요.
//! 처음 로그인하면 기본 사용법을, 사업 탭에서는 내 회사 업종(식품·컴퓨터·인공지능·원자력)에 맞는 사용법을 한 번씩 보여 줘요.
//! 본 것은 기기에 기억하고, 설정 → 튜토리얼 다시 보기로 다시 볼 수 있어요.

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::dom::{self, by_id, esc};

#[derive(Clone, Copy)]
struct Step {
    sel: &'static str,
    title: &'static str,
    text: &'static str,
    /// true면 그곳을 눌러야 다음으로 가요
    click: bool,
    /// 이 곳이 화면에 나타나면 다음으로 가요 (핫바 넘기기처럼 누르기가 아닌 동작)
    until: &'static str,
}
const fn st(sel: &'static str, title: &'static str, text: &'static str, click: bool) -> Step {
    Step { sel, title, text, click, until: "" }
}
const fn swipe(sel: &'static str, title: &'static str, text: &'static str, until: &'static str) -> Step {
    Step { sel, title, text, click: false, until }
}

const INTRO: &[Step] = &[
    st(".tabbar .tab[data-tab=\"messages\"]", "메시지", "여기를 눌러 보세요. 친구와 대화하고, 토큰을 선물하거나 요청해요. 보낸 선물은 상대가 받기 전에 취소할 수 있어요.", true),
    st(".tabbar .tab[data-tab=\"money\"]", "돈 탭", "여기를 눌러 보세요. 내 토큰과 이자 상품을 볼 수 있어요. 이율이 높을수록 토큰이 빨리 불어나요.", true),
    st(".tabbar .tab[data-tab=\"work\"]", "일 탭", "여기를 눌러 보세요. 회사에 지원하거나 내 회사에서 일해요. 일하면 에너지가 줄어요.", true),
    st(".tabbar .tab[data-tab=\"shop\"]", "상점", "여기를 눌러 보세요. 음식과 물, 다른 회사가 만든 상품을 사요.", true),
    st("#status-bubble", "내 상태", "여기를 눌러 보세요. 배고픔·목마름·에너지를 볼 수 있어요.", true),
    st("#fab-tiles [data-k=\"h\"]", "배고픔", "여기를 눌러 보세요. 배고픔이 20% 밑으로 내려가면 몇 초마다 화면이 양옆으로 떨려요. 상점에서 음식을 사 먹으면 돌아와요.", true),
    st("#fab-tiles [data-k=\"t\"]", "목마름", "여기를 눌러 보세요. 목마름이 20% 밑으로 내려가면 화면 가장자리가 빨갛게 물들어요. 낮을수록 더 진해져요. 물을 마시면 돌아와요.", true),
    st("#fab-tiles [data-k=\"e\"]", "에너지", "여기를 눌러 보세요. 에너지가 20% 밑으로 내려가면 몇 초마다 초점이 풀렸다 돌아와요. 에너지가 없어도 일은 할 수 있어요.", true),
    st("#status-bubble", "상태 닫기", "여기를 눌러 상태를 닫아요. 이 동그라미는 늘 화면에 있어요.", true),
    swipe(".tabbar", "핫바 넘기기", "탭 바를 손가락으로 옆으로 쓱 밀어 보세요. 랭킹·인벤토리·사업·SNS 탭이 나와요.", ".tabbar .tab[data-tab=\"business\"]"),
    st(".tabbar .tab[data-tab=\"ranking\"]", "랭킹", "여기를 눌러 보세요. 토큰을 가장 많이 가진 사람들이에요. 사람을 누르면 프로필이 나와요.", true),
    st(".tabbar .tab[data-tab=\"inventory\"]", "인벤토리", "여기를 눌러 보세요. 산 물건과 만든 물건이 모여 있어요.", true),
    st(".tabbar .tab[data-tab=\"business\"]", "사업", "여기를 눌러 보세요. 10 토큰으로 회사를 만들고, 업종을 정해 상품을 만들어 팔아요.", true),
    st(".tabbar .tab[data-tab=\"untitled3\"]", "SNS", "여기를 눌러 보세요. 사진과 글을 올리고 좋아요와 댓글을 달아요.", true),
    st(".tabbar .tab-account", "메뉴", "여기를 눌러 보세요. 코인, 프로필, 친구 초대, 설정, 피드백이 나와요.", true),
    st(".qm-row[data-go=\"coin\"]", "코인", "여기를 눌러 보세요. 허락하면 채굴을 켜서 100초마다 토큰을 받아요.", true),
    st(".tabbar .tab-account", "메뉴", "메뉴를 한 번 더 눌러 보세요.", true),
    st(".qm-row[data-go=\"invite\"]", "친구 초대", "여기를 눌러 보세요. 초대 링크를 복사해 친구에게 주면, 친구 한 명마다 50 토큰을 받아요.", true),
    st(".tabbar .tab-account", "메뉴", "마지막으로 메뉴를 한 번 더 눌러 보세요.", true),
    st(".qm-row[data-go=\"settings\"]", "설정", "여기를 눌러 보세요. 테마, 알림, 진동, 튜토리얼 다시 보기가 있어요.", true),
];
// 사업 안내: 누르면 다른 화면이 열리는 곳(팔 상품 정하기)은 맨 끝에 둬요
const FOOD: &[Step] = &[
    st("#cash-amt", "회사 금고", "여기를 눌러 보세요. 재료값이 나갈 금고에 넣을 토큰 수를 적는 곳이에요.", true),
    st("#fac-n", "식품 공장", "여기를 눌러 보세요. 만들 개수를 정하면 시간·재료값·이익이 바로 나와요.", true),
    st("#hire-plus", "직원 채용", "여기를 눌러 보세요. 모집 인원을 늘리면 다른 사람이 지원해요.", true),
    st("#stf-fund-n", "급여 금고", "여기를 눌러 보세요. 직원 임금으로 쓸 토큰을 넣는 곳이에요.", true),
    st("#biz-pick", "팔 음식 정하기", "마지막이에요. 여기를 눌러 팔 음식을 정해 보세요.", true),
];
const PC: &[Step] = &[
    st("#biz-lab", "컴퓨터 연구실", "여기를 눌러 보세요. 용량·속도·전기값을 올려 출시하는 곳이에요. 숫자는 일 탭의 부품 검사로 올라요.", true),
    st("#hire-plus", "직원 채용", "여기를 눌러 보세요. 직원이 많을수록 연구가 빨라져요.", true),
    st("#stf-fund-n", "급여 금고", "여기를 눌러 보세요. 직원 임금으로 쓸 토큰을 넣는 곳이에요.", true),
    st("#biz-pick", "팔 상품 정하기", "마지막이에요. 여기를 눌러 상점에 올릴 상품을 정해 보세요.", true),
];
const AI: &[Step] = &[
    st("#biz-lab", "인공지능 연구실", "여기를 눌러 보세요. 데이터로 학습하고 출시하는 곳이에요. 전기와 컴퓨터는 다른 회사에서 사요.", true),
    st("#hire-plus", "직원 채용", "여기를 눌러 보세요. 직원이 많을수록 데이터가 빨리 모여요.", true),
    st("#stf-fund-n", "급여 금고", "여기를 눌러 보세요. 직원 임금으로 쓸 토큰을 넣는 곳이에요.", true),
    st("#biz-pick", "팔 상품 정하기", "마지막이에요. 여기를 눌러 상점에 올릴 상품을 정해 보세요.", true),
];
const NUCLEAR: &[Step] = &[
    st("#biz-lab", "원자력 발전소", "여기를 눌러 보세요. 전기를 만들어 파는 곳이에요. 성능은 일 탭의 핵연료 배합으로 올라요.", true),
    st("#cash-amt", "회사 금고", "여기를 눌러 보세요. 발전 비용이 나갈 금고에 넣을 토큰 수를 적는 곳이에요.", true),
    st("#hire-plus", "직원 채용", "여기를 눌러 보세요. 직원이 많을수록 성능이 빨리 올라요.", true),
    st("#stf-fund-n", "급여 금고", "여기를 눌러 보세요. 직원 임금으로 쓸 토큰을 넣는 곳이에요.", true),
];

thread_local! {
    static RUN: RefCell<Option<(String, Vec<Step>, usize)>> = const { RefCell::new(None) };
    static TICK: Cell<i32> = const { Cell::new(0) };
    static BOUND: Cell<bool> = const { Cell::new(false) };
}

fn done_key(name: &str) -> String {
    format!("coach-{name}")
}
/// 시험(에뮬레이터 묶음)에서는 저절로 띄우지 않아요 (화면 시험이 막혀요)
fn auto_ok() -> bool {
    let fb = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("fb")).unwrap_or(JsValue::UNDEFINED);
    !cloud::get(&fb, "emu").is_truthy() || dom::store_get("coach-test", "") == "1"
}

fn target(sel: &str) -> Option<web_sys::HtmlElement> {
    let el: web_sys::HtmlElement = dom::doc().query_selector(sel).ok().flatten()?.dyn_into().ok()?;
    let r = el.get_bounding_client_rect();
    (r.width() > 0.0 && r.height() > 0.0 && !el.closest("[hidden]").ok().flatten().is_some()).then_some(el)
}

fn root() -> Option<web_sys::HtmlElement> {
    if let Some(e) = by_id("coach") {
        return Some(e);
    }
    let app = dom::app_el()?;
    let el: web_sys::HtmlElement = dom::doc().create_element("div").ok()?.dyn_into().ok()?;
    el.set_id("coach");
    el.set_class_name("coach");
    el.set_hidden(true);
    el.set_inner_html(r#"<i class="cb t"></i><i class="cb r"></i><i class="cb b"></i><i class="cb l"></i><span class="coach-ring" aria-hidden="true"></span><div class="coach-tip" role="dialog" aria-live="polite"><p class="coach-n"></p><b class="coach-t"></b><p class="coach-x"></p><p class="coach-hint">빛나는 곳을 눌러 주세요</p></div>"#);
    let _ = app.append_child(&el);
    dom::on(&el, "click", |e| {
        if dom::closest(&e, ".coach-skip").is_some() {
            finish();
        } else if dom::closest(&e, ".coach-next").is_some() {
            next();
        }
    });
    Some(el)
}

fn bind() {
    if BOUND.replace(true) {
        return;
    }
    // 눌러야 하는 단계: 그곳을 누르면(누름은 그대로 전해져요) 다음 단계로 가요
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(|e: web_sys::Event| {
        let Some(step) = RUN.with(|r| r.borrow().as_ref().map(|(_, s, i)| s[*i])) else { return };
        if !step.click {
            return;
        }
        let Some(t) = e.target().and_then(|t| t.dyn_into::<web_sys::Node>().ok()) else { return };
        if target(step.sel).is_some_and(|el| el.contains(Some(&t))) {
            let cb2 = Closure::once_into_js(next);
            let _ = web_sys::window().map(|w| w.set_timeout_with_callback_and_timeout_and_arguments_0(cb2.unchecked_ref(), 350));
        }
    })
    .into_js_value();
    let o = web_sys::AddEventListenerOptions::new();
    o.set_capture(true);
    let _ = dom::doc().add_event_listener_with_callback_and_add_event_listener_options("click", cb.unchecked_ref(), &o);
}

/// 지금 단계를 그려요 (뚫린 곳, 고리, 말풍선 자리). 화면이 움직여도 따라가요
fn place() {
    let Some((_, steps, i)) = RUN.with(|r| r.borrow().clone()) else { return };
    let (Some(el), Some(app)) = (root(), dom::app_el()) else { return };
    let step = steps[i];
    if !step.until.is_empty() && target(step.until).is_some() {
        return next();
    }
    let Some(t) = target(step.sel) else { return };
    let a = app.get_bounding_client_rect();
    let r = t.get_bounding_client_rect();
    let pad = 6.0;
    let (x, y, w, h) = (r.left() - a.left() - pad, r.top() - a.top() - pad, r.width() + pad * 2.0, r.height() + pad * 2.0);
    let (aw, ah) = (a.width(), a.height());
    let set = |q: &str, css: String| {
        if let Some(e) = el.query_selector(q).ok().flatten().and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()) {
            let _ = e.set_attribute("style", &css);
        }
    };
    set(".cb.t", format!("left:0;top:0;width:{aw}px;height:{}px", y.max(0.0)));
    set(".cb.b", format!("left:0;top:{}px;width:{aw}px;height:{}px", y + h, (ah - y - h).max(0.0)));
    set(".cb.l", format!("left:0;top:{y}px;width:{}px;height:{h}px", x.max(0.0)));
    set(".cb.r", format!("left:{}px;top:{y}px;width:{}px;height:{h}px", x + w, (aw - x - w).max(0.0)));
    set(".coach-ring", format!("left:{x}px;top:{y}px;width:{w}px;height:{h}px"));
    // 말풍선: 아래 자리가 넉넉하면 아래, 아니면 위
    let below = ah - (y + h) > 200.0;
    let tip_x = (x + w / 2.0 - 150.0).clamp(12.0, (aw - 312.0).max(12.0));
    set(".coach-tip", if below { format!("left:{tip_x}px;top:{}px", y + h + 14.0) } else { format!("left:{tip_x}px;bottom:{}px", ah - y + 14.0) });
}

fn show() {
    let Some((name, steps, i)) = RUN.with(|r| r.borrow().clone()) else { return };
    let Some(el) = root() else { return };
    // 없는 곳(숨어 있거나 아직 안 그려진 칸)은 건너뛰어요
    let step = steps[i];
    let Some(t) = target(step.sel) else {
        return next();
    };
    t.scroll_into_view_with_scroll_into_view_options(&{
        let o = web_sys::ScrollIntoViewOptions::new();
        o.set_block(web_sys::ScrollLogicalPosition::Center);
        o
    });
    let q = |s: &str| el.query_selector(s).ok().flatten();
    if let Some(n) = q(".coach-n") {
        n.set_text_content(Some(&format!("{} / {}", i + 1, steps.len())));
    }
    if let Some(n) = q(".coach-t") {
        n.set_text_content(Some(step.title));
    }
    if let Some(n) = q(".coach-x") {
        n.set_inner_html(&esc(step.text));
    }
    dom::toggle(&el, "tap", step.click || !step.until.is_empty());
    dom::toggle(&el, "swipe", !step.until.is_empty());
    if let Some(h) = q(".coach-hint") {
        h.set_text_content(Some(if step.until.is_empty() { "빛나는 곳을 눌러 주세요" } else { "빛나는 탭 바를 옆으로 밀어 주세요" }));
    }
    el.set_hidden(false);
    let _ = el.offset_width();
    let _ = el.class_list().add_1("on");
    let _ = name;
    place();
    // 고리가 한 번 톡 튀어요 (단계마다)
    if let Some(r) = q(".coach-ring") {
        let _ = r.class_list().remove_1("pop");
        let _ = r.dyn_ref::<web_sys::HtmlElement>().map(|h| h.offset_width());
        let _ = r.class_list().add_1("pop");
    }
}

fn next() {
    let more = RUN.with(|r| {
        let mut r = r.borrow_mut();
        if let Some((_, steps, i)) = r.as_mut() {
            *i += 1;
            *i < steps.len()
        } else {
            false
        }
    });
    if more {
        show();
    } else {
        finish();
    }
}

fn finish() {
    let name = RUN.with(|r| r.borrow_mut().take().map(|(n, _, _)| n));
    if let Some(n) = name {
        dom::store_set(&done_key(&n), "1");
        if n == "intro2" {
            crate::shell::toast("준비 끝! 사업 탭에서 회사를 만들어 보세요");
        }
    }
    if let Some(w) = web_sys::window() {
        w.clear_interval_with_handle(TICK.get());
    }
    if let Some(el) = by_id("coach") {
        let _ = el.class_list().remove_1("on");
        el.set_hidden(true);
    }
}

fn run(name: &str, steps: &[Step], force: bool) {
    if RUN.with(|r| r.borrow().is_some()) || (!force && dom::store_get(&done_key(name), "") == "1") {
        return;
    }
    bind();
    RUN.with(|r| *r.borrow_mut() = Some((name.to_string(), steps.to_vec(), 0)));
    if let Some(w) = web_sys::window() {
        let cb = Closure::<dyn FnMut()>::new(place).into_js_value();
        TICK.set(w.set_interval_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 250).unwrap_or(0));
    }
    show();
}

/// 앱을 열 때: 로그인되면 처음 한 번 기본 사용법을 보여 줘요
pub fn init() {
    if !auto_ok() || dom::store_get(&done_key("intro2"), "") == "1" {
        return;
    }
    spawn_local(async {
        for _ in 0..120 {
            if cloud::signed_in() && crate::chat::live() {
                dom::sleep(1500).await;
                run("intro2", INTRO, false);
                return;
            }
            dom::sleep(1000).await;
        }
    });
}

/// 사업 탭: 내 회사 업종에 맞는 사용법을 업종마다 한 번 보여 줘요
pub fn business(kind: &str) {
    if !auto_ok() {
        return;
    }
    let (name, steps) = match kind {
        "pc" => ("biz-pc", PC),
        "ai" => ("biz-ai", AI),
        "nuclear" => ("biz-nuclear", NUCLEAR),
        _ => ("biz-food", FOOD),
    };
    if dom::store_get(&done_key(name), "") == "1" {
        return;
    }
    let name = name.to_string();
    spawn_local(async move {
        dom::sleep(900).await;
        if dom::screen_active("business") {
            run(&name, steps, false);
        }
    });
}

/// 설정 → 튜토리얼 다시 보기: 본 기록을 지우고 기본 사용법부터 다시 보여 줘요 (사업 탭 사용법은 사업 탭을 열면 다시 나와요)
pub fn replay() {
    for k in ["intro", "intro2", "biz-food", "biz-pc", "biz-ai", "biz-nuclear"] {
        dom::store_set(&done_key(k), "");
    }
    run("intro2", INTRO, true);
}
