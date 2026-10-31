/* 사업: 팔 상품 정하기 (전체 화면), 판매 목록, 원가 계산.
   식품은 물·음식·에너지 식품 가운데 고르고, 모양·뚜껑·내용물·색을 차례로 정해요.
   칸에서 고른 것은 확인 버튼을 눌러야 다음으로 넘어가요. 그림은 app/shapes.js 의 도형이에요.
   원가는 센트 단위 정수로 계산해요 (50 = 0.5 토큰). 1 토큰 = 1 달러. */
(() => {
  "use strict";

  const Shapes = window.Shapes;
  const SPRING = "cubic-bezier(0.2, 0.9, 0.25, 1.06)";
  const RM = matchMedia("(prefers-reduced-motion: reduce)");
  const ALL = Array.from({ length: 50 }, (_, k) => k);
  const ICON_COL = ["#ffd9b8", "#c8e7ff", "#d8f5d0", "#ffd6e8", "#e4dcff"];

  const r1 = (n) => Math.round(n * 10) / 10;
  const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
  const svgI = (d) =>
    `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

  // 아이콘은 모두 검은 윤곽선이에요 (선 굵기 1.8, 둥근 끝)
  const ICON = {
    food: '<path d="M2.4 12.6H21.6"/><path d="M2.4 12.6C2.4 18.2 6.8 22 12 22S21.6 18.2 21.6 12.6"/><g transform="translate(8.6 12.6) rotate(-30)"><path d="M0 0C-3 -1.6 -3 -5.04 0 -7.2C3 -5.04 3 -1.6 0 0Z"/><path d="M0 -1V-4.46"/></g><g transform="translate(12.2 12.6) rotate(0)"><path d="M0 0C-3 -1.6 -3 -5.88 0 -8.4C3 -5.88 3 -1.6 0 0Z"/><path d="M0 -1V-5.21"/></g><g transform="translate(15.8 12.6) rotate(30)"><path d="M0 0C-3 -1.6 -3 -5.04 0 -7.2C3 -5.04 3 -1.6 0 0Z"/><path d="M0 -1V-4.46"/></g><path d="M9.4 17.6a2.7 2.7 0 1 0 0.01 0Z"/><path d="M9.4 14.9V13.6"/>',
    pc: '<rect x="3" y="4.5" width="18" height="12.5" rx="2.5"/><path d="M9 20.5h6M12 17v3.5"/>',
    ai: '<path d="M10.2 3.2L14.7 2.4L17.6 4.1L19 8.6L18.4 14.5L17.2 16.6L17.2 21.3H9.3V16.1L6.9 15.7L5.6 12L4.8 9.7L5.8 7.4L6.9 4.5Z"/><path d="M13.00 8.10Q14.73 6.54 13.60 4.74M13.94 8.41Q16.25 8.16 16.40 6.04M14.52 9.21Q16.54 10.37 17.91 8.74M14.52 10.19Q15.47 12.32 17.54 11.80M13.94 10.99Q13.46 13.27 15.43 14.07M13.00 11.30Q11.27 12.86 12.40 14.66M12.06 10.99Q9.75 11.24 9.60 13.36M11.48 10.19Q9.46 9.03 8.09 10.66M11.48 9.21Q10.53 7.08 8.46 7.60M12.06 8.41Q12.54 6.13 10.57 5.33"/>',
    drop: '<path d="M12 3.5C9 8 6.5 10.8 6.5 14.5a5.5 5.5 0 0 0 11 0C17.5 10.8 15 8 12 3.5z"/>',
    fork: '<path d="M7 3v6a2.5 2.5 0 0 0 5 0V3M9.5 11.5V21M17 21V3c-2 1.5-3 4-3 7h3"/>',
    bolt: '<path d="M13 3L6 13.5h5L10 21l7-10.5h-5z"/>',
    cookie: '<circle cx="12" cy="12" r="8.5"/><circle cx="9" cy="9" r="1"/><circle cx="14.5" cy="13.5" r="1"/><circle cx="9.5" cy="15" r="1"/>',
    plate: '<circle cx="12" cy="12" r="8.5"/><circle cx="12" cy="12" r="4.5"/>',
    bag: '<path d="M5 8h14l-1 12H6zM9 8V6.5a3 3 0 0 1 6 0V8"/>',
    box: '<rect x="4" y="7" width="16" height="12" rx="2"/><path d="M4 11h16"/>',
    wrap: '<rect x="4" y="6.5" width="16" height="11" rx="5.5"/><path d="M10 6.5v11M14 6.5v11"/>',
    can: '<rect x="7" y="3.5" width="10" height="17" rx="3"/><path d="M7 7.5h10"/>',
    cup: '<path d="M5 9h11v5a5 5 0 0 1-5 5h-1a5 5 0 0 1-5-5zM16 10h1.5a2.5 2.5 0 0 1 0 5H16"/>',
    dot: '<circle cx="12" cy="12" r="3.5"/>',
    dots: '<circle cx="5.5" cy="12" r="1.6"/><circle cx="12" cy="12" r="1.6"/><circle cx="18.5" cy="12" r="1.6"/>',
    chev: '<path d="M9 5l7 7-7 7"/>',
    back: '<path d="M15 5l-7 7 7 7"/>',
    check: '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
    nuclear: '<circle cx="12" cy="12" r="1.8"/><ellipse cx="12" cy="12" rx="9" ry="3.6"/><ellipse cx="12" cy="12" rx="9" ry="3.6" transform="rotate(60 12 12)"/><ellipse cx="12" cy="12" rx="9" ry="3.6" transform="rotate(-60 12 12)"/>',
  };

  // 값이 정해진 것인지 (null 이 아니고 0 이상인 정수)
  const has = (v) => typeof v === "number" && v >= 0;
  // 센트 정수를 토큰 글자로 (50 → 0.5, 1250 → 1,250 이 아니라 12.5)
  const tok = (cents) => (cents / 100).toLocaleString("ko-KR", { maximumFractionDigits: 2 });
  const fmt = (cents) => String(+(cents / 100).toFixed(2));

  // 색 50가지: 색상 10개 × 밝기 5단계
  const HUES = [4, 28, 46, 70, 128, 160, 196, 220, 262, 318];
  const LIGHTS = [36, 50, 64, 78, 90];
  const colorOf = (i) => {
    const l = LIGHTS[Math.floor(i / 10)];
    const s = l > 85 ? 46 : l > 70 ? 56 : 66;
    return `hsl(${HUES[i % 10]} ${s}% ${l}%)`;
  };

  /* ---------- 고르는 순서 ---------- */
  const ROOT = [
    {
      id: "food", label: "식품", icon: "food", hint: "물, 음식, 에너지 식품",
      children: [
        { id: "water", label: "물", icon: "drop", hint: "페트병, 뚜껑, 뚜껑 색", leaf: "water" },
        {
          id: "meal", label: "음식", icon: "fork", hint: "간식과 식사",
          children: [
            {
              id: "snack", label: "간식", icon: "cookie", hint: "젤리, 과자, 빵",
              children: [
                { id: "jelly", label: "젤리", icon: "bag", hint: "용기 모양과 색", leaf: "jelly" },
                { id: "chips", label: "과자", icon: "bag", hint: "봉지 모양과 색", leaf: "chips" },
                { id: "bread", label: "빵", icon: "bag", hint: "봉지 모양과 색", leaf: "bread" },
              ],
            },
            {
              id: "plate", label: "식사", icon: "plate", hint: "도시락, 고기",
              children: [
                { id: "lunchbox", label: "도시락", icon: "box", hint: "용기 모양과 색", leaf: "lunchbox" },
                { id: "meat", label: "고기", icon: "box", hint: "고기 모양과 트레이 색", leaf: "meat" },
              ],
            },
          ],
        },
        {
          id: "energy", label: "에너지 식품", icon: "bolt", hint: "에너지바, 음료, 커피",
          children: [
            { id: "bar", label: "에너지바", icon: "wrap", hint: "포장, 내용물, 색", leaf: "bar" },
            { id: "drink", label: "에너지 음료", icon: "can", hint: "캔, 내용물, 색", leaf: "drink" },
            { id: "coffee", label: "커피", icon: "cup", hint: "컵, 내용물, 색", leaf: "coffee" },
          ],
        },
      ],
    },
    { id: "pc", label: "컴퓨터", icon: "pc", soon: true },
    { id: "ai", label: "인공지능", icon: "ai", soon: true },
  ];
  const NODES = new Map();
  const indexNodes = (list) =>
    list.forEach((n) => {
      NODES.set(n.id, n);
      if (n.children) indexNodes(n.children);
    });
  indexNodes(ROOT);

  // 잎 상품: 어떤 그림 모음을 쓰고, 어떤 순서로 고르는지
  const LEAF = {
    water: { key: "water", label: "물", group: "water", family: "bottle", kinds: Shapes.KINDS.bottle, flow: ["bottle", "cap", "capColor"] },
    jelly: { key: "jelly", label: "젤리", group: "snack", family: "bag", kinds: Shapes.KINDS.bag.jelly, flow: ["shape", "color"], shapeTitle: "젤리 용기 모양", colorTitle: "용기 색" },
    chips: { key: "chips", label: "과자", group: "snack", family: "bag", kinds: Shapes.KINDS.bag.chips, flow: ["shape", "color"], shapeTitle: "과자 봉지 모양", colorTitle: "봉지 색" },
    bread: { key: "bread", label: "빵", group: "snack", family: "bag", kinds: Shapes.KINDS.bag.bread, flow: ["shape", "color"], shapeTitle: "빵 봉지 모양", colorTitle: "봉지 색" },
    lunchbox: { key: "lunchbox", label: "도시락", group: "meal", family: "box", kinds: Shapes.KINDS.box, flow: ["shape", "color"], shapeTitle: "도시락 용기 모양", colorTitle: "용기 색" },
    meat: { key: "meat", label: "고기", group: "meal", family: "meat", kinds: Shapes.KINDS.meat, flow: ["shape", "color"], shapeTitle: "고기 모양", colorTitle: "트레이 색" },
    bar: { key: "bar", label: "에너지바", group: "energy", family: "wrap", kinds: Shapes.KINDS.wrap, flow: ["shape", "contents", "color"], shapeTitle: "포장 모양", colorTitle: "포장 색", contents: ["초코", "바닐라", "딸기", "녹차", "땅콩", "블루베리"] },
    drink: { key: "drink", label: "에너지 음료", group: "energy", family: "can", kinds: Shapes.KINDS.can, flow: ["shape", "contents", "color"], shapeTitle: "에너지 캔 모양", colorTitle: "캔 색", contents: ["레몬", "청포도", "복숭아", "베리", "콜라", "자몽"] },
    coffee: { key: "coffee", label: "커피", group: "energy", family: "cup", kinds: Shapes.KINDS.cup, flow: ["shape", "contents", "color"], shapeTitle: "컵 모양", colorTitle: "컵 색", contents: ["아메리카노", "라떼", "바닐라 라떼", "콜드브루", "모카", "카라멜 마키아토"] },
  };
  const CONTAINER = { bottle: "페트병", bag: "봉지", box: "용기", can: "캔", cup: "컵", wrap: "포장", meat: "트레이" };
  // 고르는 칸: 종류 칸(shape·bottle·cap), 색 칸(color·capColor), 내용물(options)
  const stepInfo = (key, leaf) => {
    if (key === "bottle") return { kind: "shape", field: "shape", title: "페트병 모양", family: "bottle", kinds: Shapes.KINDS.bottle };
    if (key === "cap") return { kind: "shape", field: "cap", title: "뚜껑 모양", family: "cap", kinds: Shapes.KINDS.cap };
    if (key === "capColor") return { kind: "color", field: "capColor", title: "뚜껑 색" };
    if (key === "shape") return { kind: "shape", field: "shape", title: leaf.shapeTitle, family: leaf.family, kinds: leaf.kinds };
    if (key === "color") return { kind: "color", field: "color", title: leaf.colorTitle };
    return { kind: "options", field: "content", title: "내용물" };
  };

  /* ---------- 원가 (1 토큰 = 1 달러) ----------
     근거 (검색한 값은 출처를 적고, 찾지 못한 값은 추정이에요):
     - 생수 페트병·뚜껑·라벨: ThePricer (병 8~20센트, 뚜껑 1~5센트, 라벨 2~10센트), 생수 1병 전체 25~75센트 범위 안
     - 음료 캔: vaza.gr 250ml 캔 €1,090/1,000개 ≈ 약 11센트 (1개 단가로 환산, 추정)
     - 커피 컵: 12온스 컵·뚜껑·슬리브 세트 22~37센트 (Walmart 100개 $21.99, Quill 500개 $186.99)
     - 도시락 용기: 17온스 크라프트 박스 92센트 (UCHANGE 3,000개 단가), 플라스틱 용기 30~62센트 (Global Sources)
     - 고기: 삼겹살 1kg 2만 원 안팎 (2026년 6월 축산물 가격표 21,000~25,000원, 도매 18,000원부터), 1달러 = 1,400원 가정
     - 봉지 필름, 내용물, 텀블러 등은 찾지 못해서 추정이에요. */
  const FX = 1400;
  const MEAT_UNIT = (20000 * 0.15) / FX; // 150g 한 팩 ≈ 2.14달러
  const SIZE_F = [0.8, 0.9, 1, 1.1, 1.2]; // 크기 5단계에 따른 용기 배율
  const COST = {
    water: { bottle: [0.07, 0.10, 0.16, 0.21, 0.25], filling: [0.066, 0.10, 0.20, 0.30, 0.40], label: 0.05 },
    cap: [0.03, 0.05, 0.03, 0.04, 0.04, 0.03, 0.04, 0.03, 0.03, 0.03],
    chips: { pack: [0.05, 0.08, 0.25, 0.07, 0.12, 0.04, 0.10, 0.25, 0.04, 0.06], inside: 0.40 },
    jelly: { pack: [0.04, 0.12, 0.08, 0.15, 0.04, 0.04, 0.08, 0.07, 0.25, 0.10], inside: 0.30 },
    bread: { pack: [0.10, 0.06, 0.05, 0.20, 0.20, 0.07, 0.08, 0.05, 0.05, 0.06], inside: 0.80 },
    lunchbox: { pack: [0.30, 0.25, 0.92, 0.40, 0.45, 0.50, 0.35, 0.60, 0.35, 0.35], inside: 2.0 },
    meat: { mult: [2.5, 1.0, 2.0, 0.6, 0.7, 0.8, 1.0, 2.0, 0.7, 0.5], tray: 0.15 },
    bar: { pack: [0.05, 0.05, 0.06, 0.12, 0.04, 0.06, 0.08, 0.10, 0.05, 0.10], inside: 0.40 },
    drink: { pack: [0.13, 0.11, 0.16, 0.18, 0.25, 0.10, 0.13, 0.13, 0.15, 0.13], inside: 0.20 },
    coffee: { pack: [0.25, 0.30, 0.20, 1.50, 0.80, 0.60, 0.28, 0.27, 0.35, 0.40], inside: 0.15 },
    logo: 0.02,
  };
  // 원가 줄을 센트 정수로 돌려줘요 (합계도 센트)
  const costLines = (p) => {
    const leaf = LEAF[p.item];
    const k = Math.floor((p.shape ?? 0) / 5);
    const s = (p.shape ?? 0) % 5;
    const lines = [];
    if (leaf.family === "bottle") {
      lines.push(["페트병", COST.water.bottle[s]]);
      lines.push(["생수와 충전", COST.water.filling[s]]);
      if (has(p.cap)) lines.push(["뚜껑", COST.cap[Math.floor(p.cap / 5)]]);
      lines.push(["라벨", COST.water.label]);
    } else if (leaf.family === "meat") {
      lines.push(["트레이", COST.meat.tray]);
      lines.push(["고기 150g", MEAT_UNIT * COST.meat.mult[k] * SIZE_F[s]]);
    } else {
      const c = COST[leaf.key];
      lines.push([CONTAINER[leaf.family], c.pack[k] * SIZE_F[s]]);
      lines.push(["내용물", c.inside * SIZE_F[s]]);
    }
    lines.push(["로고 인쇄", COST.logo]);
    const rows = lines.map(([n, usd]) => [n, Math.round(usd * 100)]);
    return { lines: rows, total: rows.reduce((t, l) => t + l[1], 0) };
  };

  /* ---------- 이름 ---------- */
  const sizeOf = (ml) => (ml >= 1000 ? `${+(ml / 1000).toFixed(2)}L` : `${ml}ml`);
  const nameOf = (p) => {
    const leaf = LEAF[p.item];
    if (p.item === "water") return `생수 ${sizeOf(Shapes.BOTTLE_ML[(p.shape ?? 0) % 5])}`;
    const flavor = leaf.contents && has(p.content) ? ` · ${leaf.contents[p.content]}` : "";
    return `${leaf.label}${flavor}`.slice(0, 30);
  };

  /* ---------- 그림 (app/shapes.js 도형을 SVG로 그려요) ---------- */
  let ctx = null;
  let uid = 0;
  const logoHTML = (r) => {
    const co = ctx.company();
    const img = co ? ctx.safeImg(co.logo, 40000) : "";
    const inner = img ? `<i style="background-image:url('${img}')"></i>` : `<b>${esc((co?.name || "사")[0])}</b>`;
    return `<span class="pkv-logo${img ? "" : " init"}" style="left:50%;top:${r1((r.cy / 200) * 100)}%;width:${r1(((r.half * 2 * 0.94) / 120) * 100)}%">${inner}</span>`;
  };
  const paths = (r, fill) => {
    const tf = r.xf ? ` transform="${r.xf}"` : ""; // 고기는 트레이 안에서 조금 크게
    let o = "";
    if (r.body) o += `<path class="pk-body" d="${r.body}" fill="${fill}"/>`;
    if (r.ext) o += `<path class="pk-ext" d="${r.ext}" fill="#cf5a63"${tf}/>`;
    if (r.win) o += `<path class="pk-win" d="${r.win}" fill="#ffffff"/>`;
    if (r.det) o += `<path class="pk-det" d="${r.det}"${tf}/>`;
    if (r.hl) o += `<path class="pk-hl" d="${r.hl}"/>`;
    return o;
  };
  // 상품 그림 (값이 아직 없는 칸은 기본 모양으로 그려요)
  const pkvHTML = (p, mini = false) => {
    const leaf = LEAF[p.item];
    const k = p.shape ?? 0;
    const r = Shapes.body(leaf.family, leaf.kinds[Math.floor(k / 5)] ?? leaf.kinds[0], k % 5);
    const water = leaf.family === "bottle";
    const fill = water ? "#eef6fb" : has(p.color) ? colorOf(p.color) : "#ffffff";
    let cap = "";
    if (water && has(p.cap)) {
      const c = Shapes.cap(Shapes.KINDS.cap[Math.floor(p.cap / 5)], r.nr, r.top, p.cap % 5);
      cap = `<path class="pk-body" d="${c.body}" fill="${has(p.capColor) ? colorOf(p.capColor) : "#2b2b2f"}"/>${c.det ? `<path class="pk-det" d="${c.det}"/>` : ""}`;
    }
    return `<div class="pkv${mini ? " pkv-sm" : ""}"><svg viewBox="0 0 120 200" aria-hidden="true">${paths(r, fill)}${cap}</svg>${logoHTML(r)}</div>`;
  };
  // 고르는 칸의 작은 그림 (흰 바탕, 검은 윤곽선)
  const thumbHTML = (family, kind, s) => {
    if (family === "cap") {
      const c = Shapes.cap(kind, 10, 60, s);
      return `<svg class="th" viewBox="0 0 120 200" aria-hidden="true"><path class="pk-body" d="M52 60H68V150H52Z" fill="#f1f1f4"/><path class="pk-body" d="${c.body}" fill="#ffffff"/>${c.det ? `<path class="pk-det" d="${c.det}"/>` : ""}</svg>`;
    }
    return `<svg class="th" viewBox="0 0 120 200" aria-hidden="true">${paths(Shapes.body(family, kind, s), "#ffffff")}</svg>`;
  };

  /* ---------- 상태 ---------- */
  let pk = null;
  let stage = null;
  let prev = null;
  let titleEl = null;
  let crumbEl = null;
  let backEl = null;
  let foot = null;
  let okBtn = null;
  let products = [];
  let hist = [];
  let pick = null; // 지금 상품 정하기의 값 (확인을 눌러서 들어온 것만)
  let sel = null; // 지금 칸에서 고른 것 (확인 전)
  let busy = false;
  let lastPrev = "";
  const anim = (el, frames, opts) => (RM.matches ? null : el.animate(frames, { fill: "both", ...opts }));
  const top = () => hist[hist.length - 1];

  const titleOf = (e) => {
    if (e.kind === "root") return "팔 상품 정하기";
    if (e.kind === "list" || e.kind === "soon") return e.node.label;
    if (e.kind === "step") return stepInfo(e.key, LEAF[pick.item]).title;
    return "이름과 가격 정하기";
  };
  const crumbOf = () => {
    const out = [];
    for (const e of hist) {
      if (e.kind === "list" || e.kind === "soon") out.push(e.node.label);
      if (e.kind === "step" && pick && e.key === LEAF[pick.item].flow[0]) out.push(LEAF[pick.item].label);
    }
    return out.join(" › ");
  };

  /* ---------- 화면 조각 ---------- */
  const rowHTML = (n, i) =>
    `<button class="pk-row" type="button" data-node="${n.id}" style="--i:${i}"><span class="pk-ic" style="--c:${ICON_COL[i % ICON_COL.length]}">${svgI(ICON[n.icon])}</span><span class="pk-main"><b>${esc(n.label)}</b><span>${esc(n.hint || "")}</span></span><span class="pk-chev">${svgI(ICON.chev)}</span></button>`;

  const stepHTML = (key) => {
    const leaf = LEAF[pick.item];
    const st = stepInfo(key, leaf);
    if (st.kind === "shape") {
      const tiles = ALL.map((k) => `<button class="pk-tile${sel === k ? " is-on" : ""}" type="button" data-v="${k}" style="--i:${k}">${thumbHTML(st.family, st.kinds[Math.floor(k / 5)], k % 5)}</button>`);
      return `<div class="pk-grid">${tiles.join("")}</div>`;
    }
    if (st.kind === "color") {
      return `<div class="pk-grid pk-swgrid">${ALL.map((k) => `<button class="pk-sw${sel === k ? " is-on" : ""}" type="button" data-v="${k}" style="--i:${k};--c:${colorOf(k)}" aria-label="색 ${k + 1}"></button>`).join("")}</div>`;
    }
    return `<div class="pk-list">${leaf.contents
      .map((c, i) => `<button class="pk-row${sel === i ? " is-on" : ""}" type="button" data-v="${i}" style="--i:${i}"><span class="pk-ic" style="--c:${ICON_COL[i % ICON_COL.length]}">${svgI(ICON.dot)}</span><span class="pk-main"><b>${esc(c)}</b></span><span class="pk-chev">${svgI(ICON.chev)}</span></button>`)
      .join("")}</div>`;
  };

  const summaryHTML = () => {
    const c = costLines(pick);
    if (pick.priceCents == null) pick.priceCents = Math.max(1, c.total * 2);
    if (pick.name == null) pick.name = nameOf(pick);
    const sug = Math.max(1, c.total * 2);
    const rows = c.lines.map(([n, v]) => `<div class="pk-cost-row"><span>${n}</span><b>${tok(v)} 토큰</b></div>`).join("");
    return `<div class="pk-sum">
      <p class="pk-sum-sub">${esc(crumbOf())}</p>
      <div class="pk-card"><input id="pk-name" class="pk-name" maxlength="30" autocomplete="off" placeholder="상품 이름" value="${esc(pick.name)}" aria-label="상품 이름"></div>
      <div class="pk-card">
        <p class="pk-lab">원가 계산</p>
        ${rows}
        <div class="pk-cost-row pk-cost-total"><span>원가 합계</span><b>${tok(c.total)} 토큰</b></div>
      </div>
      <div class="pk-card">
        <input id="pk-input" class="pk-input" inputmode="decimal" autocomplete="off" value="${fmt(pick.priceCents)}" aria-label="팔 가격">
        <div class="pk-steps">${[-100, -10, 10, 100].map((d) => `<button class="pk-step" type="button" data-step="${d}">${d > 0 ? "+" : "−"}${tok(Math.abs(d))}</button>`).join("")}</div>
        <button class="pk-chip" type="button" data-suggest="${sug}">추천 ${tok(sug)} 토큰 (원가의 2배)</button>
        <p class="pk-profit" id="pk-profit"></p>
      </div>
      <p class="pk-err" id="pk-err" role="alert"></p>
      <button class="pk-sell" id="pk-sell" type="button">이 가격으로 팔기</button>
    </div>`;
  };

  const viewHTML = (e) => {
    if (e.kind === "root") {
      return `<div class="pk-root">${ROOT.map(
        (n, i) => `<div class="pk-orbwrap"><button class="pk-orb" type="button" data-orb="${i}" aria-label="${n.label}">${svgI(ICON[n.icon])}</button><span class="pk-orb-l">${n.label}</span></div>`
      ).join("")}</div>`;
    }
    if (e.kind === "list") return `<div class="pk-list">${e.node.children.map((c, i) => rowHTML(c, i)).join("")}</div>`;
    if (e.kind === "soon") {
      return `<div class="pk-soon"><div class="pk-soon-orb">${svgI(ICON[e.node.icon])}</div><p class="pk-soon-t">${e.node.label}</p><p class="pk-soon-s">아직 준비 중이에요</p></div>`;
    }
    if (e.kind === "step") return stepHTML(e.key);
    return summaryHTML();
  };

  // 원가와 이익을 다시 써요 (입력칸은 건드리지 않아요)
  const paintProfit = () => {
    const el = pk?.querySelector("#pk-profit");
    if (!el || !pick) return;
    const c = costLines(pick);
    const pr = pick.priceCents || 0;
    const gain = pr - c.total;
    el.classList.toggle("loss", pr >= 1 && gain < 0);
    el.textContent =
      pr < 1
        ? "가격을 정해 주세요"
        : gain > 0
          ? `한 개에 ${tok(gain)} 토큰 남아요 (이익률 ${Math.round((gain / pr) * 100)}%)`
          : gain === 0
            ? "원가와 같아요. 남는 돈이 없어요"
            : `원가보다 ${tok(-gain)} 토큰 낮아요`;
  };

  /* ---------- 그림 칸 (위쪽 미리보기) ---------- */
  // 칸에서 고른 것은 확인 전이라도 미리 보여줘요
  const pickPreview = () => {
    const t = top();
    if (t && t.kind === "step" && sel !== null) return { ...pick, [stepInfo(t.key, LEAF[pick.item]).field]: sel };
    return pick;
  };
  const setPrev = (html, big, on) => {
    prev.classList.toggle("on", on);
    prev.classList.toggle("pk-big", big);
    if (html === lastPrev) return;
    lastPrev = html;
    const old = [...prev.querySelectorAll(".pk-pvbox")];
    if (html) {
      const box = document.createElement("div");
      box.className = "pk-pvbox";
      box.innerHTML = html;
      prev.appendChild(box);
      anim(box, [{ opacity: 0, transform: "scale(.9) translateY(10px)" }, { opacity: 1, transform: "none" }], { duration: 440, easing: SPRING, fill: "backwards" });
    }
    for (const o of old) {
      const a = o.animate([{ opacity: 1, transform: "none" }, { opacity: 0, transform: "scale(1.03)" }], { duration: 300, fill: "forwards" });
      a.onfinish = () => o.remove();
    }
  };
  const renderPrev = (e) => {
    if (e.kind === "step") setPrev(pkvHTML(pickPreview()), false, true);
    else if (e.kind === "summary") setPrev(pkvHTML(pick), true, true);
    else setPrev("", false, false);
  };

  /* ---------- 화면 넘기기 ---------- */
  const makeView = (html) => {
    const el = document.createElement("div");
    el.className = "pk-view";
    el.innerHTML = html;
    return el;
  };
  const swap = (el, dir) => {
    const old = [...stage.querySelectorAll(".pk-view")];
    stage.appendChild(el);
    if (RM.matches) {
      old.forEach((o) => o.remove());
      return;
    }
    anim(el, [{ opacity: 0, transform: `translate3d(${dir * 46}px,0,0)` }, { opacity: 1, transform: "translate3d(0,0,0)" }], { duration: 460, easing: SPRING, fill: "backwards" });
    for (const o of old) {
      o.style.pointerEvents = "none";
      o.animate([{ opacity: 1, transform: "none" }, { opacity: 0, transform: `translate3d(${-dir * 32}px,0,0)` }], { duration: 320, easing: "ease-in", fill: "forwards" }).onfinish = () => o.remove();
    }
  };
  const syncTop = () => {
    const e = top();
    titleEl.textContent = titleOf(e);
    crumbEl.textContent = hist.length > 1 ? crumbOf() : "";
    backEl.setAttribute("aria-label", hist.length > 1 ? "뒤로" : "닫기");
    anim(titleEl, [{ opacity: 0.2, transform: "translateY(4px)" }, { opacity: 1, transform: "none" }], { duration: 320, easing: SPRING, fill: "backwards" });
  };
  // 확인 버튼은 고르는 칸에서만 보여요. 고르기 전에는 누를 수 없어요
  const paintFoot = () => {
    const t = top();
    foot.hidden = !(t && t.kind === "step");
    okBtn.disabled = sel === null;
  };
  const show = (e, dir) => {
    if (e.kind === "step") {
      const f = stepInfo(e.key, LEAF[pick.item]).field;
      sel = has(pick[f]) ? pick[f] : null; // 이미 정한 칸으로 돌아오면 그 값을 골라 둬요
    }
    swap(makeView(viewHTML(e)), dir);
    syncTop();
    renderPrev(e);
    paintFoot();
    if (e.kind === "summary") requestAnimationFrame(paintProfit);
  };
  const push = (e) => {
    hist.push(e);
    show(e, 1);
  };
  const pop = () => {
    if (hist.length <= 1) return close();
    hist.pop();
    show(top(), -1);
  };
  const advance = () => {
    const t = top();
    if (!t || t.kind !== "step") return;
    const flow = LEAF[pick.item].flow;
    const i = flow.indexOf(t.key);
    push(i < flow.length - 1 ? { kind: "step", key: flow[i + 1] } : { kind: "summary" });
  };

  /* ---------- 고르기 ---------- */
  // 식품·컴퓨터·인공지능 동그라미: 누른 동그라미가 위쪽 제목 자리로 모여요
  const pickRoot = (i, orb) => {
    if (busy) return;
    busy = true;
    const node = ROOT[i];
    const t = pk.querySelector(".pk-title").getBoundingClientRect();
    const r = orb.getBoundingClientRect();
    const dx = t.left + t.width / 2 - (r.left + r.width / 2);
    const dy = t.top + t.height / 2 - (r.top + r.height / 2) - 6;
    anim(orb, [{ transform: "translate3d(0,0,0) scale(1)" }, { transform: `translate3d(${dx}px,${dy}px,0) scale(.5)` }], { duration: 480, easing: SPRING });
    const wrap = orb.parentElement;
    stage.querySelectorAll(".pk-orbwrap").forEach((w) => {
      if (w !== wrap) anim(w, [{ opacity: 1, transform: "none" }, { opacity: 0, transform: "scale(.7)" }], { duration: 300 });
    });
    anim(wrap.querySelector(".pk-orb-l"), [{ opacity: 1 }, { opacity: 0 }], { duration: 200 });
    setTimeout(() => {
      busy = false;
      push(node.soon ? { kind: "soon", node } : { kind: "list", node });
    }, 340);
  };
  const chooseNode = (id) => {
    const n = NODES.get(id);
    if (!n) return;
    if (n.children) return push({ kind: "list", node: n });
    if (n.leaf) {
      pick = { item: n.leaf, shape: null, cap: null, capColor: null, color: null, content: null, name: null, priceCents: null };
      push({ kind: "step", key: LEAF[n.leaf].flow[0] });
    }
  };
  // 칸에서 하나를 고르면 미리 보여줘요. 확인을 눌러야 다음으로 넘어가요
  const choose = (v, el) => {
    if (busy || !pick) return;
    sel = v;
    stage.querySelectorAll("[data-v]").forEach((s) => s.classList.toggle("is-on", s === el));
    anim(el, [{ transform: "scale(1)" }, { transform: "scale(.92)" }, { transform: "scale(1.05)" }, { transform: "scale(1)" }], { duration: 420, easing: "ease-out" });
    if (navigator.vibrate) navigator.vibrate(6);
    setPrev(pkvHTML(pickPreview()), false, true);
    paintFoot();
  };
  const confirmStep = () => {
    const t = top();
    if (busy || !t || t.kind !== "step" || sel === null) return;
    pick[stepInfo(t.key, LEAF[pick.item]).field] = sel;
    sel = null;
    advance();
  };

  /* ---------- 가격 ---------- */
  const setPrice = (cents) => {
    pick.priceCents = Math.min(99999999, Math.max(1, Math.round(cents)));
    const inp = pk.querySelector("#pk-input");
    if (inp) inp.value = fmt(pick.priceCents);
    paintProfit();
  };

  /* ---------- 팔기 ---------- */
  const burst = () => {
    const d = document.createElement("div");
    d.className = "pk-done";
    d.innerHTML = `<i>${svgI(ICON.check)}</i>`;
    pk.appendChild(d);
    anim(d.firstElementChild, [{ transform: "scale(.4)", opacity: 0 }, { transform: "scale(1.08)", opacity: 1, offset: 0.6 }, { transform: "scale(1)", opacity: 1 }], { duration: 620, easing: SPRING });
  };
  const sell = async () => {
    if (busy || !pick) return;
    const err = pk.querySelector("#pk-err");
    const btn = pk.querySelector("#pk-sell");
    const name = String(pick.name ?? "").trim().replace(/\s+/g, " ");
    if (!name || name.length > 30 || /[<>]/.test(name)) {
      err.textContent = "이름은 1~30자로, < > 없이 적어 주세요";
      return;
    }
    if (!(pick.priceCents >= 1)) {
      err.textContent = "가격을 0.01 토큰 이상으로 정해 주세요";
      return;
    }
    if (!ctx.company()) {
      err.textContent = "먼저 창업해야 팔 수 있어요";
      return;
    }
    busy = true;
    err.textContent = "";
    const leaf = LEAF[pick.item];
    const c = costLines(pick);
    const p = {
      cat: "food",
      group: leaf.group,
      item: pick.item,
      name,
      shape: pick.shape,
      color: has(pick.color) ? pick.color : -1,
      costCents: c.total,
      priceCents: pick.priceCents,
      paused: false,
      active: true,
    };
    if (leaf.family === "bottle") {
      p.cap = pick.cap;
      p.capColor = pick.capColor;
    }
    if (leaf.contents && has(pick.content)) p.content = pick.content;
    btn.classList.add("busy");
    try {
      await ctx.cloud().createProduct(ctx.company().id, p);
      burst();
      await refresh();
      ctx.toast(`${name}을(를) 팔기 시작했어요`);
      setTimeout(close, 900);
    } catch (e) {
      err.textContent = e?.code === "permission-denied" ? "올리지 못했어요. 회사가 있는지 확인해 주세요" : "올리지 못했어요. 잠시 뒤 다시 해 주세요";
      btn.classList.remove("busy");
    } finally {
      busy = false;
    }
  };

  /* ---------- 열기, 닫기 ---------- */
  const open = () => {
    if (!ctx?.company()) return;
    hist = [{ kind: "root" }];
    pick = null;
    sel = null;
    stage.innerHTML = "";
    lastPrev = "";
    setPrev("", false, false);
    stage.appendChild(makeView(viewHTML(hist[0])));
    syncTop();
    paintFoot();
    pk.hidden = false;
    anim(pk, [{ transform: "translate3d(0,100%,0)" }, { transform: "none" }], { duration: 520, easing: SPRING });
    stage.querySelectorAll(".pk-orbwrap").forEach((w, i) =>
      anim(w, [{ opacity: 0, transform: "translate3d(0,26px,0) scale(.9)" }, { opacity: 1, transform: "none" }], { duration: 560, delay: 160 + i * 80, easing: SPRING, fill: "backwards" })
    );
  };
  function close() {
    busy = false;
    const done = () => {
      pk.hidden = true;
      stage.innerHTML = "";
      prev.innerHTML = "";
      prev.classList.remove("on", "pk-big");
      lastPrev = "";
      hist = [];
      pick = null;
      sel = null;
      pk.querySelectorAll(".pk-done").forEach((d) => d.remove());
    };
    if (RM.matches || pk.hidden) return done();
    const a = pk.animate([{ transform: "translate3d(0,0,0)" }, { transform: "translate3d(0,100%,0)" }], { duration: 420, easing: "cubic-bezier(.4,0,.2,1)", fill: "forwards" });
    a.onfinish = () => {
      a.cancel();
      done();
    };
  }

  /* ---------- 판매 목록 (사업 탭) ---------- */
  const paint = () => {
    const list = document.getElementById("biz-list");
    if (!list) return;
    list.innerHTML = products
      .map(
        (p, i) =>
          `<li class="biz-item" data-id="${esc(p.id)}" style="--i:${i}"><span class="biz-thumb">${pkvHTML(p, true)}</span><span class="biz-main"><b>${esc(p.name)}</b><span>판매 ${tok(p.priceCents ?? 0)} 토큰 · 원가 ${tok(p.costCents ?? 0)} 토큰</span>${p.paused ? '<em class="biz-tag">판매 멈춤</em>' : ""}</span><button class="biz-more" type="button" data-more="${esc(p.id)}" aria-label="더보기">${svgI(ICON.dots)}</button></li>`
      )
      .join("");
    document.getElementById("biz-empty").hidden = products.length > 0;
  };
  async function refresh() {
    paintHire();
    const kindNow = ctx.company()?.kind ?? "food";
    const food = kindNow === "food";
    document.getElementById("biz-prod-title").hidden = !food;
    document.getElementById("biz-list").closest(".card").hidden = !food;
    document.getElementById("biz-lab").hidden = food;
    if (!food) {
      paintLab();
      return;
    }
    const cloud = ctx?.cloud();
    if (!cloud?.user || !ctx.company()) {
      products = [];
      paint();
      return;
    }
    try {
      products = (await cloud.listProducts(ctx.company().id)).sort((a, b) => (b.at || 0) - (a.at || 0));
    } catch (err) {
      console.warn("상품을 불러오지 못했어요:", err?.code || err);
      return;
    }
    paint();
  }

  // 아래에서 올라오는 선택 목록 (아이폰 액션 시트)
  const sheet = (title, actions) =>
    new Promise((resolve) => {
      const dim = document.createElement("div");
      dim.className = "bz-dim";
      const box = document.createElement("div");
      box.className = "bz-sheet";
      box.innerHTML = `<p class="bz-sheet-t">${esc(title)}</p><div class="bz-sheet-g">${actions
        .map((a) => `<button type="button" class="bz-act${a.tone ? " " + a.tone : ""}" data-id="${a.id}">${esc(a.label)}</button>`)
        .join("")}</div><button type="button" class="bz-act bz-cancel" data-id="">취소</button>`;
      const done = (id) => {
        dim.classList.remove("on");
        box.classList.remove("on");
        setTimeout(() => {
          dim.remove();
          box.remove();
        }, 300);
        resolve(id || null);
      };
      box.addEventListener("click", (e) => {
        const b = e.target.closest("[data-id]");
        if (b) done(b.dataset.id);
      });
      dim.addEventListener("click", () => done(null));
      document.querySelector(".app").append(dim, box);
      void box.offsetHeight; // 시작 자리를 먼저 잡아야 올라오는 움직임이 나와요
      dim.classList.add("on");
      box.classList.add("on");
    });
  // 이름 바꾸기: 입력 카드 (1~30자, < > 없이)
  const askName = (current) =>
    new Promise((resolve) => {
      const dim = document.createElement("div");
      dim.className = "bz-dim";
      const card = document.createElement("div");
      card.className = "bz-card";
      card.innerHTML = `<p class="bz-card-t">이름 바꾸기</p><input class="bz-input" maxlength="30" autocomplete="off" value="${esc(current)}" aria-label="새 이름"><p class="bz-err"></p><div class="bz-card-b"><button type="button" class="bz-btn" data-k="no">취소</button><button type="button" class="bz-btn primary" data-k="ok">저장</button></div>`;
      const input = card.querySelector(".bz-input");
      const err = card.querySelector(".bz-err");
      const done = (v) => {
        dim.classList.remove("on");
        card.classList.remove("on");
        setTimeout(() => {
          dim.remove();
          card.remove();
        }, 300);
        resolve(v);
      };
      card.addEventListener("click", (e) => {
        const b = e.target.closest("[data-k]");
        if (!b) return;
        if (b.dataset.k === "no") return done(null);
        const name = input.value.trim().replace(/\s+/g, " ");
        if (!name || name.length > 30 || /[<>]/.test(name)) {
          err.textContent = "이름은 1~30자로, < > 없이 적어 주세요";
          return;
        }
        done(name);
      });
      dim.addEventListener("click", () => done(null));
      document.querySelector(".app").append(dim, card);
      void card.offsetHeight;
      dim.classList.add("on");
      card.classList.add("on");
      input.focus();
    });
  // 더보기: 이름 바꾸기, 판매 멈추기(다시 시작), 판매 종료하기. 멈추기와 종료는 두 번 물어봐요
  const onMore = async (id) => {
    const p = products.find((x) => x.id === id);
    if (!p || busy) return;
    const choice = await sheet(p.name, [
      { id: "rename", label: "이름 바꾸기" },
      { id: "pause", label: p.paused ? "판매 다시 시작하기" : "판매 멈추기" },
      { id: "end", label: "판매 종료하기", tone: "danger" },
    ]);
    const cloud = ctx.cloud();
    try {
      if (choice === "rename") {
        const name = await askName(p.name);
        if (!name || name === p.name) return; // 그대로면 서버에 보내지 않아요
        await cloud.renameProduct(ctx.company().id, id, name);
        p.name = name;
        paint();
        ctx.toast("이름을 바꿨어요");
      } else if (choice === "pause") {
        const resume = p.paused;
        const a = await ctx.ask({
          title: resume ? "판매를 다시 시작할까요?" : "판매를 멈출까요?",
          body: resume ? "다시 시작하면 이 상품이 다시 팔려요." : "멈추면 이 상품이 팔리지 않아요. 나중에 다시 시작할 수 있어요.",
          ok: resume ? "다시 시작" : "멈추기",
          cancel: "취소",
        });
        if (!a) return;
        if (!resume && !(await ctx.ask({ title: "정말 하시겠습니까?", body: "판매를 멈춰요.", ok: "네, 멈출게요", cancel: "아니요" }))) return;
        await cloud.setProductPaused(ctx.company().id, id, !resume);
        p.paused = !resume;
        paint();
        ctx.toast(resume ? "판매를 다시 시작했어요" : "판매를 멈췄어요");
      } else if (choice === "end") {
        const a = await ctx.ask({ title: `"${p.name}" 판매를 끝낼까요?`, body: "끝내면 이 상품은 다시 팔 수 없어요.", ok: "판매 종료", cancel: "취소" });
        if (!a) return;
        if (!(await ctx.ask({ title: "정말 하시겠습니까?", body: "판매를 끝내면 되돌릴 수 없어요.", ok: "네, 끝낼게요", cancel: "아니요" }))) return;
        await cloud.endProduct(ctx.company().id, id);
        products = products.filter((x) => x.id !== id);
        setTimeout(paint, 240);
        ctx.toast(`${p.name} 판매를 끝냈어요`);
      }
    } catch (err) {
      ctx.toast("처리하지 못했어요. 잠시 뒤 다시 해 주세요");
    }
  };

  /* ---------- 이벤트 ---------- */
  const onStage = (e) => {
    if (busy) return;
    const orb = e.target.closest(".pk-orb");
    if (orb) return pickRoot(Number(orb.dataset.orb), orb);
    const node = e.target.closest("[data-node]");
    if (node) return chooseNode(node.dataset.node);
    const v = e.target.closest("[data-v]");
    if (v && top()?.kind === "step") return choose(Number(v.dataset.v), v);
    const st = e.target.closest("[data-step]");
    if (st) return setPrice((pick.priceCents || 0) + Number(st.dataset.step));
    const sg = e.target.closest("[data-suggest]");
    if (sg) return setPrice(Number(sg.dataset.suggest));
    if (e.target.closest("#pk-sell")) return sell();
  };
  const onInput = (e) => {
    if (!pick) return;
    if (e.target.id === "pk-name") pick.name = e.target.value;
    if (e.target.id === "pk-input") {
      const v = Number(e.target.value.replace(",", "."));
      if (Number.isFinite(v) && v >= 0) pick.priceCents = Math.min(99999999, Math.round(v * 100));
      paintProfit();
    }
  };
  const onBlur = (e) => {
    if (e.target.id !== "pk-input" || !pick) return;
    pick.priceCents = Math.max(1, pick.priceCents || 1);
    e.target.value = fmt(pick.priceCents);
    paintProfit();
  };

  // 사업 탭에서 쓰는 입구: 상품 정하기 버튼, 판매 목록의 더보기 버튼
  const init = (c) => {
    ctx = c;
    pk = document.createElement("section");
    pk.className = "pk";
    pk.hidden = true;
    pk.dataset.volatile = "";
    pk.setAttribute("aria-label", "팔 상품 정하기");
    pk.innerHTML = `<header class="pk-top"><button class="pk-glass" type="button" data-back aria-label="닫기">${svgI(ICON.back)}</button><div class="pk-title"><b></b><span></span></div><span class="pk-spacer" aria-hidden="true"></span></header>
      <div class="pk-prev" aria-hidden="true"></div>
      <div class="pk-stage"></div>
      <div class="pk-foot" hidden><button class="pk-ok" type="button" disabled>확인</button></div>`;
    document.querySelector(".app").appendChild(pk);
    stage = pk.querySelector(".pk-stage");
    prev = pk.querySelector(".pk-prev");
    titleEl = pk.querySelector(".pk-title b");
    crumbEl = pk.querySelector(".pk-title span");
    backEl = pk.querySelector("[data-back]");
    foot = pk.querySelector(".pk-foot");
    okBtn = pk.querySelector(".pk-ok");
    backEl.addEventListener("click", () => {
      if (!busy) pop();
    });
    okBtn.addEventListener("click", confirmStep);
    stage.addEventListener("click", onStage);
    stage.addEventListener("input", onInput);
    stage.addEventListener("focusout", onBlur);
    document.getElementById("biz-pick").addEventListener("click", open);
    document.getElementById("biz-lab").addEventListener("click", labClick);
    document.getElementById("biz-lab").addEventListener("input", (e) => { labInput(e); trainInput(e); });
    document.getElementById("hire-minus").innerHTML = svgI('<path d="M5 12h14"/>');
    document.getElementById("hire-plus").innerHTML = svgI('<path d="M12 5v14M5 12h14"/>');
    document.getElementById("hire-minus").addEventListener("click", () => stepHire(-1));
    document.getElementById("hire-plus").addEventListener("click", () => stepHire(1));
    document.getElementById("hire-go").addEventListener("click", postHire);
    document.getElementById("biz-list").addEventListener("click", (e) => {
      const b = e.target.closest("[data-more]");
      if (b) onMore(b.dataset.more);
    });
  };

  /* ---------- 직원 채용: 사람만 골라요 (인공지능 직원은 아직 막아 두었어요).
     + − 로 정한 숫자가 곧 모집 인원이에요. 모집 인원 맞추기를 누르면 그 숫자로 맞춰져요 (숫자에는 제한이 없어요) ---------- */
  let hireN = 0;
  let hireFor = null;
  let hiring = false;
  const paintHire = () => {
    const co = ctx?.company();
    if (!co) return;
    const open = co.open ?? 0;
    if (hireFor !== co.id) {
      hireFor = co.id;
      hireN = open; // 회사를 바꾸면 지금 모집 인원부터 보여요
    }
    hireN = Math.max(0, hireN);
    document.getElementById("hire-stat").textContent = `모집 중 ${open}명 · 직원 ${co.staff ?? 0}명`;
    document.getElementById("hire-num").textContent = String(hireN);
    document.getElementById("hire-minus").disabled = hireN <= 0;
    document.getElementById("hire-plus").disabled = false;
    const go = document.getElementById("hire-go");
    go.disabled = hiring || hireN === open;
    go.textContent = `모집 인원 ${hireN}명으로 맞추기`;
    document.getElementById("hire-note").textContent = "숫자에는 제한이 없어요. 정한 숫자가 그대로 모집 인원이 돼요";
  };
  const stepHire = (d) => {
    hireN = Math.max(0, hireN + d);
    paintHire();
    const el = document.getElementById("hire-num");
    el.classList.remove("pop");
    void el.offsetWidth;
    el.classList.add("pop");
  };
  const postHire = async () => {
    const co = ctx?.company();
    if (!co || hiring) return;
    const n = hireN;
    hiring = true;
    const btn = document.getElementById("hire-go");
    btn.classList.add("busy");
    try {
      await ctx.cloud().setOpenings(co.id, n);
      ctx.patchCompany({ open: n });
      ctx.toast(`모집 인원을 ${n}명으로 맞췄어요`);
    } catch (err) {
      ctx.toast("모집 인원을 맞추지 못했어요. 잠시 뒤 다시 해 주세요");
    } finally {
      hiring = false;
      btn.classList.remove("busy");
      paintHire();
    }
  };
  /* ---------- 연구실·발전소 (원자력 발전·컴퓨터·인공지능): 상품은 한 개예요. 숫자는 직원 일 탭에서 올라가요 ---------- */
  const LAB_TITLE = { nuclear: "원자력 발전", pc: "컴퓨터", ai: "인공지능" };
  const LAB_ABS_MAX = 100000; // 한 번에 만들 수 있는 전기 양의 절대 상한
  let genAmt = 100; // 만들 전기 양 (입력 칸)
  let labNow = null; // 지금 그린 연구실 숫자. 발전 중 남은 시간은 서버를 다시 읽지 않고 이 값으로 세요
  let labTick = 0;
  const fmtLab = (n) => Number(n || 0).toLocaleString("ko-KR");
  // 센트를 토큰으로 보여요 (100센트 = 1 토큰)
  const fmtTok = (cents) => (Number(cents || 0) / 100).toLocaleString("ko-KR", { maximumFractionDigits: 2 });
  const fmtSec = (s) => (s >= 60 ? `${Math.floor(s / 60)}분 ${s % 60}초` : `${s}초`);
  const maxGen = (L) => Math.min(LAB_ABS_MAX, 10 * L.nperf); // 한 번에 최대: 핵연료 성능 × 10
  const genLeft = (L) => Math.max(0, Math.ceil((L.gen.started.toMillis() + L.gen.secs * 1000 - Date.now()) / 1000));
  const genText = (L, left) => (left > 0 ? `${fmtLab(L.gen.amt)} 전기를 만들고 있어요 · 남은 시간 ${fmtSec(left)}` : `${fmtLab(L.gen.amt)} 전기를 다 만들었어요`);
  // 예상: 걸리는 시간과 비용은 같아요 (1초 = 1 토큰). 성능이 높을수록 둘 다 줄어요
  const planText = (L) => {
    if (!(genAmt >= 1 && genAmt <= maxGen(L))) return `1부터 ${fmtLab(maxGen(L))} 사이로 적어 주세요`;
    const secs = Math.ceil((genAmt * 100) / L.nperf);
    return `비용 ${fmtLab(secs)} 토큰 · 약 ${fmtSec(secs)} 걸려요${L.cash < secs * 100 ? " · 금고가 부족해요" : ""}`;
  };
  // 회사 금고 줄 (모든 회사 공통): 넣기·빼기
  const cashHTML = (L) => `<div class="lab-cash">
      <p class="lab-sub">회사 금고 <b>${fmtTok(L.cash)}</b> 토큰</p>
      <label class="lab-field"><span>넣거나 뺄 토큰</span><input id="cash-amt" class="lab-input" type="text" inputmode="numeric" autocomplete="off" maxlength="9" value="100" aria-label="금고에 넣거나 뺄 토큰"></label>
      <div class="lab-actions"><button type="button" class="btn" data-cash="out">빼기</button><button type="button" class="btn primary" data-cash="in">넣기</button></div>
    </div>`;
  // 단가 줄 (사장이 정해요): 파는 한 단위의 가격을 토큰으로 적어요
  const priceHTML = (L, label) => `<div class="lab-price">
      <label class="lab-field"><span>${label}</span><span class="lab-unit"><input id="price-amt" class="lab-input" type="text" inputmode="decimal" autocomplete="off" maxlength="9" value="${fmtTok(L.price)}" aria-label="단가 (토큰)"><b>토큰</b></span></label>
      <button type="button" class="btn" data-lab="price">단가 정하기</button>
    </div>`;
  const labHTML = (kind, L) => {
    if (kind === "nuclear") {
      const head = `<p class="lab-t">${LAB_TITLE[kind]} · 상품 1개: 전기</p>
        <p class="lab-big">${fmtLab(L.elec)}<small>전기 창고</small></p>
        <p class="lab-sub">핵연료 성능 ${fmtLab(L.nperf)} · 성능이 높을수록 적은 돈으로, 빨리, 많이 만들어요</p>`;
      if (L.gen) {
        const left = genLeft(L);
        return `${head}
          <p class="lab-sub" id="lab-left">${genText(L, left)}</p>
          <button class="btn primary" type="button" data-lab="claim" ${left > 0 ? "disabled" : ""}>창고에 받기</button>
          ${cashHTML(L)}${priceHTML(L, "전기 1단위 가격")}`;
      }
      return `${head}
        <label class="lab-field"><span>만들 전기</span><input id="lab-amt" class="lab-input" type="text" inputmode="numeric" autocomplete="off" maxlength="6" value="${genAmt}" aria-label="만들 전기 양"></label>
        <p class="lab-sub" id="lab-est">${planText(L)}</p>
        <button class="btn primary" type="button" data-lab="gen">발전 시작하기</button>
        ${cashHTML(L)}${priceHTML(L, "전기 1단위 가격")}`;
    }
    if (kind === "pc") {
      const rel = L.rel;
      const same = Boolean(rel) && rel.cap === L.cap && rel.spd === L.spd && rel.pow === L.pow;
      return `<p class="lab-t">${LAB_TITLE[kind]} · 상품 1개</p>
        <div class="lab-stats"><span>처리 용량<b>${fmtLab(L.cap)}</b></span><span>처리 속도<b>${fmtLab(L.spd)}</b></span><span>전기값<b>${fmtLab(L.pow)}</b></span></div>
        <p class="lab-sub">${rel ? `출시한 숫자 · 용량 ${fmtLab(rel.cap)} · 속도 ${fmtLab(rel.spd)} · 전기 ${fmtLab(rel.pow)}` : "아직 출시하지 않았어요"}</p>
        <button class="btn primary" type="button" data-lab="rel" ${same || !(L.cap || L.spd || L.pow) ? "disabled" : ""}>${rel ? "다시 출시하기" : "출시하기"}</button>
        ${cashHTML(L)}${priceHTML(L, "컴퓨터 1대 가격")}`;
    }
    const dev = L.aiDev;
    const released = L.aiScore >= 0;
    const trnBlock = L.trn
      ? (() => {
          const left = trnLeft(L);
          return `<p class="lab-sub" id="lab-trn-left">${trnText(L, left)}</p>
            <button class="btn primary" type="button" data-lab="train-claim" ${left > 0 ? "disabled" : ""}>학습 결과 받기</button>`;
        })()
      : `<label class="lab-field"><span>학습할 데이터 (MB)</span><input id="train-amt" class="lab-input" type="text" inputmode="numeric" autocomplete="off" maxlength="9" value="${L.aiData}" aria-label="학습할 데이터 양"></label>
        <p class="lab-sub" id="train-est">${trainPlan(L, L.aiData)}</p>
        <button class="btn primary" type="button" data-lab="train" ${L.pcN >= 1 ? "" : "disabled"}>학습 시작하기</button>`;
    return `<p class="lab-t">${LAB_TITLE[kind]} · 상품 1개</p>
      <p class="lab-big">${fmtLab(L.aiData)}<small>MB 쌓인 데이터</small></p>
      <p class="lab-sub">개발 점수 ${fmtLab(dev)}${released ? ` · 출시 점수 ${fmtLab(L.aiScore)}` : " · 아직 출시 전이에요"}</p>
      ${trnBlock}
      <button class="btn primary" type="button" data-lab="rel" ${dev < 1 || dev === L.aiScore ? "disabled" : ""}>${released ? "다시 출시하기" : "출시하기"}</button>
      ${cashHTML(L)}${priceHTML(L, "사용 1시간 가격")}
      <div class="lab-market" id="lab-market"><p class="lab-sub">시장을 불러오는 중이에요</p></div>`;
  };
  // 시장 (인공지능 회사): 전기 파는 곳과 컴퓨터 파는 곳의 단가·재고를 보고 사요
  const paintMarket = async (co, mine) => {
    const box = document.getElementById("lab-market");
    if (!box) return;
    try {
      const list = await ctx.cloud().listCompanies("");
      const pick = (kind) => list.filter((c) => c.kind === kind && c.id !== co.id).slice(0, 8);
      const nuc = pick("nuclear");
      const pcs = pick("pc");
      const [nl, pl] = await Promise.all([Promise.all(nuc.map((c) => ctx.cloud().labGet(c.id))), Promise.all(pcs.map((c) => ctx.cloud().labGet(c.id)))]);
      if (ctx.company()?.id !== co.id) return;
      const eRows = nuc.map((c, i) => ({ c, L: nl[i] })).filter(({ L }) => L.price > 0 && L.elec > 0);
      const pRows = pcs.map((c, i) => ({ c, L: pl[i] })).filter(({ L }) => L.price > 0 && L.rel);
      const row = (c, desc, kind, def) => `<div class="lab-row">
          <div class="lab-rowt"><b>${esc(c.name)}</b><span>${desc}</span></div>
          <div class="lab-rowbuy"><input id="buy-${c.id}" class="lab-input" type="text" inputmode="numeric" autocomplete="off" maxlength="6" value="${def}" aria-label="${esc(c.name)}에서 살 수량"><button type="button" class="btn primary" data-buy="${kind}" data-seller="${c.id}">사기</button></div>
        </div>`;
      box.innerHTML = `<p class="lab-t">전기 사기</p>
        ${eRows.length ? eRows.map(({ c, L }) => row(c, `1 전기 ${fmtTok(L.price)} 토큰 · 재고 ${fmtLab(L.elec)}`, "e", 10)).join("") : '<p class="lab-sub">지금 파는 원자력 발전 회사가 없어요</p>'}
        <p class="lab-t">컴퓨터 사기</p>
        ${pRows.length ? pRows.map(({ c, L }) => row(c, `1대 ${fmtTok(L.price)} 토큰 · 용량 ${fmtLab(L.rel.cap)} · 속도 ${fmtLab(L.rel.spd)} · 전기 ${fmtLab(L.rel.pow)}`, "p", 1)).join("") : '<p class="lab-sub">지금 파는 컴퓨터 회사가 없어요</p>'}
        <p class="lab-t">내 창고</p>
        <p class="lab-sub">전기 ${fmtLab(mine.elec)} · 컴퓨터 ${fmtLab(mine.pcN)}대${mine.pcM ? ` (용량 ${fmtLab(mine.pcM.cap)} · 속도 ${fmtLab(mine.pcM.spd)} · 전기 ${fmtLab(mine.pcM.pow)})` : ""}</p>`;
    } catch (err) {
      box.innerHTML = '<p class="lab-sub">시장을 불러오지 못했어요</p>';
    }
  };
  // 학습: 데이터 양으로 배치 수·전기·시간·점수를 미리 보여줘요 (규칙과 같은 식)
  const trainPlan = (L, d) => {
    if (!(L.pcN >= 1 && L.pcM)) return "먼저 컴퓨터를 사야 학습할 수 있어요";
    if (!(d >= 1 && d <= L.aiData)) return `1부터 ${fmtLab(L.aiData)} MB 사이로 적어 주세요`;
    const batches = Math.ceil(d / L.pcM.cap);
    const used = batches * L.pcM.pow;
    const secs = Math.ceil(batches / L.pcM.spd);
    const score = Math.floor((d * (L.pcM.cap + L.pcM.spd)) / 100);
    return `전기 ${fmtLab(used)} 사용 · 약 ${fmtSec(secs)} 걸려요 · 개발 점수 ${fmtLab(score)}${L.elec < used ? " · 전기가 부족해요" : ""}`;
  };
  const trnLeft = (L) => Math.max(0, Math.ceil((L.trn.started.toMillis() + L.trn.secs * 1000 - Date.now()) / 1000));
  const trnText = (L, left) => (left > 0 ? `${fmtLab(L.trn.d)} MB를 학습하고 있어요 · 남은 시간 ${fmtSec(left)}` : `${fmtLab(L.trn.d)} MB 학습을 마쳤어요`);
  const stopTick = () => {
    clearInterval(labTick);
    labTick = 0;
  };
  // 발전 중에는 1초마다 남은 시간만 고쳐요. 끝나면 받기 버튼을 켜요
  const startTick = () => {
    stopTick();
    labTick = setInterval(() => {
      const box = document.getElementById("biz-lab");
      const L = labNow;
      if (!box || !L || (!L.gen && !L.trn)) return stopTick();
      let busy = false;
      if (L.gen) {
        const left = genLeft(L);
        if (left > 0) busy = true;
        const t = box.querySelector("#lab-left");
        if (t) t.textContent = genText(L, left);
        const btn = box.querySelector('[data-lab="claim"]');
        if (btn && left <= 0) btn.disabled = false;
      }
      if (L.trn) {
        const left = trnLeft(L);
        if (left > 0) busy = true;
        const t = box.querySelector("#lab-trn-left");
        if (t) t.textContent = trnText(L, left);
        const btn = box.querySelector('[data-lab="train-claim"]');
        if (btn && left <= 0) btn.disabled = false;
      }
      if (!busy) stopTick();
    }, 1000);
  };
  const paintLab = async () => {
    const co = ctx?.company();
    const box = document.getElementById("biz-lab");
    if (!co || co.kind === "food" || !box) return;
    stopTick();
    try {
      const L = await ctx.cloud().labGet(co.id);
      if (ctx.company()?.id !== co.id) return; // 다른 회사로 바뀌었으면 그리지 않아요
      labNow = L;
      box.innerHTML = labHTML(co.kind, L);
      if (co.kind === "ai") paintMarket(co, L);
      if ((L.gen && genLeft(L) > 0) || (L.trn && trnLeft(L) > 0)) startTick();
    } catch (err) {
      box.innerHTML = '<p class="lab-sub">연구실 숫자를 불러오지 못했어요</p>';
    }
  };
  // 만들 전기 양을 적을 때: 숫자만 남기고, 예상 비용과 시간을 바로 고쳐요
  const labInput = (e) => {
    const inp = e.target.closest("#lab-amt");
    if (!inp || !labNow) return;
    const digits = inp.value.replace(/\D/g, "").slice(0, 6);
    if (inp.value !== digits) inp.value = digits;
    genAmt = digits === "" ? 0 : Number(digits);
    const est = document.getElementById("lab-est");
    if (est) est.textContent = planText(labNow);
  };
  const trainInput = (e) => {
    if (!e.target.closest("#train-amt") || !labNow) return;
    const est = document.getElementById("train-est");
    if (est) est.textContent = trainPlan(labNow, Number(String(e.target.value).replace(/,/g, "")) || 0);
  };
  // 칸에 적은 숫자 읽기 (쉼표는 무시해요). 숫자가 아니면 NaN
  const readNum = (id) => {
    const el = document.getElementById(id);
    return el ? Number(String(el.value).replace(/,/g, "").trim()) : NaN;
  };
  const labClick = async (e) => {
    const co = ctx?.company();
    if (!co) return;
    const cash = e.target.closest("[data-cash]");
    if (cash && !cash.disabled) {
      const n = readNum("cash-amt");
      if (!Number.isInteger(n) || n < 1 || n > 1000000000) return ctx.toast("1 이상의 토큰 수를 적어 주세요");
      cash.disabled = true;
      const dir = cash.dataset.cash === "in" ? 1 : -1;
      try {
        await ctx.settle(); // 쌓인 이자를 먼저 받아 두어요
        await ctx.walletOp(() => ctx.cloud().labCash(co.id, n, dir));
        ctx.toast(dir > 0 ? `${fmtLab(n)} 토큰을 금고에 넣었어요` : `${fmtLab(n)} 토큰을 지갑으로 뺐어요`);
      } catch (err) {
        ctx.toast(err?.code === "insufficient" ? "토큰이 부족해요" : "처리하지 못했어요. 잠시 뒤 다시 해 주세요");
      } finally {
        paintLab();
      }
      return;
    }
    const buy = e.target.closest("[data-buy]");
    if (buy && !buy.disabled) {
      const seller = buy.dataset.seller;
      const kind = buy.dataset.buy;
      const n = readNum(`buy-${seller}`);
      if (!Number.isInteger(n) || n < 1) return ctx.toast("1 이상의 수량을 적어 주세요");
      buy.disabled = true;
      try {
        await ctx.cloud().labBuy(co.id, seller, kind, n);
        ctx.toast(kind === "e" ? `전기 ${fmtLab(n)}을 샀어요` : `컴퓨터 ${fmtLab(n)}대를 샀어요`);
      } catch (err) {
        const msg = { insufficient: "금고에 토큰이 부족해요", stock: "재고가 부족해요", unreleased: "아직 출시하지 않은 상품이에요", bad: "수량을 확인해 주세요" }[err?.code];
        ctx.toast(msg || "처리하지 못했어요. 잠시 뒤 다시 해 주세요");
      } finally {
        paintLab();
      }
      return;
    }
    const act = e.target.closest("[data-lab]");
    if (!act || act.disabled) return;
    if (act.dataset.lab === "gen" && !(Number.isInteger(genAmt) && genAmt >= 1 && labNow && genAmt <= maxGen(labNow))) {
      return ctx.toast(`1부터 ${fmtLab(labNow ? maxGen(labNow) : 1000)} 사이로 적어 주세요`);
    }
    if (act.dataset.lab === "price" && !(readNum("price-amt") >= 0 && readNum("price-amt") <= 1000000)) {
      return ctx.toast("0 이상의 토큰 수를 적어 주세요");
    }
    act.disabled = true;
    try {
      if (act.dataset.lab === "gen") {
        const r = await ctx.cloud().labGenStart(co.id, genAmt);
        ctx.toast({ ok: `전기 ${fmtLab(genAmt)}을 만들기 시작했어요`, busy: "이미 발전 중이에요", poor: "금고에 토큰이 부족해요", max: "한 번에 만들 수 있는 양을 넘었어요" }[r] || "처리하지 못했어요");
      } else if (act.dataset.lab === "train") {
        const d = readNum("train-amt");
        const r = await ctx.cloud().labTrainStart(co.id, d);
        ctx.toast({ ok: `${fmtLab(d)} MB 학습을 시작했어요`, busy: "이미 학습 중이에요", nocomp: "먼저 컴퓨터를 사야 해요", nodata: "데이터 양을 확인해 주세요", poor: "전기가 부족해요" }[r] || "처리하지 못했어요");
      } else if (act.dataset.lab === "train-claim") {
        const got = await ctx.cloud().labTrainClaim(co.id);
        ctx.toast(got >= 0 ? `학습 결과: 개발 점수 ${fmtLab(got)}` : "아직 학습 중이에요");
      } else if (act.dataset.lab === "claim") {
        const got = await ctx.cloud().labGenClaim(co.id);
        ctx.toast(got ? `전기 ${fmtLab(got)}을 창고에 받았어요` : "아직 발전 중이에요");
      } else if (act.dataset.lab === "price") {
        const cents = Math.round(readNum("price-amt") * 100);
        await ctx.cloud().labChange(co.id, (L) => ({ ...L, price: cents }));
        ctx.toast("단가를 정했어요");
      } else if (co.kind === "pc") {
        await ctx.cloud().labChange(co.id, (L) => ({ ...L, rel: { cap: L.cap, spd: L.spd, pow: L.pow } }));
        ctx.toast("컴퓨터를 출시했어요");
      } else {
        await ctx.cloud().labChange(co.id, (L) => ({ ...L, aiScore: L.aiDev }));
        ctx.toast("인공지능을 출시했어요");
      }
    } catch (err) {
      ctx.toast("처리하지 못했어요. 잠시 뒤 다시 해 주세요");
    } finally {
      paintLab();
    }
  };
  // 회사 종류 그림 (일 탭에서도 같은 그림을 써요)
  const kindSvg = (k) => svgI(ICON[k === "pc" ? "pc" : k === "ai" ? "ai" : k === "nuclear" ? "nuclear" : "food"]);

  // 사업 탭을 열 때: 연구실 숫자를 다시 읽어요 (상품 목록은 그대로 두어요)
  const lab = () => {
    const co = ctx?.company();
    if (co && co.kind !== "food") paintLab();
  };
  window.Business = { init, refresh, kindSvg, lab };
})();
