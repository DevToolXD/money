//! 코인 탭: 모네로(XMR) 채굴. 사용자가 켤 때마다 허락을 받고, 언제든 끌 수 있어요. 앱을 열어도 저절로 켜지지 않아요.
//! 채굴은 웹 워커(app/miner.js, RandomX)가 하고, 풀과는 채굴 프록시(server/miner-proxy, WebSocket)로 이어져요.
//! 채굴한 모네로는 운영자 지갑으로 가고, 사용자는 계정 하나에 1초마다 0.01 토큰(100초에 1 토큰)을 받아요.
//! 보상은 서버 규칙이 계정마다 문서 하나로 세서, 탭을 여러 개 열어도 늘지 않아요 (firestore.rules: mining).

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::dom::{self, by_id};

/// 채굴 프록시 주소 (server/miner-proxy를 올린 뒤 wss://… 로 넣어요). 비어 있으면 켤 수 없어요
const PROXY_DEFAULT: &str = "wss://money-miner.zerodayseverside.workers.dev";
/// 기기에 따로 적은 주소(시험·운영자용)가 있으면 그것을 써요
fn proxy() -> String {
    let p = dom::store_get("coin-proxy", "");
    if p.starts_with("wss://") || p.starts_with("ws://127.0.0.1") { p } else { PROXY_DEFAULT.to_string() }
}
const WORKER: &str = "app/miner.js?v=1";

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static WORKERS: RefCell<Vec<web_sys::Worker>> = const { RefCell::new(Vec::new()) };
    static WS: RefCell<Option<web_sys::WebSocket>> = const { RefCell::new(None) };
    static THREADS: Cell<u32> = const { Cell::new(1) };
    static RATE: RefCell<Vec<f64>> = const { RefCell::new(Vec::new()) }; // 일꾼마다 해시/초
    static HASHES: Cell<f64> = const { Cell::new(0.0) };
    static SHARES: Cell<u32> = const { Cell::new(0) };
    static EARNED: Cell<f64> = const { Cell::new(0.0) };
    static NEXT_AT: Cell<f64> = const { Cell::new(0.0) };
    static STATE: RefCell<String> = RefCell::new("꺼져 있어요".into());
    static TIMER: Cell<i32> = const { Cell::new(0) };
    static BOUND: Cell<bool> = const { Cell::new(false) };
}

fn set_state(t: &str) {
    STATE.with(|s| *s.borrow_mut() = t.to_string());
    paint();
}

fn paint() {
    let on = ON.get();
    if let Some(b) = by_id("coin-toggle") {
        let _ = b.set_attribute("aria-checked", &on.to_string());
        dom::toggle(&b, "on", on);
    }
    if let Some(c) = by_id("coin-card") {
        dom::toggle(&c, "on", on);
    }
    dom::set_text("coin-state", &STATE.with(|s| s.borrow().clone()));
    let rate: f64 = RATE.with(|r| r.borrow().iter().sum());
    dom::set_text("coin-rate", &if on { format!("{} H/s", cloud::fmt_num(rate, 1)) } else { "—".into() });
    dom::set_text("coin-hashes", &cloud::fmt_num(HASHES.get(), 0));
    dom::set_text("coin-shares", &SHARES.get().to_string());
    dom::set_text("coin-earned", &format!("{} 토큰", cloud::fmt_num(EARNED.get(), 0)));
    let left = ((NEXT_AT.get() - js_sys::Date::now()) / 1000.0).ceil();
    dom::set_text("coin-next", &if on && NEXT_AT.get() > 0.0 { format!("{}초", left.max(0.0) as i64) } else { "—".into() });
    for b in dom::qsa(&dom::doc().document_element().expect("html"), "#coin-threads [data-n]") {
        let _ = b.set_attribute("aria-pressed", &(dom::data(&b, "n") == THREADS.get().to_string()).to_string());
    }
}

/// 받을 보상을 받아요 (100초마다). 서버 규칙이 계정마다 한 번만 세요
fn claim() {
    if !ON.get() {
        return;
    }
    spawn_local(async {
        match cloud::call("mineClaim", &[]).await {
            Ok(r) => {
                let amt = cloud::get(&r, "amt").as_f64().unwrap_or(0.0);
                let next = cloud::get(&r, "next").as_f64().unwrap_or(100000.0);
                NEXT_AT.set(js_sys::Date::now() + next);
                if amt > 0.0 {
                    EARNED.set(EARNED.get() + amt);
                    crate::shell::toast(&format!("채굴 보상 {} 토큰을 받았어요", amt as i64));
                }
            }
            Err(e) => cloud::warn(&format!("채굴 보상을 받지 못했어요: {}", e.code)),
        }
        paint();
    });
}

fn post_all(msg: &JsValue) {
    WORKERS.with(|w| w.borrow().iter().for_each(|w| {
        let _ = w.post_message(msg);
    }));
}

fn start_workers() {
    let n = THREADS.get().max(1);
    RATE.with(|r| *r.borrow_mut() = vec![0.0; n as usize]);
    for i in 0..n as usize {
        let opts = web_sys::WorkerOptions::new();
        opts.set_type(web_sys::WorkerType::Module);
        let Ok(w) = web_sys::Worker::new_with_options(WORKER, &opts) else {
            set_state("채굴 일꾼을 만들지 못했어요");
            return;
        };
        let cb = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
            let d = e.data();
            let g = |k: &str| cloud::get(&d, k);
            match g("type").as_string().unwrap_or_default().as_str() {
                "rate" => {
                    let (n, ms) = (g("n").as_f64().unwrap_or(0.0), g("ms").as_f64().unwrap_or(1.0));
                    HASHES.set(HASHES.get() + n);
                    RATE.with(|r| {
                        if let Some(x) = r.borrow_mut().get_mut(i) {
                            *x = n * 1000.0 / ms.max(1.0);
                        }
                    });
                }
                "share" => {
                    let msg = dom::obj(&[("type", "submit".into()), ("job_id", g("job_id")), ("nonce", g("nonce")), ("result", g("result"))]);
                    WS.with(|w| {
                        if let (Some(ws), Ok(s)) = (w.borrow().as_ref(), js_sys::JSON::stringify(&msg)) {
                            let _ = ws.send_with_str(&String::from(s));
                        }
                    });
                }
                "status" => {
                    if g("s").as_string().as_deref() == Some("init") {
                        set_state("채굴 준비 중이에요 (처음엔 조금 걸려요)");
                    } else {
                        set_state("채굴 중이에요");
                    }
                }
                _ => {}
            }
        })
        .into_js_value();
        w.set_onmessage(Some(cb.unchecked_ref()));
        WORKERS.with(|ws| ws.borrow_mut().push(w));
    }
}

fn connect() {
    let Ok(ws) = web_sys::WebSocket::new(&proxy()) else {
        set_state("채굴 서버에 연결하지 못했어요");
        return stop();
    };
    let uid = cloud::cloud().map(|c| cloud::get(&c, "uid").as_string().unwrap_or_default()).unwrap_or_default();
    let ws2 = ws.clone();
    let open = Closure::<dyn FnMut()>::new(move || {
        let rig: String = uid.chars().take(12).collect();
        let _ = ws2.send_with_str(&format!(r#"{{"type":"hello","rig":"{rig}"}}"#));
        set_state("채굴 서버에 연결했어요");
    })
    .into_js_value();
    ws.set_onopen(Some(open.unchecked_ref()));
    let msg = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(|e: web_sys::MessageEvent| {
        let Some(t) = e.data().as_string() else { return };
        let Ok(d) = js_sys::JSON::parse(&t) else { return };
        match cloud::get(&d, "type").as_string().unwrap_or_default().as_str() {
            "job" => post_all(&d),
            "accepted" => {
                SHARES.set(SHARES.get() + 1);
                paint();
            }
            "error" => set_state("채굴 서버에 문제가 있어요. 잠시 뒤 다시 켜 주세요"),
            _ => {}
        }
    })
    .into_js_value();
    ws.set_onmessage(Some(msg.unchecked_ref()));
    let close = Closure::<dyn FnMut()>::new(|| {
        if ON.get() {
            set_state("채굴 서버와 연결이 끊겼어요");
            stop();
        }
    })
    .into_js_value();
    ws.set_onclose(Some(close.unchecked_ref()));
    WS.with(|w| *w.borrow_mut() = Some(ws));
}

fn start() {
    if ON.get() {
        return;
    }
    if proxy().is_empty() {
        crate::shell::toast("채굴 서버를 아직 연결하지 않았어요");
        set_state("채굴 서버 준비 중이에요 (운영자가 서버를 올리면 켤 수 있어요)");
        return;
    }
    if !cloud::signed_in() {
        return crate::shell::toast("로그인하고 채굴할 수 있어요");
    }
    ON.set(true);
    set_state("채굴을 시작하는 중이에요");
    start_workers();
    connect();
    spawn_local(async {
        let _ = cloud::call("mineStart", &[]).await;
        NEXT_AT.set(js_sys::Date::now() + 100000.0);
        paint();
    });
    if let Some(w) = web_sys::window() {
        let cb = Closure::<dyn FnMut()>::new(|| {
            paint();
            if NEXT_AT.get() > 0.0 && js_sys::Date::now() >= NEXT_AT.get() {
                NEXT_AT.set(js_sys::Date::now() + 100000.0);
                claim();
            }
        })
        .into_js_value();
        TIMER.set(w.set_interval_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 1000).unwrap_or(0));
    }
}

pub fn stop() {
    if !ON.replace(false) {
        return;
    }
    claim_final();
    post_all(&dom::obj(&[("type", "stop".into())]));
    WORKERS.with(|w| w.borrow_mut().drain(..).for_each(|w| w.terminate()));
    WS.with(|w| {
        if let Some(ws) = w.borrow_mut().take() {
            ws.set_onclose(None);
            let _ = ws.close();
        }
    });
    if let Some(w) = web_sys::window() {
        w.clear_interval_with_handle(TIMER.get());
    }
    RATE.with(|r| r.borrow_mut().clear());
    NEXT_AT.set(0.0);
    set_state("꺼져 있어요");
}
/// 끌 때 그동안 쌓인 만큼 받아요
fn claim_final() {
    spawn_local(async {
        if let Ok(r) = cloud::call("mineClaim", &[]).await {
            let amt = cloud::get(&r, "amt").as_f64().unwrap_or(0.0);
            if amt > 0.0 {
                EARNED.set(EARNED.get() + amt);
                paint();
            }
        }
    });
}

async fn ask_and_start() {
    if proxy().is_empty() {
        return start();
    }
    let ok = crate::chat::ctx()
        .ask(
            "채굴을 시작할까요?",
            "이 기기의 CPU로 모네로(XMR)를 채굴해요. 배터리를 많이 쓰고 기기가 따뜻해질 수 있어요. 채굴한 모네로는 운영자 지갑으로 가고, 켜 둔 동안 계정 하나에 1초마다 0.01 토큰(100초에 1 토큰)을 받아요. 언제든 끌 수 있어요.",
            "허락하고 시작",
            "취소",
        )
        .await;
    if ok {
        start();
    }
}

/// 코인 탭을 열 때
pub fn open() {
    if !BOUND.replace(true) {
        if let Some(b) = by_id("coin-toggle") {
            dom::on(&b, "click", |_| {
                if ON.get() {
                    stop();
                } else {
                    spawn_local(ask_and_start());
                }
            });
        }
        if let Some(t) = by_id("coin-threads") {
            dom::on(&t, "click", |e| {
                let Some(b) = dom::closest(&e, "[data-n]") else { return };
                if ON.get() {
                    return crate::shell::toast("채굴을 끈 뒤에 바꿀 수 있어요");
                }
                THREADS.set(dom::data(&b, "n").parse().unwrap_or(1));
                paint();
            });
        }
        // 창을 닫으면 멈춰요
        if let Some(w) = web_sys::window() {
            let cb = Closure::<dyn FnMut()>::new(stop).into_js_value();
            let _ = w.add_event_listener_with_callback("pagehide", cb.unchecked_ref());
        }
    }
    paint();
}
