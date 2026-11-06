//! 설정 탭: 화면 모드(자동·밝게·어둡게), 새 메시지 알림, 진동, 움직임 줄이기, 튜토리얼 다시 보기, 이 기기에 저장된 정보 지우기, 로그아웃.

use wasm_bindgen_futures::spawn_local;

use crate::dom::{self, by_id};

fn paint() {
    let theme = dom::store_get("theme", "auto");
    for b in dom::qsa(&dom::doc().document_element().expect("html"), "#set-theme [data-theme-pick]") {
        let on = dom::data(&b, "theme-pick") == theme;
        let _ = b.set_attribute("aria-pressed", &on.to_string());
        let _ = b.set_attribute("aria-checked", &on.to_string());
    }
    if let Some(t) = by_id("toggle-vibrate") {
        let _ = t.set_attribute("aria-checked", &(dom::store_get("vibrate", "on") != "off").to_string());
    }
    if let Some(t) = by_id("toggle-motion") {
        let _ = t.set_attribute("aria-checked", &(dom::store_get("motion", "") == "less").to_string());
    }
    if let Some(r) = dom::doc().document_element() {
        let _ = if dom::store_get("motion", "") == "less" { r.set_attribute("data-reduce", "") } else { r.remove_attribute("data-reduce") };
    }
}

pub fn init() {
    paint();
    beta_init();
    if let Some(g) = by_id("set-theme") {
        dom::on(&g, "click", |e| {
            if let Some(b) = dom::closest(&e, "[data-theme-pick]") {
                crate::shell::set_theme(&dom::data(&b, "theme-pick"));
                paint();
            }
        });
    }
    if let Some(t) = by_id("toggle-vibrate") {
        dom::on(&t, "click", |_| {
            let on = dom::store_get("vibrate", "on") == "off";
            dom::store_set("vibrate", if on { "on" } else { "off" });
            paint();
            dom::vibrate(10);
        });
    }
    if let Some(t) = by_id("toggle-motion") {
        dom::on(&t, "click", |_| {
            let less = dom::store_get("motion", "") != "less";
            dom::store_set("motion", if less { "less" } else { "" });
            paint();
        });
    }
    if let Some(b) = by_id("set-tutorial") {
        dom::on(&b, "click", |_| crate::coach::replay());
    }
    if let Some(b) = by_id("set-logout") {
        dom::on(&b, "click", |_| {
            if let Some(l) = by_id("logout") {
                l.click();
            }
        });
    }
    if let Some(b) = by_id("set-clear") {
        dom::on(&b, "click", |_| {
            spawn_local(async {
                if !crate::chat::ctx().ask("이 기기에 저장된 정보를 지울까요?", "화면 설정, 고른 회사, 튜토리얼 기록 같은 이 기기의 기억을 지우고 다시 열어요. 계정과 토큰은 그대로예요.", "지우기", "취소").await {
                    return;
                }
                // 지우면 안 되는 것: 계정을 만든 기기 표시(중복 가입 막기), 로그인 시도 제한
                let keep = ["acct-made", "login-lock"];
                if let Some(st) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
                    let n = st.length().unwrap_or(0);
                    let keys: Vec<String> = (0..n).filter_map(|i| st.key(i).ok().flatten()).collect();
                    for k in keys {
                        if !keep.iter().any(|x| k.ends_with(x)) && !k.starts_with("firebase") {
                            let _ = st.remove_item(&k);
                        }
                    }
                }
                let _ = web_sys::window().map(|w| w.location().reload());
            });
        });
    }
}

/// 관리 탭: 베타 종료 (관리자, 한 번만). 두 번 묻고, 확인 글자까지 맞아야 해요
pub fn beta_init() {
    let Some(b) = by_id("beta-end") else { return };
    dom::on(&b, "click", |_| {
        spawn_local(async {
            let c = crate::chat::ctx();
            if let Ok(v) = cloud::call("betaState", &[]).await {
                if v.as_string().as_deref() == Some("done") {
                    return c.toast("베타 종료는 이미 했어요");
                }
            }
            if !c.ask("베타를 종료할까요?", "모든 사람의 계정 데이터, 토큰, 회사, 대화, 게시물이 지워져요. 되돌릴 수 없어요.", "계속", "취소").await {
                return;
            }
            if !c.ask("정말 모두 지울까요?", "딱 한 번만 할 수 있어요. 관리자 데이터도 지워지고, 끝나면 로그아웃돼요.", "모두 지우기", "그만두기").await {
                return;
            }
            let Some(btn) = by_id("beta-end") else { return };
            let _ = btn.set_attribute("disabled", "");
            btn.set_text_content(Some("지우는 중이에요… 창을 닫지 마세요"));
            match cloud::call("betaEnd", &[]).await {
                Ok(n) => {
                    c.toast(&format!("베타를 종료했어요. 문서 {}개를 지웠어요", n.as_f64().unwrap_or(0.0) as i64));
                    btn.set_text_content(Some("베타 종료를 마쳤어요"));
                    dom::sleep(1500).await;
                    if let Some(l) = by_id("logout") {
                        l.click();
                    }
                }
                Err(e) => {
                    let _ = btn.remove_attribute("disabled");
                    btn.set_text_content(Some("다시 시도하기"));
                    c.toast(&format!("다 지우지 못했어요. {} 다시 누르면 이어서 지워요", cloud::why(&e.code)));
                }
            }
        });
    });
}

/// 설정 탭을 열 때
pub fn open() {
    paint();
}
