//! 공지: 설정의 공지 화면(목록)과, 아직 안 본 공지를 한 번씩 보여주는 전체 화면.
//!
//! 예전 `app/notice.js`와 같은 class·id를 써요. CSS와 화면 시험(tests/e2e/notice.js)을 그대로 쓰기 위해서예요.

use std::any::Any;
use std::cell::{Cell, RefCell};

use cloud::{date_ko, Notice};
use leptos::mount::mount_to;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

#[derive(Clone, PartialEq)]
enum ListState {
    /// 로그인 안 함
    Out,
    Loading,
    Ready(Vec<Notice>),
    Failed,
}

thread_local! {
    /// 공지 화면 목록의 상태 (init에서 만들어요)
    static LIST: Cell<Option<RwSignal<ListState>>> = const { Cell::new(None) };
    /// 전체 화면 공지를 보여주는 중인지
    static SHOWING: Cell<bool> = const { Cell::new(false) };
    /// 지금 떠 있는 전체 화면 공지 (닫을 때 지워요)
    static FULL: RefCell<Option<(web_sys::Element, Box<dyn Any>)>> = const { RefCell::new(None) };
}

/// 설정의 공지 화면(`#notices-root`)에 목록을 붙여요
pub fn init() {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return };
    let Some(root) = doc.get_element_by_id("notices-root") else { return };
    let Ok(root) = root.dyn_into::<web_sys::HtmlElement>() else { return };
    root.set_inner_html("");
    mount_to(root, || {
        let state = RwSignal::new(ListState::Loading);
        let open = RwSignal::new(None::<String>);
        LIST.set(Some(state));
        view! { <NoticeList state open/> }
    })
    .forget();
}

/// 공지 화면을 열 때: 올라온 공지를 모두 읽어요
pub fn refresh() {
    let Some(state) = LIST.get() else { return };
    if !cloud::signed_in() {
        state.set(ListState::Out);
        return;
    }
    state.set(ListState::Loading);
    spawn_local(async move {
        let got = async {
            let ids = cloud::notice_ids().await?;
            cloud::notice_list(&ids).await
        }
        .await;
        state.set(match got {
            Ok(list) => ListState::Ready(list),
            Err(e) => {
                cloud::warn(&format!("공지를 불러오지 못했어요: {e}"));
                ListState::Failed
            }
        });
    });
}

/// 아직 안 본 공지를 하나씩 전체 화면으로 보여줘요 (닫으면 다음 것을 찾아요)
pub fn check() {
    if SHOWING.get() || !cloud::signed_in() {
        return;
    }
    SHOWING.set(true);
    spawn_local(async {
        match cloud::unseen_notice().await {
            Ok(Some(n)) => show_full(n),
            Ok(None) => SHOWING.set(false),
            Err(e) => {
                cloud::warn(&format!("공지를 확인하지 못했어요: {e}"));
                SHOWING.set(false);
            }
        }
    });
}

fn show_full(n: Notice) {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return SHOWING.set(false) };
    let (Ok(el), Some(body)) = (doc.create_element("div"), doc.body()) else { return SHOWING.set(false) };
    el.set_class_name("notice-full");
    let _ = el.set_attribute("role", "dialog");
    let _ = el.set_attribute("aria-modal", "true");
    let _ = el.set_attribute("aria-labelledby", "notice-full-title");
    let _ = body.append_child(&el);
    let handle = mount_to(el.clone().unchecked_into(), move || view! { <FullNotice n/> });
    FULL.with(|f| *f.borrow_mut() = Some((el, Box::new(handle))));
}

/// 전체 화면 공지를 지우고 다음 공지를 찾아요
fn finish() {
    if let Some((el, handle)) = FULL.with(|f| f.borrow_mut().take()) {
        drop(handle);
        el.remove();
    }
    SHOWING.set(false);
    check();
}

#[component]
fn NoticeList(state: RwSignal<ListState>, open: RwSignal<Option<String>>) -> impl IntoView {
    let note = |text: &'static str| {
        view! { <div class="card notice-empty"><p class="card-body">{text}</p></div> }.into_any()
    };
    move || match state.get() {
        ListState::Out => note("로그인하면 공지를 볼 수 있어요"),
        ListState::Loading => view! { <div class="card skeletons" style="--rows: 3" aria-hidden="true"></div> }.into_any(),
        ListState::Failed => note("공지를 불러오지 못했어요. 잠시 뒤 다시 열어 주세요"),
        ListState::Ready(list) if list.is_empty() => note("아직 올라온 공지가 없어요"),
        ListState::Ready(list) => view! {
            <div class="notice-list">
                {list.into_iter().map(|n| view! { <NoticeItem n open/> }).collect_view()}
            </div>
        }
        .into_any(),
    }
}

/// 목록의 공지 한 개. 누르면 펼쳐져요
#[component]
fn NoticeItem(n: Notice, open: RwSignal<Option<String>>) -> impl IntoView {
    let id = n.id.clone();
    let is_open = {
        let id = id.clone();
        Memo::new(move |_| open.with(|o| o.as_deref() == Some(id.as_str())))
    };
    let toggle = {
        let id = id.clone();
        move |_| open.update(|o| *o = if o.as_deref() == Some(id.as_str()) { None } else { Some(id.clone()) })
    };
    let has_poll = n.options.as_ref().is_some_and(|o| !o.is_empty());
    let body = n.body.clone();
    let options = n.options.clone().unwrap_or_default();
    view! {
        <article class="card notice-item" class:is-open=move || is_open.get()>
            <button type="button" class="notice-head" data-id=id aria-expanded=move || if is_open.get() { "true" } else { "false" } on:click=toggle>
                <span class="notice-head-text"><b>{n.title.clone()}</b><span>{date_ko(n.at)}</span></span>
                {has_poll.then(|| view! { <span class="notice-pill">"투표"</span> })}
                <svg class="notice-chev" viewBox="0 0 8 14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <path d="M1 1l6 6-6 6"/>
                </svg>
            </button>
            <Show when=move || is_open.get()>
                <div class="notice-body">
                    <p>{body.clone()}</p>
                    {(!options.is_empty()).then(|| {
                        let opts = options.clone();
                        view! { <ol class="notice-opts">{opts.into_iter().map(|o| view! { <li>{o}</li> }).collect_view()}</ol> }
                    })}
                </div>
            </Show>
        </article>
    }
}

/// 한 번 보여주는 전체 화면 공지. 투표가 있으면 하나를 골라야 "투표하고 닫기"가 켜져요
#[component]
fn FullNotice(n: Notice) -> impl IntoView {
    let pick = RwSignal::new(None::<usize>);
    let busy = RwSignal::new(false);
    let poll: Option<Vec<String>> = n.poll().map(|p| p.to_vec());
    let has_poll = poll.is_some();
    let id = n.id.clone();
    let close = move |vote: bool| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        let id = id.clone();
        let choice = pick.get_untracked();
        spawn_local(async move {
            if vote {
                if let Some(c) = choice {
                    if let Err(e) = cloud::vote_notice(&id, c as u32).await {
                        cloud::warn(&format!("투표하지 못했어요: {e}"));
                    }
                }
            }
            if let Err(e) = cloud::mark_notice_seen(&id).await {
                cloud::warn(&format!("본 공지를 적지 못했어요: {e}"));
            }
            finish();
        });
    };
    let close_skip = close.clone();
    view! {
        <div class="notice-full-card">
            <span class="notice-pill">"공지"</span>
            <h2 id="notice-full-title" class="notice-full-title">{n.title.clone()}</h2>
            <p class="notice-full-date">{date_ko(n.at)}</p>
            <p class="notice-full-text">{n.body.clone()}</p>
            {poll.map(|opts| view! {
                <div class="notice-poll" role="radiogroup" aria-label="투표">
                    <span class="notice-poll-title">"투표 · 하나를 골라 주세요"</span>
                    {opts.into_iter().enumerate().map(|(i, o)| view! {
                        <button
                            type="button"
                            class="notice-opt"
                            class:is-on=move || pick.get() == Some(i)
                            role="radio"
                            aria-checked=move || if pick.get() == Some(i) { "true" } else { "false" }
                            data-i=i.to_string()
                            disabled=move || busy.get()
                            on:click=move |_| pick.set(Some(i))
                        >
                            {o}
                        </button>
                    }).collect_view()}
                </div>
            })}
            <div class="notice-full-foot">
                <button
                    type="button"
                    class="btn primary notice-ok"
                    disabled=move || busy.get() || (has_poll && pick.get().is_none())
                    on:click=move |_| close(true)
                >
                    {if has_poll { "투표하고 닫기" } else { "확인했어요" }}
                </button>
                {has_poll.then(|| view! {
                    <button type="button" class="notice-skip" disabled=move || busy.get() on:click=move |_| close_skip(false)>
                        "투표 안 하고 닫기"
                    </button>
                })}
                <p class="notice-hint">"지난 공지는 설정의 공지에서 다시 볼 수 있어요"</p>
            </div>
        </div>
    }
}
