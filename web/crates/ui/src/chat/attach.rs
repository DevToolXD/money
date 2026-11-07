//! + 메뉴(카메라·사진·그리기·투표·토큰), 사진 보내기, 사진 크게 보기, 카드 누르기(선물 받기·요청 보내기·투표)

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use super::{ctx, open_chat, send_payload, toast};
use crate::dom::{self, by_id, safe_img};

/// 사진을 줄여서 글자(데이터 주소)로 만들어요. cap(글자 수)을 넘지 않을 때까지 화질과 크기를 줄여요. 서버 규칙도 같은 한도로 막아요
pub async fn read_image(file: web_sys::File, max: f64, cap: usize, q: f64, square: bool) -> Result<String, String> {
    if !file.type_().starts_with("image/") || file.size() > 25.0 * 1024.0 * 1024.0 {
        return Err("사진이 아니에요".into());
    }
    let url = web_sys::Url::create_object_url_with_blob(&file).map_err(|_| "사진을 열지 못했어요".to_string())?;
    let img = web_sys::HtmlImageElement::new().map_err(|_| "사진을 열지 못했어요".to_string())?;
    let loaded = js_sys::Promise::new(&mut |res, rej| {
        img.set_onload(Some(&res));
        img.set_onerror(Some(&rej));
    });
    img.set_src(&url);
    let r = JsFuture::from(loaded).await;
    let _ = web_sys::Url::revoke_object_url(&url);
    r.map_err(|_| "사진을 열지 못했어요".to_string())?;
    let (w, h) = (img.natural_width() as f64, img.natural_height() as f64);
    let sw = if square { w.min(h) } else { w };
    let sh = if square { sw } else { h };
    let (sx, sy) = ((w - sw) / 2.0, (h - sh) / 2.0);
    let mut k = (max / sw.max(sh)).min(1.0);
    for _ in 0..6 {
        let c: web_sys::HtmlCanvasElement = dom::doc().create_element("canvas").map_err(|_| "x")?.unchecked_into();
        c.set_width((sw * k).round().max(1.0) as u32);
        c.set_height((sh * k).round().max(1.0) as u32);
        if let Some(ctx) = c.get_context("2d").ok().flatten().and_then(|x| x.dyn_into::<web_sys::CanvasRenderingContext2d>().ok()) {
            let _ = ctx.draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(&img, sx, sy, sw, sh, 0.0, 0.0, c.width() as f64, c.height() as f64);
        }
        let mut s = q;
        while s >= 0.34 {
            if let Ok(out) = c.to_data_url_with_type_and_encoder_options("image/jpeg", &JsValue::from_f64(s)) {
                if out.len() <= cap {
                    return Ok(out);
                }
            }
            s -= 0.08;
        }
        k *= 0.8;
    }
    Err("사진이 너무 커요".into())
}
/// 파일 칸에서 고른 첫 파일 (고른 뒤 칸은 비워요)
pub fn take_file(id: &str) -> Option<web_sys::File> {
    let inp: web_sys::HtmlInputElement = by_id(id)?.dyn_into().ok()?;
    let f = inp.files()?.get(0);
    inp.set_value("");
    f
}

async fn send_photo(file: Option<web_sys::File>) {
    let (Some(file), Some(c)) = (file, open_chat()) else { return };
    let r = async {
        let img = read_image(file, 720.0, 130000, 0.66, false).await.map_err(|_| ())?;
        send_payload(&c, dom::obj(&[("type", "photo".into()), ("img", JsValue::from_str(&img))])).await.map_err(|_| ())
    }
    .await;
    if r.is_err() {
        toast("사진을 보내지 못했어요");
    }
}

async fn on_card(e: web_sys::Event) {
    let Some(c) = open_chat() else { return };
    if let Some(media) = dom::closest(&e, "[data-view]") {
        let i: usize = dom::data(&media, "view").parse().unwrap_or(usize::MAX);
        let img = c.msgs.get(i).map(|m| cloud::get(&m.raw, "img").as_string().unwrap_or_default()).unwrap_or_default();
        let src = safe_img(&img, 140000);
        if src.is_empty() {
            return;
        }
        if let Some(v) = by_id("viewer-img") {
            let _ = v.set_attribute("src", src);
        }
        if let Some(v) = by_id("viewer") {
            v.set_hidden(false);
            leptos::prelude::request_animation_frame(move || {
                let _ = v.class_list().add_1("is-open");
            });
        }
        return;
    }
    if let Some(act) = dom::closest(&e, "[data-act]") {
        let i: usize = dom::data(&act, "i").parse().unwrap_or(usize::MAX);
        let Some(m) = c.msgs.get(i).map(|m| m.raw.clone()) else { return };
        if cloud::get(&m, "done").is_truthy() {
            return;
        }
        let amount = js_sys::Number::from(cloud::get(&m, "amount")).value_of();
        let mid = cloud::get(&m, "id").as_string().unwrap_or_default();
        let ko = cloud::fmt_num(amount, 3);
        let cx = ctx();
        if dom::data(&act, "act") == "cancel" {
            if !cx.ask(&format!("토큰 {ko} 선물을 취소할까요?"), "상대가 아직 받지 않아서 토큰이 그대로 돌아와요.", "선물 취소", "닫기").await {
                return;
            }
            let (cid, mid2) = (c.id.clone(), mid.clone());
            let f = Closure::once_into_js(move || cloud::call_now("cancelGift", &[JsValue::from_str(&cid), JsValue::from_str(&mid2)]).unwrap_or(JsValue::UNDEFINED));
            match JsFuture::from(crate::money::wallet_op(f.unchecked_into())).await {
                Ok(_) => {
                    crate::money::log(&dom::obj(&[("t", js_sys::Date::now().into()), ("name", JsValue::from_str(&format!("{}님에게 보낸 선물 취소", c.name))), ("cat", "송금".into()), ("icon", "gift".into()), ("amt", amount.into())]));
                    toast(&format!("선물을 취소했어요. 토큰 {ko}이 돌아왔어요"));
                }
                Err(_) => toast("취소하지 못했어요. 상대가 이미 받았을 수 있어요"),
            }
        } else if dom::data(&act, "act") == "accept" {
            let (cid, mid2) = (c.id.clone(), mid.clone());
            let f = Closure::once_into_js(move || cloud::call_now("claimGift", &[JsValue::from_str(&cid), JsValue::from_str(&mid2), amount.into()]).unwrap_or(JsValue::UNDEFINED));
            match JsFuture::from(crate::money::wallet_op(f.unchecked_into())).await {
                Ok(_) => {
                    crate::money::log(&dom::obj(&[("t", js_sys::Date::now().into()), ("name", JsValue::from_str(&format!("{}님의 선물", c.name))), ("cat", "송금".into()), ("icon", "gift".into()), ("amt", amount.into())]));
                    toast(&format!("토큰 {ko}을 받았어요"));
                }
                Err(_) => toast("받지 못했어요. 잠시 뒤 다시 해 주세요"),
            }
        } else {
            let bal = crate::money::balance();
            if bal.is_none_or(|b| b < amount) {
                cx.ask("토큰이 부족해요", &format!("내 토큰은 {}이에요.", cloud::fmt_num(bal.unwrap_or(0.0), 3)), "확인", "").await;
                return;
            }
            if !cx.ask(&format!("토큰 {ko}을 보낼까요?"), &format!("{}님의 요청이에요.", c.name), "보내기", "취소").await {
                return;
            }
            let (cid, other) = (c.id.clone(), c.other.clone());
            let f = Closure::once_into_js(move || cloud::call_now("payRequest", &[JsValue::from_str(&cid), JsValue::from_str(&other), JsValue::from_str(&mid), amount.into()]).unwrap_or(JsValue::UNDEFINED));
            match JsFuture::from(crate::money::wallet_op(f.unchecked_into())).await {
                Ok(_) => {
                    crate::money::log(&dom::obj(&[("t", js_sys::Date::now().into()), ("name", JsValue::from_str(&format!("{}님에게 선물", c.name))), ("cat", "송금".into()), ("icon", "gift".into()), ("amt", (-amount).into())]));
                    toast("보냈어요");
                }
                Err(_) => toast("보내지 못했어요. 잠시 뒤 다시 해 주세요"),
            }
        }
        return;
    }
    if let Some(v) = dom::closest(&e, "[data-vote]") {
        let i: usize = dom::data(&v, "i").parse().unwrap_or(usize::MAX);
        if let Some(m) = c.msgs.get(i) {
            let k: f64 = dom::data(&v, "vote").parse().unwrap_or(0.0);
            if cloud::call("vote", &[JsValue::from_str(&c.id), cloud::get(&m.raw, "id"), k.into()]).await.is_err() {
                toast("투표하지 못했어요");
            }
        }
    }
}

pub fn init() {
    let back = by_id("attach-back");
    if let (Some(plus), Some(bk)) = (by_id("plus"), back.clone()) {
        dom::on(&plus, "click", move |_| {
            if open_chat().is_some() {
                let _ = bk.class_list().add_1("is-open");
            }
        });
    }
    if let Some(bk) = back.clone() {
        let b2 = bk.clone();
        dom::on(&bk, "pointerdown", move |e| {
            if e.target().is_some_and(|t| t == ***b2) {
                let _ = b2.class_list().remove_1("is-open");
            }
        });
    }
    for id in ["file-camera", "file-photo"] {
        if let Some(f) = by_id(id) {
            dom::on(&f, "change", move |_| leptos::task::spawn_local(send_photo(take_file(id))));
        }
    }
    if let Some(a) = by_id("attach") {
        dom::on(&a, "click", move |e| {
            let Some(b) = dom::closest(&e, "[data-a]") else { return };
            if let Some(bk) = &back {
                let _ = bk.class_list().remove_1("is-open");
            }
            match dom::data(&b, "a").as_str() {
                "camera" => by_id("file-camera").into_iter().for_each(|f| f.click()),
                "photo" => by_id("file-photo").into_iter().for_each(|f| f.click()),
                "draw" => super::draw::open(),
                "poll" => super::poll::open(),
                "token" => {
                    if let Some(c) = open_chat() {
                        super::gift::open(&c.id, &c.other, &c.name);
                    }
                }
                _ => {}
            }
        });
    }
    if let Some(b) = by_id("bubbles") {
        dom::on(&b, "click", |e| leptos::task::spawn_local(on_card(e)));
    }
    if let Some(v) = by_id("viewer") {
        let v2 = v.clone();
        dom::on(&v, "click", move |_| {
            let _ = v2.class_list().remove_1("is-open");
            let v3 = v2.clone();
            leptos::prelude::set_timeout(move || v3.set_hidden(true), std::time::Duration::from_millis(250));
        });
    }
}
