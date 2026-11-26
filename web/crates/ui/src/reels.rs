//! 릴스 탭: SNS에 올라온 게시물을 화면 하나에 하나씩, 위아래로 넘기며 봐요 (넘기면 딱 맞춰 멈춰요).
//! 사진이 있으면 화면 가득, 글만 있으면 큰 글씨로 보여줘요. 오른쪽에 좋아요·댓글, 아래에 쓴 사람과 글.
//! 게시물은 SNS 탭이 듣고 있는 최근 40개를 그대로 써서 서버를 더 읽지 않아요.

use wasm_bindgen::prelude::*;

use crate::dom::{self, by_id, esc, safe_img};

const HEART: &str = r#"<path d="M12 20.5s-7.5-4.6-7.5-10.4A4.3 4.3 0 0 1 12 7.3a4.3 4.3 0 0 1 7.5 2.8c0 5.8-7.5 10.4-7.5 10.4z"/>"#;
const BUBBLE: &str = r#"<path d="M20 11.5c0 4-3.8 7-8.5 7-1 0-2-.1-2.9-.4L4.5 19.5l1.3-3.3C4.7 15 4 13.4 4 11.5c0-4 3.8-7 8-7s8 3 8 7z"/>"#;
const GRADS: [&str; 5] = [
    "linear-gradient(160deg,#5b6cff,#8f5bd8 55%,#f06292)",
    "linear-gradient(160deg,#0ea5e9,#22c55e)",
    "linear-gradient(160deg,#f97316,#ef4444 60%,#a21caf)",
    "linear-gradient(160deg,#111827,#334155 60%,#0ea5e9)",
    "linear-gradient(160deg,#14b8a6,#6366f1)",
];

fn svg(d: &str) -> String {
    format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{d}</svg>"#)
}
fn person(uid: &str) -> (String, String) {
    let p = cloud::call_now("peek", &[JsValue::from_str(uid)]).unwrap_or(JsValue::NULL);
    let g = |k: &str| cloud::get(&p, k).as_string().unwrap_or_default();
    let n = g("name");
    (if n.is_empty() { g("id") } else { n }, g("photo"))
}

pub fn repaint() {
    let Some(el) = by_id("reels") else { return };
    if !cloud::signed_in() {
        return dom::set_html_if(&el, r#"<div class="rl-empty"><b>로그인하면 릴스를 볼 수 있어요</b></div>"#);
    }
    let posts = crate::sns::reel_posts();
    if posts.is_empty() {
        return dom::set_html_if(&el, r#"<div class="rl-empty"><b>아직 게시물이 없어요</b><p>SNS에서 첫 게시물을 올리면 여기서 넘겨 볼 수 있어요</p></div>"#);
    }
    let html: String = posts
        .iter()
        .enumerate()
        .map(|(i, (id, by, text, img, at, likes, liked, rc))| {
            let (n, ph) = person(by);
            let pic = safe_img(img, 300000);
            let bg = if pic.is_empty() { format!("background:{}", GRADS[i % GRADS.len()]) } else { format!("background-image:url({pic})") };
            let big = if pic.is_empty() && !text.is_empty() { format!(r#"<p class="rl-big">{}</p>"#, esc(text)) } else { String::new() };
            let cap = if !pic.is_empty() && !text.is_empty() { format!(r#"<p class="rl-cap">{}</p>"#, esc(text)) } else { String::new() };
            format!(
                r#"<section class="rl-s{photo}" data-id="{id}" style="top:{top}%"><div class="rl-bg" style="{bg}"></div>{big}<div class="rl-side"><button type="button" class="rl-btn{on}" data-act="like" aria-label="좋아요">{heart}<span>{lk}</span></button><button type="button" class="rl-btn" data-act="replies" aria-label="댓글">{bubble}<span>{rc}</span></button></div><div class="rl-info"><button type="button" class="rl-who" data-uid="{by}">{av}<b>{name}{bl}</b><small>{ago}</small></button>{cap}</div></section>"#,
                photo = if pic.is_empty() { "" } else { " has-photo" },
                top = i * 100,
                id = esc(id),
                on = if *liked { " on" } else { "" },
                heart = svg(HEART),
                lk = cloud::fmt_num(*likes, 0),
                bubble = svg(BUBBLE),
                rc = cloud::fmt_num(*rc, 0),
                by = esc(by),
                av = crate::presence::wrap(dom::avatar_html(&n, &ph, "avatar contact rl-av"), by),
                name = esc(&n),
                bl = crate::profile::blue(by),
                ago = crate::sns::ago(*at),
            )
        })
        .collect::<String>();
    let n = posts.len();
    if IDX.get() >= n {
        IDX.set(n - 1);
    }
    dom::set_html_if(&el, &format!(r#"<div class="rl-track" id="rl-track">{html}</div>"#));
    // 다시 그려도 보던 칸에 그대로 있어요
    place(0.0, false);
}

/* ---------- 넘기기 (쇼츠처럼): 손가락을 따라오다가, 놓으면 0.3초 안에 다음·이전 칸으로 딱 넘어가요 ---------- */

/// 칸 위치에 손가락이 끈 만큼(dy px)을 더해 옮겨요. snap이면 짧은 움직임으로
fn place(dy: f64, snap: bool) {
    let Some(t) = by_id("rl-track") else { return };
    let st = t.style();
    let _ = st.set_property("transition", if snap { "transform 0.3s cubic-bezier(.2,.8,.2,1)" } else { "none" });
    let _ = st.set_property("transform", &format!("translate3d(0, calc({}% + {dy:.1}px), 0)", -(IDX.get() as f64) * 100.0));
}
fn go_to(i: usize) {
    let n = crate::sns::reel_posts().len();
    if n == 0 {
        return;
    }
    let i = i.min(n - 1);
    let changed = i != IDX.get();
    IDX.set(i);
    place(0.0, true);
    if changed {
        dom::vibrate(5);
        leptos::prelude::set_timeout(settle, std::time::Duration::from_millis(320));
    }
}

fn bind() {
    let Some(el) = by_id("reels") else { return };
    // 손가락 넘기기: 누를 때 화면 높이를 한 번만 재고, 움직이는 동안은 transform만 바꿔요
    let e1 = el.clone();
    dom::on(&el, "pointerdown", move |e| {
        let Some(p) = wasm_bindgen::JsCast::dyn_ref::<web_sys::PointerEvent>(&e) else { return };
        if !p.is_primary() {
            return;
        }
        DRAG.set((p.client_y() as f64, js_sys::Date::now(), true, false, e1.client_height().max(1) as f64, 0.0));
    });
    dom::on(&el, "pointermove", |e| {
        let (y0, t0, on, moved, h, _) = DRAG.get();
        if !on {
            return;
        }
        let Some(p) = wasm_bindgen::JsCast::dyn_ref::<web_sys::PointerEvent>(&e) else { return };
        let mut dy = p.client_y() as f64 - y0;
        if !moved && dy.abs() < 8.0 {
            return;
        }
        // 첫 칸에서 아래로, 마지막 칸에서 위로 끌면 고무줄처럼 덜 따라와요
        let n = crate::sns::reel_posts().len();
        if (IDX.get() == 0 && dy > 0.0) || (IDX.get() + 1 >= n && dy < 0.0) {
            dy *= 0.3;
        }
        DRAG.set((y0, t0, true, true, h, dy));
        place(dy, false);
    });
    for ev in ["pointerup", "pointercancel"] {
        dom::on(&el, ev, |_| {
            let (_, t0, on, moved, h, dy) = DRAG.get();
            if !on {
                return;
            }
            DRAG.set((0.0, 0.0, false, moved, h, 0.0));
            if !moved {
                return;
            }
            let v = dy / (js_sys::Date::now() - t0).max(1.0); // px/ms
            let i = IDX.get();
            if dy < -h * 0.18 || v < -0.35 {
                go_to(i + 1);
            } else if (dy > h * 0.18 || v > 0.35) && i > 0 {
                go_to(i - 1);
            } else {
                place(0.0, true);
            }
        });
    }
    // 컴퓨터: 바퀴를 굴리면 한 칸씩
    dom::on(&el, "wheel", |e| {
        let Some(w) = wasm_bindgen::JsCast::dyn_ref::<web_sys::WheelEvent>(&e) else { return };
        e.prevent_default();
        if js_sys::Date::now() - WHEEL.get() < 450.0 || w.delta_y().abs() < 12.0 {
            return;
        }
        WHEEL.set(js_sys::Date::now());
        let i = IDX.get();
        if w.delta_y() > 0.0 {
            go_to(i + 1);
        } else if i > 0 {
            go_to(i - 1);
        }
    });
    dom::on(&el, "click", |e| {
        // 끌어서 넘긴 뒤 손을 뗄 때의 누름은 버튼으로 치지 않아요
        if DRAG.get().3 {
            DRAG.set((0.0, 0.0, false, false, 0.0, 0.0));
            e.stop_propagation();
            return;
        }
        if let Some(b) = dom::closest(&e, "[data-uid]") {
            let uid = dom::data(&b, "uid");
            return crate::profile::show(&uid, &person(&uid).0, None);
        }
        let Some(b) = dom::closest(&e, "[data-act]") else { return double_tap(&e) };
        let Some(s) = dom::closest(&e, ".rl-s") else { return };
        let id = dom::data(&s, "id");
        match dom::data(&b, "act").as_str() {
            "like" => crate::sns::toggle_like(&id),
            _ => crate::reel_comments::open(&id),
        }
    });
}

/// 두 번 톡 누르면 좋아요 (인스타처럼). 아이폰은 dblclick을 잘 안 보내서 두 번 누른 시간을 직접 재요
fn double_tap(e: &web_sys::Event) {
    let Some(s) = dom::closest(e, ".rl-s") else { return };
    let id = dom::data(&s, "id");
    let t = js_sys::Date::now();
    let (lid, lt) = LAST_TAP.with(|l| l.borrow().clone());
    if lid != id || t - lt > 350.0 {
        LAST_TAP.with(|l| *l.borrow_mut() = (id, t));
        return;
    }
    LAST_TAP.with(|l| *l.borrow_mut() = (String::new(), 0.0));
    // 가운데에 큰 하트가 톡 튀어나왔다 사라져요
    if let Ok(h) = dom::doc().create_element("div") {
        h.set_class_name("rl-heart");
        h.set_inner_html(&svg(HEART));
        let _ = s.append_child(&h);
        leptos::prelude::set_timeout(move || h.remove(), std::time::Duration::from_millis(820));
    }
    dom::vibrate(8);
    let liked = crate::sns::reel_posts().iter().any(|p| p.0 == id && p.6);
    if !liked {
        crate::sns::toggle_like(&id);
    }
}

thread_local! {
    static LAST_TAP: std::cell::RefCell<(String, f64)> = const { std::cell::RefCell::new((String::new(), 0.0)) };
    static BOUND: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// 스크롤이 멈추면 보는 중인 게시물을 알려주는 시계, 지금 보는 게시물 번호
    /// 지금 보는 칸, 끌기 (시작 y, 시각, 누르는 중, 움직였는지, 화면 높이, 끈 거리), 마지막 바퀴 시각
    static IDX: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static DRAG: std::cell::Cell<(f64, f64, bool, bool, f64, f64)> = const { std::cell::Cell::new((0.0, 0.0, false, false, 0.0, 0.0)) };
    static WHEEL: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
    static WATCHING: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// 스크롤이 멈춘 뒤 한 번만: 지금 화면의 게시물을 본 것으로 적고, 6초 넘게 보면 그 사람에게 관심을 더해요 (추천 알고리즘)
fn settle() {
    let i = IDX.get();
    let posts = crate::sns::reel_posts();
    let Some(p) = posts.get(i) else { return };
    let (id, by) = (p.0.clone(), p.1.clone());
    if WATCHING.with(|w| *w.borrow() == id) {
        return;
    }
    WATCHING.with(|w| *w.borrow_mut() = id.clone());
    crate::algo::seen(&id);
    leptos::prelude::set_timeout(
        move || {
            if WATCHING.with(|w| *w.borrow() == id) && dom::screen_active("reels") && !dom::doc_hidden() {
                crate::algo::bump(&by, 0.5);
            }
        },
        std::time::Duration::from_millis(6000),
    );
}
/// 릴스 탭을 열 때
pub fn open() {
    if !BOUND.replace(true) {
        bind();
    }
    crate::sns::ensure_watch();
    crate::sns::gifts_refresh(false);
    let before = crate::sns::reel_posts().first().map(|p| p.0.clone());
    crate::sns::rerank(true, false);
    repaint();
    // 추천 순서가 새로 정해졌으면 맨 위부터 봐요
    if crate::sns::reel_posts().first().map(|p| p.0.clone()) != before {
        IDX.set(0);
        place(0.0, false);
    }
    leptos::prelude::set_timeout(settle, std::time::Duration::from_millis(1200));
}
