//! 직원 관리 (사장만 보여요): 회사 발전도와 종결까지 예상, 급여 금고 채우기, 직원마다 시급·일한 횟수·하루 평균 속도·마지막 활동, 해고.
//! 직원 목록은 회사를 고르고 30초 안에는 다시 읽지 않아요 (서버를 아껴요). 임금은 시간당 토큰이고, 직원이 정수 시간 단위로 받아 가요 (급여 금고에서).

use std::cell::{Cell, RefCell};

use cloud::{fmt_num, get};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use super::{co_str, company, ctx, mine_co};
use crate::dom::{self, avatar_html, by_id, esc};

const FRESH_MS: f64 = 30000.0;
const DAY: f64 = 86_400_000.0;

#[derive(Clone, Default)]
struct Member {
    uid: String,
    at: f64,
    wage: f64,
    n: f64,
    seen: f64,
}

thread_local! {
    static LIST: RefCell<Vec<Member>> = const { RefCell::new(Vec::new()) };
    static LAB: RefCell<Option<[f64; 6]>> = const { RefCell::new(None) };
    static FOR: RefCell<String> = const { RefCell::new(String::new()) };
    static AT: Cell<f64> = const { Cell::new(0.0) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
    static BOUND: Cell<bool> = const { Cell::new(false) };
}

fn num(v: &JsValue, k: &str) -> f64 {
    get(v, k).as_f64().unwrap_or(0.0)
}
fn name_photo(uid: &str) -> (String, String) {
    let p = cloud::call_now("peek", &[JsValue::from_str(uid)]).unwrap_or(JsValue::NULL);
    let g = |k: &str| get(&p, k).as_string().unwrap_or_default();
    let n = g("name");
    (if n.is_empty() { g("id") } else { n }, g("photo"))
}
fn ago(ms: f64) -> String {
    if ms <= 0.0 {
        return "활동 기록 없음".into();
    }
    let s = ((js_sys::Date::now() - ms) / 1000.0).max(0.0);
    if s < 120.0 {
        "방금 활동".into()
    } else if s < 3600.0 {
        format!("{}분 전 활동", (s / 60.0) as i64)
    } else if s < 86400.0 {
        format!("{}시간 전 활동", (s / 3600.0) as i64)
    } else {
        format!("{}일 전 활동", (s / 86400.0) as i64)
    }
}
fn svg(d: &str) -> String {
    format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{d}</svg>"#)
}

/// 사업 탭을 그릴 때마다 불러요
pub fn paint() {
    let sec = by_id("biz-staff").and_then(|e| e.closest(".biz-blk").ok().flatten());
    let Some(co) = company().filter(|_| mine_co() && cloud::signed_in()) else {
        if let Some(s) = sec {
            let _ = s.set_attribute("hidden", "");
        }
        return;
    };
    if let Some(s) = sec {
        let _ = s.remove_attribute("hidden");
    }
    bind();
    let cid = co_str(&co, "id");
    let same = FOR.with(|f| *f.borrow() == cid);
    if same && js_sys::Date::now() - AT.get() < FRESH_MS {
        return render();
    }
    if !same {
        FOR.with(|f| *f.borrow_mut() = cid.clone());
        LIST.with(|l| l.borrow_mut().clear());
        LAB.with(|l| *l.borrow_mut() = None);
    }
    render();
    load(cid);
}

fn load(cid: String) {
    AT.set(js_sys::Date::now());
    spawn_local(async move {
        let staff = cloud::call("listStaff", &[JsValue::from_str(&cid)]).await;
        let lab = cloud::lab_get(&cid).await;
        if FOR.with(|f| *f.borrow() != cid) {
            return;
        }
        if let Ok(v) = staff {
            let list: Vec<Member> = js_sys::Array::from(&v).iter().map(|m| Member { uid: get(&m, "uid").as_string().unwrap_or_default(), at: num(&m, "at"), wage: num(&m, "wage"), n: num(&m, "n"), seen: num(&m, "seen") }).collect();
            let who: js_sys::Array = list.iter().map(|m| JsValue::from_str(&m.uid)).collect();
            LIST.with(|l| *l.borrow_mut() = list);
            let _ = cloud::call("fetchProfiles", &[who.into()]).await;
        } else {
            AT.set(0.0);
        }
        if let Ok(l) = lab {
            if !l.is_null() && !l.is_undefined() {
                let k = ["nperf", "cap", "spd", "pow", "aiData", "fperf"];
                let mut a = [0.0; 6];
                for (i, key) in k.iter().enumerate() {
                    a[i] = num(&l, key);
                }
                if a[0] == 0.0 {
                    a[0] = 100.0;
                }
                LAB.with(|x| *x.borrow_mut() = Some(a));
            }
        }
        render();
    });
}

fn render() {
    let Some(el) = by_id("biz-staff") else { return };
    let Some(co) = company() else { return };
    let kind = co_str(&co, "kind");
    let pay = num(&co, "pay") / 100.0;
    let founded = num(&co, "at");
    // 발전도
    let prog = LAB.with(|l| l.borrow().map(|a| crate::balance::progress(&kind, &a)));
    let days = ((js_sys::Date::now() - founded) / DAY).max(1.0 / 24.0);
    let dev = match prog {
        Some(p) => {
            let pct = p * 100.0;
            let eta = if p >= 1.0 {
                "종결에 닿았어요".to_string()
            } else if p <= 0.0 {
                "아직 시작 단계예요".to_string()
            } else {
                // 지금까지 평균 속도로 계속 일한다고 보면 (올라갈수록 느려지니 실제로는 더 걸려요)
                let left = days * (1.0 - p) / p;
                format!("지금 평균 속도라면 종결까지 약 {}일 더 걸려요 (갈수록 느려져요)", fmt_num(left.ceil(), 0))
            };
            format!(
                r#"<div class="stf-dev"><div class="stf-dh"><b>회사 발전</b><span>{}%</span></div><div class="stf-bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="{}"><i style="width:{}%"></i></div><p class="stf-note">하루 평균 {}%p · {}</p></div>"#,
                fmt_num(pct, 2),
                pct.round(),
                pct.clamp(0.0, 100.0).max(if pct > 0.0 { 1.0 } else { 0.0 }),
                fmt_num(pct / days, 3),
                eta
            )
        }
        None => r#"<div class="stf-dev"><div class="stf-dh"><b>회사 발전</b><span>…</span></div></div>"#.to_string(),
    };
    let fund = format!(
        r#"<div class="stf-fund"><div><b>급여 금고</b><span>{} 토큰</span></div><label class="stf-in"><input id="stf-fund-n" type="number" inputmode="numeric" min="1" step="1" placeholder="토큰" aria-label="급여 금고에 넣을 토큰"></label><button type="button" class="btn" data-act="fund">채우기</button></div><p class="stf-note">직원은 시간당 임금을 정수 시간 단위로 급여 금고에서 받아 가요. 금고가 비면 받을 수 없어요.</p>"#,
        fmt_num(pay, 2)
    );
    let members = LIST.with(|l| l.borrow().clone());
    let rows: String = members
        .iter()
        .map(|m| {
            let (n, ph) = name_photo(&m.uid);
            let d = ((js_sys::Date::now() - m.at) / DAY).max(1.0);
            format!(
                r#"<li class="stf-row" data-uid="{u}">{av}<div class="stf-main"><b>{n}</b><span>{since}일째 · 총 {cnt}번</span><span>하루 {avg}번 · {seen}</span></div><label class="stf-wage"><span>시급</span><input type="number" inputmode="numeric" min="0" max="1000000" step="1" value="{w}" data-uid="{u}" aria-label="{n}의 시급 토큰"></label><button type="button" class="stf-fire" data-act="fire" data-uid="{u}" aria-label="{n} 해고">{x}</button></li>"#,
                u = esc(&m.uid),
                av = avatar_html(&n, &ph, "avatar contact stf-av"),
                n = esc(&n),
                since = d.floor() as i64,
                cnt = fmt_num(m.n, 0),
                avg = fmt_num(m.n / d, 1),
                seen = ago(m.seen),
                w = m.wage as i64,
                x = svg(r#"<path d="M9 7V5.500A1.500 1.500 0 0 1 10.500 4h3A1.500 1.500 0 0 1 15 5.500V7M5 7h14M7 7l.7 11.200A1.500 1.500 0 0 0 9.200 19.500h5.600a1.500 1.500 0 0 0 1.500-1.300L17 7"/>"#),
            )
        })
        .collect();
    let list = if members.is_empty() { r#"<p class="stf-empty">직원이 아직 없어요. 위에서 채용 공고를 올려 보세요</p>"#.to_string() } else { format!(r#"<ul class="stf-list">{rows}</ul>"#) };
    // 쓰고 있던 숫자는 다시 그려도 남겨 둬요
    let typing = by_id("stf-fund-n").and_then(|i| i.dyn_into::<web_sys::HtmlInputElement>().ok()).map(|i| i.value()).unwrap_or_default();
    el.set_inner_html(&format!(r#"{dev}<h3 class="stf-h">급여</h3>{fund}<h3 class="stf-h">직원 {}명</h3>{list}"#, members.len()));
    if !typing.is_empty() {
        if let Some(i) = by_id("stf-fund-n").and_then(|i| i.dyn_into::<web_sys::HtmlInputElement>().ok()) {
            i.set_value(&typing);
        }
    }
}

fn bind() {
    if BOUND.replace(true) {
        return;
    }
    let Some(el) = by_id("biz-staff") else { return };
    dom::on(&el, "click", |e| {
        let Some(b) = dom::closest(&e, "[data-act]") else { return };
        match dom::data(&b, "act").as_str() {
            "fund" => fund(),
            "fire" => fire(dom::data(&b, "uid")),
            _ => {}
        }
    });
    dom::on(&el, "change", |e| {
        let Some(t) = e.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) else { return };
        if t.closest(".stf-wage").ok().flatten().is_none() {
            return;
        }
        let uid = t.get_attribute("data-uid").unwrap_or_default();
        let w = t.value().trim().parse::<f64>().ok().filter(|v| v.is_finite()).map(|v| v.floor()).unwrap_or(-1.0);
        if !(0.0..=1_000_000.0).contains(&w) {
            t.set_value("0");
            return ctx().toast("임금은 0부터 1,000,000 토큰까지 정수로 적어 주세요");
        }
        spawn_local(async move {
            match cloud::call("setWage", &[JsValue::from_str(&uid), JsValue::from(w)]).await {
                Ok(_) => {
                    LIST.with(|l| l.borrow_mut().iter_mut().filter(|m| m.uid == uid).for_each(|m| m.wage = w));
                    ctx().toast(&format!("시급을 {} 토큰으로 정했어요", fmt_num(w, 0)));
                }
                Err(_) => ctx().toast("임금을 정하지 못했어요. 잠시 뒤 다시 해 주세요"),
            }
        });
    });
}

fn fund() {
    let Some(co) = company() else { return };
    let amt = by_id("stf-fund-n").and_then(|i| i.dyn_into::<web_sys::HtmlInputElement>().ok()).and_then(|i| i.value().trim().parse::<f64>().ok()).map(|v| v.floor()).unwrap_or(0.0);
    if !(amt >= 1.0) || amt > 100_000_000.0 {
        return ctx().toast("넣을 토큰을 1 이상 정수로 적어 주세요");
    }
    if crate::money::balance().is_none_or(|b| b < amt) {
        return ctx().toast("지갑 토큰이 모자라요");
    }
    if BUSY.replace(true) {
        return;
    }
    let cid = co_str(&co, "id");
    spawn_local(async move {
        match cloud::call("fundPay", &[JsValue::from_str(&cid), JsValue::from(amt)]).await {
            Ok(_) => {
                let cur = company().map(|c| num(&c, "pay")).unwrap_or(0.0);
                let _ = ctx().call("patchCompany", &[dom::obj(&[("pay", JsValue::from(cur + amt * 100.0))])]);
                if let Some(i) = by_id("stf-fund-n").and_then(|i| i.dyn_into::<web_sys::HtmlInputElement>().ok()) {
                    i.set_value("");
                }
                ctx().toast(&format!("급여 금고에 {} 토큰을 넣었어요", fmt_num(amt, 0)));
                render();
            }
            Err(_) => ctx().toast("넣지 못했어요. 잠시 뒤 다시 해 주세요"),
        }
        BUSY.set(false);
    });
}

fn fire(uid: String) {
    let Some(co) = company() else { return };
    let (name, _) = name_photo(&uid);
    let cid = co_str(&co, "id");
    spawn_local(async move {
        let c = ctx();
        if !c.ask(&format!("{name}님을 해고할까요?"), "해고하면 바로 회사를 떠나요. 아직 못 받은 임금은 받을 수 없게 돼요.", "해고하기", "취소").await {
            return;
        }
        if !c.ask("정말 하시겠습니까?", "되돌릴 수 없어요. 자리는 다시 채용 공고로 채워야 해요.", "네, 해고할게요", "아니요").await {
            return;
        }
        if BUSY.replace(true) {
            return;
        }
        match cloud::call("fireStaff", &[JsValue::from_str(&cid), JsValue::from_str(&uid)]).await {
            Ok(_) => {
                LIST.with(|l| l.borrow_mut().retain(|m| m.uid != uid));
                let staff = company().map(|x| num(&x, "staff")).unwrap_or(1.0);
                let _ = c.call("patchCompany", &[dom::obj(&[("staff", JsValue::from((staff - 1.0).max(0.0)))])]);
                super::hire::paint();
                c.toast(&format!("{name}님을 해고했어요"));
                render();
            }
            Err(_) => c.toast("해고하지 못했어요. 잠시 뒤 다시 해 주세요"),
        }
        BUSY.set(false);
    });
}
