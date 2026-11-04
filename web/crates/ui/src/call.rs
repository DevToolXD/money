//! 음성 통화: 두 기기가 소리를 바로 주고받아요(WebRTC). 연결 정보만 Firestore(calls)로 건네요.
//! 화면은 아이폰 통화 화면처럼: 위에 시간과 이름, 아래 동그란 유리 버튼 여섯 개(스피커·녹음·소리 끔·더 보기·종료·키패드).
//! 녹음은 이 기기에만 저장해요(IndexedDB). 메시지 위쪽의 줄 세 개 버튼이 녹음 저장소예요.

use std::cell::{Cell, RefCell};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};

use crate::dom;

/* ---------- 작은 도움 ---------- */

fn g() -> JsValue {
    js_sys::global().into()
}
fn get(o: &JsValue, k: &str) -> JsValue {
    if o.is_null() || o.is_undefined() {
        return JsValue::UNDEFINED;
    }
    js_sys::Reflect::get(o, &JsValue::from_str(k)).unwrap_or(JsValue::UNDEFINED)
}
fn set(o: &JsValue, k: &str, v: &JsValue) {
    let _ = js_sys::Reflect::set(o, &JsValue::from_str(k), v);
}
fn m(o: &JsValue, name: &str, args: &[JsValue]) -> Result<JsValue, JsValue> {
    let f: js_sys::Function = get(o, name).dyn_into()?;
    f.apply(o, &args.iter().cloned().collect())
}
async fn aw(v: Result<JsValue, JsValue>) -> Result<JsValue, JsValue> {
    let v = v?;
    match v.clone().dyn_into::<js_sys::Promise>() {
        Ok(p) => JsFuture::from(p).await,
        Err(_) => Ok(v),
    }
}
fn cloud() -> JsValue {
    get(&g(), "cloud")
}
fn cb1(f: impl FnMut(JsValue) + 'static) -> JsValue {
    Closure::<dyn FnMut(JsValue)>::new(f).into_js_value()
}
fn win() -> web_sys::Window {
    web_sys::window().expect("window")
}
fn json(v: &JsValue) -> String {
    js_sys::JSON::stringify(v).map(String::from).unwrap_or_default()
}

/* ---------- 상태 ---------- */

#[derive(Clone, PartialEq)]
enum Phase {
    Idle,
    Calling,  // 거는 중 (상대가 아직 안 받음)
    Ringing,  // 걸려 옴 (내가 아직 안 받음)
    Talking,  // 통화 중
}

struct Call {
    id: String,
    peer_name: String,
    peer_photo: String,
    phase: Phase,
    pc: JsValue,
    local: JsValue,
    remote: JsValue,
    audio: Option<web_sys::HtmlElement>,
    unsubs: Vec<JsValue>,
    started: f64,
    timer: i32,
    muted: bool,
    speaker: bool,
    rec: JsValue,
    rec_chunks: js_sys::Array,
    rec_ctx: JsValue,
    pending: Vec<String>, // 방 번호가 나오기 전에 모인 내 후보
    offer: JsValue,
}

thread_local! {
    static CALL: RefCell<Option<Call>> = const { RefCell::new(None) };
    static SEEN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static LISTENING: Cell<bool> = const { Cell::new(false) };
}
fn with<R>(f: impl FnOnce(&mut Call) -> R) -> Option<R> {
    CALL.with(|c| c.borrow_mut().as_mut().map(f))
}
fn phase() -> Phase {
    with(|c| c.phase.clone()).unwrap_or(Phase::Idle)
}

/* ---------- 화면 ---------- */

const ICONS: [(&str, &str, &str); 6] = [
    ("speaker", "스피커", r#"<path d="M4 9.5h3.2L12 5.5v13l-4.8-4H4z"/><path d="M15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11"/>"#),
    ("rec", "녹음", r#"<circle cx="12" cy="12" r="7.5"/><circle cx="12" cy="12" r="3.2" fill="currentColor"/>"#),
    ("mute", "소리 끔", r#"<path d="M12 3.5a3 3 0 0 0-3 3v5a3 3 0 0 0 6 0v-5a3 3 0 0 0-3-3z"/><path d="M6 11.5a6 6 0 0 0 12 0M12 17.5V21M4 4l16 16"/>"#),
    ("more", "더 보기", r#"<circle cx="6" cy="12" r="1.3" fill="currentColor"/><circle cx="12" cy="12" r="1.3" fill="currentColor"/><circle cx="18" cy="12" r="1.3" fill="currentColor"/>"#),
    ("end", "종료", r#"<path d="M3.5 13.8c4.6-4.4 12.4-4.4 17 0l-2 2.3-3.4-1.4v-2.4a10.5 10.5 0 0 0-6.2 0v2.4l-3.4 1.4z" fill="currentColor"/>"#),
    ("pad", "키패드", r#"<circle cx="7" cy="6" r="1.4"/><circle cx="12" cy="6" r="1.4"/><circle cx="17" cy="6" r="1.4"/><circle cx="7" cy="11" r="1.4"/><circle cx="12" cy="11" r="1.4"/><circle cx="17" cy="11" r="1.4"/><circle cx="7" cy="16" r="1.4"/><circle cx="12" cy="16" r="1.4"/><circle cx="17" cy="16" r="1.4"/><circle cx="12" cy="21" r="1.4"/>"#),
];
const ANSWER_D: &str = r#"<path d="M6.6 3.5h3l1.5 4-2 1.3a11 11 0 0 0 6.1 6.1l1.3-2 4 1.5v3a2 2 0 0 1-2.2 2A17 17 0 0 1 4.6 5.7a2 2 0 0 1 2-2.2z" fill="currentColor"/>"#;

fn svg(d: &str) -> String {
    format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{d}</svg>"#)
}

fn screen() -> Option<web_sys::HtmlElement> {
    if let Some(e) = dom::by_id("call") {
        return Some(e);
    }
    let app = dom::app_el()?;
    let e: web_sys::HtmlElement = dom::doc().create_element("section").ok()?.dyn_into().ok()?;
    e.set_id("call");
    e.set_class_name("call");
    let _ = e.set_attribute("aria-label", "통화");
    e.set_hidden(true);
    let _ = app.append_child(&e);
    dom::on(&e, "click", |ev| {
        if let Some(b) = dom::closest(&ev, "[data-call]") {
            on_button(&dom::data(&b, "call"), &b);
        }
        if let Some(k) = dom::closest(&ev, "[data-key]") {
            tone(&dom::data(&k, "key"));
        }
    });
    Some(e)
}

fn paint() {
    let Some(e) = screen() else { return };
    let Some((name, photo, ph, muted, speaker, rec)) = with(|c| (c.peer_name.clone(), c.peer_photo.clone(), c.phase.clone(), c.muted, c.speaker, c.rec.is_truthy())) else {
        e.set_hidden(true);
        return;
    };
    let status = match ph {
        Phase::Calling => "거는 중…".to_string(),
        Phase::Ringing => "전화가 왔어요".to_string(),
        _ => "00:00".to_string(),
    };
    let buttons = if ph == Phase::Ringing {
        format!(
            r#"<div class="call-grid two"><div class="call-cell"><button class="call-btn red" type="button" data-call="decline" aria-label="거절">{}</button><span>거절</span></div><div class="call-cell"><button class="call-btn green" type="button" data-call="answer" aria-label="받기">{}</button><span>받기</span></div></div>"#,
            svg(ICONS[4].2),
            svg(ANSWER_D)
        )
    } else {
        let cells: String = ICONS
            .iter()
            .map(|(k, label, d)| {
                let on = (*k == "mute" && muted) || (*k == "speaker" && speaker) || (*k == "rec" && rec);
                let cls = if *k == "end" { " red" } else if on { " on" } else { "" };
                let label = if *k == "rec" && rec { "녹음 중" } else { label };
                format!(r#"<div class="call-cell"><button class="call-btn{cls}" type="button" data-call="{k}" aria-label="{label}" aria-pressed="{on}">{}</button><span>{label}</span></div>"#, svg(d))
            })
            .collect();
        format!(r#"<div class="call-grid">{cells}</div>"#)
    };
    e.set_inner_html(&format!(
        r#"<div class="call-bg" aria-hidden="true"></div><div class="call-top"><p class="call-time" id="call-time">{status}</p><h2 class="call-name">{}</h2></div><div class="call-av">{}</div><div class="call-pad" id="call-pad" hidden>{}</div>{buttons}"#,
        dom::esc(&name),
        dom::avatar_html(&name, &photo, "avatar contact"),
        ["1", "2", "3", "4", "5", "6", "7", "8", "9", "*", "0", "#"].iter().map(|k| format!(r#"<button type="button" class="call-key" data-key="{k}">{k}</button>"#)).collect::<String>()
    ));
    if e.hidden() {
        e.set_hidden(false);
        let _ = e.class_list().remove_1("out");
        let _ = e.offset_width();
        let _ = e.class_list().add_1("in");
    }
    tick();
}

fn tick() {
    let Some((ph, started)) = with(|c| (c.phase.clone(), c.started)) else { return };
    if ph != Phase::Talking || started == 0.0 {
        return;
    }
    let secs = ((js_sys::Date::now() - started) / 1000.0).floor() as u64;
    dom::set_text("call-time", &format!("{:02}:{:02}", secs / 60, secs % 60));
}

fn close_screen() {
    let Some(e) = dom::by_id("call") else { return };
    let _ = e.class_list().remove_1("in");
    let _ = e.class_list().add_1("out");
    let cb = Closure::once_into_js(move || {
        if CALL.with(|c| c.borrow().is_none()) {
            e.set_hidden(true);
        }
    });
    let _ = win().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 380);
}

/* ---------- 연결 ---------- */

fn new_pc() -> Result<JsValue, JsValue> {
    let ctor: js_sys::Function = get(&g(), "RTCPeerConnection").dyn_into()?;
    let ice = js_sys::Array::of1(&dom::obj(&[("urls", js_sys::Array::of2(&"stun:stun.l.google.com:19302".into(), &"stun:stun1.l.google.com:19302".into()).into())]));
    let pc = js_sys::Reflect::construct(&ctor, &js_sys::Array::of1(&dom::obj(&[("iceServers", ice.into())])))?;
    // 내 연결 후보: 방 번호가 있으면 바로 보내고, 없으면 모아 둬요
    set(
        &pc,
        "onicecandidate",
        &cb1(|ev| {
            let c = get(&ev, "candidate");
            if c.is_null() || c.is_undefined() {
                return;
            }
            let text = json(&m(&c, "toJSON", &[]).unwrap_or(c));
            let id = with(|x| x.id.clone()).unwrap_or_default();
            if id.is_empty() {
                with(|x| x.pending.push(text));
            } else {
                let _ = m(&cloud(), "callCand", &[JsValue::from_str(&id), JsValue::from_str(&text)]);
            }
        }),
    );
    // 상대 소리
    set(
        &pc,
        "ontrack",
        &cb1(|ev| {
            let streams = get(&ev, "streams");
            let st = js_sys::Reflect::get_u32(&streams, 0).unwrap_or(JsValue::UNDEFINED);
            with(|x| x.remote = st.clone());
            if let Some(a) = with(|x| x.audio.clone()).flatten() {
                set(&a, "srcObject", &st);
                let _ = m(&a, "play", &[]);
            }
        }),
    );
    set(
        &pc,
        "onconnectionstatechange",
        &cb1(|_| {
            let s = with(|x| get(&x.pc, "connectionState").as_string().unwrap_or_default()).unwrap_or_default();
            match s.as_str() {
                "connected" => talking(),
                "failed" => {
                    crate::shell::toast("연결하지 못했어요. 인터넷을 확인해 주세요");
                    hang_up(true);
                }
                _ => {}
            }
        }),
    );
    Ok(pc)
}

async fn mic() -> Result<JsValue, JsValue> {
    let md = get(&win().navigator(), "mediaDevices");
    aw(m(&md, "getUserMedia", &[dom::obj(&[("audio", true.into())])])).await
}

fn start_audio_el() -> Option<web_sys::HtmlElement> {
    let a: web_sys::HtmlElement = dom::doc().create_element("audio").ok()?.dyn_into().ok()?;
    let _ = a.set_attribute("autoplay", "");
    let _ = a.set_attribute("playsinline", "");
    a.set_hidden(true);
    let _ = dom::app_el()?.append_child(&a);
    Some(a)
}

fn add_tracks(pc: &JsValue, local: &JsValue) {
    if let Ok(tr) = m(local, "getTracks", &[]).and_then(|t| t.dyn_into::<js_sys::Array>().map_err(|e| e)) {
        for t in tr.iter() {
            let _ = m(pc, "addTrack", &[t, local.clone()]);
        }
    }
}

fn listen_cands(id: &str) {
    let pc = with(|x| x.pc.clone()).unwrap_or(JsValue::NULL);
    let un = m(
        &cloud(),
        "callCands",
        &[
            JsValue::from_str(id),
            cb1(move |c| {
                if let Ok(v) = js_sys::JSON::parse(&c.as_string().unwrap_or_default()) {
                    let _ = m(&pc, "addIceCandidate", &[v]);
                }
            }),
        ],
    )
    .unwrap_or(JsValue::UNDEFINED);
    with(|x| x.unsubs.push(un));
}

fn watch(id: &str) {
    let un = m(
        &cloud(),
        "callWatch",
        &[
            JsValue::from_str(id),
            cb1(|v| {
                if v.is_null() {
                    return;
                }
                let state = get(&v, "state").as_string().unwrap_or_default();
                if state == "ended" {
                    return hang_up(false);
                }
                // 거는 쪽: 상대가 받으면 answer가 와요
                let answer = get(&v, "answer");
                if answer.is_object() && phase() == Phase::Calling {
                    let pc = with(|x| x.pc.clone()).unwrap_or(JsValue::NULL);
                    if get(&pc, "currentRemoteDescription").is_null() {
                        let _ = m(&pc, "setRemoteDescription", &[answer]);
                    }
                }
            }),
        ],
    )
    .unwrap_or(JsValue::UNDEFINED);
    with(|x| x.unsubs.push(un));
}

fn talking() {
    let first = with(|x| {
        let f = x.phase != Phase::Talking;
        x.phase = Phase::Talking;
        if x.started == 0.0 {
            x.started = js_sys::Date::now();
        }
        f
    })
    .unwrap_or(false);
    if !first {
        return;
    }
    let t = win()
        .set_interval_with_callback_and_timeout_and_arguments_0(Closure::<dyn FnMut()>::new(tick).into_js_value().unchecked_ref(), 500)
        .unwrap_or(0);
    with(|x| x.timer = t);
    paint();
}

fn new_call(id: &str, name: &str, photo: &str, ph: Phase) {
    CALL.with(|c| {
        *c.borrow_mut() = Some(Call {
            id: id.into(),
            peer_name: name.into(),
            peer_photo: photo.into(),
            phase: ph,
            pc: JsValue::NULL,
            local: JsValue::NULL,
            remote: JsValue::NULL,
            audio: start_audio_el(),
            unsubs: Vec::new(),
            started: 0.0,
            timer: 0,
            muted: false,
            speaker: false,
            rec: JsValue::NULL,
            rec_chunks: js_sys::Array::new(),
            rec_ctx: JsValue::NULL,
            pending: Vec::new(),
            offer: JsValue::NULL,
        })
    });
}

/// 전화 걸기 (대화 위쪽 전화 버튼)
pub fn dial(to: &str, chat: &str, name: &str, photo: &str) {
    if phase() != Phase::Idle {
        return;
    }
    new_call("", name, photo, Phase::Calling);
    paint();
    let (to, chat) = (to.to_string(), chat.to_string());
    spawn_local(async move {
        let r: Result<(), JsValue> = async {
            let local = mic().await?;
            let pc = new_pc()?;
            add_tracks(&pc, &local);
            with(|x| {
                x.local = local.clone();
                x.pc = pc.clone();
            });
            let offer = aw(m(&pc, "createOffer", &[])).await?;
            aw(m(&pc, "setLocalDescription", &[offer.clone()])).await?;
            let o = dom::obj(&[("type", get(&offer, "type")), ("sdp", get(&offer, "sdp"))]);
            let id = aw(m(&cloud(), "callStart", &[JsValue::from_str(&to), JsValue::from_str(&chat), o])).await?.as_string().unwrap_or_default();
            if phase() == Phase::Idle {
                let _ = m(&cloud(), "callEnd", &[JsValue::from_str(&id)]);
                return Ok(());
            }
            let pend = with(|x| {
                x.id = id.clone();
                std::mem::take(&mut x.pending)
            })
            .unwrap_or_default();
            for c in pend {
                let _ = m(&cloud(), "callCand", &[JsValue::from_str(&id), JsValue::from_str(&c)]);
            }
            watch(&id);
            listen_cands(&id);
            // 45초 동안 안 받으면 끊어요
            let id2 = id.clone();
            let cb = Closure::once_into_js(move || {
                if phase() == Phase::Calling && with(|x| x.id == id2).unwrap_or(false) {
                    crate::shell::toast("상대가 받지 않았어요");
                    hang_up(true);
                }
            });
            let _ = win().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 45000);
            Ok(())
        }
        .await;
        if r.is_err() {
            crate::shell::toast("마이크를 쓸 수 없어서 전화를 걸지 못했어요");
            hang_up(true);
        }
    });
}

/// 로그인한 뒤: 나에게 걸려 오는 전화를 들어요
pub fn listen() {
    if LISTENING.get() {
        return;
    }
    LISTENING.set(true);
    let _ = m(
        &cloud(),
        "callIncoming",
        &[cb1(|list| {
            let Ok(a) = list.dyn_into::<js_sys::Array>() else { return };
            for v in a.iter() {
                let id = get(&v, "id").as_string().unwrap_or_default();
                let at = get(&v, "at").as_f64().unwrap_or(0.0);
                // 오래된 것(1분 넘게 울린 것)과 이미 본 것은 빼요
                if SEEN.with(|s| s.borrow().contains(&id)) || js_sys::Date::now() - at > 60000.0 {
                    continue;
                }
                SEEN.with(|s| s.borrow_mut().push(id.clone()));
                if phase() != Phase::Idle {
                    let _ = m(&cloud(), "callEnd", &[JsValue::from_str(&id)]); // 통화 중이면 바로 끊어요
                    continue;
                }
                let from = get(&v, "from").as_string().unwrap_or_default();
                let p = m(&cloud(), "peek", &[JsValue::from_str(&from)]).unwrap_or(JsValue::NULL);
                let name = get(&p, "name").as_string().filter(|s| !s.is_empty()).unwrap_or_else(|| "알 수 없는 사람".into());
                let photo = get(&p, "photo").as_string().unwrap_or_default();
                new_call(&id, &name, &photo, Phase::Ringing);
                with(|x| x.offer = get(&v, "offer"));
                watch(&id);
                paint();
                dom::vibrate(400);
                break;
            }
        })],
    );
}

async fn answer() -> Result<(), JsValue> {
    let (id, offer) = with(|x| (x.id.clone(), x.offer.clone())).ok_or(JsValue::NULL)?;
    let local = mic().await?;
    let pc = new_pc()?;
    add_tracks(&pc, &local);
    with(|x| {
        x.local = local.clone();
        x.pc = pc.clone();
    });
    aw(m(&pc, "setRemoteDescription", &[offer])).await?;
    let ans = aw(m(&pc, "createAnswer", &[])).await?;
    aw(m(&pc, "setLocalDescription", &[ans.clone()])).await?;
    listen_cands(&id);
    aw(m(&cloud(), "callAnswer", &[JsValue::from_str(&id), dom::obj(&[("type", get(&ans, "type")), ("sdp", get(&ans, "sdp"))])])).await?;
    let pend = with(|x| std::mem::take(&mut x.pending)).unwrap_or_default();
    for c in pend {
        let _ = m(&cloud(), "callCand", &[JsValue::from_str(&id), JsValue::from_str(&c)]);
    }
    talking();
    Ok(())
}

/// 끊기. tell이면 서버에도 끝났다고 적어요
pub fn hang_up(tell: bool) {
    let Some(c) = CALL.with(|c| c.borrow_mut().take()) else { return };
    if c.rec.is_truthy() {
        stop_rec(&c.rec);
    }
    if tell && !c.id.is_empty() {
        let _ = m(&cloud(), "callEnd", &[JsValue::from_str(&c.id)]);
    }
    for u in &c.unsubs {
        if let Some(f) = u.dyn_ref::<js_sys::Function>() {
            let _ = f.call0(&JsValue::NULL);
        }
    }
    if let Ok(tr) = m(&c.local, "getTracks", &[]).and_then(|t| t.dyn_into::<js_sys::Array>().map_err(|e| e)) {
        for t in tr.iter() {
            let _ = m(&t, "stop", &[]);
        }
    }
    let _ = m(&c.pc, "close", &[]);
    if c.timer != 0 {
        win().clear_interval_with_handle(c.timer);
    }
    if let Some(a) = c.audio {
        a.remove();
    }
    if c.started > 0.0 {
        let secs = ((js_sys::Date::now() - c.started) / 1000.0).floor() as u64;
        crate::shell::toast(&format!("통화를 마쳤어요 · {:02}:{:02}", secs / 60, secs % 60));
    }
    close_screen();
}

/* ---------- 버튼 ---------- */

fn on_button(k: &str, b: &web_sys::HtmlElement) {
    match k {
        "end" => hang_up(true),
        "decline" => hang_up(true),
        "answer" => spawn_local(async {
            if answer().await.is_err() {
                crate::shell::toast("마이크를 쓸 수 없어서 받지 못했어요");
                hang_up(true);
            }
        }),
        "mute" => {
            let on = with(|x| {
                x.muted = !x.muted;
                if let Ok(tr) = m(&x.local, "getAudioTracks", &[]).and_then(|t| t.dyn_into::<js_sys::Array>().map_err(|e| e)) {
                    for t in tr.iter() {
                        set(&t, "enabled", &(!x.muted).into());
                    }
                }
                x.muted
            })
            .unwrap_or(false);
            mark(b, on);
        }
        "speaker" => {
            // 웹에서는 수화기·스피커를 고를 수 없어서, 상대 소리를 크게/작게 바꿔요
            let on = with(|x| {
                x.speaker = !x.speaker;
                if let Some(a) = &x.audio {
                    set(a, "volume", &(if x.speaker { 1.0 } else { 0.6 }).into());
                }
                x.speaker
            })
            .unwrap_or(false);
            mark(b, on);
        }
        "rec" => {
            let rec = with(|x| x.rec.clone()).unwrap_or(JsValue::NULL);
            if rec.is_truthy() {
                stop_rec(&rec);
                with(|x| x.rec = JsValue::NULL);
                crate::shell::toast("녹음을 저장했어요");
            } else if start_rec().is_err() {
                crate::shell::toast("이 기기에서는 녹음할 수 없어요");
            }
            paint();
        }
        "pad" => {
            if let Some(p) = dom::by_id("call-pad") {
                p.set_hidden(!p.hidden());
                mark(b, !p.hidden());
            }
        }
        "more" => open_store(),
        _ => {}
    }
}
fn mark(b: &web_sys::HtmlElement, on: bool) {
    dom::toggle(b, "on", on);
    let _ = b.set_attribute("aria-pressed", &on.to_string());
}

// 키패드 소리 (두 음을 섞은 짧은 소리)
fn tone(k: &str) {
    const ROWS: [f64; 4] = [697.0, 770.0, 852.0, 941.0];
    const COLS: [f64; 3] = [1209.0, 1336.0, 1477.0];
    let i = "123456789*0#".find(k).unwrap_or(0);
    let (r, c) = (ROWS[i / 3], COLS[i % 3]);
    let Ok(ctor) = get(&g(), "AudioContext").dyn_into::<js_sys::Function>() else { return };
    let Ok(ctx) = js_sys::Reflect::construct(&ctor, &js_sys::Array::new()) else { return };
    let gain = m(&ctx, "createGain", &[]).unwrap_or(JsValue::NULL);
    set(&get(&gain, "gain"), "value", &0.08.into());
    let _ = m(&gain, "connect", &[get(&ctx, "destination")]);
    let now = get(&ctx, "currentTime").as_f64().unwrap_or(0.0);
    for f in [r, c] {
        let o = m(&ctx, "createOscillator", &[]).unwrap_or(JsValue::NULL);
        set(&get(&o, "frequency"), "value", &f.into());
        let _ = m(&o, "connect", &[gain.clone()]);
        let _ = m(&o, "start", &[]);
        let _ = m(&o, "stop", &[(now + 0.15).into()]);
    }
    let c2 = ctx.clone();
    let cb = Closure::once_into_js(move || {
        let _ = m(&c2, "close", &[]);
    });
    let _ = win().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 400);
}

/* ---------- 녹음 (이 기기에만 저장) ---------- */

fn start_rec() -> Result<(), JsValue> {
    let (local, remote) = with(|x| (x.local.clone(), x.remote.clone())).ok_or(JsValue::NULL)?;
    let ctor: js_sys::Function = get(&g(), "AudioContext").dyn_into()?;
    let ctx = js_sys::Reflect::construct(&ctor, &js_sys::Array::new())?;
    let dest = m(&ctx, "createMediaStreamDestination", &[])?;
    for s in [local, remote] {
        if s.is_object() {
            if let Ok(src) = m(&ctx, "createMediaStreamSource", &[s]) {
                let _ = m(&src, "connect", &[dest.clone()]);
            }
        }
    }
    let rc: js_sys::Function = get(&g(), "MediaRecorder").dyn_into()?;
    let rec = js_sys::Reflect::construct(&rc, &js_sys::Array::of1(&get(&dest, "stream")))?;
    let chunks = js_sys::Array::new();
    let ch2 = chunks.clone();
    set(
        &rec,
        "ondataavailable",
        &cb1(move |ev| {
            ch2.push(&get(&ev, "data"));
        }),
    );
    let name = with(|x| x.peer_name.clone()).unwrap_or_default();
    let t0 = js_sys::Date::now();
    let ch3 = chunks.clone();
    let ctx2 = ctx.clone();
    let mime = get(&rec, "mimeType").as_string().unwrap_or_else(|| "audio/webm".into());
    set(
        &rec,
        "onstop",
        &cb1(move |_| {
            let _ = m(&ctx2, "close", &[]);
            let opts = web_sys::BlobPropertyBag::new();
            opts.set_type(&mime);
            let Ok(blob) = web_sys::Blob::new_with_blob_sequence_and_options(&ch3, &opts) else { return };
            let item = dom::obj(&[("id", js_sys::Date::now().into()), ("name", JsValue::from_str(&name)), ("at", t0.into()), ("dur", ((js_sys::Date::now() - t0) / 1000.0).round().into()), ("blob", blob.into())]);
            spawn_local(async move {
                let _ = db_put(item).await;
            });
        }),
    );
    m(&rec, "start", &[])?;
    with(|x| {
        x.rec = rec;
        x.rec_chunks = chunks;
        x.rec_ctx = ctx;
    });
    Ok(())
}
fn stop_rec(rec: &JsValue) {
    let _ = m(rec, "stop", &[]);
}

// IndexedDB: money-rec / rec
async fn db() -> Result<JsValue, JsValue> {
    let idb = get(&g(), "indexedDB");
    let req = m(&idb, "open", &["money-rec".into(), 1.into()])?;
    let p = js_sys::Promise::new(&mut |res, rej| {
        let r2 = req.clone();
        set(
            &req,
            "onupgradeneeded",
            &cb1(move |_| {
                let d = get(&r2, "result");
                let _ = m(&d, "createObjectStore", &["rec".into(), dom::obj(&[("keyPath", "id".into())])]);
            }),
        );
        let r3 = req.clone();
        set(&req, "onsuccess", &cb1(move |_| {
            let _ = res.call1(&JsValue::NULL, &get(&r3, "result"));
        }));
        set(&req, "onerror", &cb1(move |e| {
            let _ = rej.call1(&JsValue::NULL, &e);
        }));
    });
    JsFuture::from(p).await
}
async fn store_req(mode: &str, f: impl FnOnce(&JsValue) -> Result<JsValue, JsValue>) -> Result<JsValue, JsValue> {
    let d = db().await?;
    let tx = m(&d, "transaction", &["rec".into(), mode.into()])?;
    let st = m(&tx, "objectStore", &["rec".into()])?;
    let req = f(&st)?;
    let p = js_sys::Promise::new(&mut |res, rej| {
        let r2 = req.clone();
        set(&req, "onsuccess", &cb1(move |_| {
            let _ = res.call1(&JsValue::NULL, &get(&r2, "result"));
        }));
        set(&req, "onerror", &cb1(move |e| {
            let _ = rej.call1(&JsValue::NULL, &e);
        }));
    });
    JsFuture::from(p).await
}
async fn db_put(item: JsValue) -> Result<JsValue, JsValue> {
    store_req("readwrite", |st| m(st, "put", &[item])).await
}
async fn db_all() -> Result<JsValue, JsValue> {
    store_req("readonly", |st| m(st, "getAll", &[])).await
}
async fn db_del(id: f64) -> Result<JsValue, JsValue> {
    store_req("readwrite", |st| m(st, "delete", &[id.into()])).await
}

/* ---------- 녹음 저장소 (메시지 위쪽 줄 세 개 버튼) ---------- */

thread_local! {
    static URLS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn store_el() -> Option<web_sys::HtmlElement> {
    if let Some(e) = dom::by_id("rec-store") {
        return Some(e);
    }
    let app = dom::app_el()?;
    let e: web_sys::HtmlElement = dom::doc().create_element("section").ok()?.dyn_into().ok()?;
    e.set_id("rec-store");
    e.set_class_name("rec-store");
    let _ = e.set_attribute("aria-label", "녹음 저장소");
    e.set_hidden(true);
    let _ = app.append_child(&e);
    dom::on(&e, "click", |ev| {
        if dom::closest(&ev, "[data-rec-close]").is_some() {
            close_store();
            return;
        }
        if let Some(b) = dom::closest(&ev, "[data-rec-del]") {
            let id: f64 = dom::data(&b, "rec-del").parse().unwrap_or(0.0);
            spawn_local(async move {
                if !crate::shell::ask("녹음을 지울까요?", "이 기기에서 지워져요", "지우기", "취소").await {
                    return;
                }
                let _ = db_del(id).await;
                paint_store().await;
            });
        }
    });
    Some(e)
}

/// 녹음 저장소 열기
pub fn open_store() {
    let Some(e) = store_el() else { return };
    e.set_hidden(false);
    let _ = e.class_list().remove_1("out");
    let _ = e.offset_width();
    let _ = e.class_list().add_1("in");
    spawn_local(paint_store());
}
fn close_store() {
    let Some(e) = dom::by_id("rec-store") else { return };
    let _ = e.class_list().remove_1("in");
    let _ = e.class_list().add_1("out");
    let cb = Closure::once_into_js(move || {
        if e.class_list().contains("out") {
            e.set_hidden(true);
        }
    });
    let _ = win().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 380);
}
async fn paint_store() {
    let Some(e) = store_el() else { return };
    URLS.with(|u| {
        for url in u.borrow_mut().drain(..) {
            let _ = web_sys::Url::revoke_object_url(&url);
        }
    });
    let list: Vec<JsValue> = db_all().await.ok().and_then(|v| v.dyn_into::<js_sys::Array>().ok()).map(|a| a.to_vec()).unwrap_or_default();
    let mut list = list;
    list.sort_by(|a, b| get(b, "at").as_f64().unwrap_or(0.0).partial_cmp(&get(a, "at").as_f64().unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal));
    let rows: String = list
        .iter()
        .map(|r| {
            let url = get(r, "blob").dyn_into::<web_sys::Blob>().ok().and_then(|b| web_sys::Url::create_object_url_with_blob(&b).ok()).unwrap_or_default();
            URLS.with(|u| u.borrow_mut().push(url.clone()));
            let dur = get(r, "dur").as_f64().unwrap_or(0.0) as u64;
            let id = get(r, "id").as_f64().unwrap_or(0.0);
            format!(
                r#"<li class="rec-row"><div class="rec-main"><b>{}</b><span>{} · {:02}:{:02}</span><audio controls preload="metadata" src="{}"></audio></div><button class="rec-del" type="button" data-rec-del="{id}" aria-label="지우기">{}</button></li>"#,
                dom::esc(&get(r, "name").as_string().unwrap_or_default()),
                cloud::date_ko(get(r, "at").as_f64().unwrap_or(0.0)),
                dur / 60,
                dur % 60,
                dom::esc(&url),
                svg(r#"<path d="M5 7h14M10 7V5h4v2M7 7l1 12h8l1-12"/>"#)
            )
        })
        .collect();
    let body = if rows.is_empty() { r#"<p class="rec-empty">아직 녹음이 없어요. 통화 중에 녹음 버튼을 눌러 보세요</p>"#.to_string() } else { format!(r#"<ul class="rec-list">{rows}</ul>"#) };
    e.set_inner_html(&format!(
        r#"<header class="rec-head"><button class="glass-btn round" type="button" data-rec-close aria-label="닫기">{}</button><h2>녹음 저장소</h2><span></span></header><p class="rec-note">통화 녹음은 이 기기에만 저장돼요</p>{body}"#,
        svg(r#"<path d="M15 5l-7 7 7 7"/>"#)
    ));
}
