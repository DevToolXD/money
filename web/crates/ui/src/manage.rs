//! 관리 (오너만): 토큰 지급, 공지 올리기(투표 포함), 계정 데이터 지우기.
//! 행동마다 다리(firebase.src.js)가 인증토큰 3개를 만들어 서버 규칙이 확인해요. 예전 `app/manage.js`와 같은 id·class예요.

use std::cell::{Cell, RefCell};
use std::time::Duration;

use cloud::{Notice, ProfileHit};
use leptos::html;
use leptos::mount::mount_to;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

use crate::ctx::Ctx;

#[derive(Clone)]
struct Row {
    n: Notice,
    tally: Option<Vec<u32>>,
}

thread_local! {
    static CTX: RefCell<Ctx> = RefCell::new(Ctx::default());
    static ROWS: Cell<Option<RwSignal<Vec<Row>, LocalStorage>>> = const { Cell::new(None) };
}

fn ctx() -> Ctx {
    CTX.with(|c| c.borrow().clone())
}

fn msg_of(e: &cloud::CloudError) -> String {
    match e.code.as_str() {
        "nouser" => "그런 아이디가 없어요".into(),
        "permission-denied" => "확인이 맞지 않거나 시간이 지났어요. 다시 해 주세요".into(),
        _ if e.message.is_empty() => "처리하지 못했어요. 잠시 뒤 다시 해 주세요".into(),
        _ => e.message.clone(),
    }
}

pub fn init(c: Ctx) {
    CTX.with(|x| *x.borrow_mut() = c);
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return };
    let Some(root) = doc.get_element_by_id("manage-root") else { return };
    let Ok(root) = root.dyn_into::<web_sys::HtmlElement>() else { return };
    root.set_inner_html("");
    mount_to(root, || {
        let rows = RwSignal::new_local(Vec::<Row>::new());
        ROWS.set(Some(rows));
        view! { <Manage rows/> }
    })
    .forget();
}

/// 관리 화면을 열 때: 올린 공지 10개와 투표 집계를 읽어요
pub fn refresh() {
    let Some(rows) = ROWS.get() else { return };
    if !cloud::signed_in() {
        return;
    }
    spawn_local(async move {
        let got = async {
            let ids = cloud::notice_ids().await?;
            let ids: Vec<String> = ids.into_iter().take(10).collect();
            cloud::notice_list(&ids).await
        }
        .await;
        match got {
            Ok(list) => {
                let mut out = Vec::new();
                for n in list {
                    let tally = if n.options.is_some() { cloud::notice_tally(&n).await.ok() } else { None };
                    out.push(Row { n, tally });
                }
                rows.set(out);
            }
            Err(e) => {
                cloud::warn(&format!("공지를 불러오지 못했어요: {e}"));
                rows.set(Vec::new());
            }
        }
    });
}

fn tally_text(r: &Row) -> String {
    match (&r.n.options, &r.tally) {
        (Some(opts), Some(t)) => opts.iter().enumerate().map(|(i, o)| format!("{o} {}명", t.get(i).copied().unwrap_or(0))).collect::<Vec<_>>().join(" · "),
        _ => String::new(),
    }
}

/// 아이디·이름 입력칸 + 자동 검색 목록
#[component]
fn PersonField(id: &'static str, sug_id: &'static str, placeholder: &'static str, input: NodeRef<html::Input>) -> impl IntoView {
    let hits = RwSignal::new(Vec::<ProfileHit>::new());
    let seq = StoredValue::new(0u32);
    let on_input = move |_| {
        let my = seq.get_value() + 1;
        seq.set_value(my);
        set_timeout(
            move || {
                if seq.get_value() != my {
                    return;
                }
                let text = input.get_untracked().map(|i| i.value()).unwrap_or_default();
                if text.trim().is_empty() {
                    hits.set(Vec::new());
                    return;
                }
                spawn_local(async move {
                    match cloud::search_profiles(&text).await {
                        Ok(list) if seq.get_value() == my => hits.set(list),
                        Ok(_) => {}
                        Err(e) => cloud::warn(&format!("검색하지 못했어요: {e}")),
                    }
                });
            },
            Duration::from_millis(250),
        );
    };
    view! {
        <label class="lab-field">
            <span>"아이디 또는 이름"</span>
            <input node_ref=input id=id class="lab-input" type="text" autocomplete="off" autocapitalize="none" maxlength="20" placeholder=placeholder on:input=on_input/>
        </label>
        <div class="mg-sug" id=sug_id hidden=move || hits.with(|h| h.is_empty())>
            <For each=move || hits.get() key=|p| p.uid.clone() let:p>
                {
                    let pid = p.id.clone();
                    view! {
                        <button type="button" class="mg-sug-row" data-for=id data-id=p.id.clone() on:click=move |_| {
                            if let Some(i) = input.get_untracked() {
                                i.set_value(&pid);
                            }
                            hits.set(Vec::new());
                        }>
                            <b>{p.name.clone()}</b>
                            <span>{p.id.clone()}</span>
                        </button>
                    }
                }
            </For>
        </div>
    }
}

#[component]
fn Manage(rows: RwSignal<Vec<Row>, LocalStorage>) -> impl IntoView {
    let busy = RwSignal::new(false);
    let grant_id = NodeRef::<html::Input>::new();
    let grant_amt = NodeRef::<html::Input>::new();
    let n_title = NodeRef::<html::Input>::new();
    let n_text = NodeRef::<html::Textarea>::new();
    let n_opts = NodeRef::<html::Textarea>::new();
    let del_id = NodeRef::<html::Input>::new();

    // 행동 하나: 누르는 동안 막고, 끝나면 알려주고 목록을 다시 읽어요
    let act = move |job: std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), cloud::CloudError>>>>, ok: String, clear: Box<dyn Fn()>| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        spawn_local(async move {
            match job.await {
                Ok(()) => {
                    ctx().toast(&ok);
                    clear();
                    refresh();
                }
                Err(e) => ctx().toast(&msg_of(&e)),
            }
            busy.set(false);
        });
    };

    let grant = move |_| {
        let id = grant_id.get_untracked().map(|i| i.value()).unwrap_or_default().trim().to_string();
        let digits: String = grant_amt.get_untracked().map(|i| i.value()).unwrap_or_default().chars().filter(|c| c.is_ascii_digit()).collect();
        let amt: u32 = digits.parse().unwrap_or(0);
        if id.is_empty() {
            return ctx().toast("받을 사람 아이디를 적어 주세요");
        }
        if !(1..=1_000_000).contains(&amt) {
            return ctx().toast("토큰은 1부터 100만까지 적어 주세요");
        }
        let id2 = id.clone();
        act(
            Box::pin(async move { cloud::admin_grant(&id2, amt).await }),
            format!("{id} 님에게 {amt} 토큰을 지급했어요"),
            Box::new(move || {
                for r in [grant_id, grant_amt] {
                    if let Some(i) = r.get_untracked() {
                        i.set_value("");
                    }
                }
            }),
        );
    };
    let post = move |_| {
        let title = n_title.get_untracked().map(|i| i.value()).unwrap_or_default().trim().to_string();
        let body = n_text.get_untracked().map(|i| i.value()).unwrap_or_default().trim().to_string();
        let options: Vec<String> = n_opts.get_untracked().map(|i| i.value()).unwrap_or_default().lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        if title.is_empty() {
            return ctx().toast("공지 제목을 적어 주세요");
        }
        if body.is_empty() {
            return ctx().toast("공지 내용을 적어 주세요");
        }
        if options.len() == 1 {
            return ctx().toast("투표 항목은 2개 이상 적어 주세요");
        }
        act(
            Box::pin(async move { cloud::admin_notice(&title, &body, &options).await }),
            "공지를 올렸어요".into(),
            Box::new(move || {
                if let Some(i) = n_title.get_untracked() {
                    i.set_value("");
                }
                for r in [n_text, n_opts] {
                    if let Some(i) = r.get_untracked() {
                        i.set_value("");
                    }
                }
            }),
        );
    };
    let delete = move |_| {
        let id = del_id.get_untracked().map(|i| i.value()).unwrap_or_default().trim().to_string();
        if id.is_empty() {
            return ctx().toast("지울 사람 아이디를 적어 주세요");
        }
        let sure = web_sys::window()
            .and_then(|w| w.confirm_with_message(&format!("{id} 님의 프로필, 지갑, 설정, 상태를 지울까요? 되돌릴 수 없어요.")).ok())
            .unwrap_or(false);
        if !sure {
            return;
        }
        let id2 = id.clone();
        act(
            Box::pin(async move { cloud::admin_delete(&id2).await }),
            format!("{id} 님의 데이터를 지웠어요"),
            Box::new(move || {
                if let Some(i) = del_id.get_untracked() {
                    i.set_value("");
                }
            }),
        );
    };

    view! {
        <h2 class="section-title">"토큰 지급"</h2>
        <div class="card mg-card">
            <PersonField id="mg-grant-id" sug_id="mg-grant-sug" placeholder="받을 사람 아이디나 이름" input=grant_id/>
            <label class="lab-field"><span>"토큰"</span><input node_ref=grant_amt id="mg-grant-amt" class="lab-input" type="text" inputmode="numeric" maxlength="7" placeholder="예: 100"/></label>
            <button type="button" class="btn primary" id="mg-grant" class:busy=move || busy.get() on:click=grant>"지급하기"</button>
        </div>
        <h2 class="section-title">"공지"</h2>
        <div class="card mg-card mg-compose">
            <label class="lab-field"><span>"제목 (40자까지)"</span><input node_ref=n_title id="mg-notice-title" class="lab-input mg-center" type="text" maxlength="40" placeholder="공지 제목"/></label>
            <label class="lab-field"><span>"내용 (2000자까지)"</span><textarea node_ref=n_text id="mg-notice-text" class="lab-input mg-text mg-center" maxlength="2000" placeholder="모두에게 보일 공지"></textarea></label>
            <label class="lab-field"><span>"투표 항목 (선택, 한 줄에 하나씩 2~6개)"</span><textarea node_ref=n_opts id="mg-notice-opts" class="lab-input mg-opts mg-center" maxlength="300" placeholder="비워 두면 투표가 없어요"></textarea></label>
            <button type="button" class="btn primary" id="mg-notice" class:busy=move || busy.get() on:click=post>"공지 올리기"</button>
        </div>
        <h2 class="section-title">"올린 공지"</h2>
        <div class="card mg-card" id="mg-notices">
            {move || {
                let list = rows.get();
                if list.is_empty() {
                    view! { <p class="card-body">"아직 공지가 없어요"</p> }.into_any()
                } else {
                    list.into_iter()
                        .map(|r| {
                            let poll = r.n.options.is_some();
                            let tally = tally_text(&r);
                            view! {
                                <div class="mg-notice">
                                    <b>{r.n.title.clone()}</b>
                                    <span>{format!("{}{}", cloud::when_ko(r.n.at), if poll { " · 투표" } else { "" })}</span>
                                    {poll.then(|| view! { <span>{tally}</span> })}
                                </div>
                            }
                        })
                        .collect_view()
                        .into_any()
                }
            }}
        </div>
        <h2 class="section-title">"계정 데이터 지우기"</h2>
        <div class="card mg-card">
            <p class="card-body">"그 사람의 프로필, 지갑, 설정, 상태를 지워요. 로그인 계정 자체는 웹에서 지울 수 없어요."</p>
            <PersonField id="mg-del-id" sug_id="mg-del-sug" placeholder="지울 사람 아이디나 이름" input=del_id/>
            <button type="button" class="btn mg-danger" id="mg-del" class:busy=move || busy.get() on:click=delete>"지우기"</button>
        </div>
    }
}
