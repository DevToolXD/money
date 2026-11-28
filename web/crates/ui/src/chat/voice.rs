//! 음성 메모 (아이메시지 화면 영상을 프레임 단위로 보고 맞췄어요).
//!   입력칸이 비면 오른쪽에 얇은 소리 막대 아이콘이 보여요 → 누르면 입력줄이 + 까지 덮으며 넓어져요
//!   녹음 중: 점선 파형에 소리 크기대로 빨간 막대가 들어와 왼쪽으로 흘러요 · 시간(빨강) · 빨간 멈춤 버튼
//!   멈추면: 왼쪽 X(지우기) · 재생 버튼 · 작은 파형 · 길이 · 파란 보내기 버튼
//! 소리는 MediaRecorder로 녹음해서 data:audio 글자로 바꿔 메시지(type "voice")에 담아요 (서버 규칙: 180000자·60초 이하).
//! 브라우저 소리 기능은 web-sys 기능 목록에 없어서 js_sys로 직접 불러요.

use std::cell::{Cell, RefCell};

use js_sys::{Array, Function, Promise, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};

use super::{open_chat, queue_send, toast};
use crate::dom::{self, by_id};

/// 가장 길게 녹음하는 시간 (초). 메시지 한 개가 서버 규칙의 크기(180000자) 안에 들어가게 짧게 해요
const MAX_S: f64 = 30.0;
const MAX_CHARS: usize = 180_000;
/// 이보다 짧게 누르면 실수로 보고 버려요
const MIN_S: f64 = 0.8;

#[derive(Clone, Copy, PartialEq)]
enum St {
    Idle,
    Rec,
    Review,
}

thread_local! {
    static ST: Cell<St> = const { Cell::new(St::Idle) };
    static GEN: Cell<u32> = const { Cell::new(0) };
    static RECORDER: RefCell<JsValue> = const { RefCell::new(JsValue::UNDEFINED) };
    static STREAM: RefCell<JsValue> = const { RefCell::new(JsValue::UNDEFINED) };
    static ACTX: RefCell<JsValue> = const { RefCell::new(JsValue::UNDEFINED) };
    static ANALYSER: RefCell<JsValue> = const { RefCell::new(JsValue::UNDEFINED) };
    static CHUNKS: RefCell<Vec<JsValue>> = const { RefCell::new(Vec::new()) };
    static LEVELS: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static T0: Cell<f64> = const { Cell::new(0.0) };
    static DUR: Cell<f64> = const { Cell::new(0.0) };
    static BLOB: RefCell<JsValue> = const { RefCell::new(JsValue::UNDEFINED) };
    static URL: RefCell<String> = const { RefCell::new(String::new()) };
    static PREVIEW: RefCell<JsValue> = const { RefCell::new(JsValue::UNDEFINED) };
    static PLAY: RefCell<(JsValue, i64)> = const { RefCell::new((JsValue::UNDEFINED, -1)) };
    static BOUND: Cell<bool> = const { Cell::new(false) };
}

/* ---------- 브라우저 호출 도우미 ---------- */

fn g(o: &JsValue, k: &str) -> JsValue {
    Reflect::get(o, &JsValue::from_str(k)).unwrap_or(JsValue::UNDEFINED)
}
fn call(o: &JsValue, name: &str, args: &[JsValue]) -> Result<JsValue, JsValue> {
    let f: Function = g(o, name).dyn_into().map_err(|_| JsValue::from_str("no method"))?;
    f.apply(o, &args.iter().cloned().collect::<Array>())
}
fn win() -> JsValue {
    js_sys::global().into()
}
fn construct(name: &str, args: &[JsValue]) -> Result<JsValue, JsValue> {
    let c: Function = g(&win(), name).dyn_into().map_err(|_| JsValue::from_str("no class"))?;
    Reflect::construct(&c, &args.iter().cloned().collect::<Array>())
}
async fn aw(v: Result<JsValue, JsValue>) -> Result<JsValue, JsValue> {
    let v = v?;
    match v.dyn_into::<Promise>() {
        Ok(p) => JsFuture::from(p).await,
        Err(v) => Ok(v),
    }
}
fn now() -> f64 {
    js_sys::Date::now()
}
fn mmss(s: f64) -> String {
    let s = s.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

fn b64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/* ---------- 그림 ---------- */

const PLAY_SVG: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5.2v13.6a1 1 0 0 0 1.5.9l11-6.8a1 1 0 0 0 0-1.8l-11-6.8A1 1 0 0 0 8 5.2z" fill="currentColor"/></svg>"#;
const PAUSE_SVG: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="6.5" y="5" width="4" height="14" rx="1.2" fill="currentColor"/><rect x="13.5" y="5" width="4" height="14" rx="1.2" fill="currentColor"/></svg>"#;
const X_SVG: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18"/></svg>"#;
const UP_SVG: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 19V5M5 12l7-7 7 7"/></svg>"#;

/// 파형 그리기: levels(0~9) 막대. live면 점선 바탕에 빨간 막대가 오른쪽에서 들어와요
fn draw_wave(canvas_sel: &str, levels: &[u8], live: bool) {
    let Some(cv) = dom::doc().query_selector(canvas_sel).ok().flatten().and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok()) else { return };
    let dpr = web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0).clamp(1.0, 3.0);
    let (cw, ch) = (cv.client_width() as f64, cv.client_height() as f64);
    if cw < 4.0 || ch < 4.0 {
        return;
    }
    let (pw, ph) = ((cw * dpr) as u32, (ch * dpr) as u32);
    if cv.width() != pw || cv.height() != ph {
        cv.set_width(pw);
        cv.set_height(ph);
    }
    let Some(cx) = cv.get_context("2d").ok().flatten().and_then(|c| c.dyn_into::<web_sys::CanvasRenderingContext2d>().ok()) else { return };
    let _ = cx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);
    cx.clear_rect(0.0, 0.0, cw, ch);
    let (step, bw) = (if live { 5.0 } else { 4.0 }, if live { 2.4 } else { 2.2 });
    let n = (cw / step).floor() as usize;
    let mid = ch / 2.0;
    for k in 0..n {
        let x = cw - (n - k) as f64 * step + (step - bw) / 2.0;
        let li = if live {
            // 맨 오른쪽이 가장 새 소리
            let back = n - 1 - k;
            levels.len().checked_sub(1 + back).map(|i| levels[i])
        } else if levels.is_empty() {
            None
        } else {
            Some(levels[(k * levels.len() / n).min(levels.len() - 1)])
        };
        let (h, col) = match li {
            Some(v) => {
                let h = 2.4 + v as f64 / 9.0 * (ch - 3.0);
                (h, if live { "#ff3b30" } else { "#1c1c1e" })
            }
            None => (2.4, if live { "rgba(255,59,48,0.42)" } else { "rgba(60,60,67,0.25)" }),
        };
        // 끝이 둥근 세로 선으로 막대를 그려요
        cx.set_stroke_style_str(col);
        cx.set_line_width(bw);
        cx.set_line_cap("round");
        cx.begin_path();
        let cxm = x + bw / 2.0;
        let half = (h - bw).max(0.0) / 2.0;
        cx.move_to(cxm, mid - half);
        cx.line_to(cxm, mid + half);
        cx.stroke();
    }
}

/* ---------- 흐름 ---------- */

fn composer() -> Option<web_sys::HtmlElement> {
    by_id("composer")
}
fn set_rec_ui(html: &str, animate: bool) {
    let Some(c) = composer() else { return };
    let _ = c.class_list().add_1("rec");
    if let Some(old) = by_id("vrec") {
        old.remove();
    }
    let Ok(el) = dom::doc().create_element("div") else { return };
    el.set_id("vrec");
    el.set_class_name(if animate { "vrec enter" } else { "vrec" });
    el.set_inner_html(html);
    let _ = c.append_child(&el);
}
fn clear_ui() {
    if let Some(v) = by_id("vrec") {
        v.remove();
    }
    if let Some(c) = composer() {
        let _ = c.class_list().remove_1("rec");
    }
}

/// 마이크가 되고 정해진 파일 종류가 있나
fn pick_mime() -> String {
    let mr = g(&win(), "MediaRecorder");
    if !mr.is_function() {
        return String::new();
    }
    for m in ["audio/mp4", "audio/webm;codecs=opus", "audio/webm", "audio/ogg;codecs=opus"] {
        if call(&mr, "isTypeSupported", &[m.into()]).ok().and_then(|v| v.as_bool()).unwrap_or(false) {
            return m.to_string();
        }
    }
    String::new()
}

async fn start() {
    if ST.get() != St::Idle || open_chat().is_none() {
        return;
    }
    let md = g(&g(&win(), "navigator"), "mediaDevices");
    if !md.is_object() || !g(&win(), "MediaRecorder").is_function() {
        return toast("이 기기에서는 음성 메모를 쓸 수 없어요");
    }
    ST.set(St::Rec);
    let constraints = dom::obj(&[("audio", true.into())]);
    let stream = match aw(call(&md, "getUserMedia", &[constraints])).await {
        Ok(s) => s,
        Err(_) => {
            ST.set(St::Idle);
            return toast("마이크를 쓸 수 없어요. 설정에서 마이크를 허용해 주세요");
        }
    };
    let mime = pick_mime();
    let opts = if mime.is_empty() { dom::obj(&[("audioBitsPerSecond", 24000.into())]) } else { dom::obj(&[("mimeType", JsValue::from_str(&mime)), ("audioBitsPerSecond", 24000.into())]) };
    let Ok(rec) = construct("MediaRecorder", &[stream.clone(), opts]).or_else(|_| construct("MediaRecorder", &[stream.clone()])) else {
        stop_tracks(&stream);
        ST.set(St::Idle);
        return toast("이 기기에서는 음성 메모를 쓸 수 없어요");
    };
    CHUNKS.with(|c| c.borrow_mut().clear());
    LEVELS.with(|l| l.borrow_mut().clear());
    let on_data = Closure::<dyn FnMut(JsValue)>::new(|e: JsValue| {
        let d = g(&e, "data");
        if d.is_object() {
            CHUNKS.with(|c| c.borrow_mut().push(d));
        }
    });
    let _ = Reflect::set(&rec, &"ondataavailable".into(), on_data.as_ref());
    on_data.forget();
    // 소리 크기 재기
    if let Ok(ctx) = construct("AudioContext", &[]).or_else(|_| construct("webkitAudioContext", &[])) {
        if let Ok(src) = call(&ctx, "createMediaStreamSource", &[stream.clone()]) {
            if let Ok(an) = call(&ctx, "createAnalyser", &[]) {
                let _ = Reflect::set(&an, &"fftSize".into(), &512.into());
                let _ = call(&src, "connect", &[an.clone()]);
                ANALYSER.with(|a| *a.borrow_mut() = an);
            }
        }
        ACTX.with(|a| *a.borrow_mut() = ctx);
    }
    if call(&rec, "start", &[]).is_err() {
        stop_tracks(&stream);
        ST.set(St::Idle);
        return toast("녹음을 시작하지 못했어요");
    }
    RECORDER.with(|r| *r.borrow_mut() = rec);
    STREAM.with(|s| *s.borrow_mut() = stream);
    T0.set(now());
    let gen = GEN.get().wrapping_add(1);
    GEN.set(gen);
    set_rec_ui(
        r#"<canvas class="vwave" aria-hidden="true"></canvas><span class="vtime" id="vtime">0:00</span><button type="button" class="vstop" id="vstop" aria-label="녹음 멈추기"><i></i></button>"#,
        !dom::reduce_motion(),
    );
    dom::vibrate(10);
    spawn_local(async move {
        let mut buf = vec![0u8; 512];
        while ST.get() == St::Rec && GEN.get() == gen {
            let el = (now() - T0.get()) / 1000.0;
            let lvl = ANALYSER.with(|a| {
                let an = a.borrow();
                if !an.is_object() {
                    return 0u8;
                }
                let arr = Uint8Array::new_with_length(512);
                let _ = call(&an, "getByteTimeDomainData", &[arr.clone().into()]);
                arr.copy_to(&mut buf[..]);
                // 128을 가운데로 한 흔들림의 크기 (0~9)
                let rms = (buf.iter().map(|&b| ((b as f64 - 128.0) / 128.0).powi(2)).sum::<f64>() / buf.len() as f64).sqrt();
                ((rms * 22.0).sqrt() * 9.0).clamp(0.0, 9.0).round() as u8
            });
            LEVELS.with(|l| l.borrow_mut().push(lvl));
            dom::set_text("vtime", &mmss(el));
            LEVELS.with(|l| draw_wave(".vrec .vwave", &l.borrow(), true));
            if el >= MAX_S {
                stop_rec().await;
                return;
            }
            dom::sleep(70).await;
        }
    });
}

fn stop_tracks(stream: &JsValue) {
    if let Ok(tracks) = call(stream, "getTracks", &[]) {
        for t in Array::from(&tracks).iter() {
            let _ = call(&t, "stop", &[]);
        }
    }
}
fn release_audio() {
    STREAM.with(|s| {
        let s = s.replace(JsValue::UNDEFINED);
        if s.is_object() {
            stop_tracks(&s);
        }
    });
    ACTX.with(|a| {
        let a = a.replace(JsValue::UNDEFINED);
        if a.is_object() {
            let _ = call(&a, "close", &[]);
        }
    });
    ANALYSER.with(|a| *a.borrow_mut() = JsValue::UNDEFINED);
}

/// 녹음을 멈추고 미리 듣기 줄로 바꿔요
async fn stop_rec() {
    if ST.get() != St::Rec {
        return;
    }
    ST.set(St::Review);
    let el = (now() - T0.get()) / 1000.0;
    let rec = RECORDER.with(|r| r.replace(JsValue::UNDEFINED));
    // 멈춘 알림을 기다려요 (마지막 조각이 이때 와요)
    let done = Promise::new(&mut |res, _| {
        if rec.is_object() {
            let res2 = res.clone();
            let cb = Closure::once_into_js(move || {
                let _ = res2.call0(&JsValue::NULL);
            });
            let _ = Reflect::set(&rec, &"onstop".into(), &cb);
        } else {
            let _ = res.call0(&JsValue::NULL);
        }
    });
    if rec.is_object() {
        let _ = call(&rec, "stop", &[]);
    }
    let _ = JsFuture::from(done).await;
    release_audio();
    if el < MIN_S {
        return reset();
    }
    let parts: Array = CHUNKS.with(|c| c.borrow().iter().cloned().collect());
    let ty = if rec.is_object() { g(&rec, "mimeType").as_string().unwrap_or_default() } else { String::new() };
    let Ok(blob) = construct("Blob", &[parts.into(), dom::obj(&[("type", JsValue::from_str(&ty))])]) else { return reset() };
    let url = call(&g(&win(), "URL"), "createObjectURL", &[blob.clone()]).ok().and_then(|v| v.as_string()).unwrap_or_default();
    BLOB.with(|b| *b.borrow_mut() = blob);
    URL.with(|u| *u.borrow_mut() = url);
    DUR.set(el.min(MAX_S));
    let len = mmss(el.min(MAX_S));
    set_rec_ui(
        &format!(r#"<button type="button" class="vx" id="vx" aria-label="지우기">{X_SVG}</button><div class="vrev"><button type="button" class="vpl" id="vpl" aria-label="미리 듣기">{PLAY_SVG}</button><canvas class="vwave sm" aria-hidden="true"></canvas><span class="vchip">{len}</span><button type="button" class="vsend" id="vsend" aria-label="보내기">{UP_SVG}</button></div>"#),
        false,
    );
    if let Some(v) = by_id("vrec") {
        let _ = v.class_list().add_1("review");
    }
    LEVELS.with(|l| draw_wave(".vrec .vwave", &l.borrow(), false));
    dom::vibrate(8);
}

/// 모두 치우고 처음 상태로
pub fn reset() {
    GEN.set(GEN.get().wrapping_add(1));
    let rec = RECORDER.with(|r| r.replace(JsValue::UNDEFINED));
    if rec.is_object() {
        let _ = Reflect::set(&rec, &"onstop".into(), &JsValue::NULL);
        let _ = call(&rec, "stop", &[]);
    }
    release_audio();
    stop_preview();
    URL.with(|u| {
        let u = std::mem::take(&mut *u.borrow_mut());
        if !u.is_empty() {
            let _ = call(&g(&win(), "URL"), "revokeObjectURL", &[JsValue::from_str(&u)]);
        }
    });
    BLOB.with(|b| *b.borrow_mut() = JsValue::UNDEFINED);
    CHUNKS.with(|c| c.borrow_mut().clear());
    clear_ui();
    ST.set(St::Idle);
}

fn stop_preview() {
    PREVIEW.with(|p| {
        let a = p.replace(JsValue::UNDEFINED);
        if a.is_object() {
            let _ = call(&a, "pause", &[]);
        }
    });
}
fn toggle_preview() {
    let playing = PREVIEW.with(|p| p.borrow().is_object());
    let set_icon = |on: bool| {
        if let Some(b) = by_id("vpl") {
            b.set_inner_html(if on { PAUSE_SVG } else { PLAY_SVG });
        }
    };
    if playing {
        stop_preview();
        return set_icon(false);
    }
    let url = URL.with(|u| u.borrow().clone());
    let Ok(a) = construct("Audio", &[JsValue::from_str(&url)]) else { return };
    let ended = Closure::once_into_js(|| {
        PREVIEW.with(|p| *p.borrow_mut() = JsValue::UNDEFINED);
        if let Some(b) = by_id("vpl") {
            b.set_inner_html(PLAY_SVG);
        }
    });
    let _ = Reflect::set(&a, &"onended".into(), &ended);
    guard_play(&a, || {
        stop_preview();
        if let Some(b) = by_id("vpl") {
            b.set_inner_html(PLAY_SVG);
        }
    });
    PREVIEW.with(|p| *p.borrow_mut() = a);
    set_icon(true);
}

/// 재생을 시작해요. 못 틀면(소리 종류를 못 읽는 기기 등) 안내하고 표시를 되돌려요
fn guard_play(a: &JsValue, undo: impl FnOnce() + 'static) {
    let Ok(p) = call(a, "play", &[]) else { return undo() };
    if let Ok(p) = p.dyn_into::<Promise>() {
        let undo = std::rc::Rc::new(RefCell::new(Some(undo)));
        let cb = Closure::once(move |_e: JsValue| {
            if let Some(u) = undo.borrow_mut().take() {
                u();
            }
            toast("이 소리는 재생하지 못했어요");
        });
        let _ = p.catch(&cb);
        cb.forget();
    }
}

/// 파형을 글자(숫자 0~9, 40개 안쪽)로 줄여요
fn bars_text() -> String {
    LEVELS.with(|l| {
        let l = l.borrow();
        if l.is_empty() {
            return String::new();
        }
        let n = l.len().min(40);
        (0..n).map(|k| char::from(b'0' + l[(k * l.len() / n).min(l.len() - 1)])).collect()
    })
}

async fn send() {
    if ST.get() != St::Review {
        return;
    }
    let Some(c) = open_chat() else { return reset() };
    let blob = BLOB.with(|b| b.borrow().clone());
    if !blob.is_object() {
        return reset();
    }
    if let Some(b) = by_id("vsend") {
        let _ = b.set_attribute("disabled", "");
    }
    let buf = match aw(call(&blob, "arrayBuffer", &[])).await {
        Ok(b) => b,
        Err(_) => return toast("음성을 읽지 못했어요"),
    };
    let bytes = Uint8Array::new(&buf).to_vec();
    // 기기마다 이름 뒤에 코덱이 붙거나 공백이 섞여 와요(audio/mp4; codecs=...). 서버가 받는 모양(audio/mp4 같은 이름만)으로 정리해요
    let mime = g(&blob, "type").as_string().map(|m| m.split(';').next().unwrap_or("").trim().to_lowercase()).filter(|m| m.starts_with("audio/") && m.chars().all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '-' || c == '+')).unwrap_or_else(|| "audio/mp4".into());
    let audio = format!("data:{mime};base64,{}", b64(&bytes));
    if audio.len() > MAX_CHARS {
        if let Some(b) = by_id("vsend") {
            let _ = b.remove_attribute("disabled");
        }
        return toast("음성이 너무 길어서 보낼 수 없어요. 더 짧게 녹음해 주세요");
    }
    let dur = DUR.get().round().clamp(1.0, 60.0);
    let payload = dom::obj(&[("type", "voice".into()), ("audio", JsValue::from_str(&audio)), ("dur", dur.into()), ("bars", JsValue::from_str(&bars_text()))]);
    let (id, other) = (c.id.clone(), c.other.clone());
    let sent = queue_send(move || cloud::call_now("send", &[JsValue::from_str(&id), JsValue::from_str(&other), payload, JsValue::from_str("음성 메시지")])).await;
    let ok = sent.is_ok();
    if ok {
        dom::vibrate(8);
        reset();
    } else {
        if let Some(b) = by_id("vsend") {
            let _ = b.remove_attribute("disabled");
        }
        // 왜 안 갔는지 알 수 있게 오류 이름을 같이 보여 줘요 (예: permission-denied)
        let code = sent.err().and_then(|e| g(&e, "code").as_string().or_else(|| e.as_string())).unwrap_or_default();
        toast(&if code.is_empty() { "음성을 보내지 못했어요".to_string() } else { format!("음성을 보내지 못했어요 ({})", dom::cut(&code, 40)) });
    }
}

/* ---------- 말풍선의 재생 ---------- */

/// 대화에 온 음성 한 칸 (재생 버튼 · 파형 · 길이)
pub fn card_html(m: &JsValue, me: bool, i: usize) -> String {
    let s = |k: &str| cloud::get(m, k).as_string().unwrap_or_default();
    let dur = js_sys::Number::from(cloud::get(m, "dur")).value_of();
    let dur = if dur.is_finite() { dur.clamp(1.0, 60.0) } else { 1.0 };
    let bars: Vec<u32> = s("bars").chars().filter_map(|c| c.to_digit(10)).take(40).collect();
    let bars: Vec<u32> = if bars.is_empty() { vec![2; 24] } else { bars };
    let n = bars.len();
    let spans: String = bars.iter().enumerate().map(|(k, v)| format!(r#"<i style="height:{}px;--k:{k}"></i>"#, 4 + v * 2)).collect();
    format!(
        r#"<div class="bubble vm{}" style="--n:{n};--d:{dur}s" data-vm="{i}"><button type="button" class="vplay" data-vi="{i}" aria-label="재생">{PLAY_SVG}</button><div class="vbars">{spans}</div><span class="vlen">{}</span></div>"#,
        if me { " me" } else { "" },
        mmss(dur)
    )
}

fn play_message(i: usize) {
    let Some(c) = open_chat() else { return };
    let Some(m) = c.msgs.get(i).map(|m| m.raw.clone()) else { return };
    let src = cloud::get(&m, "audio").as_string().unwrap_or_default();
    if !src.starts_with("data:audio/") {
        return;
    }
    let (prev, prev_i) = PLAY.with(|p| p.replace((JsValue::UNDEFINED, -1)));
    if prev.is_object() {
        let _ = call(&prev, "pause", &[]);
        mark_playing(prev_i, false);
    }
    if prev_i == i as i64 {
        return;
    }
    let Ok(a) = construct("Audio", &[JsValue::from_str(&src)]) else { return };
    let idx = i as i64;
    let ended = Closure::once_into_js(move || {
        PLAY.with(|p| {
            if p.borrow().1 == idx {
                *p.borrow_mut() = (JsValue::UNDEFINED, -1);
            }
        });
        mark_playing(idx, false);
    });
    let _ = Reflect::set(&a, &"onended".into(), &ended);
    guard_play(&a, move || {
        PLAY.with(|p| {
            if p.borrow().1 == idx {
                *p.borrow_mut() = (JsValue::UNDEFINED, -1);
            }
        });
        mark_playing(idx, false);
    });
    PLAY.with(|p| *p.borrow_mut() = (a, idx));
    mark_playing(idx, true);
}
fn mark_playing(i: i64, on: bool) {
    let Some(card) = dom::doc().query_selector(&format!(r#"[data-vm="{i}"]"#)).ok().flatten() else { return };
    // 처음부터 다시 돌게 한 번 뺐다 넣어요
    let _ = card.class_list().remove_1("playing");
    if on {
        let _ = card.client_width();
        let _ = card.class_list().add_1("playing");
    }
    if let Some(b) = card.query_selector(".vplay").ok().flatten() {
        b.set_inner_html(if on { PAUSE_SVG } else { PLAY_SVG });
    }
}

/// 대화를 닫거나 다른 대화로 가면 녹음·재생을 멈춰요
pub fn leave() {
    if ST.get() != St::Idle {
        reset();
    }
    let (a, i) = PLAY.with(|p| p.replace((JsValue::UNDEFINED, -1)));
    if a.is_object() {
        let _ = call(&a, "pause", &[]);
        mark_playing(i, false);
    }
}

pub fn init() {
    if BOUND.replace(true) {
        return;
    }
    // 입력칸이 비었을 때의 소리 아이콘 = 음성 메모. 글자가 있으면 보내기(파란 화살표)예요
    if let Some(b) = by_id("send") {
        dom::on(&b, "click", |e| {
            if b_ready() {
                return;
            }
            e.prevent_default();
            e.stop_propagation();
            spawn_local(start());
        });
    }
    if let Some(c) = composer() {
        dom::on(&c, "click", |e| {
            if dom::closest(&e, "#vstop").is_some() {
                spawn_local(stop_rec());
            } else if dom::closest(&e, "#vx").is_some() {
                reset();
            } else if dom::closest(&e, "#vpl").is_some() {
                toggle_preview();
            } else if dom::closest(&e, "#vsend").is_some() {
                e.prevent_default();
                spawn_local(send());
            }
        });
    }
    if let Some(b) = by_id("bubbles") {
        dom::on(&b, "click", |e| {
            if let Some(p) = dom::closest(&e, ".vplay") {
                if let Ok(i) = dom::data(&p, "vi").parse::<usize>() {
                    play_message(i);
                }
            }
        });
    }
}

fn b_ready() -> bool {
    by_id("send").is_some_and(|b| b.class_list().contains("ready"))
}
