//! 여러 화면이 같이 쓰는 작은 부품

use leptos::prelude::*;

/// 안내 카드 (위 작은 글씨, 큰 제목, 설명). 예전 JS의 noteHTML과 같은 모양이에요
pub fn note(eyebrow: &'static str, title: &'static str, body: &'static str) -> AnyView {
    view! {
        <div class="card shop-note">
            <div class="art-text">
                <p class="eyebrow">{eyebrow}</p>
                <h2 class="card-title">{title}</h2>
                <p class="card-body">{body}</p>
            </div>
        </div>
    }
    .into_any()
}

/// 안내 카드 + "다시 해 보기" 버튼
pub fn note_retry(eyebrow: &'static str, title: &'static str, body: &'static str, retry_class: &'static str, on_retry: impl Fn() + 'static) -> AnyView {
    view! {
        <div class="card shop-note">
            <div class="art-text">
                <p class="eyebrow">{eyebrow}</p>
                <h2 class="card-title">{title}</h2>
                <p class="card-body">{body}</p>
                <button type="button" class=format!("btn {retry_class}") on:click=move |_| on_retry()>"다시 해 보기"</button>
            </div>
        </div>
    }
    .into_any()
}

/// 불러오는 중 (회색 줄)
pub fn skeleton(rows: u32) -> AnyView {
    view! { <div class="card skeletons" style=format!("--rows: {rows}") aria-hidden="true"></div> }.into_any()
}
