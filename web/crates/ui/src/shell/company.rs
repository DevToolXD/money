//! 사업 탭의 틀: 내 회사들, 고른 회사(기기에 기억), 회사 바꾸기(물러났다 들어오는 움직임), 회사 프로필(업종 바꾸기), 창업 시트

use std::cell::{Cell, RefCell};

use js_sys::Object;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::{sheet_by_id, sheet_open, toast};
use crate::dom::{self, avatar_html, by_id, esc, safe_img};

thread_local! {
    static COMPANIES: RefCell<Vec<JsValue>> = const { RefCell::new(Vec::new()) };
    /// 지금 고른 회사. undefined: 불러오는 중, null: 아직 회사가 없어요
    static COMPANY: RefCell<JsValue> = const { RefCell::new(JsValue::UNDEFINED) };
    static SWAP_T: Cell<i32> = const { Cell::new(0) };
    static CO_KIND: RefCell<String> = RefCell::new("food".into());
    static FOUND_KIND: RefCell<String> = RefCell::new("food".into());
    static FOUND_LOGO: RefCell<String> = const { RefCell::new(String::new()) };
    static FOUNDING: Cell<bool> = const { Cell::new(false) };
}
fn kind_label(k: &str) -> &'static str {
    match k {
        "pc" => "컴퓨터 회사",
        "ai" => "인공지능 회사",
        "nuclear" => "원자력 발전 회사",
        _ => "식품 회사",
    }
}
fn s(v: &JsValue, k: &str) -> String {
    cloud::get(v, k).as_string().unwrap_or_default()
}
/// 내 회사 전부 (사장인 곳)
pub fn all() -> Vec<JsValue> {
    COMPANIES.with(|c| c.borrow().clone())
}
pub fn current() -> JsValue {
    COMPANY.with(|c| c.borrow().clone())
}
fn has_co() -> bool {
    let c = current();
    !(c.is_null() || c.is_undefined())
}
/// 고른 회사 값을 바꿔요 (모집 인원처럼 서버에 쓴 뒤 화면 값만 맞출 때)
pub fn patch(p: &JsValue) {
    if !has_co() {
        return;
    }
    let base = Object::assign(&Object::new(), current().unchecked_ref());
    let next: JsValue = Object::assign(&base, p.unchecked_ref()).into();
    let id = s(&next, "id");
    COMPANIES.with(|cs| {
        for c in cs.borrow_mut().iter_mut() {
            if s(c, "id") == id {
                *c = next.clone();
            }
        }
    });
    COMPANY.with(|c| *c.borrow_mut() = next);
}

pub(crate) fn render() {
    let co = current();
    dom::set_hidden("biz-loading", !co.is_undefined());
    dom::set_hidden("biz-none", !co.is_null());
    let has = has_co();
    dom::set_hidden("biz-dash", !has);
    dom::set_hidden("biz-logo", !has);
    if !has {
        return;
    }
    let (name, logo) = (s(&co, "name"), s(&co, "logo"));
    let mark = avatar_html(&name, &logo, "avatar contact");
    if let Some(l) = by_id("biz-logo") {
        l.set_inner_html(&mark);
    }
    if let Some(m) = by_id("biz-mark") {
        m.set_inner_html(&mark);
    }
    dom::set_text("biz-name", &name);
    let at = cloud::get(&co, "at").as_f64().filter(|v| *v > 0.0).unwrap_or_else(js_sys::Date::now);
    let d = js_sys::Date::new(&JsValue::from_f64(at));
    dom::set_text("biz-since", &format!("{}년 {}월 {}일에 창업했어요 · {}", d.get_full_year(), d.get_month() + 1, d.get_date(), kind_label(&s(&co, "kind"))));
    let list = COMPANIES.with(|c| c.borrow().clone());
    if let Some(sw) = by_id("biz-switch") {
        sw.set_hidden(list.len() < 2);
        let id = s(&co, "id");
        sw.set_inner_html(&list.iter().map(|c| format!(r#"<button type="button" class="gg wk-pill" aria-pressed="{}" data-co="{}">{}</button>"#, s(c, "id") == id, esc(&s(c, "id")), esc(&s(c, "name")))).collect::<String>());
    }
}
/// 회사를 고르면 그 회사의 사업 화면을 그려요 (고른 회사는 기기에 기억해요)
fn commit(id: &str) {
    let list = COMPANIES.with(|c| c.borrow().clone());
    let co = list.iter().find(|c| s(c, "id") == id).or(list.first()).cloned().unwrap_or(JsValue::NULL);
    dom::store_set("biz-pick", &if co.is_null() { String::new() } else { s(&co, "id") });
    COMPANY.with(|c| *c.borrow_mut() = co);
    render();
    if has_co() {
        crate::biz::refresh();
    }
    if dom::screen_active("work") {
        crate::work::refresh(true);
    }
}
/// 회사를 바꿀 때: 지금 화면이 살짝 물러났다가 새 회사 화면이 오른쪽에서 들어와요
fn select(id: &str) {
    let w = web_sys::window().expect("window");
    let next = COMPANIES.with(|c| c.borrow().iter().any(|c| s(c, "id") == id));
    let dash = by_id("biz-dash");
    if !has_co() || !next || s(&current(), "id") == id || dash.as_ref().is_none_or(|d| d.hidden()) {
        return commit(id);
    }
    let Some(dash) = dash else { return };
    w.clear_timeout_with_handle(SWAP_T.get());
    let _ = dash.class_list().remove_1("swap-in");
    let _ = dash.class_list().add_1("swap-out");
    let id = id.to_string();
    let cb = Closure::once_into_js(move || {
        commit(&id);
        let _ = dash.class_list().remove_1("swap-out");
        let _ = dash.class_list().add_1("swap-in");
        let d2 = dash.clone();
        let cb2 = Closure::once_into_js(move || {
            let _ = d2.class_list().remove_1("swap-in");
        });
        if let Some(w) = web_sys::window() {
            SWAP_T.set(w.set_timeout_with_callback_and_timeout_and_arguments_0(cb2.unchecked_ref(), 380).unwrap_or(0));
        }
    });
    SWAP_T.set(w.set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 180).unwrap_or(0));
}
/// 로그인한 뒤 내 회사들을 읽어요
pub fn load() {
    leptos::task::spawn_local(async {
        let list = match cloud::call("listMyCompanies", &[]).await {
            Ok(v) => js_sys::Array::from(&v).iter().collect(),
            Err(e) => {
                cloud::warn(&format!("회사를 불러오지 못했어요: {}", e.code));
                Vec::new()
            }
        };
        COMPANIES.with(|c| *c.borrow_mut() = list);
        select(&dom::store_get("biz-pick", ""));
    });
}

/* ---------- 회사 프로필: 업종은 여기서 골라서 "업종 바꾸기"를 눌러야 바뀌어요 ---------- */

fn paint_co_kinds() {
    let k = CO_KIND.with(|k| k.borrow().clone());
    for b in dom::qsa(&dom::doc().document_element().expect("html"), "#co-kinds [data-kind]") {
        let _ = b.set_attribute("aria-pressed", &(dom::data(&b, "kind") == k).to_string());
    }
    if let Some(b) = by_id("co-save").and_then(|b| b.dyn_into::<web_sys::HtmlButtonElement>().ok()) {
        b.set_disabled(!has_co() || s(&current(), "kind") == k);
    }
}
thread_local! {
    static CO_LOGO: RefCell<Option<String>> = const { RefCell::new(None) };
}
/// 회사 이름과 마크 저장 (사장만 이 판을 열어요)
async fn save_info() {
    if !has_co() {
        return;
    }
    let co = current();
    let name = by_id("co-name-in").and_then(|i| i.dyn_into::<web_sys::HtmlInputElement>().ok()).map(|i| i.value().trim().to_string()).unwrap_or_default();
    let chars = name.chars().count();
    if chars == 0 || chars > 20 || name.contains('<') || name.contains('>') {
        return dom::set_text("co-edit-err", "이름은 1~20자로, < > 없이 적어 주세요");
    }
    let logo = CO_LOGO.with(|l| l.borrow().clone()).unwrap_or_else(|| s(&co, "logo"));
    if name == s(&co, "name") && logo == s(&co, "logo") {
        return sheet_open(false, None);
    }
    match cloud::call("setCompanyInfo", &[JsValue::from_str(&s(&co, "id")), JsValue::from_str(&name), JsValue::from_str(&logo)]).await {
        Ok(_) => {
            let p = crate::dom::obj(&[("name", JsValue::from_str(&name)), ("logo", JsValue::from_str(&logo))]);
            patch(&p);
            render();
            if dom::screen_active("work") {
                crate::work::refresh(true);
            }
            sheet_open(false, None);
            toast("회사 정보를 저장했어요");
        }
        Err(_) => dom::set_text("co-edit-err", "저장하지 못했어요. 잠시 뒤 다시 해 주세요"),
    }
}
async fn save_kind() {
    let k = CO_KIND.with(|k| k.borrow().clone());
    if !has_co() || s(&current(), "kind") == k {
        return;
    }
    let Some(btn) = by_id("co-save") else { return };
    let _ = btn.class_list().add_1("busy");
    match cloud::call("setCompanyKind", &[JsValue::from_str(&s(&current(), "id")), JsValue::from_str(&k)]).await {
        Ok(_) => {
            patch(&dom::obj(&[("kind", JsValue::from_str(&k))]));
            render();
            crate::work::refresh(true);
            sheet_open(false, None);
            toast(&format!("업종을 {}로 바꿨어요", kind_label(&k)));
        }
        Err(_) => dom::set_text("co-err", "업종을 바꾸지 못했어요. 잠시 뒤 다시 해 주세요"),
    }
    let _ = btn.class_list().remove_1("busy");
    paint_co_kinds();
}

/* ---------- 창업 ---------- */

fn paint_found_kind() {
    let k = FOUND_KIND.with(|k| k.borrow().clone());
    if let Some(box_) = by_id("found-kinds") {
        for b in dom::qsa(&box_, "button") {
            let _ = b.set_attribute("aria-pressed", &(dom::data(&b, "kind") == k).to_string());
        }
    }
}
fn paint_found_logo() {
    let logo = FOUND_LOGO.with(|l| l.borrow().clone());
    let ok = safe_img(&logo, 40000);
    if let Some(img) = by_id("found-logo-img") {
        let _ = img.style().set_property("background-image", &if ok.is_empty() { String::new() } else { format!("url({ok})") });
    }
    if let Some(b) = by_id("found-logo") {
        dom::toggle(&b, "has-img", !ok.is_empty());
    }
}
fn open_founding() {
    if !crate::chat::live() {
        if cloud::signed_in() {
            return toast("아직 불러오는 중이에요. 잠시 뒤 다시 눌러 주세요");
        }
        return super::login::show_login(true);
    }
    let bal = crate::money::balance();
    if bal.is_none_or(|b| b < 10.0) {
        leptos::task::spawn_local(async {
            match crate::money::ensure(10.0).await {
                Ok(_) => open_founding(),
                Err(m) => toast(&format!("창업에는 10 토큰이 필요해요. {m}")),
            }
        });
        return;
    }
    if let Some(i) = by_id("found-name").and_then(|i| i.dyn_into::<web_sys::HtmlInputElement>().ok()) {
        i.set_value("");
    }
    FOUND_KIND.with(|k| *k.borrow_mut() = "food".into());
    paint_found_kind();
    FOUND_LOGO.with(|l| l.borrow_mut().clear());
    dom::set_text("found-err", "");
    paint_found_logo();
    dom::set_text("found-cost", &format!("10 토큰이 들어요 · 내 토큰 {}", crate::money::w(bal.unwrap_or(0.0))));
    sheet_by_id(true, "found-sheet");
}
async fn found() {
    if FOUNDING.get() {
        return;
    }
    let name = crate::biz::clean_name(&dom::input_value("found-name"));
    if name.is_empty() {
        return dom::set_text("found-err", "회사 이름을 적어 주세요");
    }
    if name.chars().count() > 20 || name.contains(['<', '>']) {
        return dom::set_text("found-err", "이름은 20자 이하로, < > 없이 적어 주세요");
    }
    if let Err(m) = crate::money::ensure(10.0).await {
        return dom::set_text("found-err", &m);
    }
    // 업종은 한 번 정하면 바꿀 수 없어서 두 번 물어요
    let kind0 = FOUND_KIND.with(|k| k.borrow().clone());
    let label = kind_label(&kind0);
    if !super::ask(&format!("{label}로 정할까요?"), "이 회사는 이 업종의 상품만 만들고 팔 수 있어요", "정하기", "취소").await {
        return;
    }
    if !super::ask("정말 정할까요?", "한 번 정하면 바꿀 수 없어요", "네, 창업할게요", "다시 고를게요").await {
        return;
    }
    FOUNDING.set(true);
    let btn = by_id("found-go");
    if let Some(b) = &btn {
        let _ = b.class_list().add_1("busy");
    }
    dom::set_text("found-err", "");
    let logo = FOUND_LOGO.with(|l| l.borrow().clone());
    let kind = FOUND_KIND.with(|k| k.borrow().clone());
    let (n2, l2) = (name.clone(), logo.clone());
    let k2 = kind.clone();
    let f = Closure::once_into_js(move || cloud::call_now("foundCompany", &[JsValue::from_str(&n2), JsValue::from_str(&l2), JsValue::from_str(&k2)]).unwrap_or(JsValue::UNDEFINED));
    match wasm_bindgen_futures::JsFuture::from(crate::money::wallet_op(f.unchecked_into())).await {
        Ok(cid) => {
            let cid = cid.as_string().unwrap_or_default();
            let uid = cloud::cloud().map(|c| s(&c, "uid")).unwrap_or_default();
            let co = dom::obj(&[
                ("id", JsValue::from_str(&cid)),
                ("name", JsValue::from_str(&name)),
                ("logo", JsValue::from_str(&logo)),
                ("kind", JsValue::from_str(&kind)),
                ("kindSet", true.into()),
                ("owner", JsValue::from_str(&uid)),
                ("open", 0.into()),
                ("staff", 0.into()),
                ("products", js_sys::Array::new().into()),
                ("at", js_sys::Date::now().into()),
            ]);
            COMPANIES.with(|c| c.borrow_mut().push(co));
            crate::money::log(&dom::obj(&[("t", js_sys::Date::now().into()), ("name", "창업".into()), ("cat", "사업".into()), ("icon", "store".into()), ("amt", (-10).into())]));
            sheet_open(false, None);
            select(&cid);
            toast(&format!("{name}을(를) 창업했어요"));
        }
        Err(e) => dom::set_text(
            "found-err",
            if cloud::get(&e, "code").as_string().as_deref() == Some("permission-denied") { "창업하지 못했어요. 이미 회사가 있거나 토큰이 부족해요" } else { "창업하지 못했어요. 잠시 뒤 다시 해 주세요" },
        ),
    }
    FOUNDING.set(false);
    if let Some(b) = btn {
        let _ = b.class_list().remove_1("busy");
    }
}

pub fn init() {
    if let Some(sw) = by_id("biz-switch") {
        dom::on(&sw, "click", |e| {
            if let Some(b) = dom::closest(&e, "[data-co]") {
                select(&dom::data(&b, "co"));
            }
        });
    }
    if let Some(k) = by_id("co-kinds") {
        dom::on(&k, "click", |e| {
            if let Some(b) = dom::closest(&e, "[data-kind]") {
                CO_KIND.with(|k| *k.borrow_mut() = dom::data(&b, "kind"));
                paint_co_kinds();
            }
        });
    }
    if let Some(l) = by_id("biz-logo") {
        dom::on(&l, "click", |_| {
            if !has_co() {
                return;
            }
            let co = current();
            CO_KIND.with(|k| *k.borrow_mut() = s(&co, "kind"));
            if let Some(m) = by_id("co-mark") {
                m.set_inner_html(&avatar_html(&s(&co, "name"), &s(&co, "logo"), "avatar contact"));
            }
            dom::set_text("co-name", &s(&co, "name"));
            CO_LOGO.with(|l| *l.borrow_mut() = None);
            if let Some(i) = by_id("co-name-in").and_then(|i| i.dyn_into::<web_sys::HtmlInputElement>().ok()) {
                i.set_value(&s(&co, "name"));
            }
            dom::set_text("co-edit-err", "");
            dom::set_text("co-since", kind_label(&s(&co, "kind")));
            dom::set_text("co-err", "");
            paint_co_kinds();
            sheet_by_id(true, "company-sheet");
        });
    }
    if let Some(b) = by_id("co-photo") {
        dom::on(&b, "click", |_| {
            if let Some(f) = by_id("co-file") {
                f.click();
            }
        });
    }
    if let Some(f) = by_id("co-file") {
        dom::on(&f, "change", |_| {
            let Some(file) = crate::chat::take_file("co-file") else { return };
            leptos::task::spawn_local(async move {
                match crate::chat::read_image(file, 192.0, 36000, 0.8, true).await {
                    Ok(img) => {
                        if let Some(m) = by_id("co-mark") {
                            m.set_inner_html(&avatar_html(&s(&current(), "name"), &img, "avatar contact"));
                        }
                        CO_LOGO.with(|l| *l.borrow_mut() = Some(img));
                    }
                    Err(_) => dom::set_text("co-edit-err", "사진을 불러오지 못했어요"),
                }
            });
        });
    }
    if let Some(b) = by_id("co-edit-save") {
        dom::on(&b, "click", |_| leptos::task::spawn_local(save_info()));
    }
    if let Some(b) = by_id("co-save") {
        dom::on(&b, "click", |_| leptos::task::spawn_local(save_kind()));
    }
    if let Some(k) = by_id("found-kinds") {
        dom::on(&k, "click", |e| {
            if let Some(b) = dom::closest(&e, "[data-kind]") {
                FOUND_KIND.with(|k| *k.borrow_mut() = dom::data(&b, "kind"));
                paint_found_kind();
            }
        });
    }
    for id in ["biz-start", "biz-more"] {
        if let Some(b) = by_id(id) {
            dom::on(&b, "click", |_| open_founding());
        }
    }
    if let Some(b) = by_id("found-cancel") {
        dom::on(&b, "click", |_| sheet_open(false, None));
    }
    if let Some(b) = by_id("found-logo") {
        dom::on(&b, "click", |_| {
            if let Some(f) = by_id("file-logo") {
                f.click();
            }
        });
    }
    if let Some(f) = by_id("file-logo") {
        dom::on(&f, "change", |_| {
            let Some(file) = crate::chat::take_file("file-logo") else { return };
            leptos::task::spawn_local(async move {
                match crate::chat::read_image(file, 192.0, 36000, 0.8, true).await {
                    Ok(img) => {
                        FOUND_LOGO.with(|l| *l.borrow_mut() = img);
                        paint_found_logo();
                    }
                    Err(_) => dom::set_text("found-err", "사진을 불러오지 못했어요"),
                }
            });
        });
    }
    if let Some(b) = by_id("found-go") {
        dom::on(&b, "click", |_| leptos::task::spawn_local(found()));
    }
    render();
}
