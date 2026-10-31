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
      .map((c) => `<button class="pin${c.unread ? " is-unread" : ""}" type="button" data-id="${c.id}">${c.unread ? '<i class="dot"></i>' : ""}<span class="avatar contact">${esc(initials(c.name))}</span>${esc(c.name)}</button>`)
      .join("");
    const shown = chats.filter((c) => !pins.includes(c) && match(c));
    list.innerHTML = shown
      .map(
        (c) => `<li><div class="chat-row" role="button" tabindex="0" data-id="${c.id}">${c.unread ? '<i class="dot"></i>' : ""}<span class="avatar contact">${esc(initials(c.name))}</span><div class="chat-main"><div class="chat-top"><b>${esc(c.name)}</b><span class="meta">${c.muted ? '<svg class="mute-ico" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M6 16V11a6 6 0 0 1 9-5.2M18 11v5l2 2H5M4 4l16 16"/></svg>' : ""}${c.time}${chev}</span></div><p class="chat-prev">${esc(c.last)}</p></div><button class="del" type="button" data-del="${c.id}">삭제</button></div></li>`
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
  const threadHTML = (chat, fresh = -1) => {
    const msgs = chat.msgs;
    let html = `<div class="stamp">오늘 ${chat.time.includes(":") ? chat.time : "오후 2:14"}</div>`;
    msgs.forEach(([who, text, by], i) => {
      const next = msgs[i + 1];
      const last = !next || next[0] !== who || next[2] !== by;
      const b = `<div class="bubble${who === "me" ? " me" : ""}${last ? " last" : ""}${i === fresh ? " fresh" : ""}" data-i="${i}">${esc(text)}</div>`;
      if (who === "me") html += `<div class="line me">${b}</div>`;
      else {
        if (chat.group && last) html += `<div class="sender">${esc(by || "")}</div>`;
        html += `<div class="line">${last ? `<span class="avatar contact">${esc(initials(by || chat.name))}</span>` : '<span style="width:38px"></span>'}${b}</div>`;
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
    document.getElementById("thread-avatar").textContent = initials(openChat.name);
    document.getElementById("thread-badge").textContent = chats.filter((c) => c.unread).length;
    applyWall();
    if (openChat.live) {
      const chat = openChat;
      unsubMsgs?.();
      chat.msgs = [];
      renderThread();
      unsubMsgs = window.cloud.watchMessages(chat.id, (list) => {
        const grew = list.length > chat.msgs.length;
        chat.msgs = list.map((m) => [m.from === window.cloud.uid ? "me" : "them", m.text, chat.name]);
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
  const newId = document.getElementById("new-id");
  const newErr = document.getElementById("new-error");
  document.getElementById("compose").addEventListener("click", () => {
    if (!live) return showLogin(true);
    newId.value = "";
    newErr.textContent = "";
    sheetOpen(true, newSheet);
  });
  document.getElementById("new-cancel").addEventListener("click", () => sheetOpen(false));
  document.getElementById("new-go").addEventListener("click", async () => {
    const id = newId.value.trim();
    if (!id) return;
    newErr.textContent = "";
    try {
      const u = await window.cloud.findUser(id);
      if (!u) return (newErr.textContent = "그런 아이디가 없어요");
      if (u.uid === window.cloud.uid) return (newErr.textContent = "내 아이디예요");
      const cid = await window.cloud.openChat(u.uid, u.name || u.id);
      if (!chats.some((c) => c.id === cid)) chats.unshift({ id: cid, name: u.name || u.id, live: true, last: "", time: "", lastAt: Date.now(), unread: false, msgs: [] });
      sheetOpen(false);
      renderChats();
      open(cid);
    } catch (err) {
      newErr.textContent = "상대를 찾지 못했어요. 잠시 뒤 다시 해 주세요";
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
      window.cloud.send(chat.id, text).catch((err) => console.warn("보내지 못했어요:", err));
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
  const recent = book.slice().sort((a, b) => b.t - a.t);
  const monthSpent = daily.slice(DAYS - today0.getDate()).reduce((s, v) => s + v, 0);
  const BUDGET = 300000;
  const BALANCE = 1250000;
  const txRow = (e) => `<li class="t-row tx"><span class="t-ico sm">${ico(e.icon)}</span><span class="t-main"><b class="ts-name">${esc(e.name)}</b><span class="t-label">${hm(e.t)} · ${esc(e.cat)}</span></span><b class="t-amt${e.amt > 0 ? " plus" : ""}">${signed(e.amt)}</b></li>`;
  const groups = (list) => {
    let out = "";
    let last = "";
    for (const e of list) {
      const g = dayName(e.t);
      if (g !== last) out += `${last ? "</ul>" : ""}<p class="t-date">${g}</p><ul class="t-rows">`;
      last = g;
      out += txRow(e);
    }
    return out + (last ? "</ul>" : "");
  };
  const chev2 = '<svg class="tchev" viewBox="0 0 8 14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M1 1l6 6-6 6"/></svg>';
  document.getElementById("money-root").innerHTML = `
    <div class="t-card"><div class="t-pad"><h2 class="t-title plain">내 토큰</h2>
      <div class="t-bal"><b>${BALANCE.toLocaleString("ko-KR")} 토큰</b></div></div>
      <div class="t-line"><span class="t-ico cardico">${ico("card")}</span><span class="ts-name">내 카드</span><span class="t-right">이번 달 ${monthSpent.toLocaleString("ko-KR")} 토큰</span></div></div>
    <button class="t-card t-tap" id="open-chart" type="button"><div class="t-pad"><h2 class="t-title">이번 달 쓴 돈${chev2}</h2>
      <b class="t-big">${W(monthSpent)}</b>
      <div class="t-meter"><span style="width:${Math.min(100, Math.round((monthSpent / BUDGET) * 100))}%"></span></div>
      <p class="t-note">예산 ${BUDGET.toLocaleString("ko-KR")} 토큰 중 ${Math.round((monthSpent / BUDGET) * 100)}%를 썼어요</p></div></button>
    <button class="t-card t-tap" id="open-history" type="button"><div class="t-pad"><h2 class="t-title">사용내역${chev2}</h2>
      ${groups(recent.slice(0, 5))}</div></button>`;

  const histSheet = document.getElementById("history-sheet");
  const chartSheet = document.getElementById("chart-sheet");
  document.getElementById("history-list").innerHTML = groups(recent.slice(0, 80));
  document.getElementById("open-history").addEventListener("click", () => sheetOpen(true, histSheet));
  document.querySelectorAll("[data-close]").forEach((b) => b.addEventListener("click", () => sheetOpen(false)));

  /* 쓴 돈 꺾은선 그래프: 년 · 월 · 일 */
  const series = (r) => {
    const out = [];
    if (r === "d") for (let i = 29; i >= 0; i--) { const d = new Date(today0.getFullYear(), today0.getMonth(), today0.getDate() - i); out.push({ l: `${d.getMonth() + 1}/${d.getDate()}`, v: daily[DAYS - 1 - i] }); }
    else if (r === "m") {
      for (let k = 11; k >= 0; k--) {
        const first = new Date(today0.getFullYear(), today0.getMonth() - k, 1);
        const next = new Date(first.getFullYear(), first.getMonth() + 1, 1);
        let v = 0;
        for (let i = 0; i < DAYS; i++) { const d = new Date(today0.getFullYear(), today0.getMonth(), today0.getDate() - (DAYS - 1 - i)); if (d >= first && d < next) v += daily[i]; }
        out.push({ l: `${first.getMonth() + 1}월`, v });
      }
    } else {
      for (let k = 2; k >= 0; k--) {
        const y = today0.getFullYear() - k;
        let v = 0;
        for (let i = 0; i < DAYS; i++) { const d = new Date(today0.getFullYear(), today0.getMonth(), today0.getDate() - (DAYS - 1 - i)); if (d.getFullYear() === y) v += daily[i]; }
        out.push({ l: `${y}년`, v });
      }
    }
    return out;
  };
  const chart = document.getElementById("chart");
  const tip = document.getElementById("chart-tip");
  const LABEL = { d: "최근 30일 쓴 합계", m: "최근 12개월 쓴 합계", y: "그동안 쓴 합계" };
  let range = "m";
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
  document.getElementById("open-chart").addEventListener("click", () => { sheetOpen(true, chartSheet); drawChart(range); });

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
    run((c) => c.signUp(id, pw, name));
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
    avatarEl.className = custom ? "avatar" : "avatar-default";
    avatarEl.style.width = avatarEl.style.height = "";
    avatarEl.textContent = custom ? name[0] : "";
  };
  applyProfile();
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
        if (data.profile) {
          store.set("profile-name", data.profile.name);
          store.set("profile-bio", data.profile.bio);
          applyProfile();
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
