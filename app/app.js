(() => {
  "use strict";

  const root = document.documentElement;
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
      // 큰 제목이 없는 화면(메시지)은 작은 제목이 늘 보여요
      topbar.style.setProperty("--p", clamp(y / 16, 0, 1));
      topTitle.style.opacity = 1;
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
  const chats = [
    { id: "minji", name: "민지", last: "내일 점심 뭐 먹을까?", time: "오후 2:14", unread: true, pin: true, msgs: [["them", "오늘 뭐 해?"], ["me", "집에서 쉬고 있어"], ["them", "내일 점심 뭐 먹을까?"]], reply: "좋아! 그럼 그때 봐" },
    { id: "mom", name: "엄마", last: "밥은 먹었니?", time: "오전 11:02", unread: true, pin: true, msgs: [["them", "밥은 먹었니?"]], reply: "그래, 조심히 다녀" },
    { id: "junho", name: "준호", last: "ㅋㅋㅋ 그거 봤어?", time: "어제", unread: false, pin: true, msgs: [["them", "ㅋㅋㅋ 그거 봤어?"], ["me", "아직 못 봤어"]], reply: "꼭 봐봐" },
    { id: "team", name: "회사 동료들", group: true, last: "회의 시간 3시로 바뀌었어요", time: "어제", unread: false, msgs: [["them", "회의 시간 3시로 바뀌었어요", "수진"], ["them", "확인 부탁드려요", "도현"], ["me", "네 확인했어요"]], reply: "감사합니다", replyBy: "수진" },
    { id: "jiyun", name: "이지윤", last: "응", time: "화요일", unread: false, msgs: [["them", "응"]], reply: "ㅇㅋ" },
    { id: "taeho", name: "박태호", last: "?", time: "월요일", unread: false, msgs: [["them", "?"]], reply: "ㅋㅋ" },
    { id: "delivery", name: "택배 안내", last: "(광고) 배송이 시작되었어요. 오늘 중으로 도착할 예정이에요", time: "월요일", unread: false, msgs: [["them", "(광고) 배송이 시작되었어요. 오늘 중으로 도착할 예정이에요"]], reply: "문의는 고객센터로 부탁드려요" },
  ];
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
    emptyMsg.hidden = shown.length > 0 || pins.length > 0;
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
    if (!live) return open("minji");
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
      const cid = await window.cloud.openChat(u.uid, u.id);
      if (!chats.some((c) => c.id === cid)) chats.unshift({ id: cid, name: u.id, live: true, last: "", time: "", lastAt: Date.now(), unread: false, msgs: [] });
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


  /* ---------- 로그인 ---------- */
  const loginEl = document.getElementById("login");
  const loginForm = document.getElementById("login-form");
  const emailEl = document.getElementById("login-email");
  const pwEl = document.getElementById("login-pw");
  const errEl = document.getElementById("login-error");
  const submitBtn = document.getElementById("login-submit");
    let signup = false;
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
    if (on) setMode(false, false);
  };
  document.getElementById("open-login").addEventListener("click", () => showLogin(true));
  document.getElementById("login-close").addEventListener("click", () => showLogin(false));
  document.getElementById("login-switch").addEventListener("click", () => setMode(!signup));
  const run = async (fn) => {
    errEl.textContent = "";
    submitBtn.disabled = true;
    await window.cloudReady; // 연결이 아직 끝나지 않았을 수 있어요
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
  submitBtn.addEventListener("click", () => {
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
    run((c) => (signup ? c.signUp(id, pw) : c.signIn(id, pw)));
  });
  document.getElementById("logout").addEventListener("click", async () => {
    await window.cloud?.signOut();
    location.reload();
  });
  const renderAuth = (cloud) => {
    const signedIn = Boolean(cloud) && !cloud.user.anonymous;
    const acct = screens.get("account");
    acct.classList.toggle("guest", !signedIn);
    acct.classList.toggle("member", signedIn);
    app.classList.toggle("guest-account", !signedIn);
    document.getElementById("auth-card").hidden = signedIn;
    document.getElementById("auth-row").hidden = !signedIn;
    if (signedIn) document.getElementById("auth-email").textContent = cloud.user.id;
  };

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
  document.getElementById("share-profile").addEventListener("click", () => {
    if (navigator.share) navigator.share({ title: nameEl.textContent, url: location.href }).catch(() => {});
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
  window.cloudReady.then((cloud) => {
    booted = true;
    boot(cloud);
  });
  setTimeout(() => {
    if (!booted) boot(null);
  }, 8000);
})();
