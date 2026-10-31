/* 상품 그림 도형. 사진이 아니라 검은 윤곽선 SVG예요 (흰 바탕, 색은 고른 색으로 채워요).
   가운데 x=60, 세로는 0~200. 종류(kind)마다 크기 5단계(s=0~4)를 그려요.
   결과: { body, ext, det, hl, win, tray, cy, half, top, nr }. body 는 색을 채우는 몸통,
   ext 는 고기처럼 따로 채우는 안쪽, det 은 이음새·라벨 선, hl 은 유리 빛, win 은 창이에요. */
(() => {
  "use strict";

  const C = 60;
  const SW = [0.8, 0.9, 1, 1.08, 1.16];
  const SH = [0.84, 0.93, 1, 1.07, 1.12];
  const CW = [0.9, 0.95, 1, 1.05, 1.1]; // 뚜껑 크기
  const CH = [0.85, 0.93, 1, 1.07, 1.14];
  const f = (n) => Math.round(n * 10) / 10;
  const mx = (s, x) => C + (x - C) * SW[s];
  const my = (s, y) => 100 + (y - 100) * SH[s];
  const P = (s, x, y) => [mx(s, x), my(s, y)];

  // 점들을 부드럽게 이어 닫힌 모양을 만들어요 (닫힌 Catmull-Rom → 3차 베지에)
  const closed = (pts) => {
    const Q = pts.filter((p, i) => i === 0 || Math.hypot(p[0] - pts[i - 1][0], p[1] - pts[i - 1][1]) > 0.3);
    if (Q.length > 2 && Math.hypot(Q[0][0] - Q[Q.length - 1][0], Q[0][1] - Q[Q.length - 1][1]) < 0.3) Q.pop();
    const n = Q.length;
    let d = `M${f(Q[0][0])} ${f(Q[0][1])}`;
    for (let i = 0; i < n; i++) {
      const p0 = Q[(i - 1 + n) % n], p1 = Q[i], p2 = Q[(i + 1) % n], p3 = Q[(i + 2) % n];
      d += `C${f(p1[0] + (p2[0] - p0[0]) / 6)} ${f(p1[1] + (p2[1] - p0[1]) / 6)} ${f(p2[0] - (p3[0] - p1[0]) / 6)} ${f(p2[1] - (p3[1] - p1[1]) / 6)} ${f(p2[0])} ${f(p2[1])}`;
    }
    return d + "Z";
  };
  // 오른쪽 옆선(절대 좌표, 위→아래)을 왼쪽에 대칭으로 붙여요
  const sym = (R) => closed([...R, ...R.slice(0, -1).reverse().map(([x, y]) => [2 * C - x, y])]);
  // 가운데에서 떨어진 거리(d)와 세로(y)로 적은 옆선을 크기에 맞춰요
  const prof = (s, R) => {
    const Q = R.map(([d, y]) => [C + d * SW[s], my(s, y)]);
    return { d: sym(Q), Q };
  };
  // 세로 y에서 옆선까지의 반지름
  const halfAt = (Q, y) => {
    for (let i = 0; i < Q.length - 1; i++) {
      const [x0, y0] = Q[i], [x1, y1] = Q[i + 1];
      if ((y0 <= y && y <= y1) || (y1 <= y && y <= y0)) {
        const t = y1 === y0 ? 0 : (y - y0) / (y1 - y0);
        return Math.abs(x0 + (x1 - x0) * t - C);
      }
    }
    return 18;
  };

  // 기본 도형들 (절대 좌표)
  const rrA = (x, y, w, h, r) => {
    r = Math.max(0, Math.min(r, w / 2, h / 2));
    const L = f(x), T = f(y), R = f(x + w), B = f(y + h);
    const a = f(x + r), b = f(y + r), c = f(x + w - r), e = f(y + h - r);
    return `M${a} ${T}H${c}Q${R} ${T} ${R} ${b}V${e}Q${R} ${B} ${c} ${B}H${a}Q${L} ${B} ${L} ${e}V${b}Q${L} ${T} ${a} ${T}Z`;
  };
  const ell = (x, y, rx, ry) => `M${f(x - rx)} ${f(y)}A${f(rx)} ${f(ry)} 0 1 0 ${f(x + rx)} ${f(y)}A${f(rx)} ${f(ry)} 0 1 0 ${f(x - rx)} ${f(y)}Z`;
  const line = (x1, y1, x2, y2) => `M${f(x1)} ${f(y1)}L${f(x2)} ${f(y2)}`;
  // 물결(실링 자국): 가로 방향으로 n번 굽어요
  const wave = (x1, x2, y, amp, n) => {
    const st = (x2 - x1) / n;
    let o = "";
    for (let i = 0; i < n; i++) o += `Q${f(x1 + st * i + st / 2)} ${f(y - amp)} ${f(x1 + st * (i + 1))} ${f(y)}`;
    return o;
  };
  // 명령 목록(기준 좌표)을 크기에 맞춰 path 로 바꿔요. W 는 현재 점에서 가로 물결
  const path = (s, cmds) => {
    let out = "";
    let cur = null;
    for (const c of cmds) {
      const op = c[0];
      if (op === "Z") {
        out += "Z";
        continue;
      }
      if (op === "W") {
        const b = P(s, c[1], c[2]);
        out += wave(cur[0], b[0], cur[1], c[3] * SH[s], c[4]);
        cur = b;
        continue;
      }
      const pts = [];
      for (let i = 1; i < c.length; i += 2) pts.push(P(s, c[i], c[i + 1]));
      out += op + pts.map((p) => `${f(p[0])} ${f(p[1])}`).join(" ");
      cur = pts[pts.length - 1];
    }
    return out;
  };
  // 기준 좌표의 사각형 (왼쪽 위와 오른쪽 아래)
  const box = (w, h) => [C - w, 100 - h / 2, C + w, 100 + h / 2];
  const rS = (s, x, y, w, h, r) => {
    const a = P(s, x, y);
    return rrA(a[0], a[1], w * SW[s], h * SH[s], r * Math.min(SW[s], SH[s]));
  };
  const eS = (s, x, y, rx, ry) => {
    const a = P(s, x, y);
    return ell(a[0], a[1], rx * SW[s], ry * SH[s]);
  };
  const lS = (s, x1, y1, x2, y2) => {
    const a = P(s, x1, y1), b = P(s, x2, y2);
    return line(a[0], a[1], b[0], b[1]);
  };
  const sS = (s, pts) => sym(pts.map(([x, y]) => P(s, x, y))); // 대칭 모양 (기준 좌표의 오른쪽 옆선)
  const cS = (s, pts) => closed(pts.map(([x, y]) => P(s, x, y))); // 대칭이 아닌 닫힌 모양

  /* ---------- 페트병 (물) ---------- */
  const bottle = (s, R, o = {}) => {
    const { d, Q } = prof(s, R);
    const cy = my(s, o.label ?? 128);
    const half = halfAt(Q, cy);
    let det = "";
    if (o.ribs) det += o.ribs.map((x) => lS(s, C + x, 92, C + x, 168)).join("");
    if (o.rings) {
      det += o.rings
        .map((y) => {
          const yy = my(s, y), h = halfAt(Q, yy);
          return `M${f(C - h + 1)} ${f(yy)}Q${C} ${f(yy + 3)} ${f(C + h - 1)} ${f(yy)}`;
        })
        .join("");
    }
    if (o.labelRect) det += rS(s, C - 30, 96, 60, 52, 4);
    const hx = C - halfAt(Q, my(s, 110)) * 0.6;
    return { body: d, det, hl: line(hx, my(s, 84), hx, my(s, 164)), cy, half, top: Q[0][1], nr: Q[1][0] - C };
  };
  const BOTTLE = {
    classic: (s) => bottle(s, [[0, 26], [12, 26], [12, 46], [16, 52], [30, 62], [33, 80], [33, 172], [31, 182], [26, 188], [0, 188]], { label: 128 }),
    square: (s) => bottle(s, [[0, 26], [12, 26], [12, 44], [21, 50], [32, 56], [34, 66], [34, 172], [33, 181], [29, 186], [0, 186]], { label: 128, labelRect: true }),
    rib: (s) => bottle(s, [[0, 26], [12, 26], [12, 46], [16, 52], [30, 62], [33, 80], [33, 172], [31, 182], [26, 188], [0, 188]], { label: 128, ribs: [-24, -12, 12, 24] }),
    slim: (s) => bottle(s, [[0, 24], [11, 24], [11, 40], [15, 58], [25, 70], [26, 86], [26, 176], [24, 186], [0, 186]], { label: 130 }),
    waist: (s) => bottle(s, [[0, 28], [12, 28], [12, 48], [24, 60], [34, 76], [34, 92], [26, 112], [27, 140], [34, 156], [34, 176], [31, 186], [0, 186]], { label: 140 }),
    sport: (s) => bottle(s, [[0, 22], [9, 22], [9, 40], [14, 46], [28, 56], [32, 70], [32, 176], [30, 186], [0, 186]], { label: 124 }),
    flat: (s) => bottle(s, [[0, 46], [16, 46], [24, 52], [40, 62], [42, 78], [42, 150], [40, 164], [34, 170], [0, 170]], { label: 110 }),
    ring: (s) => bottle(s, [[0, 26], [12, 26], [12, 46], [16, 52], [30, 62], [32, 80], [32, 172], [30, 182], [26, 188], [0, 188]], { label: 128, rings: [96, 108, 120, 132, 144, 156] }),
    sparkle: (s) => bottle(s, [[0, 26], [11, 26], [11, 46], [17, 56], [30, 68], [34, 96], [33, 120], [27, 160], [24, 182], [0, 184]], { label: 110 }),
    jug: (s) => bottle(s, [[0, 24], [18, 24], [18, 34], [24, 42], [34, 52], [35, 70], [35, 172], [33, 184], [0, 184]], { label: 120 }),
  };

  /* ---------- 뚜껑 (페트병 목 위에 얹혀요, 절대 좌표) ---------- */
  const screwRidges = (w, y, h) => [-0.6, -0.3, 0, 0.3, 0.6].map((q) => line(C + q * w, y + 3, C + q * w, y + h - 3)).join("");
  const CAP = {
    screw: (nr, T, s) => {
      const w = (nr + 2.5) * CW[s], h = 15 * CH[s], y = T - 13 * CH[s];
      return { body: rrA(C - w, y, 2 * w, h, 3), det: screwRidges(w, y, h) };
    },
    flip: (nr, T, s) => {
      const w = (nr + 3) * CW[s], y = T - 10 * CH[s], h = 12 * CH[s];
      return { body: rrA(C - w, y, 2 * w, h, 3) + rrA(C - w - 1, y - 5 * CH[s], 9 * CW[s], 7 * CH[s], 2), det: line(C - w + 2, y + 3, C + w - 2, y + 3) };
    },
    dome: (nr, T, s) => {
      const w = (nr + 3) * CW[s], yb = T + 2, yt = T - 18 * CH[s];
      return { body: sym([[C, yt], [C + w * 0.5, yt], [C + w * 0.92, yt + 4 * CH[s]], [C + w, yt + 9 * CH[s]], [C + w, yb], [C, yb]]), det: "" };
    },
    ring: (nr, T, s) => {
      const b = CAP.screw(nr, T, s), w = (nr + 3.8) * CW[s], y = T + 4 * CH[s];
      return { body: b.body, det: b.det + rrA(C - w, y, 2 * w, 3.6 * CH[s], 1.5) };
    },
    tall: (nr, T, s) => {
      const w = (nr + 2) * CW[s], h = 24 * CH[s], y = T - 22 * CH[s];
      return { body: rrA(C - w, y, 2 * w, h, 3), det: screwRidges(w, y, h) };
    },
    flat: (nr, T, s) => {
      const w = (nr + 5) * CW[s], h = 9 * CH[s], y = T - 7 * CH[s];
      return { body: rrA(C - w, y, 2 * w, h, 3), det: "" };
    },
    knob: (nr, T, s) => {
      const w = (nr + 2.5) * CW[s], h = 12 * CH[s], y = T - 10 * CH[s], r = 5.5 * CW[s];
      return { body: rrA(C - w, y, 2 * w, h, 3) + ell(C, y - 1, r, r), det: "" };
    },
    click: (nr, T, s) => {
      const b = CAP.screw(nr, T, s), w = (nr + 2.5) * CW[s], y = T - 6 * CH[s];
      return { body: b.body, det: line(C - w, y, C + w, y) };
    },
    cone: (nr, T, s) => {
      const w = (nr + 1) * CW[s], w2 = (nr + 4) * CW[s], yt = T - 14 * CH[s], yb = T + 2;
      return { body: sym([[C, yt], [C + w, yt], [C + w2, yb], [C, yb]]), det: "" };
    },
    square: (nr, T, s) => {
      const w = (nr + 3) * CW[s], h = 14 * CH[s], y = T - 12 * CH[s];
      return { body: rrA(C - w, y, 2 * w, h, 2), det: "" };
    },
  };

  /* ---------- 과자·젤리·빵 봉지 ---------- */
  const pillowS = (s, b) => path(s, [["M", b[0], b[1]], ["W", b[2], b[1], 3, 6], ["L", b[2], b[3]], ["W", b[0], b[3], -3, 6], ["Z"]]);
  // 캔처럼 생긴 통 (뚜껑 타원 포함)
  const tubeS = (s, w, h, rim) => {
    const b = box(w, h);
    return { body: sS(s, [[C, b[1]], [b[2], b[1] + rim], [b[2], b[3] - rim], [C, b[3]]]), top: b[1] + rim };
  };
  const BAG = {
    pillow: (s) => { const b = box(30, 110); return { body: pillowS(s, b), det: lS(s, b[0] + 4, b[1] + 9, b[2] - 4, b[1] + 9) }; },
    standup: (s) => {
      const b = box(28, 118);
      return { body: path(s, [["M", b[0], b[1]], ["W", b[2], b[1], 3, 6], ["L", b[2], b[3]], ["Q", b[2], b[3] + 9, b[2] - 6, b[3] + 9], ["L", b[0] + 6, b[3] + 9], ["Q", b[0], b[3] + 9, b[0], b[3]], ["Z"]]), det: lS(s, b[0] + 4, b[1] + 12, b[2] - 4, b[1] + 12) };
    },
    tube: (s) => { const t = tubeS(s, 22, 96, 6); return { body: t.body, det: eS(s, C, t.top, 21.5, 4.5) }; },
    window: (s) => { const b = box(30, 110); return { body: pillowS(s, b), win: rS(s, C - 18, 84, 36, 34, 6), det: lS(s, b[0] + 4, b[1] + 9, b[2] - 4, b[1] + 9) }; },
    big: (s) => {
      const b = box(34, 128);
      return { body: path(s, [["M", b[0], b[1]], ["W", b[2], b[1], 3, 6], ["L", b[2], b[3]], ["Q", b[2], b[3] + 9, b[2] - 6, b[3] + 9], ["L", b[0] + 6, b[3] + 9], ["Q", b[0], b[3] + 9, b[0], b[3]], ["Z"]]), det: lS(s, b[0] + 4, b[1] + 14, b[2] - 4, b[1] + 14) + lS(s, b[0] + 4, b[1] + 8, b[2] - 4, b[1] + 8) };
    },
    mini: (s) => { const b = box(22, 80); return { body: pillowS(s, b), det: lS(s, b[0] + 3, b[1] + 8, b[2] - 3, b[1] + 8) }; },
    kraft: (s) => {
      const b = box(30, 112);
      return { body: path(s, [["M", b[0], b[1]], ["L", b[2], b[1]], ["L", C + 33, b[3]], ["L", C - 33, b[3]], ["Z"]]), det: lS(s, b[0], b[1] + 10, b[2], b[1] + 10) };
    },
    canister: (s) => { const t = tubeS(s, 22, 92, 5); return { body: t.body, det: eS(s, C, t.top, 21, 3.6) + lS(s, C - 22, t.top + 8, C + 22, t.top + 8) }; },
    sachet: (s) => {
      const b = box(30, 92);
      return { body: rrA(...P(s, b[0], b[1]), 2 * 30 * SW[s], 92 * SH[s], 6), det: line(...P(s, C - 6, b[1]), ...P(s, C, b[1] + 8)) + line(...P(s, C, b[1] + 8), ...P(s, C + 6, b[1])) + lS(s, b[0] + 3, b[3] - 6, b[2] - 3, b[3] - 6) };
    },
    tri: (s) => {
      const b = box(30, 110);
      return { body: path(s, [["M", b[0], b[1] + 22], ["L", C, b[1]], ["L", b[2], b[1] + 22], ["L", b[2], b[3]], ["L", b[0], b[3]], ["Z"]]), det: lS(s, b[0], b[1] + 22, b[2], b[1] + 22) };
    },
    stick: (s) => { const b = box(16, 124); return { body: pillowS(s, b), det: lS(s, b[0] + 3, b[1] + 8, b[2] - 3, b[1] + 8) }; },
    cup: (s) => {
      const b = box(26, 100);
      const cupBody = sS(s, [[C, b[1] + 14], [b[2], b[1] + 14], [C + 26 * 0.9, b[3]], [C, b[3]]]);
      return { body: cupBody + rS(s, b[0] - 3, b[1] + 6, 58, 9, 3), det: "" };
    },
    tub: (s) => { const b = box(30, 70); return { body: rS(s, b[0] - 4, b[1] + 20, 68, b[3] - b[1] - 20, 6) + rS(s, b[0] - 6, b[1] + 10, 72, 12, 4), det: lS(s, b[0], b[1] + 40, b[2], b[1] + 40) }; },
    blister: (s) => {
      const b = box(30, 110);
      const dots = [-0.5, 0, 0.5].map((q) => eS(s, C + q * 54, b[1] + 30, 6.5, 6.5)).join("") + [-0.5, 0, 0.5].map((q) => eS(s, C + q * 54, b[3] - 30, 6.5, 6.5)).join("");
      return { body: rS(s, b[0], b[1] + 10, 60, 90, 8), det: dots };
    },
    tall: (s) => {
      const b = box(26, 150);
      return { body: pillowS(s, b), det: eS(s, C, b[1] + 10, 4, 3) + lS(s, C, b[1] + 13, C, b[1] + 22) };
    },
    bakery: (s) => {
      const b = box(36, 96);
      return { body: rS(s, b[0], b[1] + 22, 72, b[3] - b[1] - 22, 3) + path(s, [["M", b[0], b[1] + 22], ["L", C, b[1]], ["L", b[2], b[1] + 22], ["Z"]]), det: "" };
    },
    round: (s) => { const t = tubeS(s, 26, 64, 8); return { body: t.body, det: eS(s, C, t.top, 25.5, 4.6) }; },
    wedge: (s) => { const b = box(34, 86); return { body: path(s, [["M", b[0], b[3]], ["L", b[2], b[3]], ["L", b[2], b[1]], ["Z"]]), det: lS(s, b[0] + 4, b[3] - 8, b[2] - 6, b[3] - 8) }; },
    roll: (s) => { const b = box(24, 84); return { body: rS(s, b[0], b[1], 48, 84, 8), det: lS(s, b[0] + 4, b[1] + 8, b[2] - 4, b[1] + 8) + lS(s, b[0] + 4, b[1] + 14, b[2] - 4, b[1] + 14) }; },
  };

  /* ---------- 도시락 용기 ---------- */
  const BOX = {
    bento: (s) => { const b = box(38, 70); return { body: rS(s, b[0], b[1], 76, 70, 6), det: lS(s, b[0] + 4, b[1] + 22, b[2] - 4, b[1] + 22) }; },
    round: (s) => { const t = tubeS(s, 30, 80, 8); return { body: t.body, det: eS(s, C, t.top, 29.5, 5) }; },
    kraft: (s) => { const b = box(38, 70); return { body: rS(s, b[0], b[1], 76, 70, 3) + path(s, [["M", b[0], b[1]], ["L", C, b[1] - 16], ["L", b[2], b[1]], ["Z"]]), det: "" }; },
    wedge: (s) => { const b = box(40, 70); return { body: path(s, [["M", b[0], b[3]], ["L", b[2], b[3]], ["L", b[2], b[1]], ["Z"]]), det: lS(s, b[0] + 4, b[3] - 10, b[2] - 10, b[3] - 10) }; },
    twotier: (s) => { const b = box(36, 84); return { body: rS(s, b[0], b[1], 72, 38, 5) + rS(s, b[0], b[1] + 44, 72, 40, 5), det: "" }; },
    bowl: (s) => {
      const b = box(38, 80);
      return { body: sS(s, [[C, b[1] + 24], [b[2], b[1] + 24], [C + 32, b[3] - 16], [C + 19, b[3]], [C, b[3]]]) + path(s, [["M", b[0] - 2, b[1] + 24], ["Q", C, b[1] - 6, b[2] + 2, b[1] + 24], ["Z"]]), det: "" };
    },
    tall: (s) => { const b = box(24, 100); return { body: rS(s, b[0], b[1], 48, 100, 4), det: lS(s, b[0] + 4, b[1] + 14, b[2] - 4, b[1] + 14) }; },
    microwave: (s) => { const b = box(40, 64); return { body: rS(s, b[0], b[1], 80, 64, 10), det: lS(s, C - 12, b[1] + 8, C + 12, b[1] + 8) + lS(s, C - 12, b[1] + 13, C + 12, b[1] + 13) }; },
    tray: (s) => { const b = box(44, 50); return { body: rS(s, b[0] - 4, b[1] + 15, 96, 26, 6), det: lS(s, C - 14, b[1] + 15, C - 14, b[1] + 41) + lS(s, C + 14, b[1] + 15, C + 14, b[1] + 41) }; },
    bento2: (s) => { const b = box(38, 70); return { body: rS(s, b[0], b[1], 76, 70, 6), det: lS(s, C - 13, b[1] + 4, C - 13, b[3] - 4) + lS(s, C + 13, b[1] + 4, C + 13, b[3] - 4) }; },
  };

  /* ---------- 고기 (트레이 위에 고기 모양) ---------- */
  const tray = (s) => rS(s, C - 50, 44, 100, 112, 8);
  const MEAT_COST = { steak: 6, samgyeop: 4, galbi: 5, drum: 3, sausage: 2, ham: 2, bacon: 3, cubes: 3, meatball: 2, breast: 3 };
  const MEAT = {
    steak: (s) => ({ tray: tray(s), ext: cS(s, [[C - 28, 94], [C - 22, 83], [C - 4, 78], [C + 16, 81], [C + 28, 94], [C + 26, 110], [C + 12, 120], [C - 10, 118], [C - 26, 108]]), det: lS(s, C - 12, 96, C - 4, 92) + lS(s, C + 6, 104, C + 14, 100) + lS(s, C - 4, 110, C + 4, 108) }),
    samgyeop: (s) => ({ tray: tray(s), ext: rS(s, C - 26, 74, 52, 16, 4) + rS(s, C - 26, 96, 52, 16, 4) + rS(s, C - 26, 118, 52, 16, 4), det: lS(s, C - 20, 82, C + 20, 82) + lS(s, C - 20, 104, C + 20, 104) + lS(s, C - 20, 126, C + 20, 126) }),
    galbi: (s) => ({ tray: tray(s), ext: rS(s, C - 32, 88, 64, 24, 11) + eS(s, C + 32, 100, 9, 9), det: eS(s, C + 32, 100, 3, 3) + lS(s, C - 22, 100, C + 18, 100) }),
    drum: (s) => ({ tray: tray(s), ext: eS(s, C - 6, 100, 26, 20) + rS(s, C + 14, 97, 18, 6, 3) + eS(s, C + 34, 94, 5, 5) + eS(s, C + 34, 106, 5, 5), det: eS(s, C - 6, 100, 14, 10) }),
    sausage: (s) => ({ tray: tray(s), ext: rS(s, C - 32, 78, 64, 12, 6) + rS(s, C - 32, 96, 64, 12, 6) + rS(s, C - 32, 114, 64, 12, 6), det: lS(s, C - 10, 78, C - 10, 90) + lS(s, C + 10, 96, C + 10, 108) + lS(s, C - 6, 114, C - 6, 126) }),
    ham: (s) => ({ tray: tray(s), ext: eS(s, C, 100, 34, 26), det: eS(s, C, 100, 22, 16) + eS(s, C, 100, 6, 5) }),
    bacon: (s) => ({ tray: tray(s), ext: rS(s, C - 32, 72, 64, 8, 4) + rS(s, C - 32, 88, 64, 8, 4) + rS(s, C - 32, 104, 64, 8, 4) + rS(s, C - 32, 120, 64, 8, 4), det: lS(s, C - 24, 76, C + 24, 76) + lS(s, C - 24, 92, C + 24, 92) + lS(s, C - 24, 108, C + 24, 108) }),
    cubes: (s) => {
      let ext = "";
      for (let i = 0; i < 3; i++) for (let j = 0; j < 2; j++) ext += rS(s, C - 30 + i * 20 + 1, 76 + j * 22, 18, 18, 4);
      return { tray: tray(s), ext, det: "" };
    },
    meatball: (s) => ({ tray: tray(s), ext: eS(s, C - 16, 88, 12, 12) + eS(s, C + 12, 86, 12, 12) + eS(s, C - 4, 112, 13, 13) + eS(s, C + 20, 110, 11, 11), det: lS(s, C - 20, 84, C - 14, 82) + lS(s, C + 8, 82, C + 14, 80) }),
    breast: (s) => ({ tray: tray(s), ext: eS(s, C, 100, 32, 22), det: path(s, [["M", C - 18, 96], ["Q", C, 104, C + 18, 96]]) }),
  };

  /* ---------- 에너지 음료 캔 ---------- */
  const canStd = (s, R, o = {}) => {
    const { d, Q } = prof(s, R);
    let det = o.rim === false ? "" : `M${f(C - R[1][0] * SW[s] + 1)} ${f(my(s, 42))}Q${C} ${f(my(s, 48))} ${f(C + R[1][0] * SW[s] - 1)} ${f(my(s, 42))}`;
    if (o.tab) det += rS(s, C - 6, 28, 12, 7, 2);
    if (o.ridges) det += o.ridges.map((y) => `M${f(C - halfAt(Q, my(s, y)) + 1)} ${f(my(s, y))}Q${C} ${f(my(s, y) + 3)} ${f(C + halfAt(Q, my(s, y)) - 1)} ${f(my(s, y))}`).join("");
    if (o.bands) det += o.bands.map((y) => lS(s, C - 31, y, C + 31, y)).join("");
    return { body: d, det, hl: line(C - halfAt(Q, my(s, 110)) * 0.6, my(s, 80), C - halfAt(Q, my(s, 110)) * 0.6, my(s, 160)), cy: my(s, o.label ?? 110), half: halfAt(Q, my(s, o.label ?? 110)) };
  };
  const CAN = {
    standard: (s) => canStd(s, [[0, 36], [25, 36], [27, 40], [29, 46], [31, 54], [31, 164], [30, 174], [27, 181], [0, 183]], { label: 110 }),
    slim: (s) => canStd(s, [[0, 36], [19, 36], [21, 40], [22, 46], [23, 54], [23, 166], [22, 176], [20, 182], [0, 184]], { label: 112 }),
    tall: (s) => canStd(s, [[0, 30], [21, 30], [23, 34], [25, 40], [27, 46], [30, 54], [32, 164], [30, 176], [26, 184], [0, 186]], { label: 106 }),
    wide: (s) => canStd(s, [[0, 34], [32, 34], [34, 40], [36, 48], [37, 58], [37, 162], [35, 176], [29, 184], [0, 186]], { label: 110 }),
    shot: (s) => { const r = canStd(s, [[0, 58], [10, 58], [10, 66], [16, 74], [22, 84], [22, 166], [20, 176], [14, 182], [0, 184]], { label: 124, rim: false }); return { ...r, body: r.body + rS(s, C - 10, 48, 20, 10, 3) }; },
    pouch: (s) => ({ body: rS(s, C - 27, 44, 54, 140, 14) + rS(s, C - 5, 30, 10, 16, 3), det: lS(s, C - 27, 56, C + 27, 56) + lS(s, C - 27, 150, C + 27, 150), hl: lS(s, C - 19, 72, C - 19, 140), cy: my(s, 104), half: 27 * SW[s] }),
    ridge: (s) => canStd(s, [[0, 36], [25, 36], [27, 40], [29, 46], [31, 54], [31, 164], [30, 174], [27, 181], [0, 183]], { label: 110, ridges: [78, 88, 98, 108, 118] }),
    band: (s) => canStd(s, [[0, 36], [25, 36], [27, 40], [29, 46], [31, 54], [31, 164], [30, 174], [27, 181], [0, 183]], { label: 113, bands: [98, 130] }),
    bottle: (s) => { const r = canStd(s, [[0, 24], [9, 24], [9, 36], [13, 44], [24, 56], [26, 68], [26, 168], [24, 178], [0, 182]], { label: 112, rim: false, bands: [98, 130] }); return { ...r, body: r.body + rS(s, C - 10, 16, 20, 9, 2) }; },
    tab: (s) => canStd(s, [[0, 36], [25, 36], [27, 40], [29, 46], [31, 54], [31, 164], [30, 174], [27, 181], [0, 183]], { label: 110, tab: true }),
  };

  /* ---------- 커피 컵 ---------- */
  const cupLid = (s, x, y, w, h) => rS(s, x, y, w, h, 3);
  const CUP = {
    takeaway: (s) => {
      const r = prof(s, [[0, 58], [34, 58], [32, 64], [27, 120], [23, 164], [21, 170], [0, 170]]);
      return { body: r.d + cupLid(s, C - 37, 48, 74, 10), det: lS(s, C - 29, 98, C + 29, 98) + lS(s, C - 27, 124, C + 27, 124), hl: line(C - 24, my(s, 74), C - 19, my(s, 150)), cy: my(s, 110), half: halfAt(r.Q, my(s, 110)) };
    },
    latte: (s) => {
      const r = prof(s, [[0, 46], [28, 46], [27, 70], [24, 140], [22, 172], [0, 174]]);
      return { body: r.d + path(s, [["M", C - 30, 46], ["Q", C, 22, C + 30, 46], ["Z"]]), det: "", hl: line(C - 20, my(s, 80), C - 17, my(s, 150)), cy: my(s, 110), half: halfAt(r.Q, my(s, 110)) };
    },
    icedcup: (s) => {
      const r = prof(s, [[0, 40], [32, 40], [28, 60], [24, 150], [22, 172], [0, 172]]);
      return { body: r.d + path(s, [["M", C - 33, 40], ["Q", C, 26, C + 33, 40], ["Z"]]) + rS(s, C + 6, 4, 6, 60, 3), det: "", hl: line(C - 18, my(s, 74), C - 15, my(s, 150)), cy: my(s, 110), half: halfAt(r.Q, my(s, 110)) };
    },
    tumbler: (s) => {
      const r = prof(s, [[0, 30], [30, 30], [30, 40], [31, 160], [28, 176], [0, 178]]);
      return { body: r.d + cupLid(s, C - 31, 24, 62, 8), det: lS(s, C - 29, 52, C + 29, 52), hl: line(C - 20, my(s, 60), C - 19, my(s, 160)), cy: my(s, 112), half: halfAt(r.Q, my(s, 112)) };
    },
    mug: (s) => {
      const r = prof(s, [[0, 60], [31, 60], [32, 70], [32, 170], [30, 180], [0, 180]]);
      return { body: r.d, det: path(s, [["M", C + 31, 80], ["C", C + 54, 80, C + 54, 128, C + 31, 128]]), hl: line(C - 20, my(s, 78), C - 19, my(s, 160)), cy: my(s, 118), half: halfAt(r.Q, my(s, 118)) };
    },
    glassbottle: (s) => {
      const r = prof(s, [[0, 30], [14, 30], [14, 44], [22, 52], [30, 64], [32, 80], [32, 170], [30, 180], [0, 182]]);
      return { body: r.d + rS(s, C - 15, 22, 30, 10, 2), det: lS(s, C - 30, 130, C + 30, 130), hl: line(C - 20, my(s, 84), C - 18, my(s, 160)), cy: my(s, 112), half: halfAt(r.Q, my(s, 112)) };
    },
    sleeve: (s) => {
      const r = prof(s, [[0, 58], [34, 58], [32, 64], [27, 120], [23, 164], [21, 170], [0, 170]]);
      return { body: r.d + cupLid(s, C - 37, 48, 74, 10), det: [92, 100, 108, 116, 124].map((y) => lS(s, C - 30, y, C + 30, y)).join(""), hl: line(C - 24, my(s, 74), C - 19, my(s, 88)), cy: my(s, 146), half: 22 * SW[s] };
    },
    straw: (s) => {
      const r = prof(s, [[0, 58], [34, 58], [32, 64], [27, 120], [23, 164], [21, 170], [0, 170]]);
      return { body: r.d + cupLid(s, C - 37, 48, 74, 10) + rS(s, C + 8, 2, 6, 52, 3), det: "", hl: line(C - 24, my(s, 74), C - 19, my(s, 150)), cy: my(s, 110), half: halfAt(r.Q, my(s, 110)) };
    },
    big: (s) => {
      const r = prof(s, [[0, 40], [38, 40], [36, 48], [30, 120], [26, 170], [0, 172]]);
      return { body: r.d + cupLid(s, C - 41, 30, 82, 10), det: lS(s, C - 33, 100, C + 33, 100), hl: line(C - 26, my(s, 70), C - 22, my(s, 150)), cy: my(s, 112), half: halfAt(r.Q, my(s, 112)) };
    },
    icetumbler: (s) => {
      const r = prof(s, [[0, 36], [30, 36], [29, 50], [28, 150], [26, 178], [0, 178]]);
      return { body: r.d + path(s, [["M", C - 30, 36], ["Q", C, 24, C + 30, 36], ["Z"]]), det: lS(s, C - 28, 60, C + 28, 60), hl: line(C - 20, my(s, 80), C - 18, my(s, 160)), cy: my(s, 112), half: halfAt(r.Q, my(s, 112)) };
    },
  };

  /* ---------- 에너지바 포장 ---------- */
  const WRAP = {
    flat: (s) => {
      const b = box(40, 84);
      return { body: path(s, [["M", b[0], b[1]], ["W", b[2], b[1], -3, 8], ["L", b[2], b[3]], ["W", b[0], b[3], 3, 8], ["Z"]]), det: lS(s, b[0] + 9, b[1] + 3, b[0] + 9, b[3] - 3) + lS(s, b[2] - 9, b[1] + 3, b[2] - 9, b[3] - 3), cy: my(s, 100), half: 40 * SW[s] - 6 };
    },
    twist: (s) => {
      const b = box(40, 70);
      return { body: path(s, [["M", b[0], 100], ["L", b[0] + 12, b[1]], ["L", b[2] - 12, b[1]], ["L", b[2], 100], ["L", b[2] - 12, b[3]], ["L", b[0] + 12, b[3]], ["Z"]]), det: lS(s, b[0] + 12, 100, b[2] - 12, 100) };
    },
    foil: (s) => {
      const b = box(34, 96);
      return { body: path(s, [["M", b[0], b[1]], ["L", C - 6, b[1]], ["L", C, b[1] + 8], ["L", C + 6, b[1]], ["L", b[2], b[1]], ["L", b[2], b[3]], ["L", b[0], b[3]], ["Z"]]), det: lS(s, b[0] + 4, b[3] - 6, b[2] - 4, b[3] - 6) };
    },
    carton: (s) => { const b = box(36, 70); return { body: rS(s, b[0], b[1], 72, 70, 2) + rS(s, b[0] - 1, b[1] - 10, 74, 10, 2), det: lS(s, b[0], b[1] + 8, b[2], b[1] + 8) }; },
    stick: (s) => { const b = box(18, 120); return { body: rS(s, b[0], b[1], 36, 120, 9), det: lS(s, b[0] + 4, b[1] + 10, b[2] - 4, b[1] + 10) }; },
    kraft: (s) => { const b = box(40, 80); return { body: path(s, [["M", b[0], b[1]], ["L", b[2] - 10, b[1]], ["L", b[2], b[1] + 10], ["L", b[2], b[3]], ["L", b[0], b[3]], ["Z"]]), det: lS(s, b[2] - 10, b[1], b[2] - 10, b[1] + 10) + lS(s, b[2] - 10, b[1] + 10, b[2], b[1] + 10) }; },
    window: (s) => { const b = box(40, 84); return { body: rS(s, b[0], b[1], 80, 84, 4), win: rS(s, C - 28, 84, 56, 32, 3), ext: rS(s, C - 22, 92, 44, 16, 6), det: "" }; },
    tube: (s) => { const b = box(20, 100); return { body: rS(s, b[0], b[1], 40, 100, 20), det: lS(s, b[0] + 6, b[1] + 4, b[2] - 6, b[1] + 4) + lS(s, b[0] + 6, b[3] - 4, b[2] - 6, b[3] - 4) }; },
    pillow: (s) => { const b = box(40, 84); return { body: pillowS(s, b), det: lS(s, b[0] + 4, b[1] + 8, b[2] - 4, b[1] + 8) }; },
    bar: (s) => { const b = box(40, 84); return { body: rS(s, b[0], b[1], 80, 84, 5), ext: rS(s, b[0] + 10, b[1] + 18, 60, 48, 8), det: "" }; },
  };

  /* ---------- 종류 목록 (선택 칸 순서) ---------- */
  const FAMILY = { bottle: BOTTLE, bag: BAG, box: BOX, meat: MEAT, can: CAN, cup: CUP, wrap: WRAP };
  const KINDS = {
    bottle: Object.keys(BOTTLE),
    cap: Object.keys(CAP),
    bag: { chips: ["pillow", "standup", "tube", "window", "big", "mini", "kraft", "canister", "sachet", "tri"], jelly: ["stick", "cup", "standup", "tub", "mini", "sachet", "blister", "window", "tube", "kraft"], bread: ["kraft", "tall", "pillow", "bakery", "round", "window", "standup", "wedge", "roll", "tri"] },
    box: Object.keys(BOX),
    meat: Object.keys(MEAT),
    can: Object.keys(CAN),
    cup: Object.keys(CUP),
    wrap: Object.keys(WRAP),
  };
  const BOTTLE_ML = [330, 500, 1000, 1500, 2000];

  // 한 칸(종류 × 크기)의 그림
  const body = (family, kind, s) => {
    const fn = FAMILY[family][kind] || Object.values(FAMILY[family])[0];
    const r = fn(s);
    const meat = family === "meat";
    return { body: r.body || r.tray || "", ext: r.ext || "", det: r.det || "", hl: r.hl || "", win: r.win || "", xf: meat ? "translate(60 100) scale(1.3) translate(-60 -100)" : "", cy: r.cy ?? 100, half: r.half ?? (meat ? 26 : 30), top: r.top ?? 0, nr: r.nr ?? 0 };
  };
  // 뚜껑 그림 (병 목 위에, 절대 좌표)
  const cap = (kind, nr, top, s) => {
    const fn = CAP[kind] || CAP.screw;
    const r = fn(nr, top, s);
    return { body: r.body, det: r.det || "" };
  };

  window.Shapes = { body, cap, KINDS, BOTTLE_ML, MEAT_COST, C };
})();
