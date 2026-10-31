/* 일: 사장이거나 직원이면 회사 일을 해요. 아니면 회사를 종류별로 보고, 이름으로 찾고, 골라서 지원해요 (app/work.js).
   식품 회사는 음식 만드는 일, 인공지능 회사는 가짜 검색 연습이에요. 컴퓨터 회사 일은 아직 준비 중이에요.
   만든 음식과 받은 데이터는 이 화면에서만 세어요 (서버에 남기지 않아요). 월급은 아직 없어요. */
(() => {
  "use strict";

  const KIND = { food: "식품", pc: "컴퓨터", ai: "인공지능" };
  const KIND_FULL = { food: "식품 회사", pc: "컴퓨터 회사", ai: "인공지능 회사" };
  const KIND_IDX = { all: 0, food: 1, pc: 2, ai: 3 };
  const FRESH = 30000; // 이 시간이 지나지 않았으면 다시 읽지 않아요 (서버를 아껴요)
  const RM = matchMedia("(prefers-reduced-motion: reduce)");
  const HITS = [
    { t: "날씨 기록 묶음", size: "1.2 GB" },
    { t: "문장 연습장", size: "340 MB" },
    { t: "그림 조각 모음", size: "2.8 GB" },
    { t: "숫자 표 1만 줄", size: "96 MB" },
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
  let made = 0; // 이번에 만든 음식 (기록하지 않아요)
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
    <div class="gseg wk-kinds" id="wk-kinds" role="radiogroup" aria-label="회사 종류" style="--n:4;--i:${KIND_IDX[kindSel]}">
      <button type="button" role="radio" aria-checked="${kindSel === "all"}" data-k="all">전체</button>
      <button type="button" role="radio" aria-checked="${kindSel === "food"}" data-k="food">식품</button>
      <button type="button" role="radio" aria-checked="${kindSel === "pc"}" data-k="pc">컴퓨터</button>
      <button type="button" role="radio" aria-checked="${kindSel === "ai"}" data-k="ai">인공지능</button>
    </div>
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
        <p class="wk-note">여기서 받는 데이터는 연습이라 아무 데도 저장되지 않아요.</p>
        <label class="gg wk-fake">${svgI(ICON.search, 18)}<input type="text" disabled placeholder="검색어를 입력해 보세요" aria-label="가짜 검색"></label>
        <ul class="wk-hits">${HITS.map((h, i) => `<li class="wk-hit" style="--i:${i}"><span class="wk-hit-t"><b>${esc(h.t)}</b><span>${h.size} · 연습용</span></span><button type="button" class="gg wk-get" data-i="${i}">받기</button></li>`).join("")}</ul>
        <p class="wk-count" id="wk-count">이번에 받은 데이터 ${got}개</p>
      </div>`;
    }
    return `<div class="card wk-work wk-soon">
      <div class="gg wk-soon-ic">${window.Business?.kindSvg("pc") ?? ""}</div>
      <p class="wk-head">컴퓨터 일은 아직 준비 중이에요</p>
      <p class="wk-note">회사에 다니는 것까지는 돼요. 일하는 방법은 곧 열려요.</p>
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
    }, 650);
  };
  const loadProducts = async (cid) => {
    try {
      const list = await ctx.cloud().listProducts(cid);
      const note = root.querySelector("#wk-make");
      if (!note || view !== "mine") return;
      note.textContent = list.length ? `만들 상품: ${list[0].name}` : "아직 파는 상품이 없어요. 사장님이 상품을 정하면 여기서 만들어요";
    } catch (err) {
      const note = root.querySelector("#wk-make");
      if (note) note.textContent = "만들 상품을 불러오지 못했어요";
    }
  };
  // 가짜 검색의 받기: 로딩 막대가 차고, 끝나면 받았다고만 보여줘요 (실제로 받는 것은 없어요)
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
      btn.classList.remove("busy");
      btn.classList.add("done");
      btn.disabled = true;
      btn.innerHTML = `${svgI(ICON.check, 16)}받았어요`;
      got += 1;
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
  const paintMine = (co, job, employer) => {
    view = "mine";
    const owner = Boolean(co);
    const primary = owner ? co : employer;
    const cid = owner ? co.id : employer.id;
    foodStep = 0;
    root.innerHTML = mineHTML(owner, primary, job, owner && employer ? employer : null);
    if (primary.kind === "food") {
      paintSteps();
      loadProducts(cid);
    }
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
    const seg = b.closest(".gseg");
    seg.style.setProperty("--i", String(KIND_IDX[kindSel]));
    seg.querySelectorAll("button").forEach((x) => x.setAttribute("aria-checked", String(x === b)));
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
