(() => {
  "use strict";

  const root = document.documentElement;
  // firebase.js는 모듈이라 이 파일보다 늦게 실행돼요. 올라올 때까지 기다렸다가 연결 결과를 받아요
  const cloudPromise = new Promise((resolve) => {
    const t = setInterval(() => {
      if (window.cloudReady) {
        clearInterval(t);
        window.cloudReady.then(resolve);
      }
    }, 30);
  });
  const app = document.querySelector(".app");
  const nav = document.querySelector(".tabbar");
  const screens = new Map([...document.querySelectorAll(".screen")].map((el) => [el.dataset.screen, el]));
  const scheme = matchMedia("(prefers-color-scheme: dark)");
  const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)");

  // 색 토큰(design-system)과 탭 바가 같은 테마를 보게 해요
  const applyTheme = () => {
    root.dataset.theme = scheme.matches ? "dark" : "light";
  };
  applyTheme();

  // 내용이 없는 자리 표시 줄
  for (const box of document.querySelectorAll(".skeletons")) {
    const rows = Number(getComputedStyle(box).getPropertyValue("--rows")) || 6;
    box.innerHTML = '<div class="skeleton-row"><i></i><span><b></b><b></b></span></div>'.repeat(rows);
  }

  const showScreen = (name) => {
    for (const [key, el] of screens) {
      const on = key === name;
      el.classList.toggle("is-active", on);
      if (on) app.dataset.screen = key;
      el.inert = !on;
    }
  };

  const scrollToTop = (name) =>
    screens.get(name)?.scrollTo({ top: 0, behavior: reduceMotion.matches ? "auto" : "smooth" });

  const initial = nav.querySelector("[aria-current]");
  const firstScreen = initial ? initial.dataset.tab : "messages";
  showScreen(firstScreen);
  document.getElementById("topbar-title").textContent = screens.get(firstScreen).dataset.title;

  // 기기에서 이상하게 보일 때 원인을 가르려고 둔 스위치예요.
  //   ?glass=off  뒤 화면 굴절 없이, 흐린 유리 판(CSS)만 써요
  //   ?gl=off     WebGL을 아예 안 쓰고 평범한 DOM 아이콘과 알약으로 보여줘요
  /* ---------- 위쪽 바: 큰 제목이 접히면 가운데 작은 제목과 유리가 나타나요 ---------- */
  const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));
  const topbar = document.getElementById("topbar");
  const topTitle = document.getElementById("topbar-title");
  const updateChrome = (el) => {
    if (!el || !el.classList.contains("is-active")) return;
    const y = el.scrollTop;
    const h1 = el.querySelector(".large-title h1");
    if (!h1) {
      // 큰 제목이 없는 화면(메시지)은 작은 제목이 늘 보여요. 돈 탭처럼 data-notitle이면 제목 없이 유리만 나타나요
      topbar.style.setProperty("--p", clamp(y / 16, 0, 1));
      topTitle.style.opacity = el.hasAttribute("data-notitle") ? 0 : 1;
      topTitle.style.transform = "none";
      return;
    }
    topbar.style.setProperty("--p", clamp((y - 28) / 20, 0, 1));
    const t = clamp((y - 34) / 14, 0, 1);
    topTitle.style.opacity = t;
    topTitle.style.transform = `translateY(${(1 - t) * 5}px)`;
    if (h1) {
      h1.style.opacity = clamp(1 - y / 38, 0, 1);
      h1.style.transform = y < 0 ? `scale(${1 + Math.min(-y / 350, 0.14)})` : `translateY(${-0.85 * Math.min(y, 56)}px)`;
    }
  };
  for (const el of screens.values()) el.addEventListener("scroll", () => updateChrome(el), { passive: true });

  // 이름과 소개는 이 기기에만 저장돼요
  const store = {
    get: (k, d) => { try { return localStorage.getItem(k) ?? d; } catch (_) { return d; } },
    set: (k, v) => { try { localStorage.setItem(k, v); } catch (_) { /* 저장을 못 해도 괜찮아요 */ } },
  };

  /* ---------- 파이어베이스로 저장·불러오기 ---------- */
  let hydrated = false;
  let saveTimer = 0;
  const walls = () => Object.fromEntries(chats.map((c) => [c.id, store.get("wall-" + c.id, "white")]));
  const persist = () => {
    if (!hydrated || !window.cloud || live) return;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      window.cloud
        .save({
          profile: { name: store.get("profile-name", "내 계정"), bio: store.get("profile-bio", "소개를 추가해 보세요") },
          walls: walls(),
          chats: chats.map(({ id, last, time, unread, pin, muted, msgs }) => ({ id, last, time, unread: Boolean(unread), pin: Boolean(pin), muted: Boolean(muted), msgs: msgs.map((m) => ({ w: m[0], t: m[1], b: m[2] || "" })) })),
        })
        .catch((err) => console.warn("저장하지 못했어요:", err));
    }, 800);
  };

  /* ---------- 메시지 ---------- */
  const chats = []; // 처음에는 비어 있어요. 로그인하면 진짜 대화가 들어와요
  const initials = (n) => (n === "회사 동료들" ? "회사" : n.length >= 3 ? n.slice(-2) : n);
  const list = document.getElementById("chat-list");
  const pinnedEl = document.getElementById("pinned");
  const emptyMsg = document.getElementById("chat-empty");
  const searchEl = document.getElementById("chat-search");
  const esc = (t) => t.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
  const avatarHTML = (name, photo, cls = "avatar contact") =>
    photo ? `<span class="${cls} photo" style="background-image:url(${photo})"></span>` : `<span class="${cls}">${esc(initials(name))}</span>`;
  const chev = '<svg viewBox="0 0 8 14" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M1 1l6 6-6 6"/></svg>';
  let onlyUnread = false;
  let live = false; // 로그인한 계정이면 진짜 사람과의 대화를 불러와요
  let unsubMsgs = null;
  let prefs = {};
  try { prefs = JSON.parse(store.get("prefs", "{}")); } catch (_) { /* 처음이에요 */ }
  const touchPref = (c) => {
    if (!c.live) return;
    prefs[c.id] = { pin: Boolean(c.pin), muted: Boolean(c.muted), hidden: Boolean(c.hidden) };
    store.set("prefs", JSON.stringify(prefs));
  };
  const fmtTime = (t) => {
    if (!t) return "";
    const d = new Date(t);
    const hm = `${d.getHours() < 12 ? "오전" : "오후"} ${d.getHours() % 12 || 12}:${String(d.getMinutes()).padStart(2, "0")}`;
    return d.toDateString() === new Date().toDateString() ? hm : `${d.getMonth() + 1}월 ${d.getDate()}일`;
  };
  const renderChats = () => {
    persist();
    const q = searchEl.value.trim();
    const match = (c) => (c.name.includes(q) || c.last.includes(q)) && (!onlyUnread || c.unread);
    const pins = q || onlyUnread ? [] : chats.filter((c) => c.pin);
    pinnedEl.hidden = !pins.length;
    pinnedEl.innerHTML = pins
      .map((c) => `<button class="pin${c.unread ? " is-unread" : ""}" type="button" data-id="${c.id}">${c.unread ? '<i class="dot"></i>' : ""}${avatarHTML(c.name, c.photo)}${esc(c.name)}</button>`)
      .join("");
    const shown = chats.filter((c) => !pins.includes(c) && match(c));
    list.innerHTML = shown
      .map(
        (c) => `<li><div class="chat-row" role="button" tabindex="0" data-id="${c.id}">${c.unread ? '<i class="dot"></i>' : ""}${avatarHTML(c.name, c.photo)}<div class="chat-main"><div class="chat-top"><b>${esc(c.name)}</b><span class="meta">${c.muted ? '<svg class="mute-ico" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M6 16V11a6 6 0 0 1 9-5.2M18 11v5l2 2H5M4 4l16 16"/></svg>' : ""}${c.time}${chev}</span></div><p class="chat-prev">${esc(c.last)}</p></div><button class="del" type="button" data-del="${c.id}">삭제</button></div></li>`
      )
      .join("");
    emptyMsg.textContent = chats.length ? "결과가 없어요" : live ? "메시지가 없어요. 위쪽 버튼으로 대화를 시작해 보세요" : "로그인하면 대화를 시작할 수 있어요";
    emptyMsg.hidden = shown.length > 0 || pins.length > 0;
    const sc = document.getElementById("stat-chats");
    if (sc) sc.textContent = chats.length;
  };
  renderChats();
  searchEl.addEventListener("input", renderChats);

  const thread = document.getElementById("thread");
  const bubbles = document.getElementById("bubbles");
  const input = document.getElementById("composer-input");
  const sendBtn = document.getElementById("send");
  const sendIcon = document.getElementById("send-icon");
  let openChat = null;

  // 대화 화면: 메시지 목록을 다시 그려요 (fresh는 방금 생긴 말풍선 번호)
  const claimed = new Set(JSON.parse(store.get("claimed", "[]")));
  const preview = (m) => (m.type === "photo" ? "사진" : m.type === "draw" ? "그림" : m.type === "gift" ? `토큰 ${Number(m.amount).toLocaleString("ko-KR")} 선물` : m.type === "request" ? `토큰 ${Number(m.amount).toLocaleString("ko-KR")} 요청` : m.type === "poll" ? `투표: ${m.poll?.q || ""}` : m.text || "");
  // 글이 아닌 메시지(사진, 그림, 토큰, 투표)는 말풍선 대신 카드로 그려요
  const cardHTML = (m, who, i) => {
    const t = m.type;
    if (t === "photo" || t === "draw") return `<button class="media${t === "draw" ? " drawn" : ""}" data-view="${i}" type="button"><img src="${m.img}" alt="${t === "draw" ? "그림" : "사진"}"></button>`;
    if (t === "gift" || t === "request") {
      const gift = t === "gift";
      let btn;
      if (m.done) btn = `<span class="cash-state">${gift ? (who === "me" ? "상대가 받았어요" : "받았어요") : (who === "me" ? "상대가 보냈어요" : "보냈어요")}</span>`;
      else if (who === "me") btn = `<span class="cash-state">${gift ? "상대가 받으면 완료돼요" : "요청했어요"}</span>`;
      else btn = `<button class="cash-btn" data-act="${gift ? "accept" : "pay"}" data-i="${i}" type="button">${gift ? "받기" : "보내기"}</button>`;
      return `<div class="cash"><p class="cash-tag">${gift ? "토큰 선물" : "토큰 요청"}</p><div class="cash-amt"><b>${Number(m.amount).toLocaleString("ko-KR")}</b><span>토큰</span></div>${btn}</div>`;
    }
    if (t === "poll") {
      const votes = Object.values(m.votes || {});
      const mine = (m.votes || {})[window.cloud?.uid];
      return `<div class="poll"><p class="poll-q">${esc(m.poll.q)}</p>${m.poll.options
        .map((o, k) => {
          const n = votes.filter((v) => v === k).length;
          const pct = votes.length ? Math.round((n / votes.length) * 100) : 0;
          return `<button class="poll-opt${mine === k ? " on" : ""}" data-vote="${k}" data-i="${i}" type="button"><i class="po-bar" style="width:${pct}%"></i>${o.img ? `<img src="${o.img}" alt="">` : ""}<span class="po-t">${esc(o.t || `선택지 ${k + 1}`)}</span><span class="po-n">${n}</span></button>`;
        })
        .join("")}<p class="poll-meta">${votes.length}명 투표했어요${mine === undefined ? " · 눌러서 투표" : ""}</p></div>`;
    }
    return "";
  };
  const threadHTML = (chat, fresh = -1) => {
    const msgs = chat.msgs;
    let html = `<div class="stamp">오늘 ${chat.time.includes(":") ? chat.time : "오후 2:14"}</div>`;
    msgs.forEach(([who, text, by, raw], i) => {
      const next = msgs[i + 1];
      const last = !next || next[0] !== who || next[2] !== by;
      const card = raw && raw.type && raw.type !== "text" ? cardHTML(raw, who, i) : "";
      const b = card || `<div class="bubble${who === "me" ? " me" : ""}${last ? " last" : ""}${i === fresh ? " fresh" : ""}" data-i="${i}">${esc(text)}</div>`;
      if (who === "me") html += `<div class="line me">${b}</div>`;
      else {
        if (chat.group && last) html += `<div class="sender">${esc(by || "")}</div>`;
        html += `<div class="line">${last ? avatarHTML(by || chat.name, chat.photo) : '<span style="width:38px"></span>'}${b}</div>`;
      }
    });
    if (msgs.length && msgs[msgs.length - 1][0] === "me") html += '<div class="read">전송됨</div>';
    return html;
  };
  const renderThread = (fresh = -1) => {
    bubbles.innerHTML = threadHTML(openChat, fresh);
    bubbles.scrollTo({ top: bubbles.scrollHeight, behavior: fresh >= 0 && !reduceMotion.matches ? "smooth" : "auto" });
  };
  const applyWall = () => {
    const wall = store.get("wall-" + openChat.id, "white");
    thread.dataset.wall = wall;
    document.querySelectorAll("[data-pick]").forEach((o) => o.setAttribute("aria-pressed", String(o.dataset.pick === wall)));
  };
  const open = (id) => {
    openChat = chats.find((c) => c.id === id);
    if (!openChat) return;
    openChat.unread = false;
    document.getElementById("thread-name").textContent = openChat.name;
    const tav = document.getElementById("thread-avatar");
    tav.textContent = openChat.photo ? "" : initials(openChat.name);
    tav.style.backgroundImage = openChat.photo ? `url(${openChat.photo})` : "";
    tav.classList.toggle("photo", Boolean(openChat.photo));
    document.getElementById("thread-badge").textContent = chats.filter((c) => c.unread).length;
    applyWall();
    if (openChat.live) {
      const chat = openChat;
      unsubMsgs?.();
      chat.msgs = [];
      renderThread();
      unsubMsgs = window.cloud.watchMessages(chat.id, (list) => {
        const grew = list.length > chat.msgs.length;
        chat.msgs = list.map((m) => [m.from === window.cloud.uid ? "me" : "them", preview(m), chat.name, m]);
        if (openChat === chat) {
          renderThread(grew ? list.length - 1 : -1);
          window.cloud.markRead(chat.id);
        }
      });
    } else renderThread();
    app.classList.add("in-thread");
    thread.classList.add("is-open");
    renderChats();
  };
  const close = () => {
    unsubMsgs?.();
    unsubMsgs = null;
    thread.classList.remove("is-open");
    app.classList.remove("in-thread");
    input.blur();
    tabbar.requestRender();
  };
  const WAVE = "M5 10v4M9 6v12M13 3v18M17 8v8M21 11v2";
  const ARROW = "M12 19V5M5 12l7-7 7 7";
  input.addEventListener("input", () => {
    const ready = Boolean(input.value.trim());
    sendBtn.classList.toggle("ready", ready);
    sendIcon.firstElementChild.setAttribute("d", ready ? ARROW : WAVE);
  });

  /* 꾹 눌러서 나오는 메뉴 */
  const ctxBack = document.getElementById("ctx-back");
  const ctxMenu = document.getElementById("ctx-menu");
  const ICON = {
    pin: '<path d="M9 3h6l-1 6 3 3v2H7v-2l3-3zM12 14v7"/>',
    unread: '<path d="M20 12a8 7 0 0 1-12.5 5.8L4 19l1.2-3.4A8 7 0 1 1 20 12z"/><circle cx="19" cy="5" r="2.2" fill="currentColor"/>',
    mute: '<path d="M6 16V11a6 6 0 0 1 9-5.2M18 11v5l2 2H5M10 21h4M4 4l16 16"/>',
    trash: '<path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v6M14 11v6"/>',
    copy: '<rect x="8" y="8" width="12" height="12" rx="3"/><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2"/>',
  };
  const itemHTML = (it, i) => `<button type="button" role="menuitem" data-k="${i}"${it.danger ? ' class="danger"' : ""}>${it.icon ? `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round">${ICON[it.icon]}</svg>` : ""}<span>${esc(it.label)}</span></button>`;
  const hideMenu = () => ctxBack.classList.remove("is-open", "mode-stack");
  // 대화를 꾹 누르면: 대화 미리보기 카드와 큰 메뉴
  const showChatMenu = (chat, items) => {
    const pv = document.getElementById("ctx-preview");
    const m2 = document.getElementById("ctx-menu2");
    pv.dataset.wall = store.get("wall-" + chat.id, "white");
    const tmp = { ...chat, msgs: chat.msgs.slice(-9) };
    pv.innerHTML = `<div class="pv">${threadHTML(tmp)}</div>`;
    pv.onclick = () => (hideMenu(), open(chat.id));
    m2.innerHTML = items.map(itemHTML).join("");
    m2.onclick = (e) => {
      const btn = e.target.closest("button");
      if (!btn) return;
      hideMenu();
      items[Number(btn.dataset.k)].run();
    };
    ctxBack.classList.add("is-open", "mode-stack");
  };
  const showMenu = (x, y, items) => {
    ctxMenu.innerHTML = items.map(itemHTML).join("");
    ctxBack.classList.add("is-open");
    const r = app.getBoundingClientRect();
    const w = 220;
    const h = items.length * 48 + 12;
    const left = clamp(x - r.left - w / 2, 12, r.width - w - 12);
    const top = clamp(y - r.top + 8, 12, r.height - h - 12);
    ctxMenu.style.left = left + "px";
    ctxMenu.style.top = top + "px";
    ctxMenu.style.setProperty("--ox", x - r.left - left + "px");
    ctxMenu.onclick = (e) => {
      const btn = e.target.closest("button");
      if (!btn) return;
      hideMenu();
      items[Number(btn.dataset.k)].run();
    };
  };
  ctxBack.addEventListener("pointerdown", (e) => {
    if (e.target === ctxBack || e.target.id === "ctx-stack") hideMenu();
  });
  let swallow = false;
  document.addEventListener("pointerdown", () => (swallow = false), true);
  document.addEventListener("click", (e) => {
    if (!swallow) return;
    swallow = false;
    e.stopPropagation();
    e.preventDefault();
  }, true);
  const longPress = (root, pick, items) => {
    const fire = (el, x, y) => {
      swallow = true;
      navigator.vibrate?.(12);
      const built = items(el);
      if (el.dataset.id) showChatMenu(chats.find((c) => c.id === el.dataset.id), built);
      else showMenu(x, y, built);
    };
    root.addEventListener("pointerdown", (e) => {
      const el = pick(e.target);
      if (!el || (e.pointerType === "mouse" && e.button !== 0)) return;
      const sx = e.clientX;
      const sy = e.clientY;
      const timer = setTimeout(() => fire(el, sx, sy), 450);
      const cancel = (ev) => {
        if (ev.type === "pointermove" && Math.hypot(ev.clientX - sx, ev.clientY - sy) < 10) return;
        clearTimeout(timer);
        for (const t of ["pointermove", "pointerup", "pointercancel"]) root.removeEventListener(t, cancel);
      };
      for (const t of ["pointermove", "pointerup", "pointercancel"]) root.addEventListener(t, cancel);
    });
    root.addEventListener("contextmenu", (e) => {
      const el = pick(e.target);
      if (!el) return;
      e.preventDefault();
      fire(el, e.clientX, e.clientY);
    });
  };
  const chatItems = (el) => {
    const c = chats.find((x) => x.id === el.dataset.id);
    return [
      { icon: "pin", label: c.pin ? "고정 해제" : "고정", run: () => ((c.pin = !c.pin), touchPref(c), renderChats()) },
      { icon: "unread", label: c.unread ? "읽음으로 표시" : "읽지 않음으로 표시", run: () => ((c.unread = !c.unread), renderChats()) },
      { icon: "mute", label: c.muted ? "알림 켜기" : "알림 가리기", run: () => ((c.muted = !c.muted), touchPref(c), renderChats()) },
      { icon: "trash", label: "삭제", danger: true, run: () => ((c.hidden = true), touchPref(c), chats.splice(chats.indexOf(c), 1), renderChats()) },
    ];
  };
  longPress(list, (t) => t.closest(".chat-row"), chatItems);
  longPress(pinnedEl, (t) => t.closest(".pin"), chatItems);
  longPress(bubbles, (t) => t.closest(".bubble"), (el) => {
    const i = Number(el.dataset.i);
    return [
      { icon: "copy", label: "복사", run: () => navigator.clipboard?.writeText(openChat.msgs[i][1]).catch(() => {}) },
      { icon: "trash", label: "삭제", danger: true, run: () => (openChat.msgs.splice(i, 1), (openChat.last = openChat.msgs.length ? openChat.msgs[openChat.msgs.length - 1][1] : ""), renderThread(), renderChats()) },
    ];
  });

  /* 배경화면 */
  document.getElementById("thread-name-btn").addEventListener("click", () => sheetOpen(true, document.getElementById("wall-sheet")));
  document.querySelectorAll("[data-pick]").forEach((b) =>
    b.addEventListener("click", () => {
      store.set("wall-" + openChat.id, b.dataset.pick);
      applyWall();
      persist();
    })
  );
  document.getElementById("wall-done").addEventListener("click", () => sheetOpen(false));

  list.addEventListener("click", (e) => {
    const del = e.target.closest("[data-del]");
    if (del) {
      chats.splice(chats.findIndex((c) => c.id === del.dataset.del), 1);
      renderChats();
      return;
    }
    const row = e.target.closest(".chat-row");
    if (row && !list.classList.contains("editing")) open(row.dataset.id);
  });
  pinnedEl.addEventListener("click", (e) => {
    const pin = e.target.closest(".pin");
    if (pin) open(pin.dataset.id);
  });
  const editChats = document.getElementById("edit-chats");
  editChats.addEventListener("click", () => {
    const on = list.classList.toggle("editing");
    editChats.textContent = on ? "완료" : "편집";
  });
  const filterBtn = document.getElementById("filter-unread");
  filterBtn.addEventListener("click", () => {
    onlyUnread = !onlyUnread;
    filterBtn.setAttribute("aria-pressed", String(onlyUnread));
    renderChats();
  });
  const newSheet = document.getElementById("new-sheet");
  const newQ = document.getElementById("new-q");
  const newResults = document.getElementById("new-results");
  const newEmpty = document.getElementById("new-empty");
  let pickMode = "chat"; // "chat": 대화 열기, "gift": 토큰 보내기 받는 사람 고르기
  let people = [];
  const openPicker = (mode) => {
    if (!live) return showLogin(true);
    pickMode = mode;
    document.getElementById("new-title").textContent = mode === "gift" ? "받는 사람" : "새 메시지";
    newQ.value = "";
    newResults.innerHTML = "";
    newEmpty.hidden = false;
    newEmpty.textContent = "이름의 첫 글자부터 입력해 보세요";
    sheetOpen(true, newSheet);
  };
  document.getElementById("compose").addEventListener("click", () => openPicker("chat"));
  document.getElementById("new-cancel").addEventListener("click", () => sheetOpen(false));
  let searchTimer = 0;
  let searchSeq = 0;
  newQ.addEventListener("input", () => {
    clearTimeout(searchTimer);
    const q = newQ.value.trim();
    if (!q) {
      newResults.innerHTML = "";
      newEmpty.hidden = false;
      newEmpty.textContent = "이름의 첫 글자부터 입력해 보세요";
      return;
    }
    searchTimer = setTimeout(async () => {
      const seq = ++searchSeq;
      try {
        const found = await window.cloud.searchUsers(q);
        if (seq !== searchSeq) return;
        people = found;
        newResults.innerHTML = found.map((u, i) => `<li><button class="res-row" type="button" data-i="${i}">${avatarHTML(u.name || u.id, u.photo, "avatar contact")}<span class="res-main"><b>${esc(u.name || u.id)}</b><span>@${esc(u.id)}</span></span></button></li>`).join("");
        newEmpty.hidden = found.length > 0;
        newEmpty.textContent = "일치하는 사람이 없어요";
      } catch (err) {
        newEmpty.hidden = false;
        newEmpty.textContent = "찾지 못했어요. 잠시 뒤 다시 해 주세요";
      }
    }, 250);
  });
  const ensureChat = async (u) => {
    const cid = await window.cloud.openChat(u.uid, u.name || u.id, u.photo || "", store.get("profile-photo", ""));
    if (!chats.some((c) => c.id === cid)) chats.unshift({ id: cid, name: u.name || u.id, photo: u.photo || "", live: true, last: "", time: "", lastAt: Date.now(), unread: false, msgs: [] });
    return cid;
  };
  newResults.addEventListener("click", async (e) => {
    const row = e.target.closest(".res-row");
    if (!row) return;
    const u = people[Number(row.dataset.i)];
    try {
      const cid = await ensureChat(u);
      sheetOpen(false);
      renderChats();
      if (pickMode === "gift") openGift(cid, u.name || u.id);
      else open(cid);
    } catch (err) {
      newEmpty.hidden = false;
      newEmpty.textContent = "상대와 연결하지 못했어요. 잠시 뒤 다시 해 주세요";
    }
  });
  document.getElementById("thread-back").addEventListener("click", close);
  document.getElementById("composer").addEventListener("submit", (e) => {
    e.preventDefault();
    const text = input.value.trim();
    if (!text || !openChat) return;
    const chat = openChat;
    input.value = "";
    input.dispatchEvent(new Event("input"));
    if (chat.live) {
      window.cloud.send(chat.id, { type: "text", text }, text).catch((err) => console.warn("보내지 못했어요:", err));
      return;
    }
    chat.msgs.push(["me", text]);
    chat.last = text;
    chat.time = "방금";
    renderThread(chat.msgs.length - 1);
    setTimeout(() => {
      chat.msgs.push(["them", chat.reply, chat.replyBy]);
      chat.last = chat.reply;
      if (openChat === chat) renderThread(chat.msgs.length - 1);
      else chat.unread = true;
      renderChats();
    }, 900);
  });



  /* ---------- 돈 (화폐 단위는 토큰) ---------- */
  const IC = {
    store: '<path d="M4 9l1.500-5h13L20 9M4 9v11h16V9M4 9c0 1.800 1.500 2.800 2.700 2.800S9.300 10.800 9.300 9c0 1.800 1.200 2.800 2.700 2.800s2.700-1 2.700-2.800c0 1.800 1.300 2.800 2.700 2.800S20 10.800 20 9M10 20v-5h4v5"/>',
    cup: '<path d="M5 9h11v5a5 5 0 0 1-5 5h-1a5 5 0 0 1-5-5zM16 10h1.500a2.500 2.500 0 0 1 0 5H16M8 3v3M12 3v3"/>',
    bus: '<rect x="5" y="4" width="14" height="14" rx="3"/><path d="M5 12h14M8 18v2M16 18v2M8.500 15h.01M15.500 15h.01"/>',
    card: '<rect x="3" y="6" width="18" height="12" rx="3"/><path d="M3 10h18M7 15h3"/>',
    income: '<circle cx="12" cy="12" r="8.500"/><path d="M12 16V8M8.500 11.500L12 8l3.500 3.500"/>',
    gift: '<rect x="4" y="9" width="16" height="11" rx="2"/><path d="M3 9h18v3H3zM12 9v11M12 9C10 9 8 8 8 6.500S10.500 4.500 12 9zM12 9c2 0 4-1 4-2.500S13.500 4.500 12 9z"/>',
    book: '<path d="M5 4.500h10a3 3 0 0 1 3 3V20H8a3 3 0 0 1-3-3zM8 17h10"/>',
    ticket: '<path d="M4 8a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v2a2 2 0 0 0 0 4v2a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-2a2 2 0 0 0 0-4zM14 6v12"/>',
    bag: '<path d="M5 8h14l-1 12H6zM9 8V6.500a3 3 0 0 1 6 0V8"/>',
    percent: '<circle cx="12" cy="12" r="8.500"/><path d="M15.500 8.500l-7 7M9 9.500h.01M15 14.500h.01"/>',
  };
  const ico = (k) => `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">${IC[k]}</svg>`;
  const rng = (seed) => () => ((seed = (seed + 0x6d2b79f5) | 0), (((Math.imul(seed ^ (seed >>> 15), 1 | seed) + (Math.imul(seed ^ (seed >>> 7), 61 | seed) ^ seed)) ^ 0) >>> 0) / 4294967296);
  const SHOPS = [
    ["편의점", "식비", "store", 1800, 7500],
    ["카페", "카페", "cup", 3800, 7200],
    ["교통카드", "교통", "bus", 1400, 4000],
    ["서점", "문화", "book", 9000, 28000],
    ["영화관", "문화", "ticket", 11000, 16000],
    ["쇼핑", "쇼핑", "bag", 15000, 60000],
  ];
  const DAYS = 365 * 3;
  const startOfDay = (d) => new Date(d.getFullYear(), d.getMonth(), d.getDate());
  const today0 = startOfDay(new Date());
  const book = []; // 날짜순 내역 (지출은 음수, 입금은 양수)
  const daily = new Array(DAYS).fill(0); // 하루 동안 쓴 돈. 마지막이 오늘
  {
    const rand = rng(20240131);
    for (let i = DAYS - 1; i >= 0; i--) {
      const date = new Date(today0.getFullYear(), today0.getMonth(), today0.getDate() - i);
      const n = Math.floor(rand() * 3.2);
      for (let k = 0; k < n; k++) {
        const s = SHOPS[Math.floor(rand() * rand() * SHOPS.length)];
        const amt = Math.round((s[3] + rand() * (s[4] - s[3])) / 100) * 100;
        const at = new Date(date.getTime() + (8 + k * 4 + Math.floor(rand() * 3)) * 3600e3 + Math.floor(rand() * 59) * 60e3);
        if (at.getTime() > Date.now()) continue; // 아직 안 온 시각의 내역은 없어요
        book.push({ t: at.getTime(), name: s[0], cat: s[1], icon: s[2], amt: -amt });
        daily[DAYS - 1 - i] += amt;
      }
      if (date.getDate() === 25) book.push({ t: date.getTime() + 18.5 * 3600e3, name: "월급", cat: "입금", icon: "income", amt: 400000 });
      else if (rand() < 0.02) book.push({ t: date.getTime() + 7.7 * 3600e3, name: "친구에게 받음", cat: "송금", icon: "gift", amt: 30000 });
    }
  }
  const W = (n) => `${n.toLocaleString("ko-KR")} 토큰`;
  const signed = (n) => `${n > 0 ? "+" : "-"}${Math.abs(n).toLocaleString("ko-KR")} 토큰`;
  const hm = (t) => { const d = new Date(t); return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`; };
  const dayName = (t) => {
    const d = startOfDay(new Date(t));
    const diff = Math.round((today0 - d) / 864e5);
    return diff === 0 ? "오늘" : diff === 1 ? "어제" : `${d.getMonth() + 1}월 ${d.getDate()}일`;
  };
  const staticRecent = book.slice().sort((a, b) => b.t - a.t);
  let dyn = [];
  try { dyn = JSON.parse(store.get("bank-tx", "[]")); } catch (_) { /* 처음이에요 */ }
  const allTx = () => [...dyn, ...staticRecent].sort((a, b) => b.t - a.t);
  const monthSpent = () => daily.slice(DAYS - today0.getDate()).reduce((s, v) => s + v, 0);
  const BUDGET = 300000;
  const CATC = { 식비: "#ffe6d2", 카페: "#f0e4d6", 교통: "#dbe8ff", 문화: "#ece1ff", 쇼핑: "#ffdbe6", 입금: "#ffe0e0", 송금: "#ffe0e0", 은행: "#d6f1e6" };
  const num = (n) => Math.abs(n).toLocaleString("ko-KR");
  const amtHTML = (n) => `<b class="t-amt"><i class="${n > 0 ? "pos" : "neg"}">${n > 0 ? "+" : "-"}${num(n)}</i> 토큰</b>`;
  const kindOf = (e) => (e.amt > 0 ? "받음" : e.cat === "은행" ? "이자 상품" : "사용");
  const txRow = (e) => `<li class="t-row tx"><span class="t-ico" style="--c:${CATC[e.cat] || "#eceff3"}">${ico(e.icon)}</span><span class="t-main"><b class="ts-name">${esc(e.name)}</b><span class="t-label">${esc(e.cat)}</span><span class="t-label">${dayName(e.t)} ${hm(e.t)}</span></span>${amtHTML(e.amt)}<svg class="tchev gr" viewBox="0 0 8 14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M1 1l6 6-6 6"/></svg></li>`;
  const groups = (list) => `<ul class="t-rows">${list.map(txRow).join("")}</ul>`;
  const chev2 = '<svg class="tchev" viewBox="0 0 8 14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M1 1l6 6-6 6"/></svg>';

  /* ---- 은행: 이자 상품과 복리 ---- */
  const YEAR_S = 365 * 24 * 3600;
  const RATES = [5, 10, 15, 20, 25, 30, 35, 40, 45, 50];
  // 값: 연이율이 두 배가 되면 값은 약 3.5배. 5%는 20,000 토큰, 50%는 거의 처음 가진 돈 전부예요
  const PRICE = (r) => Math.round((20000 * Math.pow(r / 5, 1.8)) / 1000) * 1000;
  const perSec = (r) => Math.pow(1 + r / 100, 1 / YEAR_S) - 1; // 연이율 r%를 복리로 초마다 쪼갠 이율
  let bank = { rate: 0, bal: 1250000, acc: 0, t: Date.now() };
  try { Object.assign(bank, JSON.parse(store.get("bank", "{}"))); } catch (_) { /* 처음이에요 */ }
  let lastCloud = 0;
  const saveBank = (force = false) => {
    store.set("bank", JSON.stringify(bank));
    store.set("bank-tx", JSON.stringify(dyn.slice(0, 200)));
    if (window.cloud && (force || Date.now() - lastCloud > 15000)) {
      lastCloud = Date.now();
      window.cloud.save({ bank }).catch(() => {});
    }
  };
  const logTx = (e) => { dyn.unshift(e); if (dyn.length > 200) dyn.length = 200; };
  // secs초 동안 쌓인 이자를 한꺼번에 계산해요. 소수 이자는 모아 두었다가 1 토큰이 되면 받아요
  const accrue = (secs) => {
    if (bank.rate <= 0 || secs <= 0) return 0;
    const total = (bank.bal + bank.acc) * Math.pow(1 + perSec(bank.rate), secs);
    const nb = Math.floor(total);
    const pay = nb - bank.bal;
    bank.acc = total - nb;
    bank.bal = nb;
    return pay;
  };
  const nextIn = () => {
    const r = perSec(bank.rate);
    const per = (bank.bal + bank.acc) * r;
    return per >= 1 ? 1 : Math.max(1, Math.ceil((1 - bank.acc) / per));
  };
  const intervalText = () => {
    if (bank.rate <= 0) return "";
    const per = bank.bal * perSec(bank.rate);
    return per >= 1 ? `1초마다 ${Math.floor(per).toLocaleString("ko-KR")} 토큰` : `약 ${Math.ceil(1 / per)}초마다 1 토큰`;
  };
  const orderIds = ["spend", "bank", "hist"];
  const savedOrder = store.get("money-order", "").split(",").filter((x) => orderIds.includes(x));
  const order = [...savedOrder, ...orderIds.filter((x) => !savedOrder.includes(x))];
  const cashArt = () => {
    // 검은 카드 위의 점 무늬: 아래 가운데에서 반원으로 퍼지는 색 점들 (바깥으로 갈수록 커지고 흐려져요)
    let d = "";
    const cx = 170;
    const cy = 232;
    for (let r = 34, ring = 0; r < 215; r += 15, ring++) {
      const n = Math.max(8, Math.round((Math.PI * r) / 15.5));
      for (let k = 0; k <= n; k++) {
        const t = k / n;
        const a = Math.PI * (1 - t);
        const x = cx + Math.cos(a) * r * 1.08;
        const y = cy - Math.sin(a) * r;
        const hue = 168 - t * 128 - (ring % 2) * 6;
        const size = 1.5 + (r / 215) * 4.2;
        const op = 1 - (r / 215) * 0.35;
        d += `<circle cx="${x.toFixed(1)}" cy="${y.toFixed(1)}" r="${size.toFixed(2)}" fill="hsl(${hue.toFixed(0)} 52% ${64 - (r / 215) * 14}%)" opacity="${op.toFixed(2)}"/>`;
      }
    }
    return `<svg class="ac-art" viewBox="0 0 340 218" preserveAspectRatio="xMidYMax slice" aria-hidden="true">${d}</svg>`;
  };
  const sv2 = (d) => `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">${d}</svg>`;
  const mv = (id, title, body, extra = "") => `<div class="movable mv" data-id="${id}" role="button" tabindex="0"${extra}><h2 class="ac-h">${title}${chev2}</h2><div class="ac-box">${body}</div></div>`;
  const CARDS = {
    spend: () => mv("spend", "이번 달 쓴 토큰", `<b class="t-big">${W(monthSpent())}</b><div class="t-meter"><span style="width:${Math.min(100, Math.round((monthSpent() / BUDGET) * 100))}%"></span></div><p class="t-note">예산 ${BUDGET.toLocaleString("ko-KR")} 토큰 중 ${Math.round((monthSpent() / BUDGET) * 100)}%를 썼어요</p>`, ' id="open-chart"'),
    bank: () => mv("bank", "이자 상품", '<b class="t-big" id="bank-rate"></b><p class="t-note" id="bank-note"></p>', ' id="open-bank"'),
    hist: () => mv("hist", "최근 거래내역", '<div id="tx-top"></div>', ' id="open-history"'),
  };
  document.getElementById("money-root").innerHTML = `
    <div class="ac-top"><button class="ac-ic" id="ac-search" type="button" aria-label="거래내역">${sv2('<circle cx="11" cy="11" r="7"/><path d="M16.500 16.500L21 21"/>')}</button><button class="ac-ic" id="ac-more" type="button" aria-label="이자 상품">${sv2('<circle cx="12" cy="12" r="9"/><path d="M8 12h.01M12 12h.01M16 12h.01"/>')}</button></div>
    <div class="ac-card"><span class="ac-word">토큰</span>${cashArt()}</div>
    <div class="ac-bal">
      <div class="ac-bal-row"><div><span class="ac-bl">잔액</span><b id="bal-num"></b></div><button class="ac-pill" id="ac-send" type="button">보내기 또는 요청</button></div>
      <button class="ac-note" id="ac-note" type="button"></button>
      <div class="t-line"><span class="t-ico cardico">${ico("card")}</span><span class="ts-name">내 카드</span><span class="t-right" id="card-month"></span></div>
    </div>
    <div id="movable">${order.map((id) => CARDS[id]()).join("")}</div>`;
  const renderMoney = () => {
    document.getElementById("bal-num").innerHTML = `${bank.bal.toLocaleString("ko-KR")}<small> 토큰</small>`;
    document.getElementById("ac-note").textContent = bank.rate > 0 ? `연 ${bank.rate}% 복리 이자 상품 가입 중 · ${intervalText()}` : "이자 상품에 가입하면 토큰이 복리로 늘어나요";
    document.getElementById("card-month").textContent = `이번 달 ${monthSpent().toLocaleString("ko-KR")} 토큰`;
    const rateEl = document.getElementById("bank-rate");
    const noteEl = document.getElementById("bank-note");
    if (bank.rate > 0) {
      rateEl.textContent = `연 ${bank.rate}% 복리`;
      noteEl.textContent = `${intervalText()} · 다음 이자까지 ${nextIn()}초`;
    } else {
      rateEl.textContent = "가입한 상품이 없어요";
      noteEl.textContent = `연 5% 상품 ${PRICE(5).toLocaleString("ko-KR")} 토큰부터 시작해요`;
    }
    document.getElementById("tx-top").innerHTML = groups(allTx().slice(0, 5));
  };
  const renderHistory = () => { document.getElementById("history-list").innerHTML = groups(allTx().slice(0, 80)); };
  renderMoney();

  const histSheet = document.getElementById("history-sheet");
  const chartSheet = document.getElementById("chart-sheet");
  const bankSheet = document.getElementById("bank-sheet");
  document.getElementById("ac-search").addEventListener("click", () => { renderHistory(); sheetOpen(true, histSheet); });
  for (const id of ["ac-more", "ac-note"]) document.getElementById(id).addEventListener("click", () => { renderProducts(); sheetOpen(true, bankSheet); });
  document.querySelectorAll("[data-close]").forEach((b) => b.addEventListener("click", () => sheetOpen(false)));

  const renderProducts = () => {
    document.getElementById("bank-bal").textContent = `내 토큰 ${W(bank.bal)}`;
    document.getElementById("bank-list").innerHTML = RATES.map((r) => {
      const price = PRICE(r);
      const own = bank.rate === r;
      const lower = r < bank.rate;
      const afford = bank.bal >= price;
      const label = own ? "가입 중" : lower ? "보유 중" : afford ? "구입" : "토큰 부족";
      const per = bank.bal * perSec(r);
      const every = per >= 1 ? `1초마다 ${Math.floor(per).toLocaleString("ko-KR")} 토큰` : `약 ${Math.ceil(1 / per)}초마다 1 토큰`;
      return `<li class="t-row prod"><span class="t-ico sm" style="--c:#d6f1e6">${ico("percent")}</span><span class="t-main"><b class="ts-name">연 ${r}% 복리</b><span class="t-label">${every}</span></span><span class="prod-side"><b class="prod-price">${price.toLocaleString("ko-KR")} 토큰</b><button class="t-btn small ${own || lower || !afford ? "grey" : ""}" data-buy="${r}" type="button" ${own || lower || !afford ? "disabled" : ""}>${label}</button></span></li>`;
    }).join("");
  };
  document.getElementById("bank-list").addEventListener("click", async (e) => {
    const btn = e.target.closest("[data-buy]");
    if (!btn || btn.disabled) return;
    const r = Number(btn.dataset.buy);
    const price = PRICE(r);
    const ok = await ask({ title: `연 ${r}% 이자 상품`, body: `${price.toLocaleString("ko-KR")} 토큰을 내고 구입할까요? 구입하면 내 토큰이 복리로 계속 늘어나요.`, ok: "구입", cancel: "취소" });
    if (!ok || bank.bal < price) return;
    accrue(Math.floor((Date.now() - bank.t) / 1000));
    bank.t = Date.now();
    bank.bal -= price;
    bank.rate = r;
    daily[DAYS - 1] += price;
    logTx({ t: Date.now(), name: "이자 상품 구입", cat: "은행", icon: "percent", amt: -price });
    saveBank(true);
    renderMoney();
    renderProducts();
  });
  // 1초마다: 쌓인 이자를 정산해요 (탭을 떠나 있던 시간도 한꺼번에 계산)
  const tick = () => {
    const secs = Math.floor((Date.now() - bank.t) / 1000);
    if (secs >= 1) {
      const pay = accrue(secs);
      bank.t += secs * 1000;
      if (pay > 0) {
        logTx({ t: Date.now(), name: "이자", cat: "은행", icon: "percent", amt: pay });
        saveBank();
      }
    }
    renderMoney();
    if (bankSheet.classList.contains("is-open")) renderProducts();
    if (histSheet.classList.contains("is-open")) renderHistory();
  };
  tick();
  setInterval(tick, 1000);
  document.addEventListener("visibilitychange", () => { if (!document.hidden) tick(); });

  document.getElementById("movable").addEventListener("click", (e) => {
    if (dragSwallow) return;
    const c = e.target.closest(".movable");
    if (!c) return;
    if (c.dataset.id === "hist") { renderHistory(); sheetOpen(true, histSheet); }
    else if (c.dataset.id === "bank") { renderProducts(); sheetOpen(true, bankSheet); }
    else { sheetOpen(true, chartSheet); drawChart(range); }
  });

  /* ---- 꾹 눌러서 카드 순서 바꾸기 (내 토큰·내 카드 칸은 고정) ---- */
  let dragSwallow = false;
  {
    const root = document.getElementById("movable");
    const scr = screens.get("money");
    let st = null;
    const contentY = (y) => y - scr.getBoundingClientRect().top + scr.scrollTop;
    const place = () => {
      const { el } = st;
      el.style.transform = `translateY(${contentY(st.y) - st.grab - el.offsetTop}px) scale(1.03)`;
    };
    const reorder = () => {
      const { el } = st;
      const sibs = [...root.children].filter((c) => c !== el);
      const center = contentY(st.y) - st.grab + el.offsetHeight / 2;
      let before = null;
      for (const s of sibs) if (center < s.offsetTop + s.offsetHeight / 2) { before = s; break; }
      if ((before || null) === el.nextElementSibling) return;
      const tops = new Map(sibs.map((s) => [s, s.offsetTop]));
      root.insertBefore(el, before);
      for (const s of sibs) {
        const d = tops.get(s) - s.offsetTop;
        if (!d) continue;
        s.style.transition = "none";
        s.style.transform = `translateY(${d}px)`;
        void s.offsetHeight;
        s.style.transition = "transform 0.3s cubic-bezier(0.22, 1, 0.36, 1)";
        s.style.transform = "";
      }
    };
    const loop = () => {
      if (!st || !st.active) return;
      const r = scr.getBoundingClientRect();
      const speed = st.y < r.top + 120 ? -10 : st.y > r.bottom - 170 ? 10 : 0;
      if (speed) scr.scrollTop += speed;
      place();
      reorder();
      st.raf = requestAnimationFrame(loop);
    };
    const end = () => {
      if (!st) return;
      clearTimeout(st.timer);
      const s = st;
      st = null;
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
      if (!s.active) return;
      cancelAnimationFrame(s.raf);
      const el = s.el;
      el.style.transition = "transform 0.35s cubic-bezier(0.22, 1, 0.36, 1), box-shadow 0.35s";
      el.style.transform = "";
      el.classList.remove("lifting");
      setTimeout(() => { el.style.transition = ""; }, 380);
      store.set("money-order", [...root.children].map((c) => c.dataset.id).join(","));
      setTimeout(() => (dragSwallow = false), 120);
    };
    const move = (e) => {
      if (!st) return;
      st.y = e.clientY;
      if (!st.active && Math.hypot(e.clientX - st.x0, e.clientY - st.y0) > 10) {
        clearTimeout(st.timer);
        st = null;
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", end);
        window.removeEventListener("pointercancel", end);
      }
    };
    root.addEventListener("pointerdown", (e) => {
      const el = e.target.closest(".movable");
      if (!el || st || (e.pointerType === "mouse" && e.button !== 0)) return;
      st = { el, x0: e.clientX, y0: e.clientY, y: e.clientY, active: false };
      st.timer = setTimeout(() => {
        st.active = true;
        dragSwallow = true;
        navigator.vibrate?.(14);
        st.grab = contentY(st.y) - el.offsetTop;
        el.style.transition = "box-shadow 0.25s";
        el.classList.add("lifting");
        loop();
      }, 450);
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", end);
      window.addEventListener("pointercancel", end);
    });
    // 끌어올린 뒤에는 화면이 같이 스크롤되지 않게 막아요
    document.addEventListener("touchmove", (e) => { if (st && st.active) e.preventDefault(); }, { passive: false });
  }

  /* 쓴 돈 꺾은선 그래프: 년 · 월 · 일 */
  // 1년: 한 달씩 12개, 1개월: 하루씩 30개, 1일: 한 시간씩 24개
  const series = (r) => {
    const out = [];
    if (r === "m") for (let i = 29; i >= 0; i--) { const d = new Date(today0.getFullYear(), today0.getMonth(), today0.getDate() - i); out.push({ l: `${d.getMonth() + 1}/${d.getDate()}`, v: daily[DAYS - 1 - i] }); }
    else if (r === "y") {
      for (let k = 11; k >= 0; k--) {
        const first = new Date(today0.getFullYear(), today0.getMonth() - k, 1);
        const next = new Date(first.getFullYear(), first.getMonth() + 1, 1);
        let v = 0;
        for (let i = 0; i < DAYS; i++) { const d = new Date(today0.getFullYear(), today0.getMonth(), today0.getDate() - (DAYS - 1 - i)); if (d >= first && d < next) v += daily[i]; }
        out.push({ l: `${first.getMonth() + 1}월`, v });
      }
    } else {
      const end = Date.now();
      const spend = allTx().filter((e) => e.amt < 0 && e.t > end - 24 * 3600e3);
      for (let h = 23; h >= 0; h--) {
        const hi = end - h * 3600e3;
        const lo = hi - 3600e3;
        out.push({ l: `${new Date(lo).getHours()}시`, v: spend.filter((e) => e.t > lo && e.t <= hi).reduce((s, e) => s - e.amt, 0) });
      }
    }
    return out;
  };
  const chart = document.getElementById("chart");
  const tip = document.getElementById("chart-tip");
  const LABEL = { y: "1년 동안 쓴 토큰", m: "1개월 동안 쓴 토큰", d: "1일 동안 쓴 토큰" };
  let range = "m"; // 처음에는 1개월
  const drawChart = (r) => {
    range = r;
    document.querySelectorAll("#chart-seg button").forEach((b) => b.setAttribute("aria-selected", String(b.dataset.r === r)));
    const pts = series(r);
    const total = pts.reduce((s, p) => s + p.v, 0);
    document.getElementById("chart-label").textContent = LABEL[r];
    document.getElementById("chart-total").textContent = W(total);
    const max = Math.max(...pts.map((p) => p.v), 1);
    const X = (i) => 14 + (i * 312) / Math.max(pts.length - 1, 1);
    const Y = (v) => 180 - (v / max) * 150;
    const line = pts.map((p, i) => `${i ? "L" : "M"}${X(i).toFixed(1)} ${Y(p.v).toFixed(1)}`).join(" ");
    chart.innerHTML = `<defs><linearGradient id="cg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#3182f6" stop-opacity="0.28"/><stop offset="1" stop-color="#3182f6" stop-opacity="0"/></linearGradient></defs>
      <path d="${line} L${X(pts.length - 1)} 190 L${X(0)} 190Z" fill="url(#cg)" class="c-area"/>
      <path d="${line}" pathLength="1" class="c-line"/>
      ${pts.map((p, i) => `<circle cx="${X(i).toFixed(1)}" cy="${Y(p.v).toFixed(1)}" r="3.500" class="c-dot" style="animation-delay:${0.5 + (i / pts.length) * 0.9}s"/>`).join("")}
      <line class="c-guide" x1="0" x2="0" y1="20" y2="190"/>`;
    const ax = document.getElementById("chart-axis");
    const every = Math.ceil(pts.length / 6);
    ax.innerHTML = pts.map((p, i) => ((i % every === 0 && pts.length - 1 - i >= Math.ceil(every / 2)) || i === pts.length - 1 ? `<span style="left:${(X(i) / 340) * 100}%">${p.l}</span>` : "")).join("");
    tip.hidden = true;
    chart._pts = { pts, X, Y };
    chart.classList.remove("go");
    void chart.getBoundingClientRect();
    chart.classList.add("go");
  };
  const scrub = (e) => {
    const c = chart._pts;
    if (!c) return;
    const r = chart.getBoundingClientRect();
    const x = ((e.clientX - r.left) / r.width) * 340;
    let best = 0;
    c.pts.forEach((_, i) => { if (Math.abs(c.X(i) - x) < Math.abs(c.X(best) - x)) best = i; });
    const g = chart.querySelector(".c-guide");
    g.setAttribute("x1", c.X(best)); g.setAttribute("x2", c.X(best));
    g.style.opacity = 1;
    tip.hidden = false;
    tip.textContent = `${c.pts[best].l} · ${W(c.pts[best].v)}`;
    tip.style.left = `${clamp((c.X(best) / 340) * 100, 18, 82)}%`;
  };
  chart.addEventListener("pointerdown", scrub);
  chart.addEventListener("pointermove", (e) => { if (e.buttons || e.pointerType === "touch") scrub(e); });
  document.getElementById("chart-seg").addEventListener("click", (e) => { const b = e.target.closest("button"); if (b) drawChart(b.dataset.r); });
  
  /* ---------- 대화 더하기: 사진, 그리기, 투표, 토큰 선물 ---------- */
  const toastEl = document.getElementById("toast");
  let toastTimer = 0;
  const toast = (t) => {
    toastEl.textContent = t;
    toastEl.classList.add("on");
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => toastEl.classList.remove("on"), 1900);
  };
  const sendTo = (cid, payload) => window.cloud.send(cid, payload, preview(payload));

  // 사진을 줄여서 글자(데이터 주소)로 만들어요. 서버에 그대로 저장돼요
  const readImage = (file, max, q = 0.72, square = false) =>
    new Promise((resolve, reject) => {
      const url = URL.createObjectURL(file);
      const img = new Image();
      img.onload = () => {
        const sw = square ? Math.min(img.width, img.height) : img.width;
        const sh = square ? sw : img.height;
        const sx = (img.width - sw) / 2;
        const sy = (img.height - sh) / 2;
        const k = Math.min(1, max / Math.max(sw, sh));
        const c = document.createElement("canvas");
        c.width = Math.max(1, Math.round(sw * k));
        c.height = Math.max(1, Math.round(sh * k));
        c.getContext("2d").drawImage(img, sx, sy, sw, sh, 0, 0, c.width, c.height);
        URL.revokeObjectURL(url);
        let out = c.toDataURL("image/jpeg", q);
        for (let s = q; out.length > 520000 && s > 0.3; s -= 0.1) out = c.toDataURL("image/jpeg", s);
        resolve(out);
      };
      img.onerror = () => reject(new Error("사진을 열지 못했어요"));
      img.src = url;
    });

  /* + 메뉴 */
  const attachBack = document.getElementById("attach-back");
  document.getElementById("plus").addEventListener("click", () => {
    if (!openChat?.live) return;
    attachBack.classList.add("is-open");
  });
  attachBack.addEventListener("pointerdown", (e) => { if (e.target === attachBack) attachBack.classList.remove("is-open"); });
  const fileCamera = document.getElementById("file-camera");
  const filePhoto = document.getElementById("file-photo");
  const sendPhoto = async (file) => {
    if (!file || !openChat?.live) return;
    const cid = openChat.id;
    try {
      const img = await readImage(file, 900, 0.72);
      await sendTo(cid, { type: "photo", img });
    } catch (err) {
      toast("사진을 보내지 못했어요");
    }
  };
  fileCamera.addEventListener("change", () => { sendPhoto(fileCamera.files[0]); fileCamera.value = ""; });
  filePhoto.addEventListener("change", () => { sendPhoto(filePhoto.files[0]); filePhoto.value = ""; });
  document.getElementById("attach").addEventListener("click", (e) => {
    const b = e.target.closest("[data-a]");
    if (!b) return;
    attachBack.classList.remove("is-open");
    const a = b.dataset.a;
    if (a === "camera") fileCamera.click();
    else if (a === "photo") filePhoto.click();
    else if (a === "draw") openDraw();
    else if (a === "poll") openPoll();
    else if (a === "token") openGift(openChat.id, openChat.name);
  });

  /* 사진 크게 보기 */
  const viewer = document.getElementById("viewer");
  bubbles.addEventListener("click", async (e) => {
    const chat = openChat;
    if (!chat) return;
    const media = e.target.closest("[data-view]");
    if (media) {
      document.getElementById("viewer-img").src = chat.msgs[Number(media.dataset.view)][3].img;
      viewer.hidden = false;
      requestAnimationFrame(() => viewer.classList.add("is-open"));
      return;
    }
    const act = e.target.closest("[data-act]");
    if (act) {
      const m = chat.msgs[Number(act.dataset.i)][3];
      if (act.dataset.act === "accept") {
        if (m.done || claimed.has(m.id)) return;
        claimed.add(m.id);
        store.set("claimed", JSON.stringify([...claimed]));
        accrue(Math.floor((Date.now() - bank.t) / 1000));
        bank.t = Date.now();
        bank.bal += Number(m.amount);
        logTx({ t: Date.now(), name: `${chat.name}님의 선물`, cat: "송금", icon: "gift", amt: Number(m.amount) });
        saveBank(true);
        renderMoney();
        window.cloud.updateMsg(chat.id, m.id, { done: true }).catch(() => {});
        toast(`토큰 ${Number(m.amount).toLocaleString("ko-KR")}을 받았어요`);
      } else if (act.dataset.act === "pay") {
        if (m.done) return;
        if (bank.bal < Number(m.amount)) return void (await ask({ title: "토큰이 부족해요", body: `내 토큰은 ${bank.bal.toLocaleString("ko-KR")}이에요.`, ok: "확인" }));
        const yes = await ask({ title: `토큰 ${Number(m.amount).toLocaleString("ko-KR")}을 보낼까요?`, body: `${chat.name}님의 요청이에요.`, ok: "보내기", cancel: "취소" });
        if (!yes) return;
        accrue(Math.floor((Date.now() - bank.t) / 1000));
        bank.t = Date.now();
        bank.bal -= Number(m.amount);
        logTx({ t: Date.now(), name: `${chat.name}님에게 선물`, cat: "송금", icon: "gift", amt: -Number(m.amount) });
        saveBank(true);
        renderMoney();
        await sendTo(chat.id, { type: "gift", amount: Number(m.amount) });
        window.cloud.updateMsg(chat.id, m.id, { done: true }).catch(() => {});
        toast("보냈어요");
      }
      return;
    }
    const vote = e.target.closest("[data-vote]");
    if (vote) {
      const m = chat.msgs[Number(vote.dataset.i)][3];
      window.cloud.updateMsg(chat.id, m.id, { [`votes.${window.cloud.uid}`]: Number(vote.dataset.vote) }).catch(() => toast("투표하지 못했어요"));
    }
  });
  viewer.addEventListener("click", () => {
    viewer.classList.remove("is-open");
    setTimeout(() => (viewer.hidden = true), 250);
  });

  /* 토큰 선물과 요청: 검은 시트. 숫자를 누르면 직접 입력할 수 있어요 */
  const giftSheet = document.getElementById("gift-sheet");
  const gNum = document.getElementById("g-num");
  const keypad = document.getElementById("keypad");
  let gift = { cid: "", name: "", amt: 100 };
  const showAmt = () => {
    gNum.textContent = gift.amt.toLocaleString("ko-KR");
    gNum.parentElement.style.fontSize = gift.amt >= 1e7 ? "46px" : gift.amt >= 1e6 ? "58px" : gift.amt >= 1e5 ? "68px" : "80px";
    document.getElementById("gift-bal").textContent = `내 토큰 ${W(bank.bal)}`;
  };
  function openGift(cid, name) {
    gift = { cid, name, amt: gift.amt > 0 ? gift.amt : 100 };
    document.getElementById("gift-to").textContent = `${name}님에게`;
    keypad.hidden = true;
    fresh = true;
    document.getElementById("g-keytoggle").textContent = "키패드 보기";
    showAmt();
    sheetOpen(true, giftSheet);
  }
  const step = (dir) => {
    const s = gift.amt >= 100000 ? 10000 : gift.amt >= 10000 ? 1000 : 100;
    gift.amt = Math.max(0, Math.min(99999999, gift.amt + dir * s));
    showAmt();
    navigator.vibrate?.(6);
  };
  for (const [id, dir] of [["g-minus", -1], ["g-plus", 1]]) {
    const btn = document.getElementById(id);
    let hold = 0;
    let rep = 0;
    const stop = () => { clearTimeout(hold); clearInterval(rep); };
    btn.addEventListener("pointerdown", () => {
      step(dir);
      hold = setTimeout(() => (rep = setInterval(() => step(dir), 90)), 380);
    });
    for (const t of ["pointerup", "pointerleave", "pointercancel"]) btn.addEventListener(t, stop);
  }
  const toggleKeys = (on) => {
    keypad.hidden = !on;
    document.getElementById("g-keytoggle").textContent = on ? "키패드 숨기기" : "키패드 보기";
  };
  document.getElementById("g-amt").addEventListener("click", () => toggleKeys(keypad.hidden));
  document.getElementById("g-keytoggle").addEventListener("click", () => toggleKeys(keypad.hidden));
  let fresh = false;
  keypad.addEventListener("click", (e) => {
    const b = e.target.closest("[data-k]");
    if (!b) return;
    const k = b.dataset.k;
    let s = fresh ? "" : String(gift.amt);
    fresh = false;
    s = k === "back" ? s.slice(0, -1) : s === "0" ? k.replace(/^0+/, "") : s + k;
    gift.amt = Math.min(99999999, Number(s || 0));
    showAmt();
    navigator.vibrate?.(5);
  });
  document.getElementById("g-send").addEventListener("click", async () => {
    if (gift.amt < 1) return toast("보낼 토큰을 정해 주세요");
    if (gift.amt > bank.bal) return void (await ask({ title: "토큰이 부족해요", body: `내 토큰은 ${bank.bal.toLocaleString("ko-KR")}이에요.`, ok: "확인" }));
    const { cid, name, amt } = gift;
    try {
      await sendTo(cid, { type: "gift", amount: amt });
    } catch (err) {
      return toast("보내지 못했어요");
    }
    accrue(Math.floor((Date.now() - bank.t) / 1000));
    bank.t = Date.now();
    bank.bal -= amt;
    logTx({ t: Date.now(), name: `${name}님에게 선물`, cat: "송금", icon: "gift", amt: -amt });
    saveBank(true);
    renderMoney();
    sheetOpen(false);
    toast(`${name}님에게 토큰 ${amt.toLocaleString("ko-KR")}을 보냈어요`);
  });
  document.getElementById("g-request").addEventListener("click", async () => {
    if (gift.amt < 1) return toast("요청할 토큰을 정해 주세요");
    try {
      await sendTo(gift.cid, { type: "request", amount: gift.amt });
      sheetOpen(false);
      toast(`${gift.name}님에게 토큰 ${gift.amt.toLocaleString("ko-KR")}을 요청했어요`);
    } catch (err) {
      toast("요청하지 못했어요");
    }
  });
  document.getElementById("ac-send")?.addEventListener("click", () => openPicker("gift"));

  /* 사진 투표 */
  const pollSheet = document.getElementById("poll-sheet");
  const pollOpts = document.getElementById("poll-opts");
  const filePoll = document.getElementById("file-poll");
  let poll = [];
  let pollPick = 0;
  const drawPollOpts = () => {
    pollOpts.innerHTML = poll
      .map((o, i) => `<div class="poll-edit"><button class="poll-photo${o.img ? " has" : ""}" data-p="${i}" type="button" aria-label="사진 고르기" ${o.img ? `style="background-image:url(${o.img})"` : ""}>${o.img ? "" : '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>'}</button><input type="text" data-t="${i}" maxlength="30" placeholder="선택지 ${i + 1}" value="${esc(o.t)}"></div>`)
      .join("");
    document.getElementById("poll-add").hidden = poll.length >= 4;
  };
  function openPoll() {
    poll = [{ t: "", img: "" }, { t: "", img: "" }];
    document.getElementById("poll-q").value = "";
    document.getElementById("poll-err").textContent = "";
    drawPollOpts();
    sheetOpen(true, pollSheet);
  }
  pollOpts.addEventListener("input", (e) => { const i = e.target.dataset.t; if (i !== undefined) poll[Number(i)].t = e.target.value; });
  pollOpts.addEventListener("click", (e) => {
    const b = e.target.closest("[data-p]");
    if (!b) return;
    pollPick = Number(b.dataset.p);
    filePoll.click();
  });
  filePoll.addEventListener("change", async () => {
    const f = filePoll.files[0];
    filePoll.value = "";
    if (!f) return;
    try {
      poll[pollPick].img = await readImage(f, 420, 0.7);
      drawPollOpts();
    } catch (err) { toast("사진을 열지 못했어요"); }
  });
  document.getElementById("poll-add").addEventListener("click", () => { if (poll.length < 4) { poll.push({ t: "", img: "" }); drawPollOpts(); } });
  document.getElementById("poll-cancel").addEventListener("click", () => sheetOpen(false));
  document.getElementById("poll-send").addEventListener("click", async () => {
    const q = document.getElementById("poll-q").value.trim();
    const opts = poll.filter((o) => o.t.trim() || o.img);
    const err = document.getElementById("poll-err");
    if (!q) return (err.textContent = "질문을 적어 주세요");
    if (opts.length < 2) return (err.textContent = "선택지를 두 개 이상 만들어 주세요");
    try {
      await sendTo(openChat.id, { type: "poll", poll: { q, options: opts.map((o) => ({ t: o.t.trim(), img: o.img })) }, votes: {} });
      sheetOpen(false);
    } catch (e) { err.textContent = "보내지 못했어요. 잠시 뒤 다시 해 주세요"; }
  });

  /* 그리기: 그려서 설명해요 */
  const drawEl = document.getElementById("draw");
  const dc = document.getElementById("draw-canvas");
  const dctx = dc.getContext("2d");
  const COLORS = ["#111111", "#0a7cff", "#ff3b30", "#34c759", "#ff9f0a"];
  let strokes = [];
  let redo = [];
  let ink = { color: COLORS[0], w: 5 };
  const palette = document.getElementById("palette");
  palette.innerHTML = COLORS.map((c, i) => `<button type="button" data-c="${c}" style="background:${c}" aria-label="색 ${i + 1}"></button>`).join("");
  palette.addEventListener("click", (e) => { const b = e.target.closest("[data-c]"); if (b) { ink.color = b.dataset.c; palette.hidden = true; } });
  document.getElementById("draw-pen").addEventListener("click", () => (palette.hidden = !palette.hidden));
  const paintAll = () => {
    dctx.clearRect(0, 0, dc.width, dc.height);
    dctx.lineCap = dctx.lineJoin = "round";
    for (const s of strokes) {
      dctx.strokeStyle = dctx.fillStyle = s.color;
      dctx.lineWidth = s.w * dpr2;
      const p = s.pts;
      if (p.length === 1) { dctx.beginPath(); dctx.arc(p[0][0], p[0][1], (s.w * dpr2) / 2, 0, 7); dctx.fill(); continue; }
      dctx.beginPath();
      dctx.moveTo(p[0][0], p[0][1]);
      for (let i = 1; i < p.length - 1; i++) dctx.quadraticCurveTo(p[i][0], p[i][1], (p[i][0] + p[i + 1][0]) / 2, (p[i][1] + p[i + 1][1]) / 2);
      dctx.lineTo(p[p.length - 1][0], p[p.length - 1][1]);
      dctx.stroke();
    }
  };
  let dpr2 = 1;
  function openDraw() {
    drawEl.hidden = false;
    strokes = [];
    redo = [];
    palette.hidden = true;
    dpr2 = Math.min(2, window.devicePixelRatio || 1);
    requestAnimationFrame(() => {
      dc.width = Math.round(drawEl.clientWidth * dpr2);
      dc.height = Math.round(drawEl.clientHeight * dpr2);
      paintAll();
      drawEl.classList.add("is-open");
    });
  }
  const closeDraw = () => { drawEl.classList.remove("is-open"); setTimeout(() => (drawEl.hidden = true), 280); };
  let cur = null;
  dc.addEventListener("pointerdown", (e) => {
    palette.hidden = true;
    dc.setPointerCapture(e.pointerId);
    const r = dc.getBoundingClientRect();
    cur = { color: ink.color, w: ink.w, pts: [[(e.clientX - r.left) * dpr2, (e.clientY - r.top) * dpr2]] };
    strokes.push(cur);
    redo = [];
    paintAll();
  });
  dc.addEventListener("pointermove", (e) => {
    if (!cur) return;
    const r = dc.getBoundingClientRect();
    cur.pts.push([(e.clientX - r.left) * dpr2, (e.clientY - r.top) * dpr2]);
    paintAll();
  });
  for (const t of ["pointerup", "pointercancel"]) dc.addEventListener(t, () => (cur = null));
  document.getElementById("draw-undo").addEventListener("click", () => { if (strokes.length) { redo.push(strokes.pop()); paintAll(); } });
  document.getElementById("draw-redo").addEventListener("click", () => { if (redo.length) { strokes.push(redo.pop()); paintAll(); } });
  document.getElementById("draw-close").addEventListener("click", closeDraw);
  document.getElementById("draw-ok").addEventListener("click", async () => {
    if (!strokes.length) return closeDraw();
    const k = Math.min(1, 900 / dc.width);
    const out = document.createElement("canvas");
    out.width = Math.round(dc.width * k);
    out.height = Math.round(dc.height * k);
    const o = out.getContext("2d");
    o.fillStyle = "#fff";
    o.fillRect(0, 0, out.width, out.height);
    o.drawImage(dc, 0, 0, out.width, out.height);
    let img = out.toDataURL("image/png");
    if (img.length > 520000) img = out.toDataURL("image/jpeg", 0.8);
    closeDraw();
    try { await sendTo(openChat.id, { type: "draw", img }); } catch (err) { toast("그림을 보내지 못했어요"); }
  });

  /* ---------- 로그인 ---------- */
  const loginEl = document.getElementById("login");
  const loginForm = document.getElementById("login-form");
  const emailEl = document.getElementById("login-email");
  const pwEl = document.getElementById("login-pw");
  const errEl = document.getElementById("login-error");
  const submitBtn = document.getElementById("login-submit");
    let signup = false;
  const alertEl = document.getElementById("alert");
  const ask = ({ title, body, ok = "확인", cancel = "" }) =>
    new Promise((resolve) => {
      document.getElementById("alert-title").textContent = title;
      document.getElementById("alert-body").textContent = body;
      document.getElementById("alert-ok").textContent = ok;
      const cancelBtn = document.getElementById("alert-cancel");
      cancelBtn.textContent = cancel;
      cancelBtn.style.display = cancel ? "" : "none";
      alertEl.hidden = false;
      requestAnimationFrame(() => alertEl.classList.add("is-open"));
      const done = (v) => {
        alertEl.classList.remove("is-open");
        document.getElementById("alert-ok").onclick = cancelBtn.onclick = null;
        resolve(v);
      };
      document.getElementById("alert-ok").onclick = () => done(true);
      cancelBtn.onclick = () => done(false);
    });
  const realNameEl = document.getElementById("login-name");
  let warned = false;
  realNameEl.addEventListener("focus", async () => {
    if (!signup || warned) return;
    warned = true;
    realNameEl.blur();
    await ask({ title: "실명으로 가입해 주세요", body: "이름은 무조건 본인의 실명이어야 해요. 별명이나 다른 사람의 이름으로는 가입할 수 없어요.", ok: "알겠어요" });
    realNameEl.focus();
  });
  const pw2El = document.getElementById("login-pw2");
  const setMode = (on, animate = true) => {
    const apply = () => {
      signup = on;
      document.getElementById("login-title").textContent = on ? "계정 만들기" : "로그인";
      document.getElementById("login-sub").textContent = on ? "아이디와 비밀번호만 정하면 바로 시작할 수 있어요." : "아이디와 비밀번호로 대화와 프로필을 어디서나 이어서 쓸 수 있어요.";
      submitBtn.textContent = on ? "계정 만들기" : "로그인";
      document.getElementById("login-switch").innerHTML = on ? "이미 계정이 있나요? <b>로그인</b>" : "계정이 없나요? <b>계정 만들기</b>";
      pwEl.autocomplete = on ? "new-password" : "current-password";
      errEl.textContent = "";
      pw2El.value = "";
    };
    // 글자는 흐려졌다가 바뀌고, 비밀번호 확인 칸은 동시에 펼쳐지거나 접혀요
    loginEl.classList.toggle("signup", on);
    if (!animate || reduceMotion.matches) return apply();
    loginEl.classList.add("swapping");
    setTimeout(() => {
      apply();
      loginEl.classList.remove("swapping");
    }, 200);
  };
  // 로그인 뒷배경: 사진 다섯 장이 번갈아 부드럽게 바뀌어요
  const slides = [...document.querySelectorAll("#slides i")];
  let slide = 0;
  let slideTimer = 0;
  const showSlide = (n) => slides.forEach((s, i) => s.classList.toggle("on", i === n));
  const showLogin = (on) => {
    clearInterval(slideTimer);
    if (on) {
      showSlide(slide);
      slideTimer = setInterval(() => showSlide((slide = (slide + 1) % slides.length)), 5000);
    }
    loginEl.hidden = false;
    requestAnimationFrame(() => loginEl.classList.toggle("is-open", on));
    if (on) {
      setMode(false, false);
      warned = false;
      realNameEl.value = "";
    }
  };
  document.getElementById("open-login").addEventListener("click", () => showLogin(true));
  document.getElementById("login-close").addEventListener("click", () => showLogin(false));
  document.getElementById("login-switch").addEventListener("click", () => setMode(!signup));
  const run = async (fn) => {
    errEl.textContent = "";
    submitBtn.disabled = true;
    await cloudPromise; // 연결이 아직 끝나지 않았을 수 있어요
    if (!window.cloud) {
      errEl.textContent = `서버에 연결하지 못했어요: ${window.cloudError || "알 수 없는 오류"}`;
      submitBtn.disabled = false;
      return;
    }
    try {
      await fn(window.cloud);
      location.reload(); // 로그인한 계정의 저장 값을 처음부터 불러와요
    } catch (err) {
      errEl.textContent = err.message;
      submitBtn.disabled = false;
    }
  };
  loginForm.addEventListener("submit", (e) => e.preventDefault());
  submitBtn.addEventListener("click", async () => {
    const id = emailEl.value.trim();
    const pw = pwEl.value;
    if (!id || !pw) {
      errEl.textContent = "아이디와 비밀번호를 입력해 주세요";
      return;
    }
    if (!/^[A-Za-z0-9_]{3,20}$/.test(id)) {
      errEl.textContent = "아이디는 영문, 숫자, _ 로 3~20자예요";
      return;
    }
    if (signup && pw !== pw2El.value) {
      errEl.textContent = "비밀번호가 서로 달라요";
      return;
    }
    if (!signup) return run((c) => c.signIn(id, pw));
    const name = realNameEl.value.trim().replace(/\s+/g, " ");
    if (!/^[가-힣]{2,6}$/.test(name) && !/^[A-Za-z][A-Za-z ]{1,38}[A-Za-z]$/.test(name)) {
      errEl.textContent = "실명을 입력해 주세요 (한글 2~6자 또는 영문 이름)";
      return;
    }
    const sure = await ask({ title: `"${name}" 님이 실명이 맞나요?`, body: "실명이 아니면 이용이 제한될 수 있어요. 가입한 뒤에는 이름을 바꾸기 어려워요.", ok: "실명이에요", cancel: "다시 입력" });
    if (!sure) return realNameEl.focus();
    run((c) => c.signUp(id, pw, name, store.get("profile-photo", "")));
  });
  document.getElementById("logout").addEventListener("click", async () => {
    await window.cloud?.signOut();
    store.set("auth-hint", "null");
    location.reload();
  });
  // 로그인한 적이 있으면 연결이 끝나기 전에도 로그인된 화면을 바로 보여줘요 (깜빡임 없이)
  const hint = () => { try { return JSON.parse(store.get("auth-hint", "null")); } catch (_) { return null; } };
  const renderAuth = (cloud) => {
    const h = cloud ? null : hint();
    const signedIn = cloud ? !cloud.user.anonymous : Boolean(h);
    const id = cloud ? cloud.user.id : h?.id || "";
    if (cloud) store.set("auth-hint", signedIn ? JSON.stringify({ id }) : "null");
    const acct = screens.get("account");
    acct.classList.toggle("guest", !signedIn);
    acct.classList.toggle("member", signedIn);
    app.classList.toggle("guest-account", !signedIn);
    document.getElementById("auth-card").hidden = signedIn;
    document.getElementById("auth-row").hidden = !signedIn;
    document.getElementById("profile-id").textContent = signedIn ? "@" + id : "";
    if (signedIn) {
      if (!store.get("joined", "")) store.set("joined", String(Date.now()));
      document.getElementById("stat-days").textContent = Math.max(1, Math.ceil((Date.now() - Number(store.get("joined", Date.now()))) / 864e5));
    }
  };


  renderAuth(null);

  /* ---------- 계정 ---------- */
  const nameEl = document.getElementById("profile-name");
  const bioEl = document.getElementById("profile-bio");
  const avatarEl = document.getElementById("profile-avatar");
  const applyProfile = () => {
    const name = store.get("profile-name", "내 계정");
    nameEl.textContent = name;
    bioEl.textContent = store.get("profile-bio", "소개를 추가해 보세요");
    // 이름을 바꾸면 기본 프로필(회색 사람 모양) 대신 이름의 첫 글자가 보여요
    const custom = name !== "내 계정";
    const photo = store.get("profile-photo", "");
    avatarEl.className = custom ? "avatar" : "avatar-default";
    avatarEl.style.width = avatarEl.style.height = "";
    avatarEl.textContent = custom ? name[0] : "";
    avatarEl.style.backgroundImage = photo ? `url(${photo})` : "";
    if (photo) { avatarEl.classList.add("has-photo"); avatarEl.textContent = ""; }
  };
  applyProfile();
  const fileProfile = document.getElementById("file-profile");
  const pickPhoto = () => fileProfile.click();
  document.getElementById("photo-edit").addEventListener("click", pickPhoto);
  avatarEl.addEventListener("click", pickPhoto);
  fileProfile.addEventListener("change", async () => {
    const f = fileProfile.files[0];
    fileProfile.value = "";
    if (!f) return;
    try {
      const data = await readImage(f, 192, 0.82, true);
      store.set("profile-photo", data);
      applyProfile();
      if (window.cloud) {
        window.cloud.save({ photo: data }).catch(() => {});
        window.cloud.setPhoto(data, chats.filter((c) => c.live).map((c) => c.id));
      }
      toast("프로필 사진을 바꿨어요");
    } catch (err) {
      toast("사진을 바꾸지 못했어요");
    }
  });
  const sheet = document.getElementById("sheet");
  const dim = document.getElementById("dim");
  const editName = document.getElementById("edit-name");
  const editBio = document.getElementById("edit-bio");
  const sheetOpen = (on, el = sheet) => {
    if (on) el.classList.add("is-open");
    else document.querySelectorAll(".sheet").forEach((s) => s.classList.remove("is-open"));
    dim.classList.toggle("is-open", on);
    app.classList.toggle("sheet-open", on);
  };
  document.querySelectorAll(".js-edit-profile").forEach((b) =>
    b.addEventListener("click", () => {
      editName.value = nameEl.textContent;
      editBio.value = store.get("profile-bio", "");
      sheetOpen(true);
    })
  );
  document.getElementById("sheet-cancel").addEventListener("click", () => sheetOpen(false));
  dim.addEventListener("click", () => sheetOpen(false));
  document.getElementById("sheet-save").addEventListener("click", () => {
    store.set("profile-name", editName.value.trim() || "내 계정");
    store.set("profile-bio", editBio.value.trim() || "소개를 추가해 보세요");
    applyProfile();
    persist();
    sheetOpen(false);
  });
  const notify = document.getElementById("toggle-notify");
  notify.addEventListener("click", () => notify.setAttribute("aria-checked", String(notify.getAttribute("aria-checked") !== "true")));
  document.querySelectorAll(".segment button").forEach((b) =>
    b.addEventListener("click", () => document.querySelectorAll(".segment button").forEach((o) => o.setAttribute("aria-selected", String(o === b))))
  );

  const params = new URLSearchParams(location.search);
  const glassOff = params.get("glass") === "off";
  const glOff = params.get("gl") === "off";

  // 탭 바 뒤 화면을 사진으로 찍어 두는 곳과, 그 사진으로 막대 유리를 그리는 탭 바
  let tabbar = null;
  const backdrop =
    glassOff || glOff
      ? null
      : new window.Backdrop({
          root: app,
          screens,
          isBusy: () => Boolean(tabbar && tabbar.busy()),
          onUpdate: (name, shot) => tabbar && tabbar.setShot(name, shot),
        });
  tabbar = new window.TabBar(nav, { backdrop, pageRoot: app, gl: !glOff });
  window.tabbar = tabbar;
  window.backdrop = backdrop;

  nav.addEventListener("tabchange", (e) => {
    const name = e.detail.tab.dataset.tab;
    showScreen(name);
    if (openChat && name !== "messages") close();
    topTitle.textContent = screens.get(name).dataset.title;
    updateChrome(screens.get(name));
    backdrop?.setActive(name);
    tabbar.requestRender();
  });
  // 이미 선택된 탭을 다시 누르면 맨 위로
  document.addEventListener("click", (e) => {
    if (e.target.closest(".to-account")) nav.querySelector('[data-tab="account"]').click();
  });
  nav.addEventListener("tabreselect", (e) => scrollToTop(e.detail.tab.dataset.tab));

  scheme.addEventListener("change", () => {
    applyTheme();
    if (!backdrop) return;
    tabbar.dropShots(); // 색이 틀린 낡은 사진은 버리고
    backdrop.invalidateAll(); // 새로 찍어요
    backdrop.ensure(backdrop.active, true);
  });

  if (backdrop) {
    backdrop.setActive(firstScreen);
    // 나머지 화면도 미리 찍어 둬요 (탭을 옮길 때 바로 유리가 나오도록)
    for (const name of screens.keys()) if (name !== firstScreen) backdrop.ensure(name);
  }
  updateChrome(screens.get(firstScreen));

  // 클라우드에 저장해 둔 것이 있으면 불러와서 덮어써요 (연결이 안 되면 이 기기 값 그대로)
  const boot = async (cloud) => {
    renderAuth(cloud);
    if (cloud && !cloud.user.anonymous && cloud.user.name && store.get("profile-name", "내 계정") === "내 계정") {
      store.set("profile-name", cloud.user.name);
      applyProfile();
    }
    if (cloud && !cloud.user.anonymous) {
      // 로그인한 계정: 샘플 대화 대신 진짜 대화를 실시간으로 받아요
      live = true;
      chats.length = 0;
      renderChats();
      cloud.watchChats((list) => {
        const old = new Map(chats.map((c) => [c.id, c]));
        const next = list
          .filter((d) => !prefs[d.id]?.hidden)
          .map((d) => {
            const other = Object.entries(d.names || {}).find(([uid]) => uid !== cloud.uid);
            return {
              id: d.id,
              live: true,
              name: other ? other[1] : "알 수 없음",
              photo: other ? d.photos?.[other[0]] || "" : "",
              last: d.last || "",
              time: fmtTime(d.lastAt),
              lastAt: d.lastAt || 0,
              unread: Boolean(d.lastFrom && d.lastFrom !== cloud.uid && (d.reads?.[cloud.uid] || 0) < d.lastAt) && openChat?.id !== d.id,
              pin: Boolean(prefs[d.id]?.pin),
              muted: Boolean(prefs[d.id]?.muted),
              msgs: old.get(d.id)?.msgs || [],
            };
          })
          .sort((a, b) => b.lastAt - a.lastAt);
        chats.splice(0, chats.length, ...next);
        if (openChat?.live) openChat = chats.find((c) => c.id === openChat.id) || openChat;
        renderChats();
      });
    }
    try {
      const data = cloud && (await cloud.load());
      if (data) {
        if (data.photo && !store.get("profile-photo", "")) { store.set("profile-photo", data.photo); applyProfile(); }
        if (data.profile) {
          store.set("profile-name", data.profile.name);
          store.set("profile-bio", data.profile.bio);
          applyProfile();
        }
        if (data.bank && data.bank.t > bank.t) {
          Object.assign(bank, data.bank);
          store.set("bank", JSON.stringify(bank));
          tick();
        }
        for (const [id, w] of Object.entries(data.walls || {})) store.set("wall-" + id, w);
        if (data.chats && !live) {
          const saved = new Map(data.chats.map((c) => [c.id, c]));
          for (let i = chats.length - 1; i >= 0; i--) if (!saved.has(chats[i].id)) chats.splice(i, 1);
          for (const c of chats) {
            const s = saved.get(c.id);
            Object.assign(c, { last: s.last, time: s.time, unread: s.unread, pin: s.pin, muted: s.muted, msgs: (s.msgs || []).map((m) => [m.w, m.t, m.b || undefined]) });
          }
        }
      }
    } catch (err) {
      console.warn("저장해 둔 값을 불러오지 못했어요:", err);
    }
    hydrated = true;
    renderChats();
  };
  // 연결은 느린 폰에서 몇 초 걸릴 수 있어요. 끝나면 언제든 바로 반영하고, 8초가 지나도 안 끝나면 일단 로그인 전 화면으로 보여요
  let booted = false;
  cloudPromise.then((cloud) => {
    booted = true;
    boot(cloud);
  });
  setTimeout(() => {
    if (!booted) boot(null);
  }, 8000);
})();
