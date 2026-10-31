/* 사업: 팔 상품 정하기 (전체 화면), 판매 목록, 원가 계산.
   식품은 물·음식·에너지 식품 가운데 고르고, 모양·뚜껑·내용물·색을 차례로 정해요.
   컴퓨터와 인공지능은 자리만 있어요. 상품 그림은 사진이 아니라 이 파일의 도형(SVG)이에요. */
(() => {
  "use strict";

  const SPRING = "cubic-bezier(0.2, 0.9, 0.25, 1.06)";
  const RM = matchMedia("(prefers-reduced-motion: reduce)");
  const ML = [330, 500, 600, 750, 1000, 1250, 1500, 1800, 2000, 2500]; // 페트병 용량 10단계
  const ALL = Array.from({ length: 50 }, (_, k) => k);
  const ICON_COL = ["#ffd9b8", "#c8e7ff", "#d8f5d0", "#ffd6e8", "#e4dcff"];

  const r1 = (n) => Math.round(n * 10) / 10;
  const won = (n) => Number(n).toLocaleString("ko-KR");
  const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
  const svgI = (d) =>
    `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

  // 아이콘은 모두 검은 윤곽선이에요 (선 굵기 1.8, 둥근 끝)
  const ICON = {
    food: '<path d="M3.5 14.5h17M5.5 14.5a6.5 6.5 0 0 1 13 0M9 18.5h6"/>',
    pc: '<rect x="3" y="4.5" width="18" height="12.5" rx="2.5"/><path d="M9 20.5h6M12 17v3.5"/>',
    ai: '<rect x="6.5" y="6.5" width="11" height="11" rx="2.5"/><rect x="10" y="10" width="4" height="4" rx="1"/><path d="M9.5 3.5v3M14.5 3.5v3M9.5 17.5v3M14.5 17.5v3M3.5 9.5h3M3.5 14.5h3M17.5 9.5h3M17.5 14.5h3"/>',
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
    chev: '<path d="M9 5l7 7-7 7"/>',
    back: '<path d="M15 5l-7 7 7 7"/>',
    check: '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
  };

  const Shapes = window.Shapes; // 그림은 app/shapes.js 에서 가져와요
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
                { id: "jelly", label: "젤리", icon: "bag", hint: "봉지 모양과 색", leaf: "jelly" },
                { id: "chips", label: "과자", icon: "bag", hint: "봉지 모양과 색", leaf: "chips" },
                { id: "bread", label: "빵", icon: "bag", hint: "봉지 모양과 색", leaf: "bread" },
              ],
            },
            {
              id: "plate", label: "식사", icon: "plate", hint: "도시락, 고기",
              children: [
                { id: "lunchbox", label: "도시락", icon: "box", hint: "용기 모양과 색", leaf: "lunchbox" },
                { id: "meat", label: "고기", icon: "box", hint: "용기 모양과 색", leaf: "meat" },
              ],
            },
          ],
        },
        {
          id: "energy", label: "에너지 식품", icon: "bolt", hint: "에너지바, 음료, 커피",
          children: [
            { id: "bar", label: "에너지바", icon: "wrap", hint: "봉투, 내용물, 색", leaf: "bar" },
            { id: "drink", label: "에너지 음료", icon: "can", hint: "용기, 내용물, 색", leaf: "drink" },
            { id: "coffee", label: "커피", icon: "cup", hint: "용기, 내용물, 색", leaf: "coffee" },
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

  // 잎 상품: 어떤 도형을 쓰고, 어떤 순서로 고르는지
  // 잎 상품: 어떤 그림 모음을 쓰고, 어떤 순서로 고르는지
  const CONTENTS = {
    bar: ["초코", "바닐라", "딸기", "녹차", "땅콩", "블루베리"],
    drink: ["레몬", "청포도", "복숭아", "베리", "콜라", "자몽"],
    coffee: ["아메리카노", "라떼", "바닐라 라떼", "콜드브루", "모카", "카라멜 마키아토"],
  };
  const LEAF = {
    water: { label: "물", group: "water", family: "bottle", kinds: Shapes.KINDS.bottle, flow: ["bottle", "cap", "capColor"], ingredient: 1 },
    jelly: { label: "젤리", group: "snack", family: "bag", kinds: Shapes.KINDS.bag.jelly, flow: ["shape", "color"], ingredient: 1, shapeTitle: "젤리 용기 모양", colorTitle: "용기 색" },
    chips: { label: "과자", group: "snack", family: "bag", kinds: Shapes.KINDS.bag.chips, flow: ["shape", "color"], ingredient: 1, shapeTitle: "과자 봉지 모양", colorTitle: "봉지 색" },
    bread: { label: "빵", group: "snack", family: "bag", kinds: Shapes.KINDS.bag.bread, flow: ["shape", "color"], ingredient: 2, shapeTitle: "빵 봉지 모양", colorTitle: "봉지 색" },
    lunchbox: { label: "도시락", group: "meal", family: "box", kinds: Shapes.KINDS.box, flow: ["shape", "color"], ingredient: 3, shapeTitle: "도시락 용기 모양", colorTitle: "용기 색" },
    meat: { label: "고기", group: "meal", family: "meat", kinds: Shapes.KINDS.meat, flow: ["shape", "color"], ingredient: 0, shapeTitle: "고기 모양", colorTitle: "트레이 색" },
    bar: { label: "에너지바", group: "energy", family: "wrap", kinds: Shapes.KINDS.wrap, flow: ["shape", "contents", "color"], ingredient: 1, shapeTitle: "포장 모양", colorTitle: "포장 색", contents: CONTENTS.bar },
    drink: { label: "에너지 음료", group: "energy", family: "can", kinds: Shapes.KINDS.can, flow: ["shape", "contents", "color"], ingredient: 1, shapeTitle: "에너지 캔 모양", colorTitle: "캔 색", contents: CONTENTS.drink },
    coffee: { label: "커피", group: "energy", family: "cup", kinds: Shapes.KINDS.cup, flow: ["shape", "contents", "color"], ingredient: 2, shapeTitle: "컵 모양", colorTitle: "컵 색", contents: CONTENTS.coffee },
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

  /* ---------- 이름, 원가 ---------- */
  const sizeOf = (ml) => (ml >= 1000 ? `${+(ml / 1000).toFixed(2)}L` : `${ml}ml`);
  const nameOf = (p) => {
    const leaf = LEAF[p.item];
    if (p.item === "water") return `생수 ${sizeOf(Shapes.BOTTLE_ML[(p.shape ?? 0) % 5])}`;
    const flavor = leaf.contents && p.content >= 0 ? ` · ${leaf.contents[p.content]}` : "";
    return `${leaf.label}${flavor}`.slice(0, 30);
  };
  // 원가: 용기(크기에 따라), 뚜껑, 고기, 내용물, 로고 인쇄를 더해요 (모두 토큰 정수)
  const costOf = (p) => {
    const leaf = LEAF[p.item];
    const s = (p.shape ?? 0) % 5;
    const lines = [];
    if (leaf.family === "bottle") {
      lines.push(["페트병", Math.ceil(Shapes.BOTTLE_ML[s] / 500)]);
      if (p.cap >= 0) lines.push(["뚜껑", Math.floor(p.cap / 5) === 1 ? 2 : 1]);
    } else if (leaf.family === "meat") {
      lines.push(["트레이", 2]);
      lines.push(["고기", Shapes.MEAT_COST[leaf.kinds[Math.floor((p.shape ?? 0) / 5)]] + Math.floor(s / 2)]);
    } else {
      lines.push([CONTAINER[leaf.family], 1 + Math.floor(s / 2)]);
    }
    if (leaf.family !== "meat") {
      const inside = leaf.ingredient + (leaf.contents && p.content >= 0 ? 1 + Math.floor(p.content / 2) : 0);
      if (inside) lines.push(["내용물", inside]);
    }
    lines.push(["로고 인쇄", 1]);
    return { lines, total: lines.reduce((t, l) => t + l[1], 0) };
  };

  /* ---------- 그림 (app/shapes.js 도형을 SVG로 그려요) ---------- */
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
  const pkvHTML = (p, mini = false) => {
    const leaf = LEAF[p.item];
    const k = p.shape ?? 0;
    const r = Shapes.body(leaf.family, leaf.kinds[Math.floor(k / 5)] ?? leaf.kinds[0], k % 5);
    const water = leaf.family === "bottle";
    const fill = water ? "#eef6fb" : p.color >= 0 ? colorOf(p.color) : "#ffffff";
    let cap = "";
    if (water && p.cap >= 0) {
      const c = Shapes.cap(Shapes.KINDS.cap[Math.floor(p.cap / 5)], r.nr, r.top, p.cap % 5);
      cap = `<path class="pk-body" d="${c.body}" fill="${p.capColor >= 0 ? colorOf(p.capColor) : "#2b2b2f"}"/>${c.det ? `<path class="pk-det" d="${c.det}"/>` : ""}`;
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
  let ctx = null;
  let pk = null;
  let stage = null;
  let prev = null;
  let titleEl = null;
  let crumbEl = null;
  let backEl = null;
  let products = [];
  let hist = [];
  let pick = null;
  let busy = false;
  let lastPrev = "";
  const RMOFF = () => RM.matches;
  const anim = (el, frames, opts) => (RMOFF() ? null : el.animate(frames, { fill: "both", ...opts }));

  const titleOf = (e) => {
    if (e.kind === "root") return "팔 상품 정하기";
    if (e.kind === "list" || e.kind === "soon") return e.node.label;
    if (e.kind === "step") return stepInfo(e.key, LEAF[pick.item]).title;
    return "판매 가격 정하기";
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
      const cur = pick[st.field];
      const tiles = ALL.map((k) => `<button class="pk-tile${cur === k ? " is-on" : ""}" type="button" data-field="${st.field}" data-v="${k}" style="--i:${k}">${thumbHTML(st.family, st.kinds[Math.floor(k / 5)], k % 5)}</button>`);
      return `<div class="pk-grid">${tiles.join("")}</div>`;
    }
    if (st.kind === "color") {
      return `<div class="pk-grid pk-swgrid">${ALL.map((k) => `<button class="pk-sw${pick[st.field] === k ? " is-on" : ""}" type="button" data-field="${st.field}" data-v="${k}" style="--i:${k};--c:${colorOf(k)}" aria-label="색 ${k + 1}"></button>`).join("")}</div>`;
    }
    return `<div class="pk-list">${leaf.contents
      .map((c, i) => `<button class="pk-row${pick.content === i ? " is-on" : ""}" type="button" data-field="content" data-v="${i}" style="--i:${i}"><span class="pk-ic" style="--c:${ICON_COL[i % ICON_COL.length]}">${svgI(ICON.dot)}</span><span class="pk-main"><b>${esc(c)}</b></span><span class="pk-chev">${svgI(ICON.chev)}</span></button>`)
      .join("")}</div>`;
  };

  const summaryHTML = () => {
    const c = costOf(pick);
    if (pick.price == null) pick.price = c.total * 2;
    const rows = c.lines.map(([n, v]) => `<div class="pk-cost-row"><span>${n}</span><b>${won(v)} 토큰</b></div>`).join("");
    const sug = c.total * 2;
    return `<div class="pk-sum">
      <h2 class="pk-sum-name">${esc(nameOf(pick))}</h2>
      <p class="pk-sum-sub">${esc(crumbOf())}</p>
      <div class="pk-card">
        <p class="pk-lab">원가 계산</p>
        ${rows}
        <div class="pk-cost-row pk-cost-total"><span>원가 합계</span><b>${won(c.total)} 토큰</b></div>
      </div>
      <div class="pk-card">
        <p class="pk-lab">팔 가격 · 1 토큰 = 1달러</p>
        <input id="pk-input" class="pk-input" inputmode="numeric" autocomplete="off" value="${pick.price}" aria-label="팔 가격">
        <div class="pk-steps">${[-10, -1, 1, 10].map((d) => `<button class="pk-step" type="button" data-step="${d}">${d > 0 ? "+" : "−"}${Math.abs(d)}</button>`).join("")}</div>
        <button class="pk-chip" type="button" data-suggest="${sug}">추천 ${won(sug)} 토큰 (원가의 2배)</button>
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

  // 가격과 이익을 다시 써요 (입력칸은 건드리지 않아요)
  const paintProfit = () => {
    const el = pk?.querySelector("#pk-profit");
    if (!el || !pick) return;
    const c = costOf(pick);
    const pr = pick.price || 0;
    const gain = pr - c.total;
    el.classList.toggle("loss", pr >= 1 && gain < 0);
    el.textContent =
      pr < 1
        ? "가격을 1 토큰 이상으로 정해 주세요"
        : gain > 0
          ? `한 개에 ${won(gain)} 토큰 남아요 (이익률 ${Math.round((gain / pr) * 100)}%)`
          : gain === 0
            ? "원가와 같아요. 남는 돈이 없어요"
            : `원가보다 ${won(-gain)} 토큰 낮아요`;
  };

  /* ---------- 그림 칸 (위쪽 미리보기) ---------- */
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
    if (e.kind === "step" || e.kind === "summary") setPrev(pkvHTML(pick), e.kind === "summary", true);
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
    if (RMOFF()) {
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
    const e = hist[hist.length - 1];
    titleEl.textContent = titleOf(e);
    crumbEl.textContent = hist.length > 1 ? crumbOf() : "";
    backEl.setAttribute("aria-label", hist.length > 1 ? "뒤로" : "닫기");
    anim(titleEl, [{ opacity: 0.2, transform: "translateY(4px)" }, { opacity: 1, transform: "none" }], { duration: 320, easing: SPRING, fill: "backwards" });
  };
  const show = (e, dir) => {
    swap(makeView(viewHTML(e)), dir);
    syncTop();
    renderPrev(e);
    if (e.kind === "summary") requestAnimationFrame(paintProfit);
  };
  const push = (e) => {
    hist.push(e);
    show(e, 1);
  };
  const pop = () => {
    if (hist.length <= 1) return close();
    hist.pop();
    show(hist[hist.length - 1], -1);
  };
  const advance = () => {
    const top = hist[hist.length - 1];
    if (!top || top.kind !== "step") return;
    const flow = LEAF[pick.item].flow;
    const i = flow.indexOf(top.key);
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
      pick = { item: n.leaf, shape: 0, cap: -1, capColor: -1, color: -1, content: -1 };
      push({ kind: "step", key: LEAF[n.leaf].flow[0] });
    }
  };
  // 모양·색·내용물 하나를 고르면 잠깐 빛나고, 다음 칸으로 넘어가요
  const choose = (field, v, el) => {
    if (busy || !pick) return;
    busy = true;
    pick[field] = v;
    stage.querySelectorAll(`[data-field="${field}"]`).forEach((s) => s.classList.toggle("is-on", s === el));
    anim(el, [{ transform: "scale(1)" }, { transform: "scale(.92)" }, { transform: "scale(1.05)" }, { transform: "scale(1)" }], { duration: 420, easing: "ease-out" });
    if (navigator.vibrate) navigator.vibrate(6);
    setPrev(pkvHTML(pick), false, true);
    setTimeout(() => {
      busy = false;
      advance();
    }, 430);
  };

  /* ---------- 가격 ---------- */
  const setPrice = (v) => {
    pick.price = Math.min(99999999, Math.max(0, Math.round(v)));
    const inp = pk.querySelector("#pk-input");
    if (inp) inp.value = pick.price;
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
    if (!(pick.price >= 1)) {
      err.textContent = "가격을 1 토큰 이상으로 정해 주세요";
      return;
    }
    if (!ctx.company()) {
      err.textContent = "먼저 창업해야 팔 수 있어요";
      return;
    }
    busy = true;
    err.textContent = "";
    const leaf = LEAF[pick.item];
    const c = costOf(pick);
    const p = {
      cat: "food",
      group: leaf.group,
      item: pick.item,
      name: nameOf(pick),
      shape: pick.shape,
      color: pick.color,
      cost: c.total,
      price: pick.price,
    };
    if (leaf.family === "bottle") {
      p.cap = pick.cap;
      p.capColor = pick.capColor;
    }
    if (leaf.contents && pick.content >= 0) p.content = pick.content;
    btn.classList.add("busy");
    try {
      await ctx.cloud().createProduct(p);
      burst();
      await refresh();
      ctx.toast(`${p.name}을(를) 팔기 시작했어요`);
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
    hist = [];
    pick = null;
    stage.innerHTML = "";
    lastPrev = "";
    setPrev("", false, false);
    hist.push({ kind: "root" });
    stage.appendChild(makeView(viewHTML(hist[0])));
    syncTop();
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
      pk.querySelectorAll(".pk-done").forEach((d) => d.remove());
    };
    if (RMOFF() || pk.hidden) return done();
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
          `<li class="biz-item" style="--i:${i}"><span class="biz-thumb">${pkvHTML(p, true)}</span><span class="biz-main"><b>${esc(p.name)}</b><span>판매 ${won(p.price)} 토큰 · 원가 ${won(p.cost)}</span></span><button class="biz-end" type="button" data-end="${esc(p.id)}">판매 종료</button></li>`
      )
      .join("");
    document.getElementById("biz-empty").hidden = products.length > 0;
  };
  async function refresh() {
    const cloud = ctx?.cloud();
    if (!cloud?.user || !ctx.company()) {
      products = [];
      paint();
      return;
    }
    try {
      products = (await cloud.listProducts()).sort((a, b) => (b.at || 0) - (a.at || 0));
    } catch (err) {
      console.warn("상품을 불러오지 못했어요:", err?.code || err);
      return;
    }
    paint();
  }
  const onList = async (e) => {
    const b = e.target.closest("[data-end]");
    if (!b || busy) return;
    const id = b.dataset.end;
    const p = products.find((x) => x.id === id);
    const ok = await ctx.ask({
      title: `"${p?.name || "이 상품"}" 판매를 끝낼까요?`,
      body: "끝내면 이 상품은 더 이상 팔리지 않아요. 다시 팔려면 새로 만들어야 해요.",
      ok: "판매 종료",
      cancel: "취소",
    });
    if (!ok) return;
    const item = b.closest(".biz-item");
    item?.classList.add("out");
    try {
      await ctx.cloud().endProduct(id);
      products = products.filter((x) => x.id !== id);
      setTimeout(paint, 240);
      ctx.toast(`${p?.name || "상품"} 판매를 끝냈어요`);
    } catch (err) {
      item?.classList.remove("out");
      ctx.toast("판매를 끝내지 못했어요");
    }
  };

  /* ---------- 이벤트 ---------- */
  const onStage = (e) => {
    if (busy) return;
    const orb = e.target.closest(".pk-orb");
    if (orb) return pickRoot(Number(orb.dataset.orb), orb);
    const node = e.target.closest("[data-node]");
    if (node) return chooseNode(node.dataset.node);
    const f = e.target.closest("[data-field]");
    if (f) return choose(f.dataset.field, Number(f.dataset.v), f);
    const st = e.target.closest("[data-step]");
    if (st) return setPrice(Math.max(1, (pick.price || 0) + Number(st.dataset.step)));
    const sg = e.target.closest("[data-suggest]");
    if (sg) return setPrice(Number(sg.dataset.suggest));
    if (e.target.closest("#pk-sell")) return sell();
  };
  const onInput = (e) => {
    if (e.target.id !== "pk-input" || !pick) return;
    const digits = e.target.value.replace(/\D/g, "").slice(0, 8);
    e.target.value = digits;
    pick.price = Number(digits) || 0;
    paintProfit();
  };
  const onBlur = (e) => {
    if (e.target.id !== "pk-input" || !pick) return;
    pick.price = Math.max(1, pick.price || 1);
    e.target.value = pick.price;
    paintProfit();
  };

  // 사업 탭에서 쓰는 입구: 상품 정하기 버튼, 판매 종료 버튼
  const init = (c) => {
    ctx = c;
    pk = document.createElement("section");
    pk.className = "pk";
    pk.hidden = true;
    pk.dataset.volatile = "";
    pk.setAttribute("aria-label", "팔 상품 정하기");
    pk.innerHTML = `<header class="pk-top"><button class="pk-glass" type="button" data-back aria-label="닫기">${svgI(ICON.back)}</button><div class="pk-title"><b></b><span></span></div><span class="pk-spacer" aria-hidden="true"></span></header>
      <div class="pk-prev" aria-hidden="true"></div>
      <div class="pk-stage"></div>`;
    document.querySelector(".app").appendChild(pk);
    stage = pk.querySelector(".pk-stage");
    prev = pk.querySelector(".pk-prev");
    titleEl = pk.querySelector(".pk-title b");
    crumbEl = pk.querySelector(".pk-title span");
    backEl = pk.querySelector("[data-back]");
    backEl.addEventListener("click", () => {
      if (!busy) pop();
    });
    stage.addEventListener("click", onStage);
    stage.addEventListener("input", onInput);
    stage.addEventListener("focusout", onBlur);
    document.getElementById("biz-pick").addEventListener("click", open);
    document.getElementById("biz-list").addEventListener("click", onList);
  };

  window.Business = { init, refresh };
})();
