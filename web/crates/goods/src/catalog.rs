//! 팔 상품 목록, 원가, 이름, 그림 (예전 app/biz.js 의 앞부분).
//! 식품은 물·음식·에너지 식품 가운데 고르고, 모양·뚜껑·내용물·색을 차례로 정해요.
//! 원가는 센트 단위 정수로 계산해요 (50 = 0.5 토큰). 1 토큰 = 1 달러.

use crate::shapes::{self, f, num, Body};

/// 아이콘은 모두 검은 윤곽선이에요 (선 굵기 1.8, 둥근 끝)
pub fn icon(name: &str) -> &'static str {
    match name {
        "food" => r#"<path d="M2.4 12.6H21.6"/><path d="M2.4 12.6C2.4 18.2 6.8 22 12 22S21.6 18.2 21.6 12.6"/><g transform="translate(8.6 12.6) rotate(-30)"><path d="M0 0C-3 -1.6 -3 -5.04 0 -7.2C3 -5.04 3 -1.6 0 0Z"/><path d="M0 -1V-4.46"/></g><g transform="translate(12.2 12.6) rotate(0)"><path d="M0 0C-3 -1.6 -3 -5.88 0 -8.4C3 -5.88 3 -1.6 0 0Z"/><path d="M0 -1V-5.21"/></g><g transform="translate(15.8 12.6) rotate(30)"><path d="M0 0C-3 -1.6 -3 -5.04 0 -7.2C3 -5.04 3 -1.6 0 0Z"/><path d="M0 -1V-4.46"/></g><path d="M9.4 17.6a2.7 2.7 0 1 0 0.01 0Z"/><path d="M9.4 14.9V13.6"/>"#,
        "pc" => r#"<rect x="3" y="4.5" width="18" height="12.5" rx="2.5"/><path d="M9 20.5h6M12 17v3.5"/>"#,
        "ai" => r#"<path d="M10.2 3.2L14.7 2.4L17.6 4.1L19 8.6L18.4 14.5L17.2 16.6L17.2 21.3H9.3V16.1L6.9 15.7L5.6 12L4.8 9.7L5.8 7.4L6.9 4.5Z"/><path d="M13.00 8.10Q14.73 6.54 13.60 4.74M13.94 8.41Q16.25 8.16 16.40 6.04M14.52 9.21Q16.54 10.37 17.91 8.74M14.52 10.19Q15.47 12.32 17.54 11.80M13.94 10.99Q13.46 13.27 15.43 14.07M13.00 11.30Q11.27 12.86 12.40 14.66M12.06 10.99Q9.75 11.24 9.60 13.36M11.48 10.19Q9.46 9.03 8.09 10.66M11.48 9.21Q10.53 7.08 8.46 7.60M12.06 8.41Q12.54 6.13 10.57 5.33"/>"#,
        "drop" => r#"<path d="M12 3.5C9 8 6.5 10.8 6.5 14.5a5.5 5.5 0 0 0 11 0C17.5 10.8 15 8 12 3.5z"/>"#,
        "fork" => r#"<path d="M7 3v6a2.5 2.5 0 0 0 5 0V3M9.5 11.5V21M17 21V3c-2 1.5-3 4-3 7h3"/>"#,
        "bolt" => r#"<path d="M13 3L6 13.5h5L10 21l7-10.5h-5z"/>"#,
        "cookie" => r#"<circle cx="12" cy="12" r="8.5"/><circle cx="9" cy="9" r="1"/><circle cx="14.5" cy="13.5" r="1"/><circle cx="9.5" cy="15" r="1"/>"#,
        "plate" => r#"<circle cx="12" cy="12" r="8.5"/><circle cx="12" cy="12" r="4.5"/>"#,
        "bag" => r#"<path d="M5 8h14l-1 12H6zM9 8V6.5a3 3 0 0 1 6 0V8"/>"#,
        "box" => r#"<rect x="4" y="7" width="16" height="12" rx="2"/><path d="M4 11h16"/>"#,
        "wrap" => r#"<rect x="4" y="6.5" width="16" height="11" rx="5.5"/><path d="M10 6.5v11M14 6.5v11"/>"#,
        "can" => r#"<rect x="7" y="3.5" width="10" height="17" rx="3"/><path d="M7 7.5h10"/>"#,
        "cup" => r#"<path d="M5 9h11v5a5 5 0 0 1-5 5h-1a5 5 0 0 1-5-5zM16 10h1.5a2.5 2.5 0 0 1 0 5H16"/>"#,
        "dot" => r#"<circle cx="12" cy="12" r="3.5"/>"#,
        "dots" => r#"<circle cx="5.5" cy="12" r="1.6"/><circle cx="12" cy="12" r="1.6"/><circle cx="18.5" cy="12" r="1.6"/>"#,
        "chev" => r#"<path d="M9 5l7 7-7 7"/>"#,
        "back" => r#"<path d="M15 5l-7 7 7 7"/>"#,
        "check" => r#"<path d="M5 12.5l4.5 4.5L19 7.5"/>"#,
        "nuclear" => r#"<circle cx="12" cy="12" r="1.8"/><ellipse cx="12" cy="12" rx="9" ry="3.6"/><ellipse cx="12" cy="12" rx="9" ry="3.6" transform="rotate(60 12 12)"/><ellipse cx="12" cy="12" rx="9" ry="3.6" transform="rotate(-60 12 12)"/>"#,
        "minus" => r#"<path d="M5 12h14"/>"#,
        "plus" => r#"<path d="M12 5v14M5 12h14"/>"#,
        _ => "",
    }
}
/// 24×24 윤곽선 아이콘 SVG
pub fn svg_i(d: &str) -> String {
    format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{d}</svg>"#)
}
/// 회사 종류 그림 (일 탭에서도 같은 그림을 써요)
pub fn kind_svg(k: &str) -> String {
    svg_i(icon(match k {
        "pc" | "ai" | "nuclear" => k,
        _ => "food",
    }))
}
pub const ICON_COL: [&str; 5] = ["#ffd9b8", "#c8e7ff", "#d8f5d0", "#ffd6e8", "#e4dcff"];

/// 색 50가지: 색상 10개 × 밝기 5단계
pub fn color_of(i: u32) -> String {
    const HUES: [u32; 10] = [4, 28, 46, 70, 128, 160, 196, 220, 262, 318];
    const LIGHTS: [u32; 5] = [36, 50, 64, 78, 90];
    let l = LIGHTS[((i / 10) as usize).min(4)];
    let s = if l > 85 {
        46
    } else if l > 70 {
        56
    } else {
        66
    };
    format!("hsl({} {s}% {l}%)", HUES[(i % 10) as usize])
}

/* ---------- 고르는 순서 ---------- */
pub struct Node {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    pub hint: &'static str,
    pub leaf: Option<&'static str>,
    pub soon: bool,
    pub children: &'static [Node],
}
const fn leaf(id: &'static str, label: &'static str, icon: &'static str, hint: &'static str) -> Node {
    Node { id, label, icon, hint, leaf: Some(id), soon: false, children: &[] }
}
const fn group(id: &'static str, label: &'static str, icon: &'static str, hint: &'static str, children: &'static [Node]) -> Node {
    Node { id, label, icon, hint, leaf: None, soon: false, children }
}
pub static ROOT: [Node; 3] = [
    group(
        "food",
        "식품",
        "food",
        "물, 음식, 에너지 식품",
        &[
            leaf("water", "물", "drop", "페트병, 뚜껑, 뚜껑 색"),
            group(
                "meal",
                "음식",
                "fork",
                "간식과 식사",
                &[
                    group("snack", "간식", "cookie", "젤리, 과자, 빵", &[leaf("jelly", "젤리", "bag", "용기 모양과 색"), leaf("chips", "과자", "bag", "봉지 모양과 색"), leaf("bread", "빵", "bag", "봉지 모양과 색")]),
                    group("plate", "식사", "plate", "도시락, 고기", &[leaf("lunchbox", "도시락", "box", "용기 모양과 색"), leaf("meat", "고기", "box", "고기 모양과 트레이 색")]),
                ],
            ),
            group("energy", "에너지 식품", "bolt", "에너지바, 음료, 커피", &[leaf("bar", "에너지바", "wrap", "포장, 내용물, 색"), leaf("drink", "에너지 음료", "can", "캔, 내용물, 색"), leaf("coffee", "커피", "cup", "컵, 내용물, 색")]),
        ],
    ),
    Node { id: "pc", label: "컴퓨터", icon: "pc", hint: "", leaf: None, soon: true, children: &[] },
    Node { id: "ai", label: "인공지능", icon: "ai", hint: "", leaf: None, soon: true, children: &[] },
];
/// id로 고르는 칸 찾기
pub fn node(id: &str) -> Option<&'static Node> {
    fn find(list: &'static [Node], id: &str) -> Option<&'static Node> {
        for n in list {
            if n.id == id {
                return Some(n);
            }
            if let Some(x) = find(n.children, id) {
                return Some(x);
            }
        }
        None
    }
    find(&ROOT, id)
}

/// 잎 상품: 어떤 그림 모음을 쓰고, 어떤 순서로 고르는지
pub struct Leaf {
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub family: &'static str,
    pub kinds: &'static [&'static str],
    pub flow: &'static [&'static str],
    pub shape_title: &'static str,
    pub color_title: &'static str,
    pub contents: &'static [&'static str],
}
pub fn leaf_of(item: &str) -> &'static Leaf {
    static L: [Leaf; 9] = [
        Leaf { key: "water", label: "물", group: "water", family: "bottle", kinds: &shapes::BOTTLE_KINDS, flow: &["bottle", "cap", "capColor"], shape_title: "", color_title: "", contents: &[] },
        Leaf { key: "jelly", label: "젤리", group: "snack", family: "bag", kinds: &shapes::BAG_JELLY, flow: &["shape", "color"], shape_title: "젤리 용기 모양", color_title: "용기 색", contents: &[] },
        Leaf { key: "chips", label: "과자", group: "snack", family: "bag", kinds: &shapes::BAG_CHIPS, flow: &["shape", "color"], shape_title: "과자 봉지 모양", color_title: "봉지 색", contents: &[] },
        Leaf { key: "bread", label: "빵", group: "snack", family: "bag", kinds: &shapes::BAG_BREAD, flow: &["shape", "color"], shape_title: "빵 봉지 모양", color_title: "봉지 색", contents: &[] },
        Leaf { key: "lunchbox", label: "도시락", group: "meal", family: "box", kinds: &shapes::BOX_KINDS, flow: &["shape", "color"], shape_title: "도시락 용기 모양", color_title: "용기 색", contents: &[] },
        Leaf { key: "meat", label: "고기", group: "meal", family: "meat", kinds: &shapes::MEAT_KINDS, flow: &["shape", "color"], shape_title: "고기 모양", color_title: "트레이 색", contents: &[] },
        Leaf { key: "bar", label: "에너지바", group: "energy", family: "wrap", kinds: &shapes::WRAP_KINDS, flow: &["shape", "contents", "color"], shape_title: "포장 모양", color_title: "포장 색", contents: &["초코", "바닐라", "딸기", "녹차", "땅콩", "블루베리"] },
        Leaf { key: "drink", label: "에너지 음료", group: "energy", family: "can", kinds: &shapes::CAN_KINDS, flow: &["shape", "contents", "color"], shape_title: "에너지 캔 모양", color_title: "캔 색", contents: &["레몬", "청포도", "복숭아", "베리", "콜라", "자몽"] },
        Leaf { key: "coffee", label: "커피", group: "energy", family: "cup", kinds: &shapes::CUP_KINDS, flow: &["shape", "contents", "color"], shape_title: "컵 모양", color_title: "컵 색", contents: &["아메리카노", "라떼", "바닐라 라떼", "콜드브루", "모카", "카라멜 마키아토"] },
    ];
    L.iter().find(|l| l.key == item).unwrap_or(&L[0])
}
fn container(family: &str) -> &'static str {
    match family {
        "bottle" => "페트병",
        "bag" => "봉지",
        "box" => "용기",
        "can" => "캔",
        "cup" => "컵",
        "wrap" => "포장",
        _ => "트레이",
    }
}

/// 고르는 칸: 종류 칸(shape·bottle·cap), 색 칸(color·capColor), 내용물(options)
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum StepKind {
    Shape,
    Color,
    Options,
}
pub struct Step {
    pub kind: StepKind,
    pub field: Field,
    pub title: &'static str,
    pub family: &'static str,
    pub kinds: &'static [&'static str],
}
pub fn step_info(key: &str, leaf: &Leaf) -> Step {
    let st = |kind, field, title, family, kinds| Step { kind, field, title, family, kinds };
    match key {
        "bottle" => st(StepKind::Shape, Field::Shape, "페트병 모양", "bottle", &shapes::BOTTLE_KINDS),
        "cap" => st(StepKind::Shape, Field::Cap, "뚜껑 모양", "cap", &shapes::CAP_KINDS),
        "capColor" => st(StepKind::Color, Field::CapColor, "뚜껑 색", "", &[]),
        "shape" => st(StepKind::Shape, Field::Shape, leaf.shape_title, leaf.family, leaf.kinds),
        "color" => st(StepKind::Color, Field::Color, leaf.color_title, "", &[]),
        _ => st(StepKind::Options, Field::Content, "내용물", "", &[]),
    }
}

/// 상품 값 (정하기 화면, 판매 목록, 상점, 인벤토리에서 같이 써요). 정하지 않은 칸은 None
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pick {
    pub item: String,
    pub shape: Option<u32>,
    pub cap: Option<u32>,
    pub cap_color: Option<u32>,
    pub color: Option<u32>,
    pub content: Option<u32>,
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Field {
    Shape,
    Cap,
    CapColor,
    Color,
    Content,
}
impl Pick {
    pub fn get(&self, f: Field) -> Option<u32> {
        match f {
            Field::Shape => self.shape,
            Field::Cap => self.cap,
            Field::CapColor => self.cap_color,
            Field::Color => self.color,
            Field::Content => self.content,
        }
    }
    pub fn set(&mut self, f: Field, v: Option<u32>) {
        match f {
            Field::Shape => self.shape = v,
            Field::Cap => self.cap = v,
            Field::CapColor => self.cap_color = v,
            Field::Color => self.color = v,
            Field::Content => self.content = v,
        }
    }
}

/* ---------- 원가 (1 토큰 = 1 달러) ----------
   근거 (검색한 값은 출처를 적고, 찾지 못한 값은 추정이에요):
   - 생수 페트병·뚜껑·라벨: ThePricer (병 8~20센트, 뚜껑 1~5센트, 라벨 2~10센트), 생수 1병 전체 25~75센트 범위 안
   - 음료 캔: vaza.gr 250ml 캔 €1,090/1,000개 ≈ 약 11센트 (1개 단가로 환산, 추정)
   - 커피 컵: 12온스 컵·뚜껑·슬리브 세트 22~37센트 (Walmart 100개 $21.99, Quill 500개 $186.99)
   - 도시락 용기: 17온스 크라프트 박스 92센트 (UCHANGE 3,000개 단가), 플라스틱 용기 30~62센트 (Global Sources)
   - 고기: 삼겹살 1kg 2만 원 안팎 (2026년 6월 축산물 가격표 21,000~25,000원, 도매 18,000원부터), 1달러 = 1,400원 가정
   - 봉지 필름, 내용물, 텀블러 등은 찾지 못해서 추정이에요. */
const FX: f64 = 1400.0;
const MEAT_UNIT: f64 = (20000.0 * 0.15) / FX; // 150g 한 팩 ≈ 2.14달러
const SIZE_F: [f64; 5] = [0.8, 0.9, 1.0, 1.1, 1.2]; // 크기 5단계에 따른 용기 배율
const W_BOTTLE: [f64; 5] = [0.07, 0.10, 0.16, 0.21, 0.25];
const W_FILL: [f64; 5] = [0.066, 0.10, 0.20, 0.30, 0.40];
const W_LABEL: f64 = 0.05;
const CAP_COST: [f64; 10] = [0.03, 0.05, 0.03, 0.04, 0.04, 0.03, 0.04, 0.03, 0.03, 0.03];
const MEAT_MULT: [f64; 10] = [2.5, 1.0, 2.0, 0.6, 0.7, 0.8, 1.0, 2.0, 0.7, 0.5];
const MEAT_TRAY: f64 = 0.15;
const LOGO: f64 = 0.02;
fn pack_cost(key: &str) -> ([f64; 10], f64) {
    match key {
        "chips" => ([0.05, 0.08, 0.25, 0.07, 0.12, 0.04, 0.10, 0.25, 0.04, 0.06], 0.40),
        "jelly" => ([0.04, 0.12, 0.08, 0.15, 0.04, 0.04, 0.08, 0.07, 0.25, 0.10], 0.30),
        "bread" => ([0.10, 0.06, 0.05, 0.20, 0.20, 0.07, 0.08, 0.05, 0.05, 0.06], 0.80),
        "lunchbox" => ([0.30, 0.25, 0.92, 0.40, 0.45, 0.50, 0.35, 0.60, 0.35, 0.35], 2.0),
        "bar" => ([0.05, 0.05, 0.06, 0.12, 0.04, 0.06, 0.08, 0.10, 0.05, 0.10], 0.40),
        "drink" => ([0.13, 0.11, 0.16, 0.18, 0.25, 0.10, 0.13, 0.13, 0.15, 0.13], 0.20),
        _ => ([0.25, 0.30, 0.20, 1.50, 0.80, 0.60, 0.28, 0.27, 0.35, 0.40], 0.15),
    }
}
/// JS Math.round (0.5는 위로)
fn js_round(v: f64) -> i64 {
    (v + 0.5).floor() as i64
}
/// 원가 줄 (센트 정수)과 합계
pub fn cost_lines(p: &Pick) -> (Vec<(&'static str, i64)>, i64) {
    let leaf = leaf_of(&p.item);
    let sh = p.shape.unwrap_or(0) as usize;
    let k = (sh / 5).min(9);
    let s = sh % 5;
    let mut lines: Vec<(&'static str, f64)> = Vec::new();
    if leaf.family == "bottle" {
        lines.push(("페트병", W_BOTTLE[s]));
        lines.push(("생수와 충전", W_FILL[s]));
        if let Some(c) = p.cap {
            lines.push(("뚜껑", CAP_COST[((c / 5) as usize).min(9)]));
        }
        lines.push(("라벨", W_LABEL));
    } else if leaf.family == "meat" {
        lines.push(("트레이", MEAT_TRAY));
        lines.push(("고기 150g", MEAT_UNIT * MEAT_MULT[k] * SIZE_F[s]));
    } else {
        let (pack, inside) = pack_cost(leaf.key);
        lines.push((container(leaf.family), pack[k] * SIZE_F[s]));
        lines.push(("내용물", inside * SIZE_F[s]));
    }
    lines.push(("로고 인쇄", LOGO));
    let rows: Vec<(&'static str, i64)> = lines.into_iter().map(|(n, usd)| (n, js_round(usd * 100.0))).collect();
    let total = rows.iter().map(|r| r.1).sum();
    (rows, total)
}

/* ---------- 이름 ---------- */
fn size_of(ml: u32) -> String {
    if ml >= 1000 {
        num(((ml as f64 / 1000.0) * 100.0).round() / 100.0) + "L"
    } else {
        format!("{ml}ml")
    }
}
pub fn name_of(p: &Pick) -> String {
    let leaf = leaf_of(&p.item);
    if p.item == "water" {
        return format!("생수 {}", size_of(shapes::BOTTLE_ML[(p.shape.unwrap_or(0) % 5) as usize]));
    }
    let flavor = match p.content {
        Some(c) if !leaf.contents.is_empty() => leaf.contents.get(c as usize).map(|t| format!(" · {t}")).unwrap_or_default(),
        _ => String::new(),
    };
    format!("{}{}", leaf.label, flavor).chars().take(30).collect()
}

/* ---------- 그림 ---------- */
fn paths(r: &Body, fill: &str) -> String {
    let tf = if r.xf.is_empty() { String::new() } else { format!(r#" transform="{}""#, r.xf) };
    let mut o = String::new();
    if !r.body.is_empty() {
        o += &format!(r#"<path class="pk-body" d="{}" fill="{fill}"/>"#, r.body);
    }
    if !r.ext.is_empty() {
        o += &format!(r##"<path class="pk-ext" d="{}" fill="#cf5a63"{tf}/>"##, r.ext);
    }
    if !r.win.is_empty() {
        o += &format!(r##"<path class="pk-win" d="{}" fill="#ffffff"/>"##, r.win);
    }
    if !r.det.is_empty() {
        o += &format!(r#"<path class="pk-det" d="{}"{tf}/>"#, r.det);
    }
    if !r.hl.is_empty() {
        o += &format!(r#"<path class="pk-hl" d="{}"/>"#, r.hl);
    }
    o
}
fn r1(n: f64) -> String {
    f(n)
}
/// 상품 그림의 몸통 (가운데 로고 자리는 logo 에 넣은 글자를 그대로 써요. 로고 글자는 부르는 쪽이 안전하게 만들어요)
pub fn pkv_parts(p: &Pick) -> (String, Body) {
    let leaf = leaf_of(&p.item);
    let k = p.shape.unwrap_or(0) as usize;
    let kind = leaf.kinds.get(k / 5).or(leaf.kinds.first()).copied().unwrap_or("");
    let r = shapes::body(leaf.family, kind, k % 5);
    let water = leaf.family == "bottle";
    let fill = if water {
        "#eef6fb".to_string()
    } else {
        p.color.map(color_of).unwrap_or_else(|| "#ffffff".into())
    };
    let mut cap = String::new();
    if water {
        if let Some(c) = p.cap {
            let (body, det) = shapes::cap(shapes::CAP_KINDS[((c / 5) as usize).min(9)], r.nr, r.top, (c % 5) as usize);
            let col = p.cap_color.map(color_of).unwrap_or_else(|| "#2b2b2f".into());
            cap = format!(r#"<path class="pk-body" d="{body}" fill="{col}"/>"#);
            if !det.is_empty() {
                cap += &format!(r#"<path class="pk-det" d="{det}"/>"#);
            }
        }
    }
    (format!(r#"<svg viewBox="0 0 120 200" aria-hidden="true">{}{cap}</svg>"#, paths(&r, &fill)), r)
}
/// 로고 자리 스타일 (left, top, width)
pub fn logo_style(r: &Body) -> String {
    format!("left:50%;top:{}%;width:{}%", r1((r.cy / 200.0) * 100.0), r1(((r.half * 2.0 * 0.94) / 120.0) * 100.0))
}
/// 상품 그림 전체. logo_inner 는 이미 안전하게 만든 HTML, has_img 는 사진 로고인지
pub fn pkv_html(p: &Pick, mini: bool, logo_inner: &str, has_img: bool) -> String {
    let (svg, r) = pkv_parts(p);
    format!(
        r#"<div class="pkv{}">{svg}<span class="pkv-logo{}" style="{}">{logo_inner}</span></div>"#,
        if mini { " pkv-sm" } else { "" },
        if has_img { "" } else { " init" },
        logo_style(&r)
    )
}
/// 고르는 칸의 작은 그림 (흰 바탕, 검은 윤곽선)
pub fn thumb_html(family: &str, kind: &str, s: usize) -> String {
    if family == "cap" {
        let (body, det) = shapes::cap(kind, 10.0, 60.0, s);
        let det = if det.is_empty() { String::new() } else { format!(r#"<path class="pk-det" d="{det}"/>"#) };
        return format!(r##"<svg class="th" viewBox="0 0 120 200" aria-hidden="true"><path class="pk-body" d="M52 60H68V150H52Z" fill="#f1f1f4"/><path class="pk-body" d="{body}" fill="#ffffff"/>{det}</svg>"##);
    }
    format!(r##"<svg class="th" viewBox="0 0 120 200" aria-hidden="true">{}</svg>"##, paths(&shapes::body(family, kind, s), "#ffffff"))
}
