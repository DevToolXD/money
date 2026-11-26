//! 추천 알고리즘 (SNS 피드와 릴스): 서버를 더 읽지 않고, 이미 듣고 있는 최근 40개 게시물의 순서만 사람마다 다르게 정해요.
//!
//! 점수 = 최신성 × (1 + 반응) × 친밀도 × 새로움 × (사진이면 릴스에서 조금 더)
//!   - 최신성: 올린 지 h시간이면 1/(h+2)^1.2 (오래될수록 천천히 내려가요)
//!   - 반응: 좋아요 + 댓글×2 + 공유×3 을 로그로 (몇 개가 몰려도 하나가 다 차지하지 않게)
//!   - 친밀도: 내가 좋아요·댓글·선물을 한 사람, 6초 넘게 본 릴스의 사람, 1:1 대화 상대는 점점 위로
//!   - 새로움: 릴스에서 이미 본 것은 뒤로, 내 글은 조금 뒤로
//! 같은 사람의 글이 줄줄이 붙지 않게, 바로 앞과 같은 사람이면 한 번 양보해요.
//! 순서는 탭을 열 때 한 번 정하고(보는 중에 섞이지 않아요), 그 뒤에 올라온 새 글은 맨 위에 붙어요.
//! 친밀도와 본 기록은 이 기기(localStorage)에만 남아요.

use std::collections::HashMap;

use crate::dom;

/// 추천에 쓰는 게시물 정보
pub struct P<'a> {
    pub id: &'a str,
    pub by: &'a str,
    pub at: f64,
    pub likes: f64,
    pub rc: f64,
    pub sh: f64,
    pub img: bool,
    pub liked: bool,
}

const AFF: &str = "algo-aff";
const SEEN: &str = "algo-seen";

fn load_map(k: &str) -> HashMap<String, f64> {
    let raw = dom::store_get(k, "");
    let Ok(v) = js_sys::JSON::parse(if raw.is_empty() { "{}" } else { &raw }) else { return HashMap::new() };
    let Ok(o) = wasm_bindgen::JsCast::dyn_into::<js_sys::Object>(v) else { return HashMap::new() };
    js_sys::Object::entries(&o).iter().filter_map(|e| {
        let a = js_sys::Array::from(&e);
        Some((a.get(0).as_string()?, a.get(1).as_f64()?))
    }).collect()
}
fn save_map(k: &str, m: &HashMap<String, f64>) {
    let o = js_sys::Object::new();
    for (key, v) in m {
        let _ = js_sys::Reflect::set(&o, &key.into(), &(*v).into());
    }
    if let Ok(s) = js_sys::JSON::stringify(&o) {
        dom::store_set(k, &String::from(s));
    }
}

/// 이 사람에게 관심을 보였어요 (좋아요 1, 댓글 2, 선물 3, 릴스를 6초 넘게 봄 0.5). 오래된 관심은 조금씩 줄어요
pub fn bump(author: &str, w: f64) {
    if author.is_empty() || author == my_uid() {
        return;
    }
    let mut m = load_map(AFF);
    for v in m.values_mut() {
        *v *= 0.98;
    }
    *m.entry(author.to_string()).or_insert(0.0) += w;
    // 너무 많아지면 약한 것부터 버려요
    if m.len() > 200 {
        let mut v: Vec<(String, f64)> = m.into_iter().collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        v.truncate(150);
        m = v.into_iter().collect();
    }
    save_map(AFF, &m);
}
/// 릴스에서 이 게시물을 봤어요 (다음에 열면 뒤로 가요)
pub fn seen(id: &str) {
    let mut m = load_map(SEEN);
    if m.contains_key(id) {
        return;
    }
    m.insert(id.to_string(), js_sys::Date::now());
    if m.len() > 300 {
        let mut v: Vec<(String, f64)> = m.into_iter().collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        v.truncate(250);
        m = v.into_iter().collect();
    }
    save_map(SEEN, &m);
}

fn my_uid() -> String {
    cloud::cloud().and_then(|c| cloud::get(&c, "uid").as_string()).unwrap_or_default()
}

/// 점수 순서대로 게시물 번호를 돌려줘요
pub fn rank(posts: &[P], reels: bool) -> Vec<String> {
    let now = js_sys::Date::now();
    let me = my_uid();
    let aff = load_map(AFF);
    let seen = if reels { load_map(SEEN) } else { HashMap::new() };
    let friends = crate::chat::partners();
    let mut scored: Vec<(f64, &P)> = posts
        .iter()
        .map(|p| {
            let h = ((now - p.at) / 3_600_000.0).max(0.0);
            let fresh = 1.0 / (h + 2.0).powf(1.2);
            let react = 1.0 + (1.0 + p.likes + p.rc * 2.0 + p.sh * 3.0).ln() * 0.6;
            let mut close = 1.0 + aff.get(p.by).copied().unwrap_or(0.0).min(20.0).sqrt() * 0.35;
            if friends.iter().any(|f| f == p.by) {
                close *= 1.4;
            }
            let mut novel = 1.0;
            if p.by == me {
                novel *= 0.7;
            }
            if reels && seen.contains_key(p.id) {
                novel *= 0.3;
            }
            if reels && p.liked {
                novel *= 0.8;
            }
            let media = if reels && p.img { 1.25 } else { 1.0 };
            (fresh * react * close * novel * media, p)
        })
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    // 같은 사람 글이 연달아 나오지 않게: 바로 앞과 같은 사람이면 다음 것을 먼저 꺼내요
    let mut out: Vec<String> = Vec::with_capacity(scored.len());
    let mut last = String::new();
    while !scored.is_empty() {
        let i = scored.iter().position(|(_, p)| p.by != last).unwrap_or(0);
        let (_, p) = scored.remove(i);
        last = p.by.to_string();
        out.push(p.id.to_string());
    }
    out
}

/// 정해 둔 순서에 맞춰 줄 세워요. 순서에 없는 새 글은 맨 위(최신순)에 붙어요
pub fn apply<T>(items: &mut [T], order: &[String], id: impl Fn(&T) -> &str) {
    let pos: HashMap<&str, usize> = order.iter().enumerate().map(|(i, s)| (s.as_str(), i)).collect();
    // 새 글은 -1 자리 (안정 정렬이라 원래의 최신순을 지켜요)
    items.sort_by_key(|t| pos.get(id(t)).map(|i| *i as i64).unwrap_or(-1));
}
