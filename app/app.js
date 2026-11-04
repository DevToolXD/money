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
    window.__paintStatus?.();
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
      const p0 = clamp(y / 16, 0, 1);
      topbar.style.setProperty("--p", p0);
      topbar.classList.toggle("on", p0 > 0);
      topTitle.style.opacity = el.hasAttribute("data-notitle") ? 0 : 1;
      topTitle.style.transform = "none";
      return;
    }
    const p = clamp((y - 28) / 20, 0, 1);
    topbar.style.setProperty("--p", p);
    topbar.classList.toggle("on", p > 0);
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

  /* ---------- 내 설정 저장: 프로필 글과 대화 배경만, 바뀐 뒤 3초 모아서 한 번에 ---------- */
  const WALLS = ["white", "sunset", "aurora", "night", "mint"];
  let wallMap = {};
  try { wallMap = JSON.parse(store.get("walls", "{}")); } catch (_) { /* 처음이에요 */ }
  let saveTimer = 0;
  const saveSettings = () => {
    if (!window.cloud?.user) return;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      const walls = Object.fromEntries(Object.entries(wallMap).filter(([, w]) => WALLS.includes(w)).slice(0, 150));
      window.cloud.saveSettings({ profile: { name: store.get("profile-name", "내 계정").slice(0, 30), bio: store.get("profile-bio", "").slice(0, 120) }, walls }).catch(() => {});
    }, 3000);
  };
  // 안전한 이미지 주소만 화면에 넣어요 (남이 보낸 값이 스타일이나 속성을 깨고 나오지 못하게)
  const IMG_RE = /^data:image\/(jpeg|png);base64,[A-Za-z0-9+/]+={0,2}$/;
  const safeImg = (s, max = 140000) => (typeof s === "string" && s.length <= max && IMG_RE.test(s) ? s : "");
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

  /* ---------- 메시지 ---------- */
  const chats = []; // 처음에는 비어 있어요. 로그인하면 진짜 대화가 들어와요
  const initials = (n) => (n === "회사 동료들" ? "회사" : n.length >= 3 ? n.slice(-2) : n);
  const list = document.getElementById("chat-list");
  const pinnedEl = document.getElementById("pinned");
  const emptyMsg = document.getElementById("chat-empty");
  const searchEl = document.getElementById("chat-search");
  const esc = (t) => t.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
  const avatarHTML = (name, photo, cls = "avatar contact") => {
    const p = safeImg(photo, 40000);
    return p ? `<span class="${cls} photo" style="background-image:url(${p})"></span>` : `<span class="${cls}">${esc(initials(name))}</span>`;
  };
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
    emptyMsg.textContent = chats.length ? "결과가 없어요" : live ? "메시지가 없어요. 위쪽 버튼으로 대화를 시작해 보세요" : window.cloud?.user || store.get("auth-hint", "null") !== "null" ? (window.cloudDown ? "서버 데이터베이스가 꺼져 있어서 대화를 못 불러왔어요" : window.__booted ? "서버에 연결하지 못했어요. 새로고침해 주세요" : "불러오는 중이에요") : "로그인하면 대화를 시작할 수 있어요";
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
  const preview = (m) => (m.type === "photo" ? "사진" : m.type === "draw" ? "그림" : m.type === "gift" ? `토큰 ${Number(m.amount).toLocaleString("ko-KR")} 선물` : m.type === "request" ? `토큰 ${Number(m.amount).toLocaleString("ko-KR")} 요청` : m.type === "poll" ? `투표: ${m.poll?.q || ""}` : m.text || "");
  // 글이 아닌 메시지(사진, 그림, 토큰, 투표)는 말풍선 대신 카드로 그려요
  const cardHTML = (m, who, i) => {
    const t = m.type;
    if (t === "photo" || t === "draw") {
      const src = safeImg(m.img);
      return src ? `<button class="media${t === "draw" ? " drawn" : ""}" data-view="${i}" type="button"><img src="${src}" alt="${t === "draw" ? "그림" : "사진"}" decoding="async" loading="lazy"></button>` : '<div class="bubble">열 수 없는 사진이에요</div>';
    }
    if (t === "gift" || t === "request") {
      const gift = t === "gift";
      const amount = Math.max(0, Math.floor(Number(m.amount) || 0));
      let btn;
      if (m.done) btn = `<span class="cash-state">${gift ? (who === "me" ? "상대가 받았어요" : "받았어요") : who === "me" ? "상대가 보냈어요" : "보냈어요"}</span>`;
      else if (who === "me") btn = `<span class="cash-state">${gift ? "상대가 받으면 완료돼요" : "요청했어요"}</span>`;
      else btn = `<button class="cash-btn" data-act="${gift ? "accept" : "pay"}" data-i="${i}" type="button">${gift ? "받기" : "보내기"}</button>`;
      return `<div class="cash"><p class="cash-tag">${gift ? "토큰 선물" : "토큰 요청"}</p><div class="cash-amt"><b>${amount.toLocaleString("ko-KR")}</b><span>토큰</span></div>${btn}</div>`;
    }
    if (t === "poll" && m.poll && Array.isArray(m.poll.options)) {
      const votes = Object.values(m.votes || {});
      const mine = (m.votes || {})[window.cloud?.uid];
      return `<div class="poll"><p class="poll-q">${esc(String(m.poll.q))}</p>${m.poll.options
        .map((o, k) => {
          const n = votes.filter((v) => v === k).length;
          const pct = votes.length ? Math.round((n / votes.length) * 100) : 0;
          const img = safeImg(o.img, 60000);
          return `<button class="poll-opt${mine === k ? " on" : ""}" data-vote="${k}" data-i="${i}" type="button"><i class="po-bar" style="width:${pct}%"></i>${img ? `<img src="${img}" alt="" decoding="async" loading="lazy">` : ""}<span class="po-t">${esc(String(o.t || `선택지 ${k + 1}`))}</span><span class="po-n">${n}</span></button>`;
        })
        .join("")}<p class="poll-meta">${votes.length}명 투표했어요${mine === undefined ? " · 눌러서 투표" : ""}</p></div>`;
    }
    return "";
  };
  // 한 줄(보낸 사람 이름표 + 말풍선이나 카드)을 만들어요. sig는 이 줄이 바뀌었는지 알아보는 표시예요
  const rowOf = (chat, i, fresh) => {
    const msgs = chat.msgs;
    const [who, text, by, raw] = msgs[i];
    const next = msgs[i + 1];
    const last = !next || next[0] !== who || next[2] !== by;
    const card = raw && raw.type && raw.type !== "text" ? cardHTML(raw, who, i) : "";
    const b = card || `<div class="bubble${who === "me" ? " me" : ""}${last ? " last" : ""}${i === fresh ? " fresh" : ""}" data-i="${i}">${esc(String(text))}</div>`;
    const prev = msgs[i - 1];
    const nameTag = chat.group && who === "them" && (!prev || prev[0] !== who || prev[2] !== by) && by ? `<span class="sender">${esc(by)}</span>` : "";
    const line = who === "me" ? `<div class="line me">${b}</div>` : `<div class="line">${last ? avatarHTML(by || chat.name, chat.photo) : '<span style="width:38px"></span>'}<div class="bcol">${nameTag}${b}</div></div>`;
    const key = raw?.id || String(i);
    const sig = `${key}|${last ? 1 : 0}|${raw?.done ? 1 : 0}|${raw?.votes ? JSON.stringify(raw.votes) : ""}|${i}`;
    return { key, sig, html: `<div class="mrow" data-k="${esc(key)}" data-sig="${esc(sig)}">${line}</div>` };
  };
  // 이미 그려진 줄은 건드리지 않고 바뀐 줄만 갈아끼우고 새 줄만 붙여요 (사진이 있는 대화에서 다시 그리느라 버벅이지 않게)
  const syncRows = (items) => {
    const cur = [...bubbles.querySelectorAll(":scope > .mrow")];
    let i = 0;
    for (; i < items.length && i < cur.length && cur[i].dataset.k === items[i].key; i++) {
      if (cur[i].dataset.sig !== items[i].sig) {
        const t = document.createElement("template");
        t.innerHTML = items[i].html;
        cur[i].replaceWith(t.content.firstElementChild);
      }
    }
    for (let k = cur.length - 1; k >= i; k--) cur[k].remove(); // 순서가 달라졌으면 뒤쪽만 지우고 다시 붙여요
    const read = bubbles.querySelector(":scope > .read");
    for (; i < items.length; i++) (read || bubbles).insertAdjacentHTML(read ? "beforebegin" : "beforeend", items[i].html);
  };
  const renderThread = (fresh = -1) => {
    const chat = openChat;
    const msgs = chat.msgs;
    const stampText = `오늘 ${chat.time.includes(":") ? chat.time : "오후 2:14"}`;
    let head = bubbles.querySelector(":scope > .stamp");
    if (!head) {
      bubbles.insertAdjacentHTML("afterbegin", `<div class="stamp"></div>`);
      head = bubbles.querySelector(":scope > .stamp");
    }
    if (head.textContent !== stampText) head.textContent = stampText;
    let more = bubbles.querySelector(":scope > .more");
    if (chat.more && !more) head.insertAdjacentHTML("afterend", '<button class="more" type="button">이전 메시지 더 보기</button>');
    else if (!chat.more && more) more.remove();
    syncRows(msgs.map((_, i) => rowOf(chat, i, fresh)));
    let read = bubbles.querySelector(":scope > .read");
    const mineLast = msgs.length && msgs[msgs.length - 1][0] === "me";
    if (mineLast && !read) bubbles.insertAdjacentHTML("beforeend", '<div class="read">전송됨</div>');
    else if (!mineLast && read) read.remove();
    if (fresh !== -2) bubbles.scrollTo({ top: bubbles.scrollHeight, behavior: fresh >= 0 && !reduceMotion.matches ? "smooth" : "auto" });
  };
  const applyWall = () => {
    const wall = wallMap[openChat.id] || "white";
    thread.dataset.wall = wall;
    window.__paintStatus?.();
    document.querySelectorAll("[data-pick]").forEach((o) => o.setAttribute("aria-pressed", String(o.dataset.pick === wall)));
  };
  let readMark = "";
  const senderNames = new Map(); // 단톡방에서 보낸 사람의 이름 (한 번 읽으면 기억해요)
  const open = (id) => {
    openChat = chats.find((c) => c.id === id);
    if (!openChat) return;
    const chat = openChat;
    chat.unread = false;
    document.getElementById("thread-name").textContent = chat.name;
    const tav = document.getElementById("thread-avatar");
    const tp = safeImg(chat.photo, 40000);
    tav.textContent = tp ? "" : initials(chat.name);
    tav.style.backgroundImage = tp ? `url(${tp})` : "";
    tav.classList.toggle("photo", Boolean(tp));
    document.getElementById("thread-badge").textContent = chats.filter((c) => c.unread).length;
    applyWall();
    unsubMsgs?.();
    chat.msgs = [];
    chat.older = [];
    chat.more = false;
    bubbles.innerHTML = "";
    renderThread(-2);
    // 최근 40개만 실시간으로 받아요. 더 옛날 것은 "이전 메시지 더 보기"로 불러요
    unsubMsgs = window.cloud.watchMessages(chat.id, async (list, hasMore) => {
      // 단톡방이면 보낸 사람 이름을 먼저 읽어요 (말풍선 위에 이름을 써요)
      if (chat.group) for (const u of new Set(list.map((m) => m.from))) if (!senderNames.has(u)) senderNames.set(u, await window.cloud.nameOf(u));
      const prevLen = chat.msgs.length - chat.older.length;
      const all = [...chat.older, ...list];
      const grew = list.length > prevLen && prevLen > 0;
      chat.msgs = all.map((m) => [m.from === window.cloud.uid ? "me" : "them", preview(m), chat.group ? senderNames.get(m.from) || "" : chat.name, m]);
      if (!chat.olderLoaded) chat.more = hasMore;
      if (openChat === chat) {
        renderThread(grew || prevLen === 0 ? (prevLen === 0 ? -1 : all.length - 1) : -2);
        const lastRaw = list[list.length - 1];
        if (lastRaw && lastRaw.from !== window.cloud.uid && readMark !== lastRaw.id) {
          readMark = lastRaw.id;
          window.cloud.markRead(chat.id);
        }
      }
    });
    document.getElementById("thread-invite").hidden = !chat.group;
    document.querySelector('#attach [data-a="token"]').hidden = Boolean(chat.group); // 단톡방에서는 토큰을 주고받지 않아요
    app.classList.add("in-thread");
    paintStatus();
    thread.classList.add("is-open");
    renderChats();
  };
  bubbles.addEventListener("click", async (e) => {
    if (!e.target.closest(".more") || !openChat) return;
    const chat = openChat;
    const oldest = chat.msgs[0]?.[3];
    if (!oldest) return;
    const h0 = bubbles.scrollHeight;
    try {
      const { list, more } = await window.cloud.loadOlder(chat.id, oldest.at);
      chat.older = [...list, ...chat.older];
      chat.olderLoaded = true;
      chat.more = more;
      chat.msgs = [...list.map((m) => [m.from === window.cloud.uid ? "me" : "them", preview(m), chat.name, m]), ...chat.msgs];
      bubbles.querySelectorAll(":scope > .mrow").forEach((r) => r.remove()); // 앞에 끼워 넣는 거라 줄을 다시 붙여요
      renderThread(-2);
      bubbles.scrollTop = bubbles.scrollHeight - h0;
    } catch (err) {
      toast("이전 메시지를 불러오지 못했어요");
    }
  });
  const close = () => {
    unsubMsgs?.();
    unsubMsgs = null;
    readMark = "";
    thread.classList.remove("is-open");
    app.classList.remove("in-thread");
    paintStatus();
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
    pv.dataset.wall = wallMap[chat.id] || "white";
    // 미리보기는 마지막 글을 먼저 보여주고, 최근 9개를 한 번만 읽어 와요 (꾹 눌렀을 때만 읽기가 생겨요)
    const draw = (msgs) => {
      const tmp = { name: chat.name, photo: chat.photo, msgs: msgs.map((m) => [m.from === window.cloud.uid ? "me" : "them", preview(m), chat.name, m]) };
      pv.innerHTML = `<div class="pv">${tmp.msgs.map((_, i) => rowOf(tmp, i, -1).html).join("")}</div>`;
    };
    draw(chat.last ? [{ id: "p0", type: "text", text: chat.last, from: chat.lastFrom }] : []);
    window.cloud.loadOlder(chat.id, Date.now() + 60000, 9).then(({ list }) => { if (ctxBack.classList.contains("mode-stack")) draw(list); }).catch(() => {});
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
    return [{ icon: "copy", label: "복사", run: () => navigator.clipboard?.writeText(String(openChat.msgs[i][1])).catch(() => {}) }];
  });

  /* 배경화면 */
  document.getElementById("thread-name-btn").addEventListener("click", () => sheetOpen(true, document.getElementById("wall-sheet")));
  document.querySelectorAll("[data-pick]").forEach((b) =>
    b.addEventListener("click", () => {
      if (WALLS.includes(b.dataset.pick)) wallMap[openChat.id] = b.dataset.pick;
      store.set("walls", JSON.stringify(wallMap));
      applyWall();
      saveSettings();
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
  let listMore = false;
  let listBusy = false;
  const rowHTML = (u, i) => `<li><button class="res-row" type="button" data-i="${i}">${avatarHTML(u.name || u.id, u.photo, "avatar contact")}<span class="res-main"><b>${esc(u.name || u.id)}</b><span>@${esc(u.id)}</span></span></button></li>`;
  // 사람 목록을 20명씩 이어서 불러와요 (끝까지 내리면 다음 20명)
  const loadPeople = async (reset) => {
    if (listBusy) return;
    listBusy = true;
    try {
      const r = await window.cloud.listUsers(reset);
      if (newQ.value.trim()) return;
      if (reset) people = [];
      const start = people.length;
      people = people.concat(r.list);
      listMore = r.more;
      newResults.insertAdjacentHTML(reset ? "afterbegin" : "beforeend", r.list.map((u, i) => rowHTML(u, start + i)).join(""));
      newEmpty.hidden = people.length > 0;
      newEmpty.textContent = "아직 다른 사람이 없어요";
    } catch (err) {
      newEmpty.hidden = false;
      newEmpty.textContent = "목록을 불러오지 못했어요. 잠시 뒤 다시 해 주세요";
    } finally {
      listBusy = false;
    }
  };
  // 단톡방: 여러 명 고르기 (만들기 또는 초대). multi가 있으면 목록을 누를 때 고르기만 해요
  let multi = null;
  const multiBtn = document.getElementById("new-multi");
  const goBtn = document.getElementById("new-go");
  const paintMulti = () => {
    const n = multi ? multi.sel.size : 0;
    goBtn.hidden = !multi;
    goBtn.disabled = n < 1;
    goBtn.textContent = multi?.mode === "invite" ? `초대하기 · ${n}명` : `단톡방 만들기 · ${n}명`;
    multiBtn.textContent = multi?.mode === "make" ? "취소" : "단톡방";
    document.getElementById("new-title").textContent = multi?.mode === "invite" ? "사람 초대" : multi ? "단톡방 만들기" : mode0();
    newResults.querySelectorAll(".res-row").forEach((r) => r.classList.toggle("picked", Boolean(multi && multi.sel.has(people[Number(r.dataset.i)]?.uid))));
  };
  const mode0 = () => (pickMode === "gift" ? "받는 사람" : "새 메시지");
  const openPicker = (mode) => {
    if (!live) {
      if (window.cloud?.user) return toast("아직 불러오는 중이에요. 잠시 뒤 다시 눌러 주세요");
      return showLogin(true);
    }
    pickMode = mode;
    multi = null;
    multiBtn.hidden = mode !== "chat";
    document.getElementById("new-title").textContent = mode === "gift" ? "받는 사람" : "새 메시지";
    newQ.value = "";
    newResults.innerHTML = "";
    people = [];
    newEmpty.hidden = false;
    newEmpty.textContent = "불러오는 중이에요";
    sheetOpen(true, newSheet);
    loadPeople(true);
  };
  newResults.parentElement.addEventListener("scroll", (e) => {
    const sc = e.target;
    if (listMore && !newQ.value.trim() && sc.scrollTop + sc.clientHeight > sc.scrollHeight - 160) loadPeople(false);
  }, { passive: true });
  document.getElementById("compose").addEventListener("click", () => openPicker("chat"));
  document.getElementById("new-cancel").addEventListener("click", () => sheetOpen(false));
  let searchTimer = 0;
  let searchSeq = 0;
  newQ.addEventListener("input", () => {
    clearTimeout(searchTimer);
    const q = newQ.value.trim();
    searchSeq++;
    if (!q) {
      newResults.innerHTML = "";
      people = [];
      newEmpty.hidden = false;
      newEmpty.textContent = "불러오는 중이에요";
      loadPeople(true);
      return;
    }
    searchTimer = setTimeout(async () => {
      const seq = ++searchSeq;
      try {
        const found = await window.cloud.searchUsers(q);
        if (seq !== searchSeq) return;
        people = found;
        newResults.innerHTML = found.map(rowHTML).join("");
        newEmpty.hidden = found.length > 0;
        newEmpty.textContent = "일치하는 사람이 없어요";
      } catch (err) {
        newEmpty.hidden = false;
        newEmpty.textContent = "찾지 못했어요. 잠시 뒤 다시 해 주세요";
      }
    }, 250);
  });
  const ensureChat = async (u) => {
    const cid = await window.cloud.openChat(u.uid);
    let c = chats.find((x) => x.id === cid);
    if (!c) {
      c = { id: cid, other: u.uid, name: u.name || u.id, photo: u.photo || "", live: true, last: "", lastFrom: "", time: "", lastAt: Date.now(), unread: false, msgs: [] };
      chats.unshift(c);
    }
    return c;
  };
  multiBtn.addEventListener("click", () => {
    multi = multi ? null : { mode: "make", sel: new Set() };
    paintMulti();
  });
  // 초대: 단톡방 안에서 사람을 골라 더해요 (이미 있는 사람은 고를 수 없어요)
  const openInvite = (chat) => {
    multi = { mode: "invite", sel: new Set(), chat };
    pickMode = "invite";
    multiBtn.hidden = true;
    newQ.value = "";
    newResults.innerHTML = "";
    people = [];
    newEmpty.hidden = false;
    newEmpty.textContent = "불러오는 중이에요";
    sheetOpen(true, newSheet);
    loadPeople(true);
    paintMulti();
  };
  document.getElementById("thread-invite").addEventListener("click", () => {
    if (openChat?.group) openInvite(openChat);
  });
  goBtn.addEventListener("click", async () => {
    if (!multi || multi.sel.size < 1) return;
    const uids = [...multi.sel];
    goBtn.disabled = true;
    try {
      if (multi.mode === "invite") {
        const members = await window.cloud.inviteMembers(multi.chat.id, uids);
        multi.chat.members = members;
        toast(`${uids.length}명을 초대했어요`);
        multi = null;
        sheetOpen(false);
        return;
      }
      const names = uids.map((u) => people.find((p) => p.uid === u)?.name || people.find((p) => p.uid === u)?.id || "");
      const joined = names.join(", ");
      const name = joined.length <= 20 ? joined : `${names[0]} 외 ${names.length - 1}명`.slice(0, 20);
      const cid = await window.cloud.createGroup(name, uids);
      chats.unshift({ id: cid, other: cid, group: true, members: [window.cloud.uid, ...uids], name, photo: "", live: true, last: "", lastFrom: "", time: "", lastAt: Date.now(), unread: false, msgs: [], older: [], more: false, olderLoaded: false });
      multi = null;
      sheetOpen(false);
      renderChats();
      open(cid);
    } catch (err) {
      toast("단톡방을 만들지 못했어요. 잠시 뒤 다시 해 주세요");
    } finally {
      paintMulti();
    }
  });
  newResults.addEventListener("click", async (e) => {
    const row = e.target.closest(".res-row");
    if (!row) return;
    const u = people[Number(row.dataset.i)];
    if (multi) {
      if (!u || (multi.mode === "invite" && (multi.chat.members || []).includes(u.uid))) return;
      if (multi.sel.has(u.uid)) multi.sel.delete(u.uid);
      else multi.sel.add(u.uid);
      paintMulti();
      return;
    }
    try {
      const c = await ensureChat(u);
      sheetOpen(false);
      renderChats();
      if (pickMode === "gift") openGift(c.id, c.other, c.name);
      else open(c.id);
    } catch (err) {
      newEmpty.hidden = false;
      newEmpty.textContent = "상대와 연결하지 못했어요. 잠시 뒤 다시 해 주세요";
    }
  });
  document.getElementById("thread-back").addEventListener("click", close);
  // 서버 규칙이 0.6초보다 빨리 연달아 보내는 걸 막아서, 보내기는 줄을 세워 0.7초 간격으로 보내요
  let sendChain = Promise.resolve();
  let lastSendAt = 0;
  const queueSend = (fn) => {
    const run = sendChain.then(async () => {
      const wait = 700 - (Date.now() - lastSendAt);
      if (wait > 0) await sleep(wait);
      try {
        return await fn();
      } finally {
        lastSendAt = Date.now();
      }
    });
    sendChain = run.catch(() => {});
    return run;
  };
  document.getElementById("composer").addEventListener("submit", (e) => {
    e.preventDefault();
    const text = input.value.trim().slice(0, 1000);
    if (!text || !openChat) return;
    const chat = openChat;
    input.value = "";
    input.dispatchEvent(new Event("input"));
    queueSend(() => window.cloud.send(chat.id, chat.other, { type: "text", text }, text)).catch(() => toast("보내지 못했어요"));
  });

  /* ---------- 돈 탭(잔액, 이자 상품, 거래내역, 그래프, 카드 순서)은 러스트(WASM)로 옮겼어요: web/crates/ui/src/money/ ---------- */
  // 지갑 숫자는 러스트가 window.__bank 에 비춰 둬요. 다른 화면은 이것으로 잔액을 바로 읽어요
  const wallet = () => window.__bank || { ready: false, rate: 0, bal: 0, t: 0 };
  const W = (n) => `${n.toLocaleString("ko-KR")} 토큰`;
  const walletOp = (fn) => window.Money.walletOp(fn); // 지갑 쓰기는 2.2초 간격으로 줄 세워요
  const claimNow = () => window.Money.claim(); // 쌓인 이자를 서버에 정산해요
  const logTx = (e) => window.Money.log(e);
  const renderMoney = () => window.Money.render();

  /* ---------- 랭킹: 토큰을 가장 많이 가진 순서 ---------- */
  const fmtTok = (n) => (n >= 1e8 ? `${(n / 1e8).toFixed(1).replace(/\.0$/, "")}억` : Number(n).toLocaleString("ko-KR"));
  const CROWN = '<svg class="rk-crown" viewBox="0 0 28 22" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 7l5.500 5L14 3l5.500 9L25 7l-2 11H5L3 7z"/><path d="M6 21h16"/></svg>';
  const rkPodium = document.getElementById("rk-podium");
  const rkList = document.getElementById("rk-list");
  const rkEmpty = document.getElementById("rk-empty");
  const rkMe = document.getElementById("rk-me");
  let rankBusy = false;
  let rankAt = 0;
  const photoOf = (uid) => window.cloud?.peek(uid)?.photo || "";
  const podCol = (u, n) =>
    u
      ? `<div class="rk-col g${n}">${CROWN}<span class="rk-n">${n}</span>${avatarHTML(u.name, photoOf(u.uid), "avatar contact")}<b class="rk-name">${esc(u.name)}</b><span class="rk-val">${fmtTok(u.bal)}</span></div>`
      : `<div class="rk-col g${n} empty"><span class="rk-n">${n}</span><b class="rk-name">—</b><span class="rk-val">&nbsp;</span></div>`;
  const paintRank = (list, myRank) => {
    const me = window.cloud?.uid;
    rkPodium.innerHTML = podCol(list[1], 2) + podCol(list[0], 1) + podCol(list[2], 3);
    rkList.innerHTML = list
      .slice(3)
      .map((u, i) => `<li class="rk-row${u.uid === me ? " me" : ""}"><span class="rk-rn">${i + 4}</span>${avatarHTML(u.name, photoOf(u.uid), "avatar contact")}<span class="rk-rname">${esc(u.name)}</span><span class="rk-rval">${fmtTok(u.bal)}</span></li>`)
      .join("");
    rkEmpty.hidden = list.length > 1;
    rkEmpty.textContent = "아직 순위에 오른 사람이 적어요";
    rkMe.hidden = !wallet().ready;
    rkMe.innerHTML = `<span class="rk-mn">${myRank}</span>${avatarHTML(store.get("profile-name", "나"), store.get("profile-photo", ""), "avatar contact")}<b>나</b><span class="rk-mval">${fmtTok(wallet().bal)} 토큰</span>`;
  };
  const loadRank = async (force) => {
    const cloud = window.cloud;
    if (!cloud?.user || !live || rankBusy) return;
    if (!force && Date.now() - rankAt < 20000) return; // 탭을 오갈 때마다 서버를 읽지 않아요
    rankBusy = true;
    try {
      if (wallet().ready) await cloud.syncRank(wallet().bal).catch(() => {}); // 내 잔액을 먼저 순위표에 올려요
      const list = (await cloud.loadRanks(force)).slice();
      await cloud.fetchProfiles(list.slice(0, 12).map((u) => u.uid)).catch(() => 0); // 위쪽 사람들 사진
      const at = list.findIndex((u) => u.uid === cloud.uid);
      let myRank = at >= 0 ? String(at + 1) : "50+";
      if (at < 0) {
        try {
          const above = await cloud.countAbove(wallet().bal);
          if (above < 50) myRank = String(above + 1);
        } catch (_) { /* 순위를 못 세면 50+로 보여줘요 */ }
      }
      paintRank(list, myRank);
      rankAt = Date.now();
    } catch (err) {
      rkEmpty.hidden = false;
      rkEmpty.textContent = "순위를 불러오지 못했어요. 잠시 뒤 다시 해 주세요";
    } finally {
      rankBusy = false;
    }
  };
  nav.addEventListener("tabreselect", (e) => { if (e.detail.tab.dataset.tab === "ranking") loadRank(true); });
  // 서버 데이터베이스가 꺼져 있으면(관리자 설정 문제) 화면 위에 이유를 알려줘요
  const showCloudDown = () => {
    const el = document.getElementById("cloud-down");
    el.hidden = false;
    renderMoney();
    renderChats();
  };
  window.addEventListener("cloud-down", showCloudDown);
  if (window.cloudDown) showCloudDown();
  document.getElementById("cloud-down").addEventListener("click", (e) => { e.currentTarget.hidden = true; });

  /* ---------- 관리 화면 (owner 전용 버튼으로 열어요, 탭 바에는 없어요) ---------- */
  const goScreen = (name) => {
    showScreen(name);
    if (name === "manage") window.Manage.refresh();
    if (name === "notices") window.Notices.refresh();
    topTitle.textContent = screens.get(name).dataset.title;
    updateChrome(screens.get(name));
    backdrop?.setActive(name);
    tabbar.requestRender();
  };
  document.getElementById("open-manage").addEventListener("click", () => goScreen("manage"));
  document.getElementById("manage-back").addEventListener("click", () => goScreen("account"));
  document.getElementById("open-notices").addEventListener("click", () => goScreen("notices"));
  document.getElementById("notices-back").addEventListener("click", () => goScreen("account"));
  nav.addEventListener("tabreselect", (e) => { if (e.detail.tab.dataset.tab === "account" && ["manage", "notices"].includes(app.dataset.screen)) goScreen("account"); });

  /* ---------- 상태: 어느 탭에서나 떠 있는 동그란 버튼. 누르면 돌면서 배고픔·목마름·에너지가 튀어나와요 ---------- */
  // 시간당 줄어드는 양 (firestore.rules의 쓰러짐 검사와 같아야 해요): 배고픔 24시간, 목마름 18시간, 에너지 30일에 100%가 다 닳아요. 에너지가 0이어도 쓰러지지 않아요
  const STATS = [
    { k: "h", label: "배고픔", rate: 100 / 24, d: '<path d="M7.500 4v5a2.500 2.500 0 0 0 5 0V4M10 4v16M17 20V4c-2.200 1.500-3 4-3 7h3"/>' },
    { k: "t", label: "목마름", rate: 100 / 18, d: '<path d="M12 3.500C9 8 6.500 10.800 6.500 14.500a5.500 5.500 0 0 0 11 0C17.500 10.800 15 8 12 3.500z"/>' },
    { k: "e", label: "에너지", rate: 100 / (30 * 24), d: '<path d="M13 3L6 13.500h5L10 21l7-10.500h-5z"/>' },
  ];
  const fab = document.getElementById("status-fab");
  // 스크롤하는 동안에는 상태 버튼을 살짝 숨겨서 글자 위로 겹치지 않게 해요. 멈추면 다시 보여요
  {
    let idleT = 0;
    const onScroll = () => {
      fab.classList.add("fab-away");
      clearTimeout(idleT);
      idleT = setTimeout(() => fab.classList.remove("fab-away"), 450);
    };
    document.querySelectorAll(".screen").forEach((sc) => sc.addEventListener("scroll", onScroll, { passive: true }));
    window.addEventListener("scroll", onScroll, { passive: true });
  }
  const stBubble = document.getElementById("status-bubble");
  const fabTiles = document.getElementById("fab-tiles");
  fabTiles.innerHTML = STATS.map((u) => `<span class="fab-tile" role="img" aria-label="${u.label}"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">${u.d}</svg><b data-k="${u.k}">--%</b></span>`).join("");
  const tiles = [...fabTiles.children];
  let fabOpen = false;
  let fabAnim = [];
  const SIZE = 46;
  const GAP = 70;
  // 버튼에서 위나 아래로 일렬로 세워요 (화면 가운데 쪽으로)
  const toggleStatus = (on) => {
    fabOpen = on;
    stBubble.classList.toggle("is-open", on);
    stBubble.setAttribute("aria-expanded", String(on));
    fabAnim.forEach((a) => a.cancel());
    fabAnim = [];
    const a = app.getBoundingClientRect();
    const r = stBubble.getBoundingClientRect();
    const th = r.top + SIZE / 2 < a.top + a.height * 0.5 ? 90 : -90; // 90도는 아래쪽
    tiles.forEach((t, i) => {
      const rad = 62 + i * GAP;
      const from = th - 260; // 돌면서 나와요
      const tf = (deg, d, sc) => `translate(-50%, -50%) rotate(${deg}deg) translateX(${d}px) rotate(${-deg}deg) scale(${sc})`;
      const frames = [{ transform: tf(from, 0, 0.2), opacity: 0 }, { transform: tf(th, rad, 1), opacity: 1 }];
      t.classList.remove("show");
      t.style.visibility = "visible";
      const anim = t.animate(on ? frames : frames.slice().reverse(), { duration: on ? 620 : 420, delay: i * 70, easing: "cubic-bezier(0.22, 1.2, 0.36, 1)", fill: "both" });
      fabAnim.push(anim);
      if (on) setTimeout(() => fabOpen && t.classList.add("show"), 520 + i * 70); // 퍼센트는 다 나온 뒤에 떠요
      else anim.onfinish = () => { if (!fabOpen) t.style.visibility = "hidden"; };
    });
  };
  tiles.forEach((t) => (t.style.visibility = "hidden"));
  // 끌어서 옮길 수 있고, 놓은 자리에 그대로 있어요. 위치는 이 기기에 기억해요
  {
    const place = (xf, yf) => {
      const W = app.clientWidth;
      const H = app.clientHeight;
      fab.style.left = `${clamp(xf * W - SIZE / 2, 2, W - SIZE - 2)}px`;
      fab.style.top = `${clamp(yf * H - SIZE / 2, 2, H - SIZE - 2)}px`;
    };
    let pos = { x: 0.9, y: 0.36 };
    try { const v = JSON.parse(store.get("status-pos", "null")); if (v && v.x >= 0 && v.x <= 1 && v.y >= 0 && v.y <= 1) pos = v; } catch (_) { /* 처음이에요 */ }
    place(pos.x, pos.y);
    window.addEventListener("resize", () => place(pos.x, pos.y));
    let drag = null;
    stBubble.addEventListener("pointerdown", (e) => {
      stBubble.setPointerCapture(e.pointerId);
      const r = stBubble.getBoundingClientRect();
      drag = { id: e.pointerId, dx: e.clientX - r.left, dy: e.clientY - r.top, x0: e.clientX, y0: e.clientY, moved: false };
    });
    stBubble.addEventListener("pointermove", (e) => {
      if (!drag || e.pointerId !== drag.id) return;
      if (!drag.moved && Math.hypot(e.clientX - drag.x0, e.clientY - drag.y0) < 6) return;
      drag.moved = true;
      const a = app.getBoundingClientRect();
      pos = { x: (e.clientX - a.left - drag.dx + SIZE / 2) / a.width, y: (e.clientY - a.top - drag.dy + SIZE / 2) / a.height };
      place(pos.x, pos.y);
    });
    stBubble.addEventListener("pointerup", (e) => {
      if (!drag || e.pointerId !== drag.id) return;
      const moved = drag.moved;
      drag = null;
      if (moved) store.set("status-pos", JSON.stringify(pos));
      else toggleStatus(!fabOpen);
    });
    stBubble.addEventListener("pointercancel", () => { drag = null; });
  }

  /* ---------- 상태가 닳는 규칙: 낮아지면 화면 가장자리가 붉어지고, 0%가 되면 쓰러져서 쉬어요 ---------- */
  const vignette = document.getElementById("vignette");
  const lockEl = document.getElementById("lock");
  let vit = null; // 서버에 기록된 상태 {h,t,e,at,deaths}. 값은 at부터 시간이 흐른 만큼 줄어든 것으로 계산해요
  let vitTimer = 0;
  let lockTimer = 0;
  let dying = false;
  const vitNow = () => {
    const hrs = Math.max(0, (Date.now() - vit.at) / 3600000);
    // 에너지가 0이 되기까지 걸리는 시간 (1%당 7.2시간). 그 뒤에는 배고픔·목마름이 두 배로 닳아요 (firestore.rules 와 같아요)
    const eff = hrs + Math.max(0, hrs - vit.e * 7.2);
    return {
      h: Math.max(0, Math.min(100, vit.h - eff * (100 / 24))),
      t: Math.max(0, Math.min(100, vit.t - eff * (100 / 18))),
      e: Math.max(0, Math.min(100, vit.e - hrs * (100 / (30 * 24)))),
    };
  };
  const stopLock = () => { clearInterval(lockTimer); lockEl.hidden = true; };
  const showLock = () => {
    lockEl.hidden = false;
    document.getElementById("lock-n").textContent = `${vit.deaths}번째 쓰러졌어요`;
    const paint = () => {
      const left = Math.max(0, vit.at - Date.now());
      if (left <= 0) { stopLock(); startVitals(); return; }
      const hh = Math.floor(left / 3600000);
      const mm = Math.floor((left % 3600000) / 60000);
      const ss = Math.floor((left % 60000) / 1000);
      document.getElementById("lock-t").textContent = `${hh > 0 ? `${hh}시간 ` : ""}${String(mm).padStart(2, "0")}분 ${String(ss).padStart(2, "0")}초 뒤에 일어나요`;
    };
    clearInterval(lockTimer);
    paint();
    lockTimer = setInterval(paint, 1000);
  };
  const shownVit = {};
  const paintVitals = async () => {
    if (!vit) return;
    if (Date.now() < vit.at) { vignette.className = "vignette"; return; } // 쉬는 중
    const v = vitNow();
    for (const u of STATS) {
      const n = Math.max(0, Math.round(v[u.k]));
      if (shownVit[u.k] !== n) {
        shownVit[u.k] = n;
        const el = fabTiles.querySelector(`[data-k="${u.k}"]`);
        if (el) el.textContent = `${n}%`;
      }
    }
    const low = Math.min(v.h, v.t, v.e);
    const dead = Math.min(v.h, v.t) <= 0; // 배고픔이나 목마름이 0이면 쓰러져요 (에너지는 안 봐요)
    vignette.className = `vignette${low <= 5 ? " v5" : low <= 10 ? " v10" : low <= 20 ? " v20" : ""}`;
    if (dead && !dying) {
      dying = true;
      try {
        vit = await window.cloud.die(vit.deaths + 1);
        vignette.className = "vignette";
        showLock();
      } catch (err) {
        console.warn("쓰러짐을 기록하지 못했어요:", err?.code || err);
      } finally {
        dying = false;
      }
    }
  };
  function startVitals() {
    clearInterval(vitTimer);
    fab.hidden = false;
    shownVit.h = shownVit.t = shownVit.e = null;
    paintVitals();
    vitTimer = setInterval(() => { if (!document.hidden) paintVitals(); }, 5000);
  }
  const loadVitalsFor = async (cloud) => {
    try {
      vit = await cloud.loadVitals();
      if (Date.now() < vit.at) showLock();
      else startVitals();
    } catch (err) {
      console.warn("상태를 불러오지 못했어요:", err?.code || err);
    }
  };
  window.__vitals = { get: () => vit, set: (v) => { vit = v; startVitals(); } }; // 시험용

  /* ---------- 사업: 창업, 대시보드, 팔 상품 ---------- */
  let companies = []; // 내 회사들
  let company; // 지금 고른 회사. undefined: 불러오는 중, null: 아직 회사가 없어요
  let pickedId = store.get("biz-pick", "");
  const bizNone = document.getElementById("biz-none");
  const bizDash = document.getElementById("biz-dash");
  const bizLoading = document.getElementById("biz-loading");
  const bizLogo = document.getElementById("biz-logo");
  const bizSwitch = document.getElementById("biz-switch");
  const KIND_LABEL = { food: "식품 회사", pc: "컴퓨터 회사", ai: "인공지능 회사", nuclear: "원자력 발전 회사" };
  const markHTML = (name, logo, cls) => avatarHTML(name, logo, cls);
  const renderBiz = () => {
    bizLoading.hidden = company !== undefined;
    bizNone.hidden = company !== null;
    bizDash.hidden = !company;
    bizLogo.hidden = !company;
    if (!company) return;
    bizLogo.innerHTML = markHTML(company.name, company.logo, "avatar contact");
    document.getElementById("biz-mark").innerHTML = markHTML(company.name, company.logo, "avatar contact");
    document.getElementById("biz-name").textContent = company.name;
    const d = new Date(company.at || Date.now());
    document.getElementById("biz-since").textContent = `${d.getFullYear()}년 ${d.getMonth() + 1}월 ${d.getDate()}일에 창업했어요 · ${KIND_LABEL[company.kind]}`;
    bizSwitch.hidden = companies.length < 2;
    bizSwitch.innerHTML = companies.map((c) => `<button type="button" class="gg wk-pill" aria-pressed="${c.id === company.id}" data-co="${esc(c.id)}">${esc(c.name)}</button>`).join("");
  };
  // 회사를 고르면 그 회사의 사업 화면을 그려요 (고른 회사는 기기에 기억해요)
  const commitCompany = (id) => {
    company = companies.find((c) => c.id === id) || companies[0] || null;
    pickedId = company ? company.id : "";
    store.set("biz-pick", pickedId);
    renderBiz();
    if (company) window.Business.refresh();
    if (screens.get("work").classList.contains("is-active")) window.Work.refresh(true);
  };
  // 회사를 바꿀 때: 지금 화면이 살짝 물러났다가 새 회사 화면이 오른쪽에서 들어와요
  let swapTimer = 0;
  const selectCompany = (id) => {
    const next = companies.find((c) => c.id === id);
    if (!company || !next || next.id === company.id || bizDash.hidden) return commitCompany(id);
    clearTimeout(swapTimer);
    bizDash.classList.remove("swap-in");
    bizDash.classList.add("swap-out");
    swapTimer = setTimeout(() => {
      commitCompany(id);
      bizDash.classList.remove("swap-out");
      bizDash.classList.add("swap-in");
      swapTimer = setTimeout(() => bizDash.classList.remove("swap-in"), 380);
    }, 180);
  };
  const loadCompany = async (cloud) => {
    try {
      companies = await cloud.listMyCompanies();
    } catch (err) {
      companies = [];
      console.warn("회사를 불러오지 못했어요:", err?.code || err);
    }
    selectCompany(pickedId);
  };
  bizSwitch.addEventListener("click", (e) => {
    const b = e.target.closest("[data-co]");
    if (b) selectCompany(b.dataset.co);
  });
  // 회사 프로필: 사업 탭 오른쪽 위의 회사 마크를 누르면 열려요. 업종은 여기서 골라서 "업종 바꾸기"를 눌러야 바뀌어요
  const coSheet = document.getElementById("company-sheet");
  let coKind = "food";
  const paintCoKinds = () => {
    document.querySelectorAll("#co-kinds [data-kind]").forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.kind === coKind)));
    document.getElementById("co-save").disabled = !company || coKind === company.kind;
  };
  document.getElementById("co-kinds").addEventListener("click", (e) => {
    const b = e.target.closest("[data-kind]");
    if (!b) return;
    coKind = b.dataset.kind;
    paintCoKinds();
  });
  bizLogo.addEventListener("click", () => {
    if (!company) return;
    coKind = company.kind;
    document.getElementById("co-mark").innerHTML = markHTML(company.name, company.logo, "avatar contact");
    document.getElementById("co-name").textContent = company.name;
    document.getElementById("co-since").textContent = `${KIND_LABEL[company.kind]}`;
    document.getElementById("co-err").textContent = "";
    paintCoKinds();
    sheetOpen(true, coSheet);
  });
  document.getElementById("co-save").addEventListener("click", async () => {
    if (!company || coKind === company.kind) return;
    const btn = document.getElementById("co-save");
    const err = document.getElementById("co-err");
    btn.classList.add("busy");
    try {
      await window.cloud.setCompanyKind(company.id, coKind);
      const next = { ...company, kind: coKind };
      companies = companies.map((c) => (c.id === company.id ? next : c));
      company = next;
      renderBiz();
      window.Work.refresh(true);
      sheetOpen(false);
      toast(`업종을 ${KIND_LABEL[coKind]}로 바꿨어요`);
    } catch (e) {
      err.textContent = "업종을 바꾸지 못했어요. 잠시 뒤 다시 해 주세요";
    } finally {
      btn.classList.remove("busy");
      paintCoKinds();
    }
  });
  // 창업
  // 사업 탭: 팔 상품 정하기 화면과 판매 목록 (app/biz.js)
  window.Business.init({
    cloud: () => window.cloud,
    walletOp: (fn) => walletOp(fn), // 지갑 쓰기는 2.2초 간격으로 줄 세워요
    settle: () => claimNow(), // 금고에 넣기 전에 쌓인 이자를 먼저 받아요
    vitals: { get: () => window.__vitals.get(), set: (v) => window.__vitals.set(v) }, // 창고에서 먹은 뒤 상태를 화면에 바로 반영해요
    company: () => company,
    safeImg: (v, max) => safeImg(v, max),
    toast: (m) => toast(m),
    ask: (o) => ask(o),
    patchCompany: (p) => {
      const next = { ...company, ...p };
      companies = companies.map((c) => (c.id === next.id ? next : c));
      company = next;
    },
  });
  // 돈 탭 (web/crates/ui/src/money/)
  window.Money.init({
    toast: (m) => toast(m),
    ask: (o) => ask(o),
    sheetOpen: (on, el) => sheetOpen(on, el),
  });
  // 일 탭: 회사 일을 하거나, 회사를 종류별로 보고 지원해요 (app/work.js)
  window.Work.init({
    cloud: () => window.cloud,
    company: () => company,
    mark: (name, logo, cls) => avatarHTML(name, logo, cls),
    toast: (m) => toast(m),
    ask: (o) => ask(o),
  });
  // 인벤토리: 산 음식과 꺼낸 음식을 골라 먹어요 (app/inv.js). 먹은 뒤 상태는 여기서 화면에 반영해요
  window.Inventory.init({
    cloud: () => window.cloud,
    toast: (m) => toast(m),
    pkv: (p, mini, co) => window.Business.pkv(p, mini, co),
    vitals: { get: () => window.__vitals.get(), set: (v) => window.__vitals.set(v) },
  });
  // 관리 (오너만) 와 공지: app/manage.js
  window.Manage.init({ cloud: () => window.cloud, toast: (m) => toast(m) });
  // 공지: 설정의 공지 화면, 그리고 아직 안 본 공지를 전체 화면으로 한 번 (app/notice.js)
  window.Notices.init({ cloud: () => window.cloud, toast: (m) => toast(m) });
  // 상점 탭: 재고가 있는 음식을 골라 사요 (app/shop.js). 산 돈은 파는 회사 금고로 들어가요
  window.Shop.init({
    cloud: () => window.cloud,
    walletOp: (fn) => walletOp(fn), // 지갑 쓰기는 2.2초 간격으로 줄 세워요
    pkv: (p, mini, co) => window.Business.pkv(p, mini, co),
    toast: (m) => toast(m),
    log: (e) => logTx(e),
  });
  const foundName = document.getElementById("found-name");
  const foundErr = document.getElementById("found-err");
  const foundGo = document.getElementById("found-go");
  let foundLogo = "";
  let founding = false;
  // 회사 종류: 식품 · 컴퓨터 · 인공지능 (고르면 가운데 흰 칸이 미끄러져 가요)
  let foundKind = "food";
  const paintFoundKind = () => {
    document.getElementById("found-kinds").querySelectorAll("button").forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.kind === foundKind)));
  };
  document.getElementById("found-kinds").addEventListener("click", (e) => {
    const b = e.target.closest("[data-kind]");
    if (!b) return;
    foundKind = b.dataset.kind;
    paintFoundKind();
  });
  const paintFoundLogo = () => {
    const img = document.getElementById("found-logo-img");
    const ok = safeImg(foundLogo, 40000);
    img.style.backgroundImage = ok ? `url(${ok})` : "";
    document.getElementById("found-logo").classList.toggle("has-img", Boolean(ok));
  };
  const openFounding = () => {
    if (!live) return window.cloud?.user ? toast("아직 불러오는 중이에요. 잠시 뒤 다시 눌러 주세요") : showLogin(true);
    if (!wallet().ready || wallet().bal < 10) return toast("토큰이 부족해요. 창업에는 10 토큰이 필요해요");
    foundName.value = "";
    foundKind = "food";
    paintFoundKind();
    foundLogo = "";
    foundErr.textContent = "";
    paintFoundLogo();
    document.getElementById("found-cost").textContent = `10 토큰이 들어요 · 내 토큰 ${W(wallet().bal)}`;
    sheetOpen(true, document.getElementById("found-sheet"));
  };
  document.getElementById("biz-start").addEventListener("click", openFounding);
  document.getElementById("biz-more").addEventListener("click", openFounding);
  document.getElementById("found-cancel").addEventListener("click", () => sheetOpen(false));
  const fileLogo = document.getElementById("file-logo");
  document.getElementById("found-logo").addEventListener("click", () => fileLogo.click());
  fileLogo.addEventListener("change", async () => {
    const f = fileLogo.files[0];
    fileLogo.value = "";
    if (!f) return;
    try {
      foundLogo = await readImage(f, 192, 36000, { q: 0.8, square: true });
      paintFoundLogo();
    } catch (err) {
      foundErr.textContent = "사진을 불러오지 못했어요";
    }
  });
  foundGo.addEventListener("click", async () => {
    if (founding) return;
    const name = foundName.value.trim().replace(/\s+/g, " ");
    if (!name) return (foundErr.textContent = "회사 이름을 적어 주세요");
    if (name.length > 20 || /[<>]/.test(name)) return (foundErr.textContent = "이름은 20자 이하로, < > 없이 적어 주세요");
    if (wallet().bal < 10) return (foundErr.textContent = "토큰이 부족해요");
    founding = true;
    foundGo.classList.add("busy");
    foundErr.textContent = "";
    try {
      const cid = await walletOp(() => window.cloud.foundCompany(name, foundLogo, foundKind));
      companies = [...companies, { id: cid, name, logo: foundLogo, kind: foundKind, owner: window.cloud.uid, open: 0, staff: 0, products: [], at: Date.now() }];
      logTx({ t: Date.now(), name: "창업", cat: "사업", icon: "store", amt: -10 });
      sheetOpen(false);
      selectCompany(cid);
      toast(`${name}을(를) 창업했어요`);
    } catch (err) {
      foundErr.textContent = err?.code === "permission-denied" ? "창업하지 못했어요. 이미 회사가 있거나 토큰이 부족해요" : "창업하지 못했어요. 잠시 뒤 다시 해 주세요";
    } finally {
      founding = false;
      foundGo.classList.remove("busy");
    }
  });

  /* ---------- 대화 더하기: 사진, 그리기, 투표, 토큰 선물 ---------- */
  const toastEl = document.getElementById("toast");
  let toastTimer = 0;
  function toast(t) {
    toastEl.textContent = t;
    toastEl.classList.add("on");
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => toastEl.classList.remove("on"), 1900);
  }
  const sendPayload = (chat, payload) => queueSend(() => window.cloud.send(chat.id, chat.other, payload, preview(payload)));

  // 사진을 줄여서 글자(데이터 주소)로 만들어요. cap(글자 수)을 넘지 않을 때까지 화질과 크기를 줄여요. 서버 규칙도 같은 한도로 막아요
  const readImage = (file, max, cap, { q = 0.7, square = false } = {}) =>
    new Promise((resolve, reject) => {
      if (!file || !/^image\//.test(file.type) || file.size > 25 * 1024 * 1024) return reject(new Error("사진이 아니에요"));
      const url = URL.createObjectURL(file);
      const img = new Image();
      img.onload = () => {
        URL.revokeObjectURL(url);
        const sw = square ? Math.min(img.width, img.height) : img.width;
        const sh = square ? sw : img.height;
        const sx = (img.width - sw) / 2;
        const sy = (img.height - sh) / 2;
        let k = Math.min(1, max / Math.max(sw, sh));
        for (let tries = 0; tries < 6; tries++) {
          const c = document.createElement("canvas");
          c.width = Math.max(1, Math.round(sw * k));
          c.height = Math.max(1, Math.round(sh * k));
          c.getContext("2d").drawImage(img, sx, sy, sw, sh, 0, 0, c.width, c.height);
          for (let s = q; s >= 0.34; s -= 0.08) {
            const out = c.toDataURL("image/jpeg", s);
            if (out.length <= cap) return resolve(out);
          }
          k *= 0.8;
        }
        reject(new Error("사진이 너무 커요"));
      };
      img.onerror = () => reject(new Error("사진을 열지 못했어요"));
      img.src = url;
    });

  /* + 메뉴 */
  const attachBack = document.getElementById("attach-back");
  document.getElementById("plus").addEventListener("click", () => {
    if (!openChat) return;
    attachBack.classList.add("is-open");
  });
  attachBack.addEventListener("pointerdown", (e) => { if (e.target === attachBack) attachBack.classList.remove("is-open"); });
  const fileCamera = document.getElementById("file-camera");
  const filePhoto = document.getElementById("file-photo");
  const sendPhoto = async (file) => {
    if (!file || !openChat) return;
    const chat = openChat;
    try {
      const img = await readImage(file, 720, 130000, { q: 0.66 });
      await sendPayload(chat, { type: "photo", img });
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
    else if (a === "token") openGift(openChat.id, openChat.other, openChat.name);
  });

  /* 사진 크게 보기, 토큰 받기·보내기, 투표 */
  const viewer = document.getElementById("viewer");
  bubbles.addEventListener("click", async (e) => {
    const chat = openChat;
    if (!chat) return;
    const media = e.target.closest("[data-view]");
    if (media) {
      const src = safeImg(chat.msgs[Number(media.dataset.view)]?.[3]?.img);
      if (!src) return;
      document.getElementById("viewer-img").src = src;
      viewer.hidden = false;
      requestAnimationFrame(() => viewer.classList.add("is-open"));
      return;
    }
    const act = e.target.closest("[data-act]");
    if (act) {
      const m = chat.msgs[Number(act.dataset.i)]?.[3];
      if (!m || m.done) return;
      const amount = Number(m.amount);
      if (act.dataset.act === "accept") {
        try {
          await walletOp(() => window.cloud.claimGift(chat.id, m.id, amount));
          logTx({ t: Date.now(), name: `${chat.name}님의 선물`, cat: "송금", icon: "gift", amt: amount });
          toast(`토큰 ${amount.toLocaleString("ko-KR")}을 받았어요`);
        } catch (err) {
          toast("받지 못했어요. 잠시 뒤 다시 해 주세요");
        }
      } else if (act.dataset.act === "pay") {
        if (!wallet().ready || wallet().bal < amount) return void (await ask({ title: "토큰이 부족해요", body: `내 토큰은 ${wallet().bal.toLocaleString("ko-KR")}이에요.`, ok: "확인" }));
        const yes = await ask({ title: `토큰 ${amount.toLocaleString("ko-KR")}을 보낼까요?`, body: `${chat.name}님의 요청이에요.`, ok: "보내기", cancel: "취소" });
        if (!yes) return;
        try {
          await walletOp(() => window.cloud.payRequest(chat.id, chat.other, m.id, amount));
          logTx({ t: Date.now(), name: `${chat.name}님에게 선물`, cat: "송금", icon: "gift", amt: -amount });
          toast("보냈어요");
        } catch (err) {
          toast("보내지 못했어요. 잠시 뒤 다시 해 주세요");
        }
      }
      return;
    }
    const vote = e.target.closest("[data-vote]");
    if (vote) {
      const m = chat.msgs[Number(vote.dataset.i)]?.[3];
      if (m) window.cloud.vote(chat.id, m.id, Number(vote.dataset.vote)).catch(() => toast("투표하지 못했어요"));
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
  let gift = { cid: "", to: "", name: "", amt: 100 };
  let shownAmt = null;
  // 숫자가 바뀌면 옛 숫자가 잔상을 남기며 위(늘 때)나 아래(줄 때)로 흩어지고, 새 숫자가 반대쪽에서 들어와요
  const rollAmt = (dir, oldText) => {
    if (reduceMotion.matches) return;
    const holder = gNum.parentElement;
    holder.querySelectorAll(".gnum-ghost").forEach((g, i, all) => { if (all.length > 6) g.remove(); });
    [0.5, 0.26].forEach((alpha, i) => {
      const g = document.createElement("span");
      g.className = "gnum-ghost";
      g.textContent = oldText;
      g.setAttribute("aria-hidden", "true");
      holder.appendChild(g);
      const a = g.animate(
        [{ opacity: alpha, transform: "translateY(0)", filter: "blur(0px)" }, { opacity: 0, transform: `translateY(${-dir * (22 + i * 12)}px)`, filter: `blur(${3 + i * 2}px)` }],
        { duration: 340 + i * 90, delay: i * 40, easing: "cubic-bezier(0.22, 1, 0.36, 1)", fill: "both" }
      );
      a.onfinish = () => g.remove();
    });
    gNum.animate(
      [{ opacity: 0.25, transform: `translateY(${dir * 16}px)`, filter: "blur(3px)" }, { opacity: 1, transform: "translateY(0)", filter: "blur(0px)" }],
      { duration: 300, easing: "cubic-bezier(0.22, 1, 0.36, 1)" }
    );
  };
  const showAmt = (animate = true) => {
    const old = gNum.textContent;
    gNum.textContent = gift.amt.toLocaleString("ko-KR");
    gNum.parentElement.style.fontSize = gift.amt >= 1e7 ? "46px" : gift.amt >= 1e6 ? "58px" : gift.amt >= 1e5 ? "68px" : "80px";
    document.getElementById("gift-bal").textContent = `내 토큰 ${W(wallet().ready ? wallet().bal : 0)}`;
    if (animate && shownAmt !== null && shownAmt !== gift.amt) rollAmt(gift.amt > shownAmt ? 1 : -1, old);
    shownAmt = gift.amt;
  };
  function openGift(cid, to, name) {
    gift = { cid, to, name, amt: gift.amt > 0 ? gift.amt : 100 };
    document.getElementById("gift-to").textContent = `${name}님에게`;
    keypad.hidden = true;
    fresh = true;
    document.getElementById("g-keytoggle").textContent = "키패드 보기";
    showAmt(false);
    sheetOpen(true, giftSheet);
  }
  // 작은 + - 는 10씩, 큰 + - 는 100씩 늘고 줄어요
  const step = (dir, size) => {
    gift.amt = Math.max(0, Math.min(99999999, gift.amt + dir * size));
    showAmt();
    navigator.vibrate?.(6);
  };
  for (const [id, dir, size] of [["g-minus", -1, 10], ["g-plus", 1, 10], ["g-minus100", -1, 100], ["g-plus100", 1, 100]]) {
    const btn = document.getElementById(id);
    let hold = 0;
    let rep = 0;
    const stop = () => { clearTimeout(hold); clearInterval(rep); };
    btn.addEventListener("pointerdown", () => {
      step(dir, size);
      hold = setTimeout(() => (rep = setInterval(() => step(dir, size), 110)), 380);
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
  let giftBusy = false;
  document.getElementById("g-send").addEventListener("click", async () => {
    if (giftBusy) return;
    if (gift.amt < 1) return toast("보낼 토큰을 정해 주세요");
    if (!wallet().ready || gift.amt > wallet().bal) return void (await ask({ title: "토큰이 부족해요", body: `내 토큰은 ${wallet().bal.toLocaleString("ko-KR")}이에요.`, ok: "확인" }));
    const { cid, to, name, amt } = gift;
    giftBusy = true;
    try {
      await queueSend(() => walletOp(() => window.cloud.sendGift(cid, to, amt)));
      logTx({ t: Date.now(), name: `${name}님에게 선물`, cat: "송금", icon: "gift", amt: -amt });
      sheetOpen(false);
      toast(`${name}님에게 토큰 ${amt.toLocaleString("ko-KR")}을 보냈어요`);
    } catch (err) {
      toast("보내지 못했어요. 잠시 뒤 다시 해 주세요");
    } finally {
      giftBusy = false;
    }
  });
  document.getElementById("g-request").addEventListener("click", async () => {
    if (giftBusy) return;
    if (gift.amt < 1) return toast("요청할 토큰을 정해 주세요");
    const { cid, to, name, amt } = gift;
    giftBusy = true;
    try {
      await queueSend(() => window.cloud.send(cid, to, { type: "request", amount: amt, done: false }, `토큰 ${amt.toLocaleString("ko-KR")} 요청`));
      sheetOpen(false);
      toast(`${name}님에게 토큰 ${amt.toLocaleString("ko-KR")}을 요청했어요`);
    } catch (err) {
      toast("요청하지 못했어요");
    } finally {
      giftBusy = false;
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
      .map((o, i) => `<div class="poll-edit"><button class="poll-photo${o.img ? " has" : ""}" data-p="${i}" type="button" aria-label="사진 고르기" ${o.img ? `style="background-image:url(${safeImg(o.img, 60000)})"` : ""}>${o.img ? "" : '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>'}</button><input type="text" data-t="${i}" maxlength="30" placeholder="선택지 ${i + 1}" value="${esc(o.t)}"></div>`)
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
      poll[pollPick].img = await readImage(f, 320, 45000, { q: 0.62 });
      drawPollOpts();
    } catch (err) { toast("사진을 열지 못했어요"); }
  });
  document.getElementById("poll-add").addEventListener("click", () => { if (poll.length < 4) { poll.push({ t: "", img: "" }); drawPollOpts(); } });
  document.getElementById("poll-cancel").addEventListener("click", () => sheetOpen(false));
  document.getElementById("poll-send").addEventListener("click", async () => {
    const q = document.getElementById("poll-q").value.trim().slice(0, 60);
    const opts = poll.filter((o) => o.t.trim() || o.img);
    const err = document.getElementById("poll-err");
    if (!q) return (err.textContent = "질문을 적어 주세요");
    if (opts.length < 2) return (err.textContent = "선택지를 두 개 이상 만들어 주세요");
    try {
      await sendPayload(openChat, { type: "poll", poll: { q, options: opts.map((o) => ({ t: o.t.trim().slice(0, 30), img: o.img })) }, votes: {} });
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
    const k = Math.min(1, 720 / dc.width);
    const out = document.createElement("canvas");
    out.width = Math.round(dc.width * k);
    out.height = Math.round(dc.height * k);
    const o = out.getContext("2d");
    o.fillStyle = "#fff";
    o.fillRect(0, 0, out.width, out.height);
    o.drawImage(dc, 0, 0, out.width, out.height);
    let img = out.toDataURL("image/png");
    for (let q = 0.8; img.length > 130000 && q > 0.3; q -= 0.1) img = out.toDataURL("image/jpeg", q);
    if (img.length > 130000) { toast("그림이 너무 커요. 조금 줄여서 다시 보내 주세요"); return; }
    closeDraw();
    const chat = openChat;
    try { await queueSend(() => window.cloud.send(chat.id, chat.other, { type: "draw", img }, "그림")); } catch (err) { toast("그림을 보내지 못했어요"); }
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
  const showSlide = (n) => {
    slides.forEach((s, i) => s.classList.toggle("on", i === n));
    window.__paintStatus?.();
  };
  // 상태바 색: 로그인 화면이 열리면 사진 쪽의 어두운 색으로, 닫히면 원래 밝은 색으로 돌려요 (홈 화면 앱에서 빈 띠가 덜 튀게)
  // 상태바 색은 지금 보이는 맨 위 색을 따라가요: 로그인(어두움) > 대화(흰색·민트) > 화면(라벤더 회색)
  let loginOn = false;
  // 로그인 사진 다섯 장의 위쪽 평균 색 (사진이 바뀌면 상태바 색도 따라가요)
  const LOGIN_COLOURS = ["#3e4250", "#2e3949", "#4e5047", "#997880", "#9b725c"];
  const paintStatus = () => {
    const meta = document.querySelector('meta[name="theme-color"][media="(prefers-color-scheme: light)"]');
    if (!meta) return;
    let colour = "#f2f2f7";
    if (loginOn) colour = LOGIN_COLOURS[slide] || "#2a2a2e";
    else if (app.classList.contains("in-thread")) colour = document.getElementById("thread")?.dataset.wall === "mint" ? "#eaf6ee" : "#ffffff";
    meta.setAttribute("content", colour);
    // 상태바 뒤쪽 띠는 페이지 바탕색이 보여요. 그래서 바탕색도 같은 색으로 맞춰요 (theme-color를 따르지 않는 기기에서도 보이게)
    document.documentElement.style.backgroundColor = colour;
    document.body.style.backgroundColor = colour;
  };
  window.__paintStatus = paintStatus;
  setTimeout(() => paintStatus(), 0); // 처음 켰을 때도 맞춰요 (그때는 초기화가 다 끝난 뒤예요)
  const showLogin = (on) => {
    loginOn = on;
    paintStatus();
    clearInterval(slideTimer);
    if (on) {
      showSlide(slide);
      slideTimer = setInterval(() => showSlide((slide = (slide + 1) % slides.length)), 5000);
    }
    clearTimeout(showLogin.hideT);
    loginEl.hidden = false;
    requestAnimationFrame(() => loginEl.classList.toggle("is-open", on));
    // 닫은 뒤에는 화면에서 완전히 빼서 뒤에서 아무것도 그리지 않게 해요
    if (!on) showLogin.hideT = setTimeout(() => { loginEl.hidden = true; slides.forEach((s) => s.classList.remove("on")); }, 700);
    if (on) {
      setMode(false, false);
      warned = false;
      realNameEl.value = "";
    }
  };
  document.querySelectorAll("#open-login, .js-open-login").forEach((b) => b.addEventListener("click", () => showLogin(true)));
  document.getElementById("login-close").addEventListener("click", () => showLogin(false));
  document.getElementById("login-switch").addEventListener("click", () => setMode(!signup));
  // 비밀번호를 계속 틀리면 잠깐 막아요 (5번 틀리면 30초, 그다음부터 두 배씩, 최대 10분)
  const lockState = () => { try { return JSON.parse(store.get("login-lock", "{}")); } catch (_) { return {}; } };
  let busy = false;
  const setBusy = (on) => {
    busy = on;
    submitBtn.classList.toggle("busy", on);
    submitBtn.setAttribute("aria-busy", String(on));
    if (!on) submitBtn.blur();
  };
  const run = async (fn, isLogin = false) => {
    errEl.textContent = "";
    const lock = lockState();
    if (isLogin && lock.until > Date.now()) {
      errEl.textContent = `시도가 너무 많아요. ${Math.ceil((lock.until - Date.now()) / 1000)}초 뒤에 다시 해 주세요`;
      return;
    }
    if (busy) return;
    setBusy(true);
    await cloudPromise; // 연결이 아직 끝나지 않았을 수 있어요
    if (!window.cloud) {
      errEl.textContent = `서버에 연결하지 못했어요: ${window.cloudError || "알 수 없는 오류"}`;
      setBusy(false);
      return;
    }
    try {
      // 서버가 답이 없어도 버튼이 영원히 돌지 않게 25초에서 끊어요
      let giveUp;
      const late = new Promise((_, rej) => { giveUp = setTimeout(() => rej(new Error("서버 응답이 너무 늦어요. 잠시 뒤 다시 해 주세요")), 25000); });
      try { await Promise.race([fn(window.cloud), late]); } finally { clearTimeout(giveUp); }
      store.set("login-lock", "{}");
      // 새로고침 없이 바로 들어가요: 가입하면 이미 로그인된 상태라 화면만 로그인한 계정으로 바꿔요
      started = false;
      renderAuth(window.cloud);
      boot(window.cloud); // 지갑·대화는 뒤에서 불러와요 (기다리지 않아요)
      showLogin(false);
      setBusy(false);
      pwEl.value = pw2El.value = "";
    } catch (err) {
      errEl.textContent = err.message;
      setBusy(false);
      if (isLogin) {
        const n = (lock.n || 0) + 1;
        store.set("login-lock", JSON.stringify({ n, until: n >= 5 ? Date.now() + Math.min(600, 30 * 2 ** (n - 5)) * 1000 : 0 }));
      }
    }
  };
  loginForm.addEventListener("submit", (e) => e.preventDefault());
  submitBtn.addEventListener("click", async () => {
    submitBtn.blur(); // 눌린 모양이 남지 않게 해요
    if (busy) return;
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
    if (signup && (pw.length < 6 || pw.length > 64)) {
      errEl.textContent = "비밀번호는 6자 이상으로 써 주세요";
      return;
    }
    if (signup && pw !== pw2El.value) {
      errEl.textContent = "비밀번호가 서로 달라요";
      return;
    }
    if (!signup) return run((c) => c.signIn(id, pw), true);
    const name = realNameEl.value.trim().replace(/\s+/g, " ");
    if (!/^[가-힣]{2,6}$/.test(name) && !/^[A-Za-z][A-Za-z ]{1,38}[A-Za-z]$/.test(name)) {
      errEl.textContent = "실명을 입력해 주세요 (한글 2~6자 또는 영문 이름)";
      return;
    }
    const sure = await ask({ title: `"${name}" 님이 실명이 맞나요?`, body: "실명이 아니면 이용이 제한될 수 있어요. 가입한 뒤에는 이름을 바꾸기 어려워요.", ok: "실명이에요", cancel: "다시 입력" });
    if (!sure) return realNameEl.focus();
    run((c) => c.signUp(id, pw, name, safeImg(store.get("profile-photo", ""), 40000)));
  });
  document.getElementById("logout").addEventListener("click", async () => {
    await window.cloud?.signOut();
    // 같이 쓰는 기기에 내 정보가 남지 않게 지워요
    try {
      for (const k of Object.keys(localStorage)) if (k.startsWith("pf:") || ["auth-hint", "bank-tx", "prefs", "walls", "profile-name", "profile-bio", "profile-photo", "money-order", "acct-ok", "joined", "login-lock"].includes(k)) localStorage.removeItem(k);
    } catch (_) { /* 지우지 못해도 로그아웃은 됐어요 */ }
    location.reload();
  });
  // 로그인한 적이 있으면 연결이 끝나기 전에도 로그인된 화면을 바로 보여줘요 (깜빡임 없이)
  const hint = () => { try { return JSON.parse(store.get("auth-hint", "null")); } catch (_) { return null; } };
  const renderAuth = (cloud) => {
    const h = cloud ? null : hint();
    const signedIn = cloud ? Boolean(cloud.user) : Boolean(h);
    const id = cloud ? cloud.user?.id || "" : h?.id || "";
    if (cloud) store.set("auth-hint", signedIn ? JSON.stringify({ id }) : "null");
    const acct = screens.get("account");
    acct.classList.toggle("guest", !signedIn);
    acct.classList.toggle("member", signedIn);
    app.classList.toggle("guest-account", !signedIn);
    // 돈 탭도 로그인해야 보여요 (지갑이 서버에 있어요)
    const mon = screens.get("money");
    mon.classList.toggle("guest", !signedIn);
    mon.classList.toggle("member", signedIn);
    screens.get("ranking").classList.toggle("guest", !signedIn);
    screens.get("business").classList.toggle("guest", !signedIn);
    screens.get("work").classList.toggle("guest", !signedIn);
    document.getElementById("auth-card").hidden = signedIn;
    document.getElementById("auth-row").hidden = !signedIn;
    document.getElementById("profile-id").textContent = signedIn ? "@" + id : "";
    document.getElementById("open-manage").hidden = !(signedIn && id.toLowerCase() === "owner"); // 관리 버튼은 아이디가 owner인 사람에게만 보여요 (서버 기능이 생기면 규칙에서도 막아요)
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
    const photo = safeImg(store.get("profile-photo", ""), 40000);
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
      const data = await readImage(f, 160, 36000, { q: 0.8, square: true });
      store.set("profile-photo", data);
      applyProfile();
      if (window.cloud?.user) await window.cloud.setPhoto(data);
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
      editName.disabled = true; // 이름은 가입할 때 정한 것이라 바꿀 수 없어요
      editBio.value = store.get("profile-bio", "");
      sheetOpen(true);
    })
  );
  document.getElementById("sheet-cancel").addEventListener("click", () => sheetOpen(false));
  dim.addEventListener("click", () => sheetOpen(false));
  document.getElementById("sheet-save").addEventListener("click", () => {
    store.set("profile-bio", editBio.value.trim() || "소개를 추가해 보세요");
    applyProfile();
    saveSettings();
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
    if (name === "money") renderMoney();
    if (name === "ranking") loadRank(false);
    if (name === "work") window.Work.refresh();
    if (name === "shop") window.Shop.refresh();
    if (name === "inventory") window.Inventory.refresh();
    if (name === "business") window.Business.refresh(); // 상품 재고와 금고를 새로 읽어요
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

  // 로그인한 계정이면 서버에서 지갑, 대화, 설정을 불러와요. 로그인하지 않았거나 연결이 안 되면 로그인 화면만 보여줘요
  let started = false;
  const boot = async (cloud) => {
    window.__booted = true;
    renderAuth(cloud);
    if (!cloud || !cloud.user || started) {
      renderChats();
      renderMoney();
      return;
    }
    started = true;
    live = true;
    if (cloud.user.name && store.get("profile-name", "내 계정") === "내 계정") {
      store.set("profile-name", cloud.user.name);
      applyProfile();
    }
    try { await cloud.ensureAccount(); } catch (err) { console.warn("계정 정보를 채우지 못했어요:", err?.code || err); toast("계정 정보를 만들지 못했어요. 새로고침해 주세요"); }
    window.Money.startWallet();
    window.Notices.check();
    loadCompany(cloud);
    loadVitalsFor(cloud);
    chats.length = 0;
    renderChats();
    cloud.watchChats((docs) => {
      const old = new Map(chats.map((c) => [c.id, c]));
      const me = cloud.uid;
      const missing = [];
      const next = docs
        .filter((d) => !prefs[d.id]?.hidden && d.last) // 아무도 글을 보내지 않은 빈 대화는 목록에 안 보여요
        .map((d) => {
          const group = d.group === true;
          const other = group ? d.id : d.members.find((u) => u !== me) || "";
          const p = group ? null : cloud.peek(other);
          if (!group && !p) missing.push(other);
          const o = old.get(d.id);
          const f = {
            id: d.id,
            other,
            group,
            members: d.members,
            live: true,
            name: group ? d.name || "단톡방" : p ? p.name || p.id : o?.name || "…",
            photo: group ? "" : p ? p.photo || "" : o?.photo || "",
            last: d.last,
            lastFrom: d.lastFrom,
            time: fmtTime(d.lastAt),
            lastAt: d.lastAt,
            unread: Boolean(d.lastFrom && d.lastFrom !== me && (d.reads[me] || 0) < d.lastAt) && openChat?.id !== d.id,
            pin: Boolean(prefs[d.id]?.pin),
            muted: Boolean(prefs[d.id]?.muted),
          };
          // 같은 대화는 같은 객체를 그대로 써요 (열려 있는 대화의 메시지 듣기가 이 객체를 붙잡고 있어요)
          return o ? Object.assign(o, f) : { ...f, msgs: [], older: [], more: false, olderLoaded: false };
        })
        .sort((a, b) => b.lastAt - a.lastAt);
      // 방금 만든 대화(아직 글이 없는 것)가 목록에서 사라지지 않게 유지해요
      for (const o of old.values()) if (o.live && !next.some((c) => c.id === o.id) && openChat?.id === o.id) next.push(o);
      chats.splice(0, chats.length, ...next);
      if (openChat) openChat = chats.find((c) => c.id === openChat.id) || openChat;
      renderChats();
      // 사람 정보가 없는 대화는 한 번에 모아서 읽어 와요 (30분 동안 기억해서 다시 읽지 않아요)
      if (missing.length) cloud.fetchProfiles(missing).then((n) => { if (n) refreshNames(); });
    });
    try {
      const data = await cloud.loadSettings();
      if (data?.profile) {
        store.set("profile-name", String(data.profile.name || "내 계정").slice(0, 30));
        store.set("profile-bio", String(data.profile.bio || "").slice(0, 120));
        applyProfile();
      }
      if (data?.walls) {
        wallMap = Object.fromEntries(Object.entries(data.walls).filter(([, w]) => WALLS.includes(w)));
        store.set("walls", JSON.stringify(wallMap));
      }
    } catch (err) {
      console.warn("저장해 둔 설정을 불러오지 못했어요:", err?.code || err);
    }
    // 내 프로필 사진이 서버에만 있으면 가져와요
    const me = cloud.peek(cloud.uid);
    if (me?.photo && !store.get("profile-photo", "")) { store.set("profile-photo", me.photo); applyProfile(); }
    else if (!me) cloud.fetchProfiles([cloud.uid]).then(() => { const p = cloud.peek(cloud.uid); if (p?.photo && !store.get("profile-photo", "")) { store.set("profile-photo", p.photo); applyProfile(); } });
  };
  // 캐시에 사람 정보가 들어오면 이름과 사진을 다시 채워요
  const refreshNames = () => {
    for (const c of chats) {
      const p = window.cloud.peek(c.other);
      if (p) { c.name = p.name || p.id; c.photo = p.photo || ""; }
    }
    renderChats();
  };
  // 연결은 느린 폰에서 몇 초 걸릴 수 있어요. 끝나면 바로 반영하고, 8초가 지나도 안 끝나면 일단 지금 화면으로 보여요
  let booted = false;
  cloudPromise.then((cloud) => {
    booted = true;
    boot(cloud);
  });
  setTimeout(() => {
    if (!booted) boot(null);
  }, 8000);
})();
