//! 관리 화면의 베타 종료(모든 데이터 한 번 지우기) 버튼. 설정 탭을 없애면서 여기로 옮겼어요.

use wasm_bindgen_futures::spawn_local;

use crate::dom::{self, by_id};

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
            if !c.ask("베타를 종료할까요?", "모든 사람의 계정 데이터, 멸공, 회사, 대화, 게시물이 지워져요. 되돌릴 수 없어요.", "계속", "취소").await {
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
