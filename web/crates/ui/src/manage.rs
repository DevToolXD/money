//! 관리 (오너만): 토큰 지급, 공지 올리기(투표 포함), 계정 데이터 지우기.
//! 행동마다 다리(firebase.src.js)가 인증토큰 3개를 만들어 서버 규칙이 확인해요. 예전 `app/manage.js`와 같은 id·class예요.

use std::cell::{Cell, RefCell};
use std::time::Duration;

use cloud::Notice;
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
    static PEOPLE: Cell<Option<RwSignal<Vec<Person>>>> = const { Cell::new(None) };
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
    // 사람 목록도 다시 읽어요 (찾기 칸이 비어 있을 때)
    if let Some(people) = PEOPLE.get() {
        let q = crate::dom::input_value("mg-search");
        spawn_local(async move { people.set(load_people(q).await) });
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

#[derive(Clone, PartialEq)]
struct Person {
    uid: String,
    id: String,
    name: String,
    photo: String,
    rank: Option<usize>,
    bal: Option<f64>,
}
fn js_s(v: &wasm_bindgen::JsValue, k: &str) -> String {
    cloud::get(v, k).as_string().unwrap_or_default()
}
/// 사람 목록 읽기: 처음에는 이름순 20명, 찾을 때는 이름·아이디 앞부분으로. 순위표에 있으면 순위와 토큰도 보여줘요
async fn load_people(text: String) -> Vec<Person> {
    let ranks = cloud::call("loadRanks", &[false.into()]).await.ok().map(|v| js_sys::Array::from(&v).to_vec()).unwrap_or_default();
    let rank_of = |uid: &str| ranks.iter().position(|r| js_s(r, "uid") == uid).map(|i| (i + 1, cloud::get(&ranks[i], "bal").as_f64().unwrap_or(0.0)));
    let raw: Vec<wasm_bindgen::JsValue> = if text.trim().is_empty() {
        cloud::call("listUsers", &[true.into()]).await.ok().map(|v| js_sys::Array::from(&cloud::get(&v, "list")).to_vec()).unwrap_or_default()
    } else {
        let hits = cloud::call("searchProfiles", &[text.into()]).await.ok().map(|v| js_sys::Array::from(&v).to_vec()).unwrap_or_default();
        // 사진은 사람 정보 캐시에서 읽어요 (없으면 한 번에 모아 읽어요)
        let uids: js_sys::Array = hits.iter().map(|h| cloud::get(h, "uid")).collect();
        let _ = cloud::call("fetchProfiles", &[uids.into()]).await;
        hits.into_iter()
            .map(|h| {
                let p = cloud::call_now("peek", &[cloud::get(&h, "uid")]).unwrap_or(wasm_bindgen::JsValue::NULL);
                let _ = js_sys::Reflect::set(&h, &"photo".into(), &cloud::get(&p, "photo"));
                h
            })
            .collect()
    };
    raw.iter()
        .map(|v| {
            let uid = js_s(v, "uid");
            let r = rank_of(&uid);
            Person { id: js_s(v, "id"), name: { let n = js_s(v, "name"); if n.is_empty() { js_s(v, "id") } else { n } }, photo: js_s(v, "photo"), rank: r.map(|x| x.0), bal: r.map(|x| x.1), uid }
        })
        .collect()
}

#[component]
fn Manage(rows: RwSignal<Vec<Row>, LocalStorage>) -> impl IntoView {
    let busy = RwSignal::new(false);
    let sel = RwSignal::new(None::<Person>);
    let people = RwSignal::new(Vec::<Person>::new());
    PEOPLE.set(Some(people));
    let seq = StoredValue::new(0u32);
    let find = move |text: String| {
        let my = seq.get_value() + 1;
        seq.set_value(my);
        set_timeout(
            move || {
                if seq.get_value() != my {
                    return;
                }
                spawn_local(async move {
                    let list = load_people(text).await;
                    if seq.get_value() == my {
                        people.set(list);
                    }
                });
            },
            Duration::from_millis(250),
        );
    };
    find(String::new());
    let grant_amt = NodeRef::<html::Input>::new();
    let n_title = NodeRef::<html::Input>::new();
    let n_text = NodeRef::<html::Textarea>::new();
    let n_opts = NodeRef::<html::Textarea>::new();

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
        let id = sel.get_untracked().map(|p| p.id).unwrap_or_default();
        let digits: String = grant_amt.get_untracked().map(|i| i.value()).unwrap_or_default().chars().filter(|c| c.is_ascii_digit()).collect();
        let amt: u32 = digits.parse().unwrap_or(0);
        if id.is_empty() {
            return ctx().toast("목록에서 사람을 골라 주세요");
        }
        if !(1..=1_000_000).contains(&amt) {
            return ctx().toast("토큰은 1부터 100만까지 적어 주세요");
        }
        let id2 = id.clone();
        act(
            Box::pin(async move { cloud::admin_grant(&id2, amt).await }),
            format!("{id} 님에게 {amt} 토큰을 지급했어요"),
            Box::new(move || {
                if let Some(i) = grant_amt.get_untracked() {
                    i.set_value("");
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
        let Some(p) = sel.get_untracked() else { return ctx().toast("목록에서 사람을 골라 주세요") };
        spawn_local(async move {
            if !ctx().ask(&format!("{} 님의 데이터를 지울까요?", p.name), "프로필, 지갑, 설정, 상태를 지워요. 되돌릴 수 없어요.", "지우기", "취소").await {
                return;
            }
            let id2 = p.id.clone();
            act(
                Box::pin(async move { cloud::admin_delete(&id2).await }),
                format!("{} 님의 데이터를 지웠어요", p.name),
                Box::new(move || {
                    sel.set(None);
                    people.update(|l| l.retain(|x| x.uid != p.uid));
                }),
            );
        });
    };

    view! {
        <h2 class="section-title">"사람 관리"</h2>
        <div class="card mg-card mg-pcard">
        <label class="mg-find">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" aria-hidden="true"><circle cx="11" cy="11" r="6.5"/><path d="M16 16l4 4"/></svg>
            <input id="mg-search" type="search" autocomplete="off" autocapitalize="none" maxlength="20" placeholder="이름으로 찾기" on:input=move |e| find(event_target_value(&e))/>
        </label>
        <ul class="mg-people" id="mg-people">
            {move || people.get().into_iter().map(|p| {
                let on = sel.with(|s| s.as_ref().map(|x| x.uid == p.uid).unwrap_or(false));
                let sub = match (p.rank, p.bal) {
                    (Some(r), Some(b)) => format!("{r}위 · {} · {}", crate::money::w(b), p.id),
                    _ => p.id.clone(),
                };
                let av = crate::dom::avatar_html(&p.name, &p.photo, "avatar contact mg-av");
                let p2 = p.clone();
                view! {
                    <li>
                        <button type="button" class="mg-person" class:on=on data-uid=p.uid.clone() on:click=move |_| {
                            let same = sel.with_untracked(|s| s.as_ref().map(|x| x.uid == p2.uid).unwrap_or(false));
                            sel.set(if same { None } else { Some(p2.clone()) });
                        }>
                            <span style="display:contents" inner_html=av></span>
                            <span class="mg-pm"><b>{p.name.clone()}</b><span>{sub}</span></span>
                        </button>
                        {on.then(|| view! {
                            <div class="mg-act">
                                <label class="lab-field"><span>"줄 토큰"</span><input node_ref=grant_amt id="mg-grant-amt" class="lab-input" type="text" inputmode="numeric" maxlength="7" placeholder="예: 100"/></label>
                                <div class="mg-act-row">
                                    <button type="button" class="btn primary" id="mg-grant" class:busy=move || busy.get() on:click=grant>"토큰 주기"</button>
                                    <button type="button" class="btn mg-danger" id="mg-del" class:busy=move || busy.get() on:click=delete>"데이터 지우기"</button>
                                </div>
                            </div>
                        })}
                    </li>
                }
            }).collect_view()}
        </ul>
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
    }
}
