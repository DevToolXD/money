/* 리퀴드 글라스 (CSS로 그린 유리 전부): Kyant Liquid Glass의 계산을 웹으로 옮겼어요.
   원본: https://github.com/Kyant0/AndroidLiquidGlass (Copyright 2025 Kyant, Apache License 2.0)

   - 45도 빛 (Highlight.Default): 가장자리 방향이 45도 빛과 나란할수록 밝은 0.5px 흰 선. 큰 판(시트·카드)은 고른 흰 선 (Highlight.Plain)
   - 채도 1.5배 (vibrancy)
   - 굴절 (lens): 가장자리에서 안쪽까지 원호 모양으로 뒤 화면을 끌어와요. 버튼 lens(12, 24), 큰 판 lens(24, 48, 깊이)
     브라우저가 SVG 필터를 backdrop-filter에 쓸 수 있을 때만 해요 (크롬·안드로이드). 아이폰 사파리는 아직 못 해서 흐림만 써요
   - 누르기 (InteractiveHighlight): 누른 자리에서 빛이 번지고, 살짝 부풀고, 끄는 쪽으로 따라와요 (스프링)
   탭 바 유리는 app/tabbar.js의 WebGL이 같은 계산으로 그려요. */
(() => {
  "use strict";

  // 유리로 그리는 것들 (어둡게 덮는 뒤판·잠금 화면처럼 유리가 아닌 것은 빼요)
  const SEL = [
    ".glass-btn", ".t-glass", ".plus", ".field-glass", ".ctx-menu", ".lg-card", ".lg-btn", ".alert-card", ".alert-btn",
    ".chart-seg", ".t-btn.grey", ".attach", ".a-ico", ".poll-opt", ".sheet.dark", ".draw-undo", ".pen-btn",
    ".status-bubble", ".fab-tile", ".found-logo", ".pk-glass", ".pk-orb", ".pk-row", ".pk-soon-orb", ".pk-card", ".pk-step",
    ".biz-more", ".biz-tag", ".bz-sheet-t", ".bz-sheet-g", ".bz-cancel", ".bz-input", ".bz-btn", ".gg", ".gseg",
    ".lab-stats span", ".lab-input", ".lab-row", ".lab-reagent", ".notice-opt", ".inv-empty-glass", ".mg-sug-row", ".pk-ic",
  ].join(",");
  const PRESSABLE = "button, a, [role='button'], .pk-ic, .a-ico";
  const DPR = () => Math.min(3, window.devicePixelRatio || 1);
  const ua = navigator.userAgent;
  // backdrop-filter에 SVG 필터(url)를 쓸 수 있는 브라우저 (크롬 계열, 아이폰이 아닌 것)
  const LENS = !/[?&]lens=off/.test(location.search) && /Chrome\/\d+/.test(ua) && !/iPhone|iPad|iPod|CriOS|FxiOS|EdgiOS/.test(ua) && CSS.supports("backdrop-filter", "url(#a)");
  const RM = matchMedia("(prefers-reduced-motion: reduce)");

  /* ---------- Kyant 계산 ---------- */
  const sdRound = (x, y, hw, hh, r) => {
    const ax = Math.abs(x) - (hw - r);
    const ay = Math.abs(y) - (hh - r);
    return Math.hypot(Math.max(ax, 0), Math.max(ay, 0)) - r + Math.min(Math.max(ax, ay), 0);
  };
  const gradRound = (x, y, hw, hh, r) => {
    const ax = Math.abs(x) - (hw - r);
    const ay = Math.abs(y) - (hh - r);
    const sx = x < 0 ? -1 : 1;
    const sy = y < 0 ? -1 : 1;
    if (ax >= 0 || ay >= 0) {
      const mx = Math.max(ax, 0);
      const my = Math.max(ay, 0);
      const l = Math.hypot(mx, my) || 1;
      return [(sx * mx) / l, (sy * my) / l];
    }
    return ay <= ax ? [sx, 0] : [0, sy];
  };
  const circleMap = (x) => 1 - Math.sqrt(Math.max(0, 1 - x * x));

  /* ---------- 45도 빛 테두리 그림 (크기마다 한 번 그려 두어요) ---------- */
  const rimCache = new Map();
  const rimURL = (w, h, r, plain) => {
    const k = DPR();
    const key = `${w}x${h}x${r}x${k}x${plain ? 1 : 0}`;
    if (rimCache.has(key)) return rimCache.get(key);
    const W = Math.max(1, Math.round(w * k));
    const H = Math.max(1, Math.round(h * k));
    const c = document.createElement("canvas");
    c.width = W;
    c.height = H;
    const ctx = c.getContext("2d");
    const img = ctx.createImageData(W, H);
    const hw = w / 2;
    const hh = h / 2;
    const gr = Math.min(r * 1.5, hw, hh);
    const put = (px, py) => {
      const x = (px + 0.5) / k - hw;
      const y = (py + 0.5) / k - hh;
      const sd = sdRound(x, y, hw, hh, r);
      // 0.5px 선 + 0.25px 흐림
      const band = Math.max(0, 1 - Math.abs(sd + 0.25) / 0.5);
      if (band <= 0) return;
      let a;
      if (plain) a = 0.38 * band;
      else {
        const g = gradRound(x, y, hw, hh, gr);
        a = 0.5 * Math.abs((g[0] + g[1]) * Math.SQRT1_2) * band;
      }
      const i = (py * W + px) * 4;
      img.data[i] = img.data[i + 1] = img.data[i + 2] = 255;
      img.data[i + 3] = Math.round(Math.min(1, a) * 255);
    };
    // 가장자리 가까운 곳만 계산해요 (안쪽은 비어 있어요)
    const edge = Math.ceil((r + 2) * k);
    for (let py = 0; py < H; py++) {
      if (py < edge || py >= H - edge) for (let px = 0; px < W; px++) put(px, py);
      else {
        for (let px = 0; px < Math.min(edge, W); px++) put(px, py);
        for (let px = Math.max(W - edge, 0); px < W; px++) put(px, py);
      }
    }
    ctx.putImageData(img, 0, 0);
    const url = `url("${c.toDataURL("image/png")}")`;
    rimCache.set(key, url);
    return url;
  };

  /* ---------- 굴절 지도 (SVG 변위 필터, 크롬만) ---------- */
  let svgRoot = null;
  const lensCache = new Map();
  const lensFilter = (w, h, r, height, amount, depth) => {
    const key = `${w}x${h}x${r}x${height}x${amount}x${depth ? 1 : 0}`;
    if (lensCache.has(key)) return lensCache.get(key);
    if (!svgRoot) {
      svgRoot = document.createElementNS("http://www.w3.org/2000/svg", "svg");
      svgRoot.setAttribute("width", "0");
      svgRoot.setAttribute("height", "0");
      svgRoot.setAttribute("aria-hidden", "true");
      svgRoot.style.cssText = "position:absolute;width:0;height:0;overflow:hidden;pointer-events:none";
      document.body.appendChild(svgRoot);
    }
    const c = document.createElement("canvas");
    c.width = Math.max(1, Math.round(w));
    c.height = Math.max(1, Math.round(h));
    const ctx = c.getContext("2d");
    const img = ctx.createImageData(c.width, c.height);
    const hw = w / 2;
    const hh = h / 2;
    const gr = Math.min(r * 1.5, hw, hh);
    const scale = amount * 2 + 2;
    for (let py = 0; py < c.height; py++) {
      for (let px = 0; px < c.width; px++) {
        const x = px + 0.5 - hw;
        const y = py + 0.5 - hh;
        let dx = 0;
        let dy = 0;
        const sd = sdRound(x, y, hw, hh, r);
        if (sd <= 0 && -sd < height) {
          const d = circleMap(1 + sd / height) * -amount;
          let [gx, gy] = gradRound(x, y, hw, hh, gr);
          if (depth) {
            const l = Math.hypot(x, y) || 1;
            gx += x / l;
            gy += y / l;
            const m = Math.hypot(gx, gy) || 1;
            gx /= m;
            gy /= m;
          }
          dx = d * gx;
          dy = d * gy;
        }
        const i = (py * c.width + px) * 4;
        img.data[i] = Math.round((0.5 + dx / scale) * 255);
        img.data[i + 1] = Math.round((0.5 + dy / scale) * 255);
        img.data[i + 2] = 128;
        img.data[i + 3] = 255;
      }
    }
    ctx.putImageData(img, 0, 0);
    const id = `lg-f${lensCache.size}`;
    const f = document.createElementNS("http://www.w3.org/2000/svg", "filter");
    f.setAttribute("id", id);
    f.setAttribute("x", "0");
    f.setAttribute("y", "0");
    f.setAttribute("width", String(w));
    f.setAttribute("height", String(h));
    f.setAttribute("filterUnits", "userSpaceOnUse");
    f.setAttribute("color-interpolation-filters", "sRGB");
    f.innerHTML = `<feImage href="${c.toDataURL("image/png")}" x="0" y="0" width="${w}" height="${h}" preserveAspectRatio="none" result="map"/><feDisplacementMap in="SourceGraphic" in2="map" scale="${scale}" xChannelSelector="R" yChannelSelector="G"/>`;
    svgRoot.appendChild(f);
    const v = `url(#${id})`;
    lensCache.set(key, v);
    return v;
  };

  /* ---------- 유리 붙이기 ---------- */
  const glass = new Set();
  const radiusOf = (el, w, h) => {
    const r = parseFloat(getComputedStyle(el).borderTopLeftRadius) || 0;
    return Math.min(r, w / 2, h / 2);
  };
  const paint = (el) => {
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    if (w < 4 || h < 4) return;
    const r = radiusOf(el, w, h);
    const big = h > 80 && w > 120; // 큰 판: 시트·카드·메뉴
    const st = el._lg;
    const key = `${w}x${h}x${r}`;
    if (st.key === key) return;
    st.key = key;
    const rim = rimURL(w, h, r, big);
    if (st.hl) st.hl.style.setProperty("--lg-rim", rim);
    else el.style.setProperty("--lg-rim", rim);
    if (LENS) {
      const f = big ? lensFilter(w, h, r, 24, 48, true) : lensFilter(w, h, r, 12, 24, false);
      const blur = big ? 16 : 2;
      el.style.setProperty("-webkit-backdrop-filter", `${f} blur(${blur}px) saturate(150%)`);
      el.style.setProperty("backdrop-filter", `${f} blur(${blur}px) saturate(150%)`);
    }
  };
  const resize = new ResizeObserver((entries) => {
    for (const e of entries) if (e.target._lg) paint(e.target);
  });
  const attach = (el) => {
    if (el._lg || el.closest(".tabbar")) return;
    const field = el.tagName === "INPUT" || el.tagName === "TEXTAREA";
    const st = { key: "", hl: null, p: 0, v: 0, target: 0, x: 0, y: 0, ox: 0, oy: 0, sx: 0, sy: 0 };
    el._lg = st;
    el.classList.add("lg");
    if (!field) {
      if (getComputedStyle(el).position === "static") el.classList.add("lg-rel");
      const hl = document.createElement("lg-hl");
      hl.setAttribute("aria-hidden", "true");
      el.appendChild(hl);
      st.hl = hl;
    } else {
      el.classList.add("lg-field");
    }
    glass.add(el);
    paint(el);
    resize.observe(el);
    if (!field && el.matches(PRESSABLE)) bindPress(el);
  };
  const scan = (root) => {
    if (root.nodeType !== 1) return;
    if (root.matches(SEL)) attach(root);
    root.querySelectorAll(SEL).forEach(attach);
  };

  /* ---------- 누르기: 빛 번짐 + 부풀기 + 끌기 (스프링 0.5, 300) ---------- */
  const active = new Set();
  let raf = 0;
  const step = (dt) => {
    for (const el of active) {
      const s = el._lg;
      // 감쇠비 0.5, 강성 300 (Kyant InteractiveHighlight와 같아요)
      const k = 300;
      const c = 2 * 0.5 * Math.sqrt(k);
      const a = -k * (s.p - s.target) - c * s.v;
      s.v += a * dt;
      s.p += s.v * dt;
      // 끄는 거리도 같은 스프링으로 돌아가요
      if (!s.down) {
        const ax = -k * s.ox - c * (s.vx || 0);
        const ay = -k * s.oy - c * (s.vy || 0);
        s.vx = (s.vx || 0) + ax * dt;
        s.vy = (s.vy || 0) + ay * dt;
        s.ox += s.vx * dt;
        s.oy += s.vy * dt;
      }
      apply(el);
      if (!s.down && Math.abs(s.p) < 0.001 && Math.abs(s.v) < 0.01 && Math.hypot(s.ox, s.oy) < 0.1) {
        s.p = 0;
        s.ox = s.oy = 0;
        apply(el);
        active.delete(el);
      }
    }
  };
  let last = 0;
  const loop = (t) => {
    const dt = Math.min(0.032, last ? (t - last) / 1000 : 0.016);
    last = t;
    step(dt);
    raf = active.size ? requestAnimationFrame(loop) : 0;
    if (!raf) last = 0;
  };
  const kick = () => {
    if (!raf) raf = requestAnimationFrame(loop);
  };
  const apply = (el) => {
    const s = el._lg;
    const p = Math.max(0, s.p);
    const w = el.offsetWidth || 1;
    const h = el.offsetHeight || 1;
    if (s.hl) {
      s.hl.style.setProperty("--lg-p", p.toFixed(3));
      s.hl.style.setProperty("--lg-x", `${Math.min(w, Math.max(0, s.x))}px`);
      s.hl.style.setProperty("--lg-y", `${Math.min(h, Math.max(0, s.y))}px`);
      s.hl.style.setProperty("--lg-r", `${Math.min(w, h) * 1.5}px`);
    }
    if (RM.matches) return;
    // 부풀기: 높이 기준 4px, 끄는 쪽으로 늘어나요. 따라오기: tanh로 부드럽게 멈춰요
    const md = Math.min(w, h);
    const tx = md * Math.tanh((0.05 * s.ox) / md);
    const ty = md * Math.tanh((0.05 * s.oy) / md);
    const base = 1 + (4 / h) * p;
    const ang = Math.atan2(s.oy, s.ox);
    const mx = Math.max(w, h);
    const ds = 4 / h;
    const sx = base + ds * Math.abs((Math.cos(ang) * s.ox) / mx) * Math.min(w / h, 1);
    const sy = base + ds * Math.abs((Math.sin(ang) * s.oy) / mx) * Math.min(h / w, 1);
    el.style.translate = p > 0.001 || s.ox || s.oy ? `${tx.toFixed(2)}px ${ty.toFixed(2)}px` : "";
    el.style.scale = p > 0.001 || s.ox || s.oy ? `${sx.toFixed(4)} ${sy.toFixed(4)}` : "";
  };
  const bindPress = (el) => {
    el.classList.add("lg-press");
    el.addEventListener("pointerdown", (e) => {
      if (e.pointerType === "mouse" && e.button !== 0) return;
      const s = el._lg;
      const r = el.getBoundingClientRect();
      s.down = true;
      s.sx = e.clientX;
      s.sy = e.clientY;
      s.x = e.clientX - r.left;
      s.y = e.clientY - r.top;
      s.ox = s.oy = 0;
      s.target = 1;
      active.add(el);
      kick();
    });
    const move = (e) => {
      const s = el._lg;
      if (!s.down) return;
      const r = el.getBoundingClientRect();
      s.x = e.clientX - r.left;
      s.y = e.clientY - r.top;
      s.ox = e.clientX - s.sx;
      s.oy = e.clientY - s.sy;
    };
    const up = () => {
      const s = el._lg;
      if (!s.down) return;
      s.down = false;
      s.target = 0;
      kick();
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
    el.addEventListener("pointerleave", up);
  };

  /* ---------- 시작 ---------- */
  const start = () => {
    if (LENS) document.documentElement.classList.add("lg-lens");
    scan(document.body);
    new MutationObserver((records) => {
      for (const r of records) {
        for (const n of r.addedNodes) if (n.nodeType === 1 && n.tagName !== "LG-HL") scan(n);
        // 화면 코드가 글자를 통째로 바꾸면(textContent) 빛 테두리도 지워져서, 다시 붙여요
        for (const n of r.removedNodes) {
          if (n.tagName === "LG-HL" && r.target._lg?.hl === n && !n.isConnected) r.target.appendChild(n);
        }
        // 클래스가 바뀌어 유리가 된 것 (.t-btn에 grey가 붙는 것처럼)
        if (r.type === "attributes" && r.target.nodeType === 1 && !r.target._lg && r.target.matches(SEL)) attach(r.target);
      }
    }).observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ["class"] });
  };
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
  else start();
  window.LiquidGlass = { lens: LENS, count: () => glass.size };
})();
