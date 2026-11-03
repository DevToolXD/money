//! 상품 그림 도형 (예전 app/shapes.js). 사진이 아니라 검은 윤곽선 SVG예요 (흰 바탕, 색은 고른 색으로 채워요).
//! 가운데 x=60, 세로는 0~200. 종류(kind)마다 크기 5단계(s=0~4)를 그려요.
//! body 는 색을 채우는 몸통, ext 는 고기처럼 따로 채우는 안쪽, det 은 이음새·라벨 선, hl 은 유리 빛, win 은 창이에요.
//! 글자 하나까지 예전 JS와 같게 나와요 (tests: shapes_same_as_js).

pub const C: f64 = 60.0;
const SW: [f64; 5] = [0.8, 0.9, 1.0, 1.08, 1.16];
const SH: [f64; 5] = [0.84, 0.93, 1.0, 1.07, 1.12];
const CW: [f64; 5] = [0.9, 0.95, 1.0, 1.05, 1.1]; // 뚜껑 크기
const CH: [f64; 5] = [0.85, 0.93, 1.0, 1.07, 1.14];
pub const BOTTLE_ML: [u32; 5] = [330, 500, 1000, 1500, 2000];

type Pt = [f64; 2];

/// JS의 Math.round(n * 10) / 10 을 JS처럼 글자로 (0.5는 위로, -0은 0)
pub fn f(n: f64) -> String {
    num((n * 10.0 + 0.5).floor() / 10.0)
}
/// JS가 숫자를 글자로 바꾸는 것과 같게 (이 범위의 숫자에서는 가장 짧은 표기가 같아요)
pub fn num(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    format!("{v}")
}
fn mx(s: usize, x: f64) -> f64 {
    C + (x - C) * SW[s]
}
fn my(s: usize, y: f64) -> f64 {
    100.0 + (y - 100.0) * SH[s]
}
#[allow(non_snake_case)]
fn P(s: usize, x: f64, y: f64) -> Pt {
    [mx(s, x), my(s, y)]
}

/// 점들을 부드럽게 이어 닫힌 모양을 만들어요 (닫힌 Catmull-Rom → 3차 베지에)
fn closed(pts: &[Pt]) -> String {
    let mut q: Vec<Pt> = Vec::with_capacity(pts.len());
    for (i, p) in pts.iter().enumerate() {
        if i == 0 || (p[0] - pts[i - 1][0]).hypot(p[1] - pts[i - 1][1]) > 0.3 {
            q.push(*p);
        }
    }
    if q.len() > 2 {
        let (a, b) = (q[0], q[q.len() - 1]);
        if (a[0] - b[0]).hypot(a[1] - b[1]) < 0.3 {
            q.pop();
        }
    }
    let n = q.len();
    let mut d = format!("M{} {}", f(q[0][0]), f(q[0][1]));
    for i in 0..n {
        let p0 = q[(i + n - 1) % n];
        let p1 = q[i];
        let p2 = q[(i + 1) % n];
        let p3 = q[(i + 2) % n];
        d += &format!(
            "C{} {} {} {} {} {}",
            f(p1[0] + (p2[0] - p0[0]) / 6.0),
            f(p1[1] + (p2[1] - p0[1]) / 6.0),
            f(p2[0] - (p3[0] - p1[0]) / 6.0),
            f(p2[1] - (p3[1] - p1[1]) / 6.0),
            f(p2[0]),
            f(p2[1])
        );
    }
    d + "Z"
}
/// 오른쪽 옆선(절대 좌표, 위→아래)을 왼쪽에 대칭으로 붙여요
fn sym(r: &[Pt]) -> String {
    let mut all: Vec<Pt> = r.to_vec();
    all.extend(r[..r.len() - 1].iter().rev().map(|[x, y]| [2.0 * C - x, *y]));
    closed(&all)
}
struct Prof {
    d: String,
    q: Vec<Pt>,
}
/// 가운데에서 떨어진 거리(d)와 세로(y)로 적은 옆선을 크기에 맞춰요
fn prof(s: usize, r: &[Pt]) -> Prof {
    let q: Vec<Pt> = r.iter().map(|[d, y]| [C + d * SW[s], my(s, *y)]).collect();
    Prof { d: sym(&q), q }
}
/// 세로 y에서 옆선까지의 반지름
fn half_at(q: &[Pt], y: f64) -> f64 {
    for i in 0..q.len().saturating_sub(1) {
        let [x0, y0] = q[i];
        let [x1, y1] = q[i + 1];
        if (y0 <= y && y <= y1) || (y1 <= y && y <= y0) {
            let t = if y1 == y0 { 0.0 } else { (y - y0) / (y1 - y0) };
            return (x0 + (x1 - x0) * t - C).abs();
        }
    }
    18.0
}

/* 기본 도형들 (절대 좌표) */
fn rr_a(x: f64, y: f64, w: f64, h: f64, r: f64) -> String {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let (l, t, rr, b) = (f(x), f(y), f(x + w), f(y + h));
    let (a, bb, c, e) = (f(x + r), f(y + r), f(x + w - r), f(y + h - r));
    format!("M{a} {t}H{c}Q{rr} {t} {rr} {bb}V{e}Q{rr} {b} {c} {b}H{a}Q{l} {b} {l} {e}V{bb}Q{l} {t} {a} {t}Z")
}
fn ell(x: f64, y: f64, rx: f64, ry: f64) -> String {
    format!(
        "M{} {}A{} {} 0 1 0 {} {}A{} {} 0 1 0 {} {}Z",
        f(x - rx),
        f(y),
        f(rx),
        f(ry),
        f(x + rx),
        f(y),
        f(rx),
        f(ry),
        f(x - rx),
        f(y)
    )
}
fn line(x1: f64, y1: f64, x2: f64, y2: f64) -> String {
    format!("M{} {}L{} {}", f(x1), f(y1), f(x2), f(y2))
}
/// 물결(실링 자국): 가로 방향으로 n번 굽어요
fn wave(x1: f64, x2: f64, y: f64, amp: f64, n: usize) -> String {
    let st = (x2 - x1) / n as f64;
    let mut o = String::new();
    for i in 0..n {
        let i = i as f64;
        o += &format!("Q{} {} {} {}", f(x1 + st * i + st / 2.0), f(y - amp), f(x1 + st * (i + 1.0)), f(y));
    }
    o
}
/// 명령 목록(기준 좌표)을 크기에 맞춰 path 로 바꿔요. W 는 현재 점에서 가로 물결 [x, y, 높이, 횟수]
fn path(s: usize, cmds: &[(char, &[f64])]) -> String {
    let mut out = String::new();
    let mut cur: Pt = [0.0, 0.0];
    for (op, a) in cmds {
        match op {
            'Z' => out.push('Z'),
            'W' => {
                let b = P(s, a[0], a[1]);
                out += &wave(cur[0], b[0], cur[1], a[2] * SH[s], a[3] as usize);
                cur = b;
            }
            _ => {
                let pts: Vec<Pt> = a.chunks(2).map(|c| P(s, c[0], c[1])).collect();
                out.push(*op);
                out += &pts.iter().map(|p| format!("{} {}", f(p[0]), f(p[1]))).collect::<Vec<_>>().join(" ");
                cur = pts[pts.len() - 1];
            }
        }
    }
    out
}
/// 기준 좌표의 사각형 (왼쪽, 위, 오른쪽, 아래)
fn bx(w: f64, h: f64) -> [f64; 4] {
    [C - w, 100.0 - h / 2.0, C + w, 100.0 + h / 2.0]
}
fn r_s(s: usize, x: f64, y: f64, w: f64, h: f64, r: f64) -> String {
    let a = P(s, x, y);
    rr_a(a[0], a[1], w * SW[s], h * SH[s], r * SW[s].min(SH[s]))
}
fn e_s(s: usize, x: f64, y: f64, rx: f64, ry: f64) -> String {
    let a = P(s, x, y);
    ell(a[0], a[1], rx * SW[s], ry * SH[s])
}
fn l_s(s: usize, x1: f64, y1: f64, x2: f64, y2: f64) -> String {
    let a = P(s, x1, y1);
    let b = P(s, x2, y2);
    line(a[0], a[1], b[0], b[1])
}
/// 대칭 모양 (기준 좌표의 오른쪽 옆선)
fn s_s(s: usize, pts: &[Pt]) -> String {
    sym(&pts.iter().map(|[x, y]| P(s, *x, *y)).collect::<Vec<_>>())
}
/// 대칭이 아닌 닫힌 모양
fn c_s(s: usize, pts: &[Pt]) -> String {
    closed(&pts.iter().map(|[x, y]| P(s, *x, *y)).collect::<Vec<_>>())
}

/// 한 칸의 그림 조각
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Body {
    pub body: String,
    pub ext: String,
    pub det: String,
    pub hl: String,
    pub win: String,
    pub xf: String,
    pub cy: f64,
    pub half: f64,
    pub top: f64,
    pub nr: f64,
}

/// 만드는 중의 조각 (값이 없는 칸은 기본값을 써요)
#[derive(Default)]
struct R {
    body: String,
    tray: String,
    ext: String,
    det: String,
    hl: String,
    win: String,
    cy: Option<f64>,
    half: Option<f64>,
    top: Option<f64>,
    nr: Option<f64>,
}
fn bd(body: String, det: String) -> R {
    R { body, det, ..R::default() }
}

/* ---------- 페트병 (물) ---------- */
#[derive(Default)]
struct BotOpt {
    label: f64,
    ribs: &'static [f64],
    rings: &'static [f64],
    label_rect: bool,
}
fn bottle(s: usize, r: &[Pt], o: BotOpt) -> R {
    let p = prof(s, r);
    let cy = my(s, o.label);
    let half = half_at(&p.q, cy);
    let mut det = String::new();
    for x in o.ribs {
        det += &l_s(s, C + x, 92.0, C + x, 168.0);
    }
    for y in o.rings {
        let yy = my(s, *y);
        let h = half_at(&p.q, yy);
        det += &format!("M{} {}Q{} {} {} {}", f(C - h + 1.0), f(yy), num(C), f(yy + 3.0), f(C + h - 1.0), f(yy));
    }
    if o.label_rect {
        det += &r_s(s, C - 30.0, 96.0, 60.0, 52.0, 4.0);
    }
    let hx = C - half_at(&p.q, my(s, 110.0)) * 0.6;
    R {
        body: p.d,
        det,
        hl: line(hx, my(s, 84.0), hx, my(s, 164.0)),
        cy: Some(cy),
        half: Some(half),
        top: Some(p.q[0][1]),
        nr: Some(p.q[1][0] - C),
        ..R::default()
    }
}
const BOT_STD: [Pt; 10] = [[0.0, 26.0], [12.0, 26.0], [12.0, 46.0], [16.0, 52.0], [30.0, 62.0], [33.0, 80.0], [33.0, 172.0], [31.0, 182.0], [26.0, 188.0], [0.0, 188.0]];
pub const BOTTLE_KINDS: [&str; 10] = ["classic", "square", "rib", "slim", "waist", "sport", "flat", "ring", "sparkle", "jug"];
fn bottle_of(kind: &str, s: usize) -> Option<R> {
    let lb = |label: f64| BotOpt { label, ..BotOpt::default() };
    Some(match kind {
        "classic" => bottle(s, &BOT_STD, lb(128.0)),
        "square" => bottle(s, &[[0.0, 26.0], [12.0, 26.0], [12.0, 44.0], [21.0, 50.0], [32.0, 56.0], [34.0, 66.0], [34.0, 172.0], [33.0, 181.0], [29.0, 186.0], [0.0, 186.0]], BotOpt { label: 128.0, label_rect: true, ..BotOpt::default() }),
        "rib" => bottle(s, &BOT_STD, BotOpt { label: 128.0, ribs: &[-24.0, -12.0, 12.0, 24.0], ..BotOpt::default() }),
        "slim" => bottle(s, &[[0.0, 24.0], [11.0, 24.0], [11.0, 40.0], [15.0, 58.0], [25.0, 70.0], [26.0, 86.0], [26.0, 176.0], [24.0, 186.0], [0.0, 186.0]], lb(130.0)),
        "waist" => bottle(s, &[[0.0, 28.0], [12.0, 28.0], [12.0, 48.0], [24.0, 60.0], [34.0, 76.0], [34.0, 92.0], [26.0, 112.0], [27.0, 140.0], [34.0, 156.0], [34.0, 176.0], [31.0, 186.0], [0.0, 186.0]], lb(140.0)),
        "sport" => bottle(s, &[[0.0, 22.0], [9.0, 22.0], [9.0, 40.0], [14.0, 46.0], [28.0, 56.0], [32.0, 70.0], [32.0, 176.0], [30.0, 186.0], [0.0, 186.0]], lb(124.0)),
        "flat" => bottle(s, &[[0.0, 46.0], [16.0, 46.0], [24.0, 52.0], [40.0, 62.0], [42.0, 78.0], [42.0, 150.0], [40.0, 164.0], [34.0, 170.0], [0.0, 170.0]], lb(110.0)),
        "ring" => bottle(s, &[[0.0, 26.0], [12.0, 26.0], [12.0, 46.0], [16.0, 52.0], [30.0, 62.0], [32.0, 80.0], [32.0, 172.0], [30.0, 182.0], [26.0, 188.0], [0.0, 188.0]], BotOpt { label: 128.0, rings: &[96.0, 108.0, 120.0, 132.0, 144.0, 156.0], ..BotOpt::default() }),
        "sparkle" => bottle(s, &[[0.0, 26.0], [11.0, 26.0], [11.0, 46.0], [17.0, 56.0], [30.0, 68.0], [34.0, 96.0], [33.0, 120.0], [27.0, 160.0], [24.0, 182.0], [0.0, 184.0]], lb(110.0)),
        "jug" => bottle(s, &[[0.0, 24.0], [18.0, 24.0], [18.0, 34.0], [24.0, 42.0], [34.0, 52.0], [35.0, 70.0], [35.0, 172.0], [33.0, 184.0], [0.0, 184.0]], lb(120.0)),
        _ => return None,
    })
}

/* ---------- 뚜껑 (페트병 목 위에 얹혀요, 절대 좌표) ---------- */
fn screw_ridges(w: f64, y: f64, h: f64) -> String {
    [-0.6, -0.3, 0.0, 0.3, 0.6].iter().map(|q| line(C + q * w, y + 3.0, C + q * w, y + h - 3.0)).collect()
}
pub const CAP_KINDS: [&str; 10] = ["screw", "flip", "dome", "ring", "tall", "flat", "knob", "click", "cone", "square"];
/// 뚜껑 그림: (몸통, 선)
pub fn cap(kind: &str, nr: f64, t: f64, s: usize) -> (String, String) {
    let screw = |nr: f64, t: f64| {
        let w = (nr + 2.5) * CW[s];
        let h = 15.0 * CH[s];
        let y = t - 13.0 * CH[s];
        (rr_a(C - w, y, 2.0 * w, h, 3.0), screw_ridges(w, y, h))
    };
    match kind {
        "flip" => {
            let w = (nr + 3.0) * CW[s];
            let y = t - 10.0 * CH[s];
            let h = 12.0 * CH[s];
            (rr_a(C - w, y, 2.0 * w, h, 3.0) + &rr_a(C - w - 1.0, y - 5.0 * CH[s], 9.0 * CW[s], 7.0 * CH[s], 2.0), line(C - w + 2.0, y + 3.0, C + w - 2.0, y + 3.0))
        }
        "dome" => {
            let w = (nr + 3.0) * CW[s];
            let yb = t + 2.0;
            let yt = t - 18.0 * CH[s];
            (sym(&[[C, yt], [C + w * 0.5, yt], [C + w * 0.92, yt + 4.0 * CH[s]], [C + w, yt + 9.0 * CH[s]], [C + w, yb], [C, yb]]), String::new())
        }
        "ring" => {
            let (b, d) = screw(nr, t);
            let w = (nr + 3.8) * CW[s];
            let y = t + 4.0 * CH[s];
            (b, d + &rr_a(C - w, y, 2.0 * w, 3.6 * CH[s], 1.5))
        }
        "tall" => {
            let w = (nr + 2.0) * CW[s];
            let h = 24.0 * CH[s];
            let y = t - 22.0 * CH[s];
            (rr_a(C - w, y, 2.0 * w, h, 3.0), screw_ridges(w, y, h))
        }
        "flat" => {
            let w = (nr + 5.0) * CW[s];
            let h = 9.0 * CH[s];
            let y = t - 7.0 * CH[s];
            (rr_a(C - w, y, 2.0 * w, h, 3.0), String::new())
        }
        "knob" => {
            let w = (nr + 2.5) * CW[s];
            let h = 12.0 * CH[s];
            let y = t - 10.0 * CH[s];
            let r = 5.5 * CW[s];
            (rr_a(C - w, y, 2.0 * w, h, 3.0) + &ell(C, y - 1.0, r, r), String::new())
        }
        "click" => {
            let (b, _) = screw(nr, t);
            let w = (nr + 2.5) * CW[s];
            let y = t - 6.0 * CH[s];
            (b, line(C - w, y, C + w, y))
        }
        "cone" => {
            let w = (nr + 1.0) * CW[s];
            let w2 = (nr + 4.0) * CW[s];
            let yt = t - 14.0 * CH[s];
            let yb = t + 2.0;
            (sym(&[[C, yt], [C + w, yt], [C + w2, yb], [C, yb]]), String::new())
        }
        "square" => {
            let w = (nr + 3.0) * CW[s];
            let h = 14.0 * CH[s];
            let y = t - 12.0 * CH[s];
            (rr_a(C - w, y, 2.0 * w, h, 2.0), String::new())
        }
        _ => screw(nr, t),
    }
}

/* ---------- 과자·젤리·빵 봉지 ---------- */
fn pillow_s(s: usize, b: [f64; 4]) -> String {
    path(s, &[('M', &[b[0], b[1]]), ('W', &[b[2], b[1], 3.0, 6.0]), ('L', &[b[2], b[3]]), ('W', &[b[0], b[3], -3.0, 6.0]), ('Z', &[])])
}
/// 캔처럼 생긴 통 (뚜껑 타원 포함): (몸통, 위)
fn tube_s(s: usize, w: f64, h: f64, rim: f64) -> (String, f64) {
    let b = bx(w, h);
    (s_s(s, &[[C, b[1]], [b[2], b[1] + rim], [b[2], b[3] - rim], [C, b[3]]]), b[1] + rim)
}
fn standup_like(s: usize, b: [f64; 4]) -> String {
    path(
        s,
        &[
            ('M', &[b[0], b[1]]),
            ('W', &[b[2], b[1], 3.0, 6.0]),
            ('L', &[b[2], b[3]]),
            ('Q', &[b[2], b[3] + 9.0, b[2] - 6.0, b[3] + 9.0]),
            ('L', &[b[0] + 6.0, b[3] + 9.0]),
            ('Q', &[b[0], b[3] + 9.0, b[0], b[3]]),
            ('Z', &[]),
        ],
    )
}
pub const BAG_CHIPS: [&str; 10] = ["pillow", "standup", "tube", "window", "big", "mini", "kraft", "canister", "sachet", "tri"];
pub const BAG_JELLY: [&str; 10] = ["stick", "cup", "standup", "tub", "mini", "sachet", "blister", "window", "tube", "kraft"];
pub const BAG_BREAD: [&str; 10] = ["kraft", "tall", "pillow", "bakery", "round", "window", "standup", "wedge", "roll", "tri"];
const BAG_ALL: [&str; 20] = ["pillow", "standup", "tube", "window", "big", "mini", "kraft", "canister", "sachet", "tri", "stick", "cup", "tub", "blister", "tall", "bakery", "round", "wedge", "roll", "x"];
fn bag_of(kind: &str, s: usize) -> Option<R> {
    Some(match kind {
        "pillow" => {
            let b = bx(30.0, 110.0);
            bd(pillow_s(s, b), l_s(s, b[0] + 4.0, b[1] + 9.0, b[2] - 4.0, b[1] + 9.0))
        }
        "standup" => {
            let b = bx(28.0, 118.0);
            bd(standup_like(s, b), l_s(s, b[0] + 4.0, b[1] + 12.0, b[2] - 4.0, b[1] + 12.0))
        }
        "tube" => {
            let (body, top) = tube_s(s, 22.0, 96.0, 6.0);
            bd(body, e_s(s, C, top, 21.5, 4.5))
        }
        "window" => {
            let b = bx(30.0, 110.0);
            R { body: pillow_s(s, b), win: r_s(s, C - 18.0, 84.0, 36.0, 34.0, 6.0), det: l_s(s, b[0] + 4.0, b[1] + 9.0, b[2] - 4.0, b[1] + 9.0), ..R::default() }
        }
        "big" => {
            let b = bx(34.0, 128.0);
            bd(standup_like(s, b), l_s(s, b[0] + 4.0, b[1] + 14.0, b[2] - 4.0, b[1] + 14.0) + &l_s(s, b[0] + 4.0, b[1] + 8.0, b[2] - 4.0, b[1] + 8.0))
        }
        "mini" => {
            let b = bx(22.0, 80.0);
            bd(pillow_s(s, b), l_s(s, b[0] + 3.0, b[1] + 8.0, b[2] - 3.0, b[1] + 8.0))
        }
        "kraft" => {
            let b = bx(30.0, 112.0);
            bd(path(s, &[('M', &[b[0], b[1]]), ('L', &[b[2], b[1]]), ('L', &[C + 33.0, b[3]]), ('L', &[C - 33.0, b[3]]), ('Z', &[])]), l_s(s, b[0], b[1] + 10.0, b[2], b[1] + 10.0))
        }
        "canister" => {
            let (body, top) = tube_s(s, 22.0, 92.0, 5.0);
            bd(body, e_s(s, C, top, 21.0, 3.6) + &l_s(s, C - 22.0, top + 8.0, C + 22.0, top + 8.0))
        }
        "sachet" => {
            let b = bx(30.0, 92.0);
            let a = P(s, b[0], b[1]);
            let p1 = P(s, C - 6.0, b[1]);
            let p2 = P(s, C, b[1] + 8.0);
            let p3 = P(s, C + 6.0, b[1]);
            bd(
                rr_a(a[0], a[1], 2.0 * 30.0 * SW[s], 92.0 * SH[s], 6.0),
                line(p1[0], p1[1], p2[0], p2[1]) + &line(p2[0], p2[1], p3[0], p3[1]) + &l_s(s, b[0] + 3.0, b[3] - 6.0, b[2] - 3.0, b[3] - 6.0),
            )
        }
        "tri" => {
            let b = bx(30.0, 110.0);
            bd(
                path(s, &[('M', &[b[0], b[1] + 22.0]), ('L', &[C, b[1]]), ('L', &[b[2], b[1] + 22.0]), ('L', &[b[2], b[3]]), ('L', &[b[0], b[3]]), ('Z', &[])]),
                l_s(s, b[0], b[1] + 22.0, b[2], b[1] + 22.0),
            )
        }
        "stick" => {
            let b = bx(16.0, 124.0);
            bd(pillow_s(s, b), l_s(s, b[0] + 3.0, b[1] + 8.0, b[2] - 3.0, b[1] + 8.0))
        }
        "cup" => {
            let b = bx(26.0, 100.0);
            let cup = s_s(s, &[[C, b[1] + 14.0], [b[2], b[1] + 14.0], [C + 26.0 * 0.9, b[3]], [C, b[3]]]);
            bd(cup + &r_s(s, b[0] - 3.0, b[1] + 6.0, 58.0, 9.0, 3.0), String::new())
        }
        "tub" => {
            let b = bx(30.0, 70.0);
            bd(r_s(s, b[0] - 4.0, b[1] + 20.0, 68.0, b[3] - b[1] - 20.0, 6.0) + &r_s(s, b[0] - 6.0, b[1] + 10.0, 72.0, 12.0, 4.0), l_s(s, b[0], b[1] + 40.0, b[2], b[1] + 40.0))
        }
        "blister" => {
            let b = bx(30.0, 110.0);
            let qs = [-0.5, 0.0, 0.5];
            let mut dots: String = qs.iter().map(|q| e_s(s, C + q * 54.0, b[1] + 30.0, 6.5, 6.5)).collect();
            dots += &qs.iter().map(|q| e_s(s, C + q * 54.0, b[3] - 30.0, 6.5, 6.5)).collect::<String>();
            bd(r_s(s, b[0], b[1] + 10.0, 60.0, 90.0, 8.0), dots)
        }
        "tall" => {
            let b = bx(26.0, 150.0);
            bd(pillow_s(s, b), e_s(s, C, b[1] + 10.0, 4.0, 3.0) + &l_s(s, C, b[1] + 13.0, C, b[1] + 22.0))
        }
        "bakery" => {
            let b = bx(36.0, 96.0);
            bd(r_s(s, b[0], b[1] + 22.0, 72.0, b[3] - b[1] - 22.0, 3.0) + &path(s, &[('M', &[b[0], b[1] + 22.0]), ('L', &[C, b[1]]), ('L', &[b[2], b[1] + 22.0]), ('Z', &[])]), String::new())
        }
        "round" => {
            let (body, top) = tube_s(s, 26.0, 64.0, 8.0);
            bd(body, e_s(s, C, top, 25.5, 4.6))
        }
        "wedge" => {
            let b = bx(34.0, 86.0);
            bd(path(s, &[('M', &[b[0], b[3]]), ('L', &[b[2], b[3]]), ('L', &[b[2], b[1]]), ('Z', &[])]), l_s(s, b[0] + 4.0, b[3] - 8.0, b[2] - 6.0, b[3] - 8.0))
        }
        "roll" => {
            let b = bx(24.0, 84.0);
            bd(r_s(s, b[0], b[1], 48.0, 84.0, 8.0), l_s(s, b[0] + 4.0, b[1] + 8.0, b[2] - 4.0, b[1] + 8.0) + &l_s(s, b[0] + 4.0, b[1] + 14.0, b[2] - 4.0, b[1] + 14.0))
        }
        _ => return None,
    })
}

/* ---------- 도시락 용기 ---------- */
pub const BOX_KINDS: [&str; 10] = ["bento", "round", "kraft", "wedge", "twotier", "bowl", "tall", "microwave", "tray", "bento2"];
fn box_of(kind: &str, s: usize) -> Option<R> {
    Some(match kind {
        "bento" => {
            let b = bx(38.0, 70.0);
            bd(r_s(s, b[0], b[1], 76.0, 70.0, 6.0), l_s(s, b[0] + 4.0, b[1] + 22.0, b[2] - 4.0, b[1] + 22.0))
        }
        "round" => {
            let (body, top) = tube_s(s, 30.0, 80.0, 8.0);
            bd(body, e_s(s, C, top, 29.5, 5.0))
        }
        "kraft" => {
            let b = bx(38.0, 70.0);
            bd(r_s(s, b[0], b[1], 76.0, 70.0, 3.0) + &path(s, &[('M', &[b[0], b[1]]), ('L', &[C, b[1] - 16.0]), ('L', &[b[2], b[1]]), ('Z', &[])]), String::new())
        }
        "wedge" => {
            let b = bx(40.0, 70.0);
            bd(path(s, &[('M', &[b[0], b[3]]), ('L', &[b[2], b[3]]), ('L', &[b[2], b[1]]), ('Z', &[])]), l_s(s, b[0] + 4.0, b[3] - 10.0, b[2] - 10.0, b[3] - 10.0))
        }
        "twotier" => {
            let b = bx(36.0, 84.0);
            bd(r_s(s, b[0], b[1], 72.0, 38.0, 5.0) + &r_s(s, b[0], b[1] + 44.0, 72.0, 40.0, 5.0), String::new())
        }
        "bowl" => {
            let b = bx(38.0, 80.0);
            bd(
                s_s(s, &[[C, b[1] + 24.0], [b[2], b[1] + 24.0], [C + 32.0, b[3] - 16.0], [C + 19.0, b[3]], [C, b[3]]]) + &path(s, &[('M', &[b[0] - 2.0, b[1] + 24.0]), ('Q', &[C, b[1] - 6.0, b[2] + 2.0, b[1] + 24.0]), ('Z', &[])]),
                String::new(),
            )
        }
        "tall" => {
            let b = bx(24.0, 100.0);
            bd(r_s(s, b[0], b[1], 48.0, 100.0, 4.0), l_s(s, b[0] + 4.0, b[1] + 14.0, b[2] - 4.0, b[1] + 14.0))
        }
        "microwave" => {
            let b = bx(40.0, 64.0);
            bd(r_s(s, b[0], b[1], 80.0, 64.0, 10.0), l_s(s, C - 12.0, b[1] + 8.0, C + 12.0, b[1] + 8.0) + &l_s(s, C - 12.0, b[1] + 13.0, C + 12.0, b[1] + 13.0))
        }
        "tray" => {
            let b = bx(44.0, 50.0);
            bd(r_s(s, b[0] - 4.0, b[1] + 15.0, 96.0, 26.0, 6.0), l_s(s, C - 14.0, b[1] + 15.0, C - 14.0, b[1] + 41.0) + &l_s(s, C + 14.0, b[1] + 15.0, C + 14.0, b[1] + 41.0))
        }
        "bento2" => {
            let b = bx(38.0, 70.0);
            bd(r_s(s, b[0], b[1], 76.0, 70.0, 6.0), l_s(s, C - 13.0, b[1] + 4.0, C - 13.0, b[3] - 4.0) + &l_s(s, C + 13.0, b[1] + 4.0, C + 13.0, b[3] - 4.0))
        }
        _ => return None,
    })
}

/* ---------- 고기 (트레이 위에 고기 모양) ---------- */
fn tray(s: usize) -> String {
    r_s(s, C - 50.0, 44.0, 100.0, 112.0, 8.0)
}
pub const MEAT_KINDS: [&str; 10] = ["steak", "samgyeop", "galbi", "drum", "sausage", "ham", "bacon", "cubes", "meatball", "breast"];
fn meat_of(kind: &str, s: usize) -> Option<R> {
    let m = |ext: String, det: String| R { tray: tray(s), ext, det, ..R::default() };
    Some(match kind {
        "steak" => m(
            c_s(s, &[[C - 28.0, 94.0], [C - 22.0, 83.0], [C - 4.0, 78.0], [C + 16.0, 81.0], [C + 28.0, 94.0], [C + 26.0, 110.0], [C + 12.0, 120.0], [C - 10.0, 118.0], [C - 26.0, 108.0]]),
            l_s(s, C - 12.0, 96.0, C - 4.0, 92.0) + &l_s(s, C + 6.0, 104.0, C + 14.0, 100.0) + &l_s(s, C - 4.0, 110.0, C + 4.0, 108.0),
        ),
        "samgyeop" => m(
            r_s(s, C - 26.0, 74.0, 52.0, 16.0, 4.0) + &r_s(s, C - 26.0, 96.0, 52.0, 16.0, 4.0) + &r_s(s, C - 26.0, 118.0, 52.0, 16.0, 4.0),
            l_s(s, C - 20.0, 82.0, C + 20.0, 82.0) + &l_s(s, C - 20.0, 104.0, C + 20.0, 104.0) + &l_s(s, C - 20.0, 126.0, C + 20.0, 126.0),
        ),
        "galbi" => m(r_s(s, C - 32.0, 88.0, 64.0, 24.0, 11.0) + &e_s(s, C + 32.0, 100.0, 9.0, 9.0), e_s(s, C + 32.0, 100.0, 3.0, 3.0) + &l_s(s, C - 22.0, 100.0, C + 18.0, 100.0)),
        "drum" => m(
            e_s(s, C - 6.0, 100.0, 26.0, 20.0) + &r_s(s, C + 14.0, 97.0, 18.0, 6.0, 3.0) + &e_s(s, C + 34.0, 94.0, 5.0, 5.0) + &e_s(s, C + 34.0, 106.0, 5.0, 5.0),
            e_s(s, C - 6.0, 100.0, 14.0, 10.0),
        ),
        "sausage" => m(
            r_s(s, C - 32.0, 78.0, 64.0, 12.0, 6.0) + &r_s(s, C - 32.0, 96.0, 64.0, 12.0, 6.0) + &r_s(s, C - 32.0, 114.0, 64.0, 12.0, 6.0),
            l_s(s, C - 10.0, 78.0, C - 10.0, 90.0) + &l_s(s, C + 10.0, 96.0, C + 10.0, 108.0) + &l_s(s, C - 6.0, 114.0, C - 6.0, 126.0),
        ),
        "ham" => m(e_s(s, C, 100.0, 34.0, 26.0), e_s(s, C, 100.0, 22.0, 16.0) + &e_s(s, C, 100.0, 6.0, 5.0)),
        "bacon" => m(
            r_s(s, C - 32.0, 72.0, 64.0, 8.0, 4.0) + &r_s(s, C - 32.0, 88.0, 64.0, 8.0, 4.0) + &r_s(s, C - 32.0, 104.0, 64.0, 8.0, 4.0) + &r_s(s, C - 32.0, 120.0, 64.0, 8.0, 4.0),
            l_s(s, C - 24.0, 76.0, C + 24.0, 76.0) + &l_s(s, C - 24.0, 92.0, C + 24.0, 92.0) + &l_s(s, C - 24.0, 108.0, C + 24.0, 108.0),
        ),
        "cubes" => {
            let mut ext = String::new();
            for i in 0..3 {
                for j in 0..2 {
                    ext += &r_s(s, C - 30.0 + i as f64 * 20.0 + 1.0, 76.0 + j as f64 * 22.0, 18.0, 18.0, 4.0);
                }
            }
            m(ext, String::new())
        }
        "meatball" => m(
            e_s(s, C - 16.0, 88.0, 12.0, 12.0) + &e_s(s, C + 12.0, 86.0, 12.0, 12.0) + &e_s(s, C - 4.0, 112.0, 13.0, 13.0) + &e_s(s, C + 20.0, 110.0, 11.0, 11.0),
            l_s(s, C - 20.0, 84.0, C - 14.0, 82.0) + &l_s(s, C + 8.0, 82.0, C + 14.0, 80.0),
        ),
        "breast" => m(e_s(s, C, 100.0, 32.0, 22.0), path(s, &[('M', &[C - 18.0, 96.0]), ('Q', &[C, 104.0, C + 18.0, 96.0])])),
        _ => return None,
    })
}

/* ---------- 에너지 음료 캔 ---------- */
#[derive(Default)]
struct CanOpt {
    label: Option<f64>,
    no_rim: bool,
    tab: bool,
    ridges: &'static [f64],
    bands: &'static [f64],
}
fn can_std(s: usize, r: &[Pt], o: CanOpt) -> R {
    let p = prof(s, r);
    let q = &p.q;
    let mut det = if o.no_rim {
        String::new()
    } else {
        format!("M{} {}Q{} {} {} {}", f(C - r[1][0] * SW[s] + 1.0), f(my(s, 42.0)), num(C), f(my(s, 48.0)), f(C + r[1][0] * SW[s] - 1.0), f(my(s, 42.0)))
    };
    if o.tab {
        det += &r_s(s, C - 6.0, 28.0, 12.0, 7.0, 2.0);
    }
    for y in o.ridges {
        let yy = my(s, *y);
        det += &format!("M{} {}Q{} {} {} {}", f(C - half_at(q, yy) + 1.0), f(yy), num(C), f(yy + 3.0), f(C + half_at(q, yy) - 1.0), f(yy));
    }
    for y in o.bands {
        det += &l_s(s, C - 31.0, *y, C + 31.0, *y);
    }
    let hx = C - half_at(q, my(s, 110.0)) * 0.6;
    let label = o.label.unwrap_or(110.0);
    R {
        hl: line(hx, my(s, 80.0), hx, my(s, 160.0)),
        cy: Some(my(s, label)),
        half: Some(half_at(q, my(s, label))),
        body: p.d,
        det,
        ..R::default()
    }
}
const CAN_STD: [Pt; 9] = [[0.0, 36.0], [25.0, 36.0], [27.0, 40.0], [29.0, 46.0], [31.0, 54.0], [31.0, 164.0], [30.0, 174.0], [27.0, 181.0], [0.0, 183.0]];
pub const CAN_KINDS: [&str; 10] = ["standard", "slim", "tall", "wide", "shot", "pouch", "ridge", "band", "bottle", "tab"];
fn can_of(kind: &str, s: usize) -> Option<R> {
    let lb = |l: f64| CanOpt { label: Some(l), ..CanOpt::default() };
    Some(match kind {
        "standard" => can_std(s, &CAN_STD, lb(110.0)),
        "slim" => can_std(s, &[[0.0, 36.0], [19.0, 36.0], [21.0, 40.0], [22.0, 46.0], [23.0, 54.0], [23.0, 166.0], [22.0, 176.0], [20.0, 182.0], [0.0, 184.0]], lb(112.0)),
        "tall" => can_std(s, &[[0.0, 30.0], [21.0, 30.0], [23.0, 34.0], [25.0, 40.0], [27.0, 46.0], [30.0, 54.0], [32.0, 164.0], [30.0, 176.0], [26.0, 184.0], [0.0, 186.0]], lb(106.0)),
        "wide" => can_std(s, &[[0.0, 34.0], [32.0, 34.0], [34.0, 40.0], [36.0, 48.0], [37.0, 58.0], [37.0, 162.0], [35.0, 176.0], [29.0, 184.0], [0.0, 186.0]], lb(110.0)),
        "shot" => {
            let mut r = can_std(s, &[[0.0, 58.0], [10.0, 58.0], [10.0, 66.0], [16.0, 74.0], [22.0, 84.0], [22.0, 166.0], [20.0, 176.0], [14.0, 182.0], [0.0, 184.0]], CanOpt { label: Some(124.0), no_rim: true, ..CanOpt::default() });
            r.body += &r_s(s, C - 10.0, 48.0, 20.0, 10.0, 3.0);
            r
        }
        "pouch" => R {
            body: r_s(s, C - 27.0, 44.0, 54.0, 140.0, 14.0) + &r_s(s, C - 5.0, 30.0, 10.0, 16.0, 3.0),
            det: l_s(s, C - 27.0, 56.0, C + 27.0, 56.0) + &l_s(s, C - 27.0, 150.0, C + 27.0, 150.0),
            hl: l_s(s, C - 19.0, 72.0, C - 19.0, 140.0),
            cy: Some(my(s, 104.0)),
            half: Some(27.0 * SW[s]),
            ..R::default()
        },
        "ridge" => can_std(s, &CAN_STD, CanOpt { label: Some(110.0), ridges: &[78.0, 88.0, 98.0, 108.0, 118.0], ..CanOpt::default() }),
        "band" => can_std(s, &CAN_STD, CanOpt { label: Some(113.0), bands: &[98.0, 130.0], ..CanOpt::default() }),
        "bottle" => {
            let mut r = can_std(s, &[[0.0, 24.0], [9.0, 24.0], [9.0, 36.0], [13.0, 44.0], [24.0, 56.0], [26.0, 68.0], [26.0, 168.0], [24.0, 178.0], [0.0, 182.0]], CanOpt { label: Some(112.0), no_rim: true, bands: &[98.0, 130.0], ..CanOpt::default() });
            r.body += &r_s(s, C - 10.0, 16.0, 20.0, 9.0, 2.0);
            r
        }
        "tab" => can_std(s, &CAN_STD, CanOpt { label: Some(110.0), tab: true, ..CanOpt::default() }),
        _ => return None,
    })
}

/* ---------- 커피 컵 ---------- */
fn cup_lid(s: usize, x: f64, y: f64, w: f64, h: f64) -> String {
    r_s(s, x, y, w, h, 3.0)
}
fn dome_lid(s: usize, w: f64, y: f64, peak: f64) -> String {
    path(s, &[('M', &[C - w, y]), ('Q', &[C, peak, C + w, y]), ('Z', &[])])
}
const TAKEAWAY: [Pt; 7] = [[0.0, 58.0], [34.0, 58.0], [32.0, 64.0], [27.0, 120.0], [23.0, 164.0], [21.0, 170.0], [0.0, 170.0]];
pub const CUP_KINDS: [&str; 10] = ["takeaway", "latte", "icedcup", "tumbler", "mug", "glassbottle", "sleeve", "straw", "big", "icetumbler"];
fn cup_of(kind: &str, s: usize) -> Option<R> {
    let mk = |body: String, det: String, hl: String, cy: f64, half: f64| R { body, det, hl, cy: Some(cy), half: Some(half), ..R::default() };
    Some(match kind {
        "takeaway" => {
            let r = prof(s, &TAKEAWAY);
            let cy = my(s, 110.0);
            mk(r.d + &cup_lid(s, C - 37.0, 48.0, 74.0, 10.0), l_s(s, C - 29.0, 98.0, C + 29.0, 98.0) + &l_s(s, C - 27.0, 124.0, C + 27.0, 124.0), line(C - 24.0, my(s, 74.0), C - 19.0, my(s, 150.0)), cy, half_at(&r.q, cy))
        }
        "latte" => {
            let r = prof(s, &[[0.0, 46.0], [28.0, 46.0], [27.0, 70.0], [24.0, 140.0], [22.0, 172.0], [0.0, 174.0]]);
            let cy = my(s, 110.0);
            mk(r.d + &dome_lid(s, 30.0, 46.0, 22.0), String::new(), line(C - 20.0, my(s, 80.0), C - 17.0, my(s, 150.0)), cy, half_at(&r.q, cy))
        }
        "icedcup" => {
            let r = prof(s, &[[0.0, 40.0], [32.0, 40.0], [28.0, 60.0], [24.0, 150.0], [22.0, 172.0], [0.0, 172.0]]);
            let cy = my(s, 110.0);
            mk(r.d + &dome_lid(s, 33.0, 40.0, 26.0) + &r_s(s, C + 6.0, 4.0, 6.0, 60.0, 3.0), String::new(), line(C - 18.0, my(s, 74.0), C - 15.0, my(s, 150.0)), cy, half_at(&r.q, cy))
        }
        "tumbler" => {
            let r = prof(s, &[[0.0, 30.0], [30.0, 30.0], [30.0, 40.0], [31.0, 160.0], [28.0, 176.0], [0.0, 178.0]]);
            let cy = my(s, 112.0);
            mk(r.d + &cup_lid(s, C - 31.0, 24.0, 62.0, 8.0), l_s(s, C - 29.0, 52.0, C + 29.0, 52.0), line(C - 20.0, my(s, 60.0), C - 19.0, my(s, 160.0)), cy, half_at(&r.q, cy))
        }
        "mug" => {
            let r = prof(s, &[[0.0, 60.0], [31.0, 60.0], [32.0, 70.0], [32.0, 170.0], [30.0, 180.0], [0.0, 180.0]]);
            let cy = my(s, 118.0);
            mk(r.d.clone(), path(s, &[('M', &[C + 31.0, 80.0]), ('C', &[C + 54.0, 80.0, C + 54.0, 128.0, C + 31.0, 128.0])]), line(C - 20.0, my(s, 78.0), C - 19.0, my(s, 160.0)), cy, half_at(&r.q, cy))
        }
        "glassbottle" => {
            let r = prof(s, &[[0.0, 30.0], [14.0, 30.0], [14.0, 44.0], [22.0, 52.0], [30.0, 64.0], [32.0, 80.0], [32.0, 170.0], [30.0, 180.0], [0.0, 182.0]]);
            let cy = my(s, 112.0);
            mk(r.d + &r_s(s, C - 15.0, 22.0, 30.0, 10.0, 2.0), l_s(s, C - 30.0, 130.0, C + 30.0, 130.0), line(C - 20.0, my(s, 84.0), C - 18.0, my(s, 160.0)), cy, half_at(&r.q, cy))
        }
        "sleeve" => {
            let r = prof(s, &TAKEAWAY);
            mk(
                r.d + &cup_lid(s, C - 37.0, 48.0, 74.0, 10.0),
                [92.0, 100.0, 108.0, 116.0, 124.0].iter().map(|y| l_s(s, C - 30.0, *y, C + 30.0, *y)).collect(),
                line(C - 24.0, my(s, 74.0), C - 19.0, my(s, 88.0)),
                my(s, 146.0),
                22.0 * SW[s],
            )
        }
        "straw" => {
            let r = prof(s, &TAKEAWAY);
            let cy = my(s, 110.0);
            mk(r.d + &cup_lid(s, C - 37.0, 48.0, 74.0, 10.0) + &r_s(s, C + 8.0, 2.0, 6.0, 52.0, 3.0), String::new(), line(C - 24.0, my(s, 74.0), C - 19.0, my(s, 150.0)), cy, half_at(&r.q, cy))
        }
        "big" => {
            let r = prof(s, &[[0.0, 40.0], [38.0, 40.0], [36.0, 48.0], [30.0, 120.0], [26.0, 170.0], [0.0, 172.0]]);
            let cy = my(s, 112.0);
            mk(r.d + &cup_lid(s, C - 41.0, 30.0, 82.0, 10.0), l_s(s, C - 33.0, 100.0, C + 33.0, 100.0), line(C - 26.0, my(s, 70.0), C - 22.0, my(s, 150.0)), cy, half_at(&r.q, cy))
        }
        "icetumbler" => {
            let r = prof(s, &[[0.0, 36.0], [30.0, 36.0], [29.0, 50.0], [28.0, 150.0], [26.0, 178.0], [0.0, 178.0]]);
            let cy = my(s, 112.0);
            mk(r.d + &dome_lid(s, 30.0, 36.0, 24.0), l_s(s, C - 28.0, 60.0, C + 28.0, 60.0), line(C - 20.0, my(s, 80.0), C - 18.0, my(s, 160.0)), cy, half_at(&r.q, cy))
        }
        _ => return None,
    })
}

/* ---------- 에너지바 포장 ---------- */
pub const WRAP_KINDS: [&str; 10] = ["flat", "twist", "foil", "carton", "stick", "kraft", "window", "tube", "pillow", "bar"];
fn wrap_of(kind: &str, s: usize) -> Option<R> {
    Some(match kind {
        "flat" => {
            let b = bx(40.0, 84.0);
            R {
                body: path(s, &[('M', &[b[0], b[1]]), ('W', &[b[2], b[1], -3.0, 8.0]), ('L', &[b[2], b[3]]), ('W', &[b[0], b[3], 3.0, 8.0]), ('Z', &[])]),
                det: l_s(s, b[0] + 9.0, b[1] + 3.0, b[0] + 9.0, b[3] - 3.0) + &l_s(s, b[2] - 9.0, b[1] + 3.0, b[2] - 9.0, b[3] - 3.0),
                cy: Some(my(s, 100.0)),
                half: Some(40.0 * SW[s] - 6.0),
                ..R::default()
            }
        }
        "twist" => {
            let b = bx(40.0, 70.0);
            bd(
                path(s, &[('M', &[b[0], 100.0]), ('L', &[b[0] + 12.0, b[1]]), ('L', &[b[2] - 12.0, b[1]]), ('L', &[b[2], 100.0]), ('L', &[b[2] - 12.0, b[3]]), ('L', &[b[0] + 12.0, b[3]]), ('Z', &[])]),
                l_s(s, b[0] + 12.0, 100.0, b[2] - 12.0, 100.0),
            )
        }
        "foil" => {
            let b = bx(34.0, 96.0);
            bd(
                path(s, &[('M', &[b[0], b[1]]), ('L', &[C - 6.0, b[1]]), ('L', &[C, b[1] + 8.0]), ('L', &[C + 6.0, b[1]]), ('L', &[b[2], b[1]]), ('L', &[b[2], b[3]]), ('L', &[b[0], b[3]]), ('Z', &[])]),
                l_s(s, b[0] + 4.0, b[3] - 6.0, b[2] - 4.0, b[3] - 6.0),
            )
        }
        "carton" => {
            let b = bx(36.0, 70.0);
            bd(r_s(s, b[0], b[1], 72.0, 70.0, 2.0) + &r_s(s, b[0] - 1.0, b[1] - 10.0, 74.0, 10.0, 2.0), l_s(s, b[0], b[1] + 8.0, b[2], b[1] + 8.0))
        }
        "stick" => {
            let b = bx(18.0, 120.0);
            bd(r_s(s, b[0], b[1], 36.0, 120.0, 9.0), l_s(s, b[0] + 4.0, b[1] + 10.0, b[2] - 4.0, b[1] + 10.0))
        }
        "kraft" => {
            let b = bx(40.0, 80.0);
            bd(
                path(s, &[('M', &[b[0], b[1]]), ('L', &[b[2] - 10.0, b[1]]), ('L', &[b[2], b[1] + 10.0]), ('L', &[b[2], b[3]]), ('L', &[b[0], b[3]]), ('Z', &[])]),
                l_s(s, b[2] - 10.0, b[1], b[2] - 10.0, b[1] + 10.0) + &l_s(s, b[2] - 10.0, b[1] + 10.0, b[2], b[1] + 10.0),
            )
        }
        "window" => {
            let b = bx(40.0, 84.0);
            R { body: r_s(s, b[0], b[1], 80.0, 84.0, 4.0), win: r_s(s, C - 28.0, 84.0, 56.0, 32.0, 3.0), ext: r_s(s, C - 22.0, 92.0, 44.0, 16.0, 6.0), ..R::default() }
        }
        "tube" => {
            let b = bx(20.0, 100.0);
            bd(r_s(s, b[0], b[1], 40.0, 100.0, 20.0), l_s(s, b[0] + 6.0, b[1] + 4.0, b[2] - 6.0, b[1] + 4.0) + &l_s(s, b[0] + 6.0, b[3] - 4.0, b[2] - 6.0, b[3] - 4.0))
        }
        "pillow" => {
            let b = bx(40.0, 84.0);
            bd(pillow_s(s, b), l_s(s, b[0] + 4.0, b[1] + 8.0, b[2] - 4.0, b[1] + 8.0))
        }
        "bar" => {
            let b = bx(40.0, 84.0);
            R { body: r_s(s, b[0], b[1], 80.0, 84.0, 5.0), ext: r_s(s, b[0] + 10.0, b[1] + 18.0, 60.0, 48.0, 8.0), ..R::default() }
        }
        _ => return None,
    })
}

/// 모음(family)의 종류 이름 목록 (선택 칸 순서). 봉지는 상품마다 달라서 따로 받아요
pub fn kinds(family: &str) -> &'static [&'static str] {
    match family {
        "bottle" => &BOTTLE_KINDS,
        "cap" => &CAP_KINDS,
        "box" => &BOX_KINDS,
        "meat" => &MEAT_KINDS,
        "can" => &CAN_KINDS,
        "cup" => &CUP_KINDS,
        "wrap" => &WRAP_KINDS,
        _ => &BAG_CHIPS,
    }
}

/// 한 칸(종류 × 크기)의 그림. 없는 종류면 그 모음의 첫 종류로 그려요
pub fn body(family: &str, kind: &str, s: usize) -> Body {
    let s = s.min(4);
    let pick = |k: &str| match family {
        "bottle" => bottle_of(k, s),
        "bag" => bag_of(k, s),
        "box" => box_of(k, s),
        "meat" => meat_of(k, s),
        "can" => can_of(k, s),
        "cup" => cup_of(k, s),
        "wrap" => wrap_of(k, s),
        _ => None,
    };
    let first = match family {
        "bag" => BAG_ALL[0],
        _ => kinds(family)[0],
    };
    let r = pick(kind).or_else(|| pick(first)).unwrap_or_default();
    let meat = family == "meat";
    Body {
        body: if r.body.is_empty() { r.tray } else { r.body },
        ext: r.ext,
        det: r.det,
        hl: r.hl,
        win: r.win,
        xf: if meat { "translate(60 100) scale(1.3) translate(-60 -100)".into() } else { String::new() },
        cy: r.cy.unwrap_or(100.0),
        half: r.half.unwrap_or(if meat { 26.0 } else { 30.0 }),
        top: r.top.unwrap_or(0.0),
        nr: r.nr.unwrap_or(0.0),
    }
}
