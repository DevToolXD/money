/* 일: 사장이거나 직원이면 회사 일을 해요. 아니면 회사를 종류별로 보고, 이름으로 찾고, 골라서 지원해요 (app/work.js).
   식품 회사는 음식 만드는 일, 인공지능 회사는 가짜 검색 연습이에요. 컴퓨터 회사 일은 아직 준비 중이에요.
   만든 음식과 받은 데이터는 이 화면에서만 세어요 (서버에 남기지 않아요). 월급은 아직 없어요. */
(() => {
  "use strict";

  const KIND = { food: "식품", pc: "컴퓨터", ai: "인공지능", nuclear: "원자력 발전" };
  const KIND_FULL = { food: "식품 회사", pc: "컴퓨터 회사", ai: "인공지능 회사", nuclear: "원자력 발전 회사" };
  const KIND_IDX = { all: 0, food: 1, pc: 2, ai: 3 };
  const FRESH = 30000; // 이 시간이 지나지 않았으면 다시 읽지 않아요 (서버를 아껴요)
  const RM = matchMedia("(prefers-reduced-motion: reduce)");
  const HITS = [
    { t: "데이터 묶음 1" },
    { t: "데이터 묶음 2" },
    { t: "데이터 묶음 3" },
    { t: "데이터 묶음 4" },
  ];
  const ICON = {
    search: '<circle cx="11" cy="11" r="6.5"/><path d="M16 16l4 4"/>',
    check: '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
  };
  const svgI = (d, size = 20) =>
    `<svg viewBox="0 0 24 24" width="${size}" height="${size}" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;
  const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
  const dateOf = (t) => {
    const d = new Date(t || Date.now());
    return `${d.getFullYear()}년 ${d.getMonth() + 1}월 ${d.getDate()}일`;
  };
  // 임시 시가총액: 창업 자본 10 토큰 + 직원 1명당 2 토큰 (매출이 생기면 여기에 더해요)
  const fmtN = (n) => Number(n || 0).toLocaleString("ko-KR");
  const capOf = (c) => 10 + 2 * (c.staff || 0);
  const byName = (a, b) => a.name.localeCompare(b.name, "ko");
  const SORTS = [
    { id: "pop", label: "인기순", by: (a, b) => b.staff - a.staff || byName(a, b) },
    { id: "cap", label: "시총순", by: (a, b) => capOf(b) - capOf(a) || byName(a, b) },
    { id: "few", label: "남은 자리 적은 순", by: (a, b) => a.open - b.open || byName(a, b) },
    { id: "many", label: "남은 자리 많은 순", by: (a, b) => b.open - a.open || byName(a, b) },
    { id: "new", label: "최근 창업순", by: (a, b) => b.at - a.at },
  ];

  let ctx = null;
  let root = null;
  let kindSel = "all";
  let sortSel = "pop";
  let term = "";
  let companies = null; // null: 아직 못 읽었어요
  let myJob = null; // 내가 다니는 회사 기록 (없으면 null)
  let view = "idle";
  let loadedAt = 0;
  let paintedCo = null; // 마지막으로 그린 때의 회사 정보 (바뀌었는지 알아봐요)
  let loadSeq = 0;
  let marketSeq = 0;
  let timer = 0;
  let busy = false;
  let foodStep = 0; // 음식 만드는 순서: 0 재료 담기 · 1 섞기 · 2 포장하기
  let made = 0; // 이번에 만든 음식
  let foodCid = null; // 음식을 만드는 회사
  let foodPid = null; // 대표 상품 (팔리는 중일 때만)
  let foodPending = 0; // 아직 보내지 않은 음식 수
  let foodTimer = 0;
  let foodRetry = 0; // 보내지 못한 횟수 (세 번까지 다시 해요)
  let got = 0; // 이번에 받은 데이터 (기록하지 않아요)

  const visible = () => {
    const by = SORTS.find((s) => s.id === sortSel).by;
    return companies.filter((c) => kindSel === "all" || c.kind === kindSel).sort(by);
  };
  const statsHTML = (c) =>
    `<div class="wk-stats"><div class="gg wk-stat"><b>${c.staff ?? 0}명</b><span>직원</span></div><div class="gg wk-stat"><b>${c.open ?? 0}명</b><span>모집 중</span></div><div class="gg wk-stat"><b>${capOf(c)}</b><span>시총 토큰</span></div></div>`;

  /* ---------- 취업: 회사 목록 ---------- */
  const marketHTML = () => `
    <p class="lead">회사를 골라 지원해요</p>
    <div class="wk-sorts wk-kinds" role="group" aria-label="회사 종류">${[["all", "전체"], ["food", "식품"], ["pc", "컴퓨터"], ["ai", "인공지능"], ["nuclear", "원자력 발전"]].map(([k, l]) => `<button type="button" class="gg wk-pill" aria-pressed="${kindSel === k}" data-k="${k}">${l}</button>`).join("")}</div>
    <label class="gg wk-search">${svgI(ICON.search, 18)}<input id="wk-q" type="search" placeholder="회사 이름으로 찾아요" autocomplete="off" maxlength="20" value="${esc(term)}" aria-label="회사 이름"></label>
    <div class="wk-sorts" role="group" aria-label="정렬">${SORTS.map((s) => `<button type="button" class="gg wk-pill" aria-pressed="${sortSel === s.id}" data-s="${s.id}">${s.label}</button>`).join("")}</div>
    <p class="wk-meta" id="wk-meta"></p>
    <div class="card wk-list loading" id="wk-list" aria-live="polite"></div>`;
  const rowHTML = (c, i) =>
    `<button type="button" class="wk-row" data-id="${esc(c.id)}" style="--i:${Math.min(i, 10)}">${ctx.mark(c.name, c.logo, "avatar contact wk-av")}<span class="wk-body"><b>${esc(c.name)}</b><span>${KIND_FULL[c.kind]} · 직원 ${c.staff}명 · 시총 ${capOf(c)} 토큰</span></span><span class="wk-tag${c.open ? "" : " off"}">${c.open ? `자리 ${c.open}` : "마감"}</span></button>`;
  const paintList = () => {
    const list = root.querySelector("#wk-list");
    const meta = root.querySelector("#wk-meta");
    if (!list || !meta) return;
    if (!companies) {
      list.innerHTML = '<p class="wk-empty">불러오는 중이에요</p>';
      meta.textContent = "";
      return;
    }
    const rows = visible();
    meta.textContent = rows.length ? `회사 ${rows.length}곳` : "";
    list.innerHTML = rows.length ? rows.map(rowHTML).join("") : `<p class="wk-empty">${term.trim() ? "이름이 맞는 회사가 없어요" : "아직 회사가 없어요"}</p>`;
  };
  const loadMarket = async () => {
    const seq = ++marketSeq;
    const list = root.querySelector("#wk-list");
    if (!list) return;
    list.classList.add("loading");
    try {
      const found = await ctx.cloud().listCompanies(term);
      if (seq !== marketSeq) return;
      companies = found;
      paintList();
    } catch (err) {
      if (seq !== marketSeq) return;
      list.innerHTML = '<p class="wk-empty">회사 목록을 불러오지 못했어요. 잠시 뒤 다시 해 주세요</p>';
    } finally {
      if (seq === marketSeq) list.classList.remove("loading");
    }
  };
  const paintMarket = () => {
    view = "market";
    root.innerHTML = marketHTML();
    paintList();
    loadMarket();
  };

  /* ---------- 지원: 회사 창 (아래에서 올라와요) ---------- */
  const openSheet = (html) => {
    const dim = document.createElement("div");
    dim.className = "wk-dim";
    const box = document.createElement("div");
    box.className = "wk-sheet";
    box.innerHTML = `<div class="grabber"></div>${html}`;
    const close = () => {
      dim.classList.remove("on");
      box.classList.remove("on");
      setTimeout(() => {
        dim.remove();
        box.remove();
      }, 420);
    };
    dim.addEventListener("click", close);
    box.addEventListener("click", (e) => {
      if (e.target.closest("[data-close]")) close();
    });
    document.querySelector(".app").append(dim, box);
    void box.offsetHeight; // 시작 자리를 먼저 잡아야 올라오는 움직임이 나와요
    dim.classList.add("on");
    box.classList.add("on");
    return { el: box, close };
  };
  const openDetail = (id) => {
    const c = (companies || []).find((x) => x.id === id);
    if (!c) return;
    const mine = c.owner === ctx.cloud().uid;
    const why = mine
      ? "내 회사에는 지원할 수 없어요"
      : myJob
        ? "이미 다니는 회사가 있어요. 먼저 그만둬야 해요"
        : c.open < 1
          ? "지금은 모집하지 않아요"
          : `지원하면 바로 입사해요. ${KIND[c.kind]} 일을 하게 돼요.`;
    const blocked = Boolean(mine || myJob || c.open < 1);
    const sheet = openSheet(`
      <div class="wk-sheet-head">${ctx.mark(c.name, c.logo, "avatar contact wk-av big")}<div><h2>${esc(c.name)}</h2><p>${KIND_FULL[c.kind]}</p></div></div>
      ${statsHTML(c)}
      <p class="wk-why">${why}</p>
      <button type="button" class="btn primary wk-apply" ${blocked ? "disabled" : ""}>지원하기</button>
      <button type="button" class="gg wk-cancel" data-close>닫기</button>`);
    sheet.el.querySelector(".wk-apply").addEventListener("click", () => apply(c, sheet));
  };
  const apply = async (c, sheet) => {
    if (busy) return;
    const yes = await ctx.ask({ title: `${c.name}에 지원할까요?`, body: `${KIND_FULL[c.kind]}에 들어가면 여기서 일하게 돼요.`, ok: "지원하기", cancel: "취소" });
    if (!yes) return;
    busy = true;
    const btn = sheet.el.querySelector(".wk-apply");
    btn.classList.add("busy");
    try {
      await ctx.cloud().applyJob(c.id, c.kind);
      sheet.close();
      ctx.toast(`${c.name}에 입사했어요`);
      refresh(true);
    } catch (err) {
      btn.classList.remove("busy");
      ctx.toast(err?.code === "permission-denied" ? "지원하지 못했어요. 자리가 없거나 이미 다니는 곳이 있어요" : "지원하지 못했어요. 잠시 뒤 다시 해 주세요");
    } finally {
      busy = false;
    }
  };

  /* ---------- 핵연료 배합 재료와 그림 ---------- */
  const MIX_MAX = 8; // 한 번에 넣을 수 있는 스포이드 방울 수
  const REAGENTS = [
    { name: "분말", rgb: [196, 58, 58] },
    { name: "안정제", rgb: [244, 196, 60] },
    { name: "촉매", rgb: [52, 110, 214] },
  ];
  const rgbCss = (c) => `rgb(${c[0]}, ${c[1]}, ${c[2]})`;
  // 재료를 방울 수만큼 섞은 색 (방울이 하나도 없으면 null)
  const mixRgb = (counts) => {
    const n = counts[0] + counts[1] + counts[2];
    if (!n) return null;
    return [0, 1, 2].map((ch) => Math.round(REAGENTS.reduce((s, r, i) => s + counts[i] * r.rgb[ch], 0) / n));
  };
  // 두 색이 얼마나 가까운지 (1은 같은 색, 0은 아주 다른 색)
  const closeOf = (a, b) => Math.max(0, Math.min(1, 1 - Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]) / 100));
  const bottleSvg = (rgb) => `<svg viewBox="0 0 40 48" aria-hidden="true"><rect x="14" y="1.5" width="12" height="6" rx="2" fill="none" stroke="#000" stroke-width="1.8"/><path d="M16 8v6Q8 17 8 26v14q0 6 6 6h12q6 0 6-6V26q0-9-8-12V8" fill="none" stroke="#000" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/><rect x="11" y="29" width="18" height="14" rx="3" fill="${rgbCss(rgb)}"/></svg>`;
  // 플라스크: 120×150 그림. 액체 높이는 방울 수에 따라 올라가요 (눈금 2·4·6·8방울)
  const FLASK_SVG = `<svg viewBox="0 0 120 150" aria-hidden="true"><defs><clipPath id="wk-flask-clip"><path d="M50 14H70V52L104 124Q109 136 96 139H24Q11 136 16 124L50 52Z"/></clipPath></defs><g clip-path="url(#wk-flask-clip)"><rect class="lab-liquid" x="0" y="60" width="120" height="90" style="transform:translateY(80px);fill:transparent"/></g><path d="M50 14H70M50 14V52L16 124Q11 136 24 139H96Q109 136 104 124L70 52V14" fill="none" stroke="#000" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"/><path d="M94.5 119.25H99.5M85.4 99.5H90.4M76.1 79.75H81.1M66.8 60H71.8" fill="none" stroke="#000" stroke-width="1.5" stroke-linecap="round"/></svg>`;
  // 스포이드: 고무 주머니와 관. 관 안의 액체는 고른 재료 색이에요
  const DROPPER_SVG = `<svg viewBox="0 0 34 70" aria-hidden="true"><rect x="10" y="1" width="14" height="22" rx="7" fill="none" stroke="#000" stroke-width="1.8"/><path d="M13 23V52L17 66L21 52V23" fill="none" stroke="#000" stroke-width="1.8" stroke-linejoin="round" stroke-linecap="round"/><rect class="lab-dropper-liq" x="14.5" y="28" width="5" height="22" rx="2" fill="#f2f2f7"/></svg>`;

  /* ---------- 일: 사장이거나 직원일 때 ---------- */
  const workHTML = (kind) => {
    if (kind === "food") {
      return `<div class="card wk-work wk-food">
        <p class="wk-head">식품 만드는 일</p>
        <p class="wk-note" id="wk-make">만들 상품을 불러오는 중이에요</p>
        <div class="wk-line" role="group" aria-label="만드는 순서">
          <button type="button" class="gg wk-step" data-step="0">재료 담기</button>
          <button type="button" class="gg wk-step" data-step="1">섞기</button>
          <button type="button" class="gg wk-step" data-step="2">포장하기</button>
        </div>
        <div class="gg wk-bar"><i id="wk-fill"></i></div>
        <p class="wk-count" id="wk-count">이번에 만든 음식 ${made}개</p>
      </div>`;
    }
    if (kind === "ai") {
      return `<div class="card wk-work wk-ai">
        <p class="wk-head">가짜 검색 연습</p>
        <p class="wk-note">아무 데이터나 받아 보세요. 받는 양은 랜덤이고, 많이 쌓일수록 인공지능이 좋아져요. 실제로 저장되는 파일은 없어요.</p>
        <div class="wk-nums"><span>쌓인 데이터 <b id="lab-data">…</b> MB</span></div>
        <label class="gg wk-fake">${svgI(ICON.search, 18)}<input type="text" disabled placeholder="검색어를 입력해 보세요" aria-label="가짜 검색"></label>
        <ul class="wk-hits">${HITS.map((h, i) => `<li class="wk-hit" style="--i:${i}"><span class="wk-hit-t"><b>${esc(h.t)}</b><span>받으면 양이 보여요</span></span><button type="button" class="gg wk-get" data-i="${i}">받기</button></li>`).join("")}</ul>
        <p class="wk-count" id="wk-count">이번에 받은 데이터 ${got}개</p>
      </div>`;
    }
    if (kind === "nuclear") {
      return `<div class="card wk-work wk-nuc">
        <p class="wk-head">핵연료 배합</p>
        <p class="wk-note">재료를 고르고 플라스크를 누르면 스포이드로 한 방울씩 떨어져요. 목표 색에 가깝게 섞은 뒤, 아래 버튼을 손을 떼지 않고 꾹 누르면 섞여요. 가까울수록 핵연료 성능이 많이 올라가요.</p>
        <div class="wk-nums"><span>핵연료 성능 <b id="lab-np">…</b></span></div>
        <div class="lab-bench">
          <div class="lab-top">
            <span class="lab-sw" id="wk-target" aria-hidden="true"></span>
            <div class="lab-meta"><b>목표 색</b><span id="wk-match">일치 0%</span><span class="lab-meter" aria-hidden="true"><i id="wk-meter"></i></span></div>
          </div>
          <div class="lab-flaskwrap">
            <div class="lab-dropper" id="wk-dropper" aria-hidden="true">${DROPPER_SVG}</div>
            <button type="button" class="lab-flask" id="wk-flask" aria-label="선택한 재료를 한 방울 떨어뜨리기">${FLASK_SVG}</button>
          </div>
          <p class="lab-drops" id="wk-drops">0 / ${MIX_MAX} 방울</p>
          <div class="lab-reagents" role="radiogroup" aria-label="재료 고르기">${REAGENTS.map((r, i) => `<button type="button" class="lab-reagent" role="radio" aria-checked="${i === 0}" data-r="${i}">${bottleSvg(r.rgb)}<b>${r.name}</b><span id="wk-rc${i}">0방울</span></button>`).join("")}</div>
          <div class="lab-actions">
            <button type="button" class="btn lab-reset" id="wk-reset">비우기</button>
            <button type="button" class="btn primary lab-stir" id="wk-stir">꾹 눌러 섞기</button>
          </div>
          <span class="lab-stirbar" aria-hidden="true"><i id="wk-stirbar"></i></span>
        </div>
        <p class="wk-count" id="wk-mixlog">재료를 고르고 플라스크를 눌러 보세요</p>
      </div>`;
    }
    return `<div class="card wk-work wk-pc">
      <p class="wk-head">컴퓨터 부품 검사</p>
      <p class="wk-note">열이 뜨거나 스파크가 튀는 곳을 누르면 용량·속도·전기 가운데 하나가 랜덤으로 올라가요. 쌓일수록 개발 중인 컴퓨터가 좋아져요.</p>
      <div class="gg wk-board" id="wk-board" aria-label="부품 검사판"><svg class="wk-chip" viewBox="0 0 120 120" aria-hidden="true"><rect x="34" y="34" width="52" height="52" rx="6"/><path d="M48 34V22M60 34V22M72 34V22M48 86v12M60 86v12M72 86v12M34 48H22M34 60H22M34 72H22M86 48h12M86 60h12M86 72h12"/></svg></div>
      <div class="wk-nums"><span>용량 <b id="lab-cap">…</b></span><span>속도 <b id="lab-spd">…</b></span><span>전기 <b id="lab-pow">…</b></span><span>검사 <b id="wk-stack">0</b>번</span></div>
    </div>`;
  };
  const quitHTML = (c) => `<button type="button" class="gg wk-quit" data-quit="${esc(c.id)}" data-name="${esc(c.name)}">${esc(c.name)}에서 그만두기</button>`;
  const mineHTML = (owner, co, job, extra) => `
    <div class="card wk-hero">${ctx.mark(co.name, co.logo, "avatar contact wk-av big")}<div><p class="wk-role">${owner ? "사장" : "직원"} · ${KIND[co.kind]}</p><h2>${esc(co.name)}</h2><p class="wk-since">${owner ? `${dateOf(co.at)}에 창업했어요` : `${dateOf(job.at)}에 입사했어요`}</p></div></div>
    ${statsHTML(co)}
    ${workHTML(co.kind)}
    ${owner ? (extra ? quitHTML(extra) : "") : quitHTML(co)}`;
  const paintSteps = () => {
    root.querySelectorAll(".wk-step").forEach((b, i) => {
      b.disabled = i !== foodStep;
      b.classList.toggle("done", i < foodStep);
    });
    const fill = root.querySelector("#wk-fill");
    if (fill) fill.style.transform = `scaleX(${foodStep / 3})`;
  };
  const countPop = (text) => {
    const c = root.querySelector("#wk-count");
    if (!c) return;
    c.textContent = text;
    c.classList.remove("pop");
    void c.offsetWidth;
    c.classList.add("pop");
  };
  const step = (n) => {
    if (busy || n !== foodStep) return;
    foodStep += 1;
    paintSteps();
    if (foodStep < 3) return;
    setTimeout(() => {
      foodStep = 0;
      made += 1;
      paintSteps();
      countPop(`이번에 만든 음식 ${made}개`);
      queueFood();
    }, 650);
  };
  // 만든 음식은 2.5초마다 한꺼번에 상품 재고로 보내요 (서버 쓰기를 줄여요). 한 번에 100개까지, 실패하면 세 번까지 다시 보내요
  const queueFood = () => {
    if (!foodPid) return;
    foodPending += 1;
    if (!foodTimer) foodTimer = setTimeout(flushFood, 2500);
  };
  const flushFood = async () => {
    clearTimeout(foodTimer);
    foodTimer = 0;
    const n = Math.min(foodPending, 100);
    if (!n || !foodCid || !foodPid) return;
    const cid = foodCid;
    const pid = foodPid;
    foodPending -= n;
    try {
      await ctx.cloud().labFoodMake(cid, pid, n);
      foodRetry = 0;
    } catch (err) {
      console.warn("만든 음식을 보내지 못했어요:", err?.code || err);
      foodPending += n;
      if (foodRetry < 3) {
        foodRetry += 1;
        if (!foodTimer) foodTimer = setTimeout(flushFood, 2500);
      }
    }
    if (foodPending && !foodTimer) foodTimer = setTimeout(flushFood, 2500);
  };
  const loadProducts = async (cid) => {
    try {
      const list = await ctx.cloud().listProducts(cid);
      foodCid = cid;
      foodPid = list.length && !list[0].paused ? list[0].id : null;
      const note = root.querySelector("#wk-make");
      if (!note || view !== "mine") return;
      note.textContent = list.length ? `만들 상품: ${list[0].name} · 만든 음식은 상점에 진열돼요` : "아직 파는 상품이 없어요. 사장님이 상품을 정하면 여기서 만들어요";
    } catch (err) {
      const note = root.querySelector("#wk-make");
      if (note) note.textContent = "만들 상품을 불러오지 못했어요";
    }
  };
  // 가짜 검색의 받기: 로딩 막대가 차고 끝나면 랜덤한 양의 데이터가 쌓여요 (실제로 받는 것은 없어요)
  const fetchHit = (btn) => {
    if (btn.classList.contains("busy") || btn.classList.contains("done")) return;
    btn.classList.add("busy");
    btn.innerHTML = '<span class="wk-meter"><i></i></span><em>0%</em>';
    const fill = btn.querySelector(".wk-meter i");
    const label = btn.querySelector("em");
    const dur = RM.matches ? 0 : 1600;
    const t0 = performance.now();
    const tick = (now) => {
      const p = dur ? Math.min(1, (now - t0) / dur) : 1;
      fill.style.transform = `scaleX(${p})`;
      label.textContent = `${Math.round(p * 100)}%`;
      if (p < 1) return requestAnimationFrame(tick);
      const mb = 20 + Math.floor(Math.random() * 381);
      btn.classList.remove("busy");
      btn.classList.add("done");
      btn.disabled = true;
      btn.innerHTML = `${svgI(ICON.check, 16)}${mb} MB`;
      got += 1;
      queueLab({ aiData: mb });
      countPop(`이번에 받은 데이터 ${got}개`);
    };
    requestAnimationFrame(tick);
  };
  const quit = async (cid, name) => {
    if (busy) return;
    const a = await ctx.ask({ title: `${name}을 그만둘까요?`, body: "그만두면 이 회사에서 일할 수 없어요. 다른 회사에 다시 지원할 수 있어요.", ok: "그만두기", cancel: "취소" });
    if (!a) return;
    const b = await ctx.ask({ title: "정말 하시겠습니까?", body: "그만두면 자리를 다시 얻어야 해요.", ok: "네, 그만둘게요", cancel: "아니요" });
    if (!b) return;
    busy = true;
    try {
      await ctx.cloud().quitJob(cid);
      ctx.toast(`${name}을 그만뒀어요`);
      refresh(true);
    } catch (err) {
      ctx.toast("그만두지 못했어요. 잠시 뒤 다시 해 주세요");
    } finally {
      busy = false;
    }
  };
  /* ---------- 연구실 숫자 (핵연료 성능·컴퓨터 숫자·데이터): 누른 것은 모아서 2.5초에 한 번 서버에 보내요 ---------- */
  const LAB_KEYS = ["nperf", "cap", "spd", "pow", "aiData"];
  const zeroPend = () => Object.fromEntries(LAB_KEYS.map((k) => [k, 0]));
  let labCid = null;
  let labView = null; // 화면에 보이는 숫자 (서버 숫자 + 아직 안 보낸 것)
  let labPend = zeroPend();
  let labTimer = 0;
  let boardTimer = 0;
  let stack = 0;
  const paintLabNums = () => {
    if (!labView || !root) return;
    const set = (id, v) => {
      const el = root.querySelector("#" + id);
      if (el) el.textContent = fmtN(v);
    };
    set("lab-np", labView.nperf);
    set("lab-cap", labView.cap);
    set("lab-spd", labView.spd);
    set("lab-pow", labView.pow);
    set("lab-data", labView.aiData);
  };
  const flushLab = async () => {
    clearTimeout(labTimer);
    labTimer = 0;
    const d = labPend;
    if (!labCid || !LAB_KEYS.some((k) => d[k])) return;
    const cid = labCid;
    labPend = zeroPend();
    try {
      await ctx.cloud().labChange(cid, (L) => {
        const next = { ...L };
        for (const k of LAB_KEYS) next[k] = L[k] + d[k];
        return next;
      });
    } catch (err) {
      console.warn("연구실 숫자를 올리지 못했어요:", err?.code || err);
    }
  };
  const queueLab = (d) => {
    for (const k of Object.keys(d)) {
      labPend[k] += d[k];
      if (labView) labView[k] += d[k];
    }
    paintLabNums();
    if (!labTimer) labTimer = setTimeout(flushLab, 2500);
  };
  const loadLabView = async (cid) => {
    try {
      const L = await ctx.cloud().labGet(cid);
      if (cid !== labCid) return;
      labView = { ...L };
      paintLabNums();
    } catch (err) {
      console.warn("연구실 숫자를 불러오지 못했어요:", err?.code || err);
    }
  };
  // 핵연료 배합: 재료를 스포이드로 한 방울씩 떨어뜨리고, 꾹 눌러 섞어요. 목표 색에 가까울수록 성능이 더 많이 올라요
  let mixCounts = [0, 0, 0];
  let mixSel = 0;
  let mixGoal = null;
  let stirStart = 0;
  let stirRaf = 0;
  const STIR_MS = 1600; // 꾹 누르고 있어야 하는 시간
  const mixTotal = () => mixCounts[0] + mixCounts[1] + mixCounts[2];
  const setLog = (t) => {
    const el = root?.querySelector("#wk-mixlog");
    if (el) el.textContent = t;
  };
  // 목표 색은 재료를 8방울 안에서 섞어 만들 수 있는 색이에요 (맞출 수 있는 문제)
  const newGoal = () => {
    for (;;) {
      const counts = [0, 1, 2].map(() => Math.floor(Math.random() * 5));
      const n = counts[0] + counts[1] + counts[2];
      if (n >= 3 && n <= MIX_MAX) return { rgb: mixRgb(counts) };
    }
  };
  const paintGoal = () => {
    const sw = root?.querySelector("#wk-target");
    if (sw && mixGoal) sw.style.setProperty("--c", rgbCss(mixGoal.rgb));
  };
  const paintReagents = () => {
    REAGENTS.forEach((_, i) => {
      root?.querySelector(`[data-r="${i}"]`)?.setAttribute("aria-checked", String(i === mixSel));
      const c = root?.querySelector(`#wk-rc${i}`);
      if (c) c.textContent = `${mixCounts[i]}방울`;
    });
    const liq = root?.querySelector("#wk-dropper .lab-dropper-liq");
    if (liq) liq.style.fill = rgbCss(REAGENTS[mixSel].rgb);
  };
  const paintMix = () => {
    const total = mixTotal();
    const rgb = mixRgb(mixCounts);
    const liquid = root?.querySelector(".lab-liquid");
    if (liquid) {
      liquid.style.transform = `translateY(${(1 - Math.min(1, total / MIX_MAX)) * 80}px)`;
      liquid.style.fill = rgb ? rgbCss(rgb) : "transparent";
    }
    const pct = rgb && mixGoal ? Math.round(closeOf(rgb, mixGoal.rgb) * 100) : 0;
    const m = root?.querySelector("#wk-match");
    if (m) m.textContent = `일치 ${pct}%`;
    const bar = root?.querySelector("#wk-meter");
    if (bar) bar.style.width = `${pct}%`;
    const d = root?.querySelector("#wk-drops");
    if (d) d.textContent = `${total} / ${MIX_MAX} 방울`;
    paintReagents();
  };
  const floatText = (text, top) => {
    const wrap = root?.querySelector(".lab-flaskwrap");
    if (!wrap) return;
    const f = document.createElement("span");
    f.className = "wk-float";
    f.textContent = text;
    f.style.left = "50%";
    f.style.top = top;
    wrap.appendChild(f);
    setTimeout(() => f.remove(), 900);
  };
  // 스포이드 한 방울: 고른 재료가 플라스크 액체 면에 떨어져요
  const dropMix = () => {
    if (stirStart) return;
    if (mixTotal() >= MIX_MAX) return setLog("더는 못 넣어요. 꾹 눌러 섞어 보세요");
    mixCounts[mixSel] += 1;
    const total = mixTotal();
    const wrap = root?.querySelector(".lab-flaskwrap");
    if (wrap) {
      // 떨어질 거리: 방울이 닿을 액체 면까지 (플라스크 그림의 좌표를 화면 픽셀로 바꿔요)
      const surface = 11.25 + (60 + (1 - total / MIX_MAX) * 80) * 1.25;
      const drop = document.createElement("span");
      drop.className = "lab-drop";
      drop.style.setProperty("--c", rgbCss(REAGENTS[mixSel].rgb));
      drop.style.setProperty("--fall", `${Math.max(0, surface - 62)}px`);
      wrap.appendChild(drop);
      setTimeout(() => drop.remove(), 600);
      const dip = wrap.querySelector("#wk-dropper");
      if (dip) {
        dip.classList.remove("dip");
        void dip.offsetWidth; // 같은 애니메이션을 다시 시작해요
        dip.classList.add("dip");
      }
    }
    paintMix();
    setLog(`${REAGENTS[mixSel].name} 한 방울을 떨어뜨렸어요`);
  };
  const pickReagent = (i) => {
    if (stirStart) return;
    mixSel = i;
    paintReagents();
  };
  const resetMix = () => {
    if (stirStart) return;
    mixCounts = [0, 0, 0];
    paintMix();
    setLog("비웠어요. 재료를 골라 다시 떨어뜨려 보세요");
  };
  const stirBar = (p) => {
    const el = root?.querySelector("#wk-stirbar");
    if (el) el.style.width = `${Math.round(p * 100)}%`;
  };
  // 꾹 누르고 있는 동안 섞여요. STIR_MS를 채우면 배합이 끝나요. 중간에 손을 떼면 처음부터예요
  const stirBegin = (e) => {
    if (stirStart || !root) return;
    if (e.type === "keydown" && e.repeat) return;
    if (!mixTotal()) return setLog("재료를 먼저 떨어뜨려 주세요");
    stirStart = Date.now();
    root.querySelector("#wk-flask")?.classList.add("is-stir");
    setLog("저어 주세요. 손을 떼면 멈춰요");
    const tick = () => {
      if (!stirStart) return;
      const p = Math.min(1, (Date.now() - stirStart) / STIR_MS);
      stirBar(p);
      if (p >= 1) return stirDone();
      stirRaf = requestAnimationFrame(tick);
    };
    stirRaf = requestAnimationFrame(tick);
  };
  const stirCancel = () => {
    if (!stirStart) return;
    cancelAnimationFrame(stirRaf);
    stirStart = 0;
    stirBar(0);
    root?.querySelector("#wk-flask")?.classList.remove("is-stir");
    setLog("조금 더 저어 주세요. 끝까지 누르고 있어야 섞여요");
  };
  // 배합 끝: 목표 색과 가까울수록 성능이 더 많이 올라요 (1~30). 기록은 바로 모아서 보내요
  const stirDone = () => {
    cancelAnimationFrame(stirRaf);
    stirStart = 0;
    stirBar(0);
    const close = closeOf(mixRgb(mixCounts), mixGoal.rgb);
    const gain = 1 + Math.floor(Math.random() * (4 + Math.round(26 * close)));
    queueLab({ nperf: gain });
    root?.querySelector("#wk-flask")?.classList.remove("is-stir");
    floatText(`성능 +${gain}`, "96px");
    setLog(`핵연료 성능이 ${gain} 올랐어요 · 목표 색과 ${Math.round(close * 100)}% 일치`);
    mixCounts = [0, 0, 0];
    mixGoal = newGoal();
    paintGoal();
    paintMix();
  };
  const startMix = () => {
    stirStart = 0;
    mixCounts = [0, 0, 0];
    mixSel = 0;
    mixGoal = newGoal();
    paintGoal();
    paintMix();
    setLog("재료를 고르고 플라스크를 눌러 한 방울씩 떨어뜨려 보세요");
    const stir = root?.querySelector("#wk-stir");
    if (!stir) return;
    stir.addEventListener("pointerdown", (e) => {
      if (e.button !== 0) return;
      try { stir.setPointerCapture(e.pointerId); } catch (err) { /* 지원하지 않으면 그냥 둬요 */ }
      stirBegin(e);
    });
    stir.addEventListener("pointerup", stirCancel);
    stir.addEventListener("pointercancel", stirCancel);
    stir.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        stirBegin(e);
      }
    });
    stir.addEventListener("keyup", (e) => {
      if (e.key === "Enter" || e.key === " ") stirCancel();
    });
  };
  // 부품 검사판: 열·스파크가 떴다가 사라져요. 누르면 용량·속도·전기 가운데 하나가 랜덤으로 쌓여요
  const STAT = [["cap", "용량"], ["spd", "속도"], ["pow", "전기"]];
  const stopGames = () => {
    clearInterval(boardTimer);
    boardTimer = 0;
    stack = 0;
    cancelAnimationFrame(stirRaf);
    stirStart = 0;
  };
  const spawnSpot = () => {
    const board = root.querySelector("#wk-board");
    if (!board || board.querySelectorAll(".wk-spot").length >= 6) return;
    const spot = document.createElement("button");
    spot.type = "button";
    spot.className = "wk-spot " + (Math.random() < 0.5 ? "heat" : "spark");
    spot.style.left = `${8 + Math.random() * 84}%`;
    spot.style.top = `${8 + Math.random() * 84}%`;
    spot.dataset.v = String(1 + Math.floor(Math.random() * 8));
    board.appendChild(spot);
    setTimeout(() => spot.remove(), 2200);
  };
  const startBoard = () => {
    stopGames();
    boardTimer = setInterval(spawnSpot, 650);
  };
  const hitSpot = (spot) => {
    const v = Number(spot.dataset.v) || 1;
    const [key, label] = STAT[Math.floor(Math.random() * STAT.length)];
    const board = root.querySelector("#wk-board");
    const at = { left: spot.style.left, top: spot.style.top };
    spot.remove();
    stack += 1;
    queueLab({ [key]: v });
    const s = root.querySelector("#wk-stack");
    if (s) s.textContent = String(stack);
    if (board) {
      const f = document.createElement("span");
      f.className = "wk-float";
      f.textContent = `${label} +${v}`;
      f.style.left = at.left;
      f.style.top = at.top;
      board.appendChild(f);
      setTimeout(() => f.remove(), 700);
    }
  };

  const paintMine = (co, job, employer) => {
    view = "mine";
    stopGames();
    flushLab(); // 이전 회사의 숫자를 먼저 보내요
    flushFood(); // 만든 음식도 먼저 보내요
    foodPid = null;
    const owner = Boolean(co);
    const primary = owner ? co : employer;
    const cid = owner ? co.id : employer.id;
    foodStep = 0;
    labCid = cid;
    labView = null;
    root.innerHTML = mineHTML(owner, primary, job, owner && employer ? employer : null);
    if (primary.kind === "food") {
      paintSteps();
      loadProducts(cid);
      return;
    }
    loadLabView(cid);
    if (primary.kind === "nuclear") startMix();
    if (primary.kind === "pc") startBoard();
  };
  const fail = (msg) => {
    view = "error";
    root.innerHTML = `<div class="card wk-work"><p class="wk-head">${esc(msg)}</p><button type="button" class="btn" id="wk-retry">다시 해 보기</button></div>`;
  };

  /* ---------- 읽기 ---------- */
  const refresh = async (force = false) => {
    if (!ctx || !root) return;
    const cloud = ctx.cloud();
    if (!cloud?.user) {
      root.innerHTML = "";
      view = "idle";
      loadedAt = 0;
      return;
    }
    const co = ctx.company();
    // 같은 회사 정보로 최근에 읽었으면 다시 읽지 않아요. 회사 정보가 바뀌었으면(모집·창업 등) 바로 다시 그려요
    if (!force && view !== "error" && co === paintedCo && Date.now() - loadedAt < FRESH) return;
    if (co === undefined) {
      view = "loading";
      root.innerHTML = '<p class="biz-loading">불러오는 중이에요</p>';
      return;
    }
    const seq = ++loadSeq;
    if (view !== "mine" && view !== "market") root.innerHTML = '<p class="biz-loading">불러오는 중이에요</p>';
    try {
      const job = await cloud.loadMyJob();
      const employer = job ? await cloud.getCompany(job.cid) : null;
      if (seq !== loadSeq) return;
      myJob = job;
      if (job && !employer) return fail("다니는 회사를 찾지 못했어요");
      loadedAt = Date.now();
      paintedCo = co;
      if (co || employer) return paintMine(co, job, employer);
      paintMarket();
    } catch (err) {
      if (seq !== loadSeq) return;
      fail("일 정보를 불러오지 못했어요");
    }
  };

  /* ---------- 이벤트 (화면 한 곳에서 받아요) ---------- */
  const onClick = (e) => {
    const t = e.target;
    const spot = t.closest(".wk-spot");
    if (spot) return hitSpot(spot);
    if (t.closest("#wk-flask")) return dropMix();
    const rg = t.closest("[data-r]");
    if (rg) return pickReagent(Number(rg.dataset.r));
    if (t.closest("#wk-reset")) return resetMix();
    const k = t.closest("[data-k]");
    if (k) return pickKind(k);
    const s = t.closest("[data-s]");
    if (s) return pickSort(s);
    const row = t.closest(".wk-row");
    if (row) return openDetail(row.dataset.id);
    const st = t.closest("[data-step]");
    if (st) return step(Number(st.dataset.step));
    const get = t.closest(".wk-get");
    if (get) return fetchHit(get);
    const q = t.closest("[data-quit]");
    if (q) return quit(q.dataset.quit, q.dataset.name);
    if (t.closest("#wk-retry")) refresh(true);
  };
  const pickKind = (b) => {
    kindSel = b.dataset.k;
    b.parentElement.querySelectorAll("[data-k]").forEach((x) => x.setAttribute("aria-pressed", String(x === b)));
    paintList();
  };
  const pickSort = (b) => {
    sortSel = b.dataset.s;
    root.querySelectorAll(".wk-pill").forEach((x) => x.setAttribute("aria-pressed", String(x === b)));
    paintList();
  };
  const onInput = (e) => {
    if (e.target.id !== "wk-q") return;
    term = e.target.value;
    clearTimeout(timer);
    timer = setTimeout(loadMarket, 320); // 글자를 치는 동안에는 기다렸다가 한 번만 찾아요
  };

  const init = (c) => {
    ctx = c;
    root = document.getElementById("work-root");
    root.addEventListener("click", onClick);
    root.addEventListener("input", onInput);
  };

  window.Work = { init, refresh };
})();
