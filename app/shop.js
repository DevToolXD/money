/* 상점: 재고가 있는 음식을 여러 회사에서 모아 보여주고, 골라서 사요.
   사는 돈은 내 지갑에서 빠지고 파는 회사의 금고로 들어가요 (서버 규칙이 지켜요: app/firebase.src.js 의 shopBuy).
   그림은 사업 탭과 같은 상품 그림이에요. 파는 회사의 로고가 가운데에 들어가요. */
(() => {
  "use strict";

  const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
  const svgI = (d) =>
    `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;
  const ICON = { minus: '<path d="M5 12h14"/>', plus: '<path d="M12 5v14M5 12h14"/>' };
  // 센트를 토큰 글자로 (80 → 0.8, 300 → 3)
  const tok = (cents) => (cents / 100).toLocaleString("ko-KR", { maximumFractionDigits: 2 });
  // 살 때 내는 토큰: 센트는 올림해서 토큰으로 내요
  const costOf = (cents, n) => Math.ceil((cents * n) / 100);
  const MAX_N = 100;

  let ctx = null;
  let root = null;
  let state = "loading"; // loading · ok · error · out (로그인 안 함)
  let items = [];
  const sel = new Map(); // 상품 키 → 고른 개수
  let busy = false;
  let seq = 0;

  const keyOf = (p) => `${p.cid}/${p.id}`;
  const amountOf = (p) => Math.max(1, Math.min(p.stock, MAX_N));
  const pickOf = (p) => Math.min(amountOf(p), Math.max(1, sel.get(keyOf(p)) ?? 1));

  const art = (p) => {
    try {
      return ctx.pkv(p, false, p.co);
    } catch (_) {
      return "";
    }
  };
  const cardHTML = (p, i) => {
    const n = pickOf(p);
    return `<article class="card shop-card" style="--i:${i}" data-key="${esc(keyOf(p))}">
      <div class="shop-art">${art(p)}</div>
      <div class="shop-body">
        <b class="shop-name">${esc(p.name)}</b>
        <span class="shop-co">${esc(p.co.name)}</span>
        <span class="shop-price">1개 ${tok(p.priceCents)} 토큰</span>
        <span class="shop-stock">재고 ${p.stock}개</span>
        <div class="shop-step" role="group" aria-label="살 개수">
          <button type="button" class="shop-round" data-d="-1" aria-label="한 개 줄이기"${n <= 1 ? " disabled" : ""}>${svgI(ICON.minus)}</button>
          <output class="shop-n" aria-live="polite">${n}</output>
          <button type="button" class="shop-round" data-d="1" aria-label="한 개 늘리기"${n >= amountOf(p) ? " disabled" : ""}>${svgI(ICON.plus)}</button>
        </div>
        <button type="button" class="btn primary shop-buy" data-buy="1">${costOf(p.priceCents, n).toLocaleString("ko-KR")} 토큰에 사기</button>
      </div>
    </article>`;
  };
  const noteHTML = (eyebrow, title, body, extra = "", pic = "") =>
    `<div class="card shop-note">${pic}<div class="art-text"><p class="eyebrow">${esc(eyebrow)}</p><h2 class="card-title">${esc(title)}</h2><p class="card-body">${esc(body)}</p>${extra}</div></div>`;

  const paint = () => {
    if (!root) return;
    if (state === "loading") {
      root.innerHTML = '<div class="card skeletons" style="--rows: 4" aria-hidden="true"></div>';
      return;
    }
    if (state === "out") {
      root.innerHTML = noteHTML("로그인이 필요해요", "로그인하면 살 수 있어요", "식품 회사가 만든 음식을 여기서 살 수 있어요. 산 돈은 파는 회사의 금고로 들어가요.");
      return;
    }
    if (state === "error") {
      root.innerHTML = noteHTML("잠깐만요", "상점을 불러오지 못했어요", "인터넷을 확인하고 다시 해 주세요.", '<button type="button" class="btn shop-retry">다시 해 보기</button>');
      return;
    }
    if (!items.length) {
      root.innerHTML = noteHTML("아직 진열된 음식이 없어요", "음식이 만들어지면 여기에 나와요", "식품 회사의 사장님이나 직원이 음식을 만들면 상점에 진열돼요.", "", '<div class="art mint"></div>');
      return;
    }
    root.innerHTML = `<h2 class="section-title">지금 살 수 있어요</h2><div class="shop-grid">${items.map(cardHTML).join("")}</div>`;
  };

  const load = async () => {
    const my = ++seq;
    if (!ctx?.cloud()?.user) {
      state = "out";
      items = [];
      paint();
      return;
    }
    try {
      const list = await ctx.cloud().listShop();
      if (my !== seq) return;
      items = list.sort((a, b) => a.co.name.localeCompare(b.co.name, "ko") || a.name.localeCompare(b.name, "ko"));
      state = "ok";
    } catch (err) {
      if (my !== seq) return;
      console.warn("상점을 불러오지 못했어요:", err?.code || err);
      state = "error";
    }
    paint();
  };

  // 개수 바꾸기: 화면의 숫자와 버튼 글자만 바꿔요 (다시 그리지 않아요)
  const step = (card, p, d) => {
    const n = Math.min(amountOf(p), Math.max(1, pickOf(p) + d));
    sel.set(keyOf(p), n);
    card.querySelector(".shop-n").textContent = n;
    card.querySelector('[data-d="-1"]').disabled = n <= 1;
    card.querySelector('[data-d="1"]').disabled = n >= amountOf(p);
    card.querySelector(".shop-buy").textContent = `${costOf(p.priceCents, n).toLocaleString("ko-KR")} 토큰에 사기`;
  };

  const msgOf = (err) => {
    if (err?.code === "permission-denied") return "지금은 살 수 없어요. 잠시 뒤에 다시 해 주세요";
    return err?.message || "사지 못했어요. 잠시 뒤에 다시 해 주세요";
  };

  const buy = async (card, p) => {
    if (!ctx.cloud()?.user) return ctx.toast("로그인하면 살 수 있어요");
    if (busy) return;
    const n = pickOf(p);
    busy = true;
    const btn = card.querySelector(".shop-buy");
    btn.classList.add("busy");
    try {
      const paid = await ctx.walletOp(() => ctx.cloud().shopBuy(p.cid, p.id, n, p.co.name));
      ctx.toast(`${p.name} ${n}개를 샀어요. 인벤토리에 넣었어요`);
      ctx.log?.({ t: Date.now(), name: `${p.name} ${n}개`, cat: "상점", icon: "store", amt: -paid });
      sel.delete(keyOf(p));
    } catch (err) {
      ctx.toast(msgOf(err));
    } finally {
      busy = false;
      btn.classList.remove("busy");
      await load(); // 재고와 목록을 서버에서 다시 읽어요
    }
  };

  const init = (c) => {
    ctx = c;
    root = document.getElementById("shop-root");
    if (!root) return;
    root.addEventListener("click", (e) => {
      if (e.target.closest(".shop-retry")) return load();
      const card = e.target.closest(".shop-card");
      if (!card) return;
      const p = items.find((x) => keyOf(x) === card.dataset.key);
      if (!p) return;
      const d = e.target.closest("[data-d]");
      if (d && !d.disabled) return step(card, p, Number(d.dataset.d));
      if (e.target.closest("[data-buy]")) buy(card, p);
    });
  };

  window.Shop = { init, refresh: load };
})();
