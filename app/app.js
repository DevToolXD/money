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
  const renderChats = () => {
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
        (c) => `<li><div class="chat-row" role="button" tabindex="0" data-id="${c.id}">${c.unread ? '<i class="dot"></i>' : ""}<span class="avatar contact">${esc(initials(c.name))}</span><div class="chat-main"><div class="chat-top"><b>${esc(c.name)}</b><span class="meta">${c.time}${chev}</span></div><p class="chat-prev">${esc(c.last)}</p></div><button class="del" type="button" data-del="${c.id}">삭제</button></div></li>`
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
  let openChat = null;
  const addLine = (who, text, by, last = true) => {
    const line = document.createElement("div");
    line.className = "line" + (who === "me" ? " me" : "");
    const b = `<div class="bubble${who === "me" ? " me" : ""}${last ? " last" : ""}">${esc(text)}</div>`;
    line.innerHTML = who === "me" ? b : `${last ? `<span class="avatar contact">${esc(initials(by || openChat.name))}</span>` : '<span style="width:38px"></span>'}${b}`;
    if (who !== "me" && openChat.group && last) {
      const s = document.createElement("div");
      s.className = "sender";
      s.textContent = by;
      bubbles.appendChild(s);
    }
    bubbles.appendChild(line);
    bubbles.scrollTo({ top: bubbles.scrollHeight, behavior: reduceMotion.matches ? "auto" : "smooth" });
  };
  const open = (id) => {
    openChat = chats.find((c) => c.id === id);
    if (!openChat) return;
    openChat.unread = false;
    document.getElementById("thread-name").textContent = openChat.name;
    const av = document.getElementById("thread-avatar");
    av.textContent = initials(openChat.name);
    document.getElementById("thread-badge").textContent = chats.filter((c) => c.unread).length;
    bubbles.innerHTML = `<div class="stamp">오늘 ${openChat.time.includes(":") ? openChat.time : "오후 2:14"}</div>`;
    openChat.msgs.forEach(([w, t, by], i, a) => addLine(w, t, by, !a[i + 1] || a[i + 1][0] !== w || a[i + 1][2] !== by));
    if (openChat.msgs[openChat.msgs.length - 1][0] === "me") bubbles.insertAdjacentHTML("beforeend", '<div class="read">전송됨</div>');
    app.classList.add("in-thread");
    thread.classList.add("is-open");
    renderChats();
  };
  const close = () => {
    thread.classList.remove("is-open");
    app.classList.remove("in-thread");
    input.blur();
    tabbar.requestRender();
  };
  const sendIcon = document.getElementById("send-icon");
  const WAVE = "M5 10v4M9 6v12M13 3v18M17 8v8M21 11v2";
  const ARROW = "M12 19V5M5 12l7-7 7 7";
  input.addEventListener("input", () => {
    const ready = Boolean(input.value.trim());
    sendBtn.classList.toggle("ready", ready);
    sendIcon.firstElementChild.setAttribute("d", ready ? ARROW : WAVE);
  });
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
  document.getElementById("compose").addEventListener("click", () => open("minji"));
  document.getElementById("thread-back").addEventListener("click", close);
  document.getElementById("composer").addEventListener("submit", (e) => {
    e.preventDefault();
    const text = input.value.trim();
    if (!text || !openChat) return;
    const chat = openChat;
    input.value = "";
    input.dispatchEvent(new Event("input"));
    chat.msgs.push(["me", text]);
    chat.last = text;
    chat.time = "방금";
    bubbles.querySelector(".read")?.remove();
    addLine("me", text);
    bubbles.insertAdjacentHTML("beforeend", '<div class="read">전송됨</div>');
    setTimeout(() => {
      chat.msgs.push(["them", chat.reply, chat.replyBy]);
      chat.last = chat.reply;
      if (openChat === chat) {
        bubbles.querySelector(".read")?.remove();
        addLine("them", chat.reply, chat.replyBy);
      } else chat.unread = true;
      renderChats();
    }, 900);
  });

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
  const sheetOpen = (on) => {
    sheet.classList.toggle("is-open", on);
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
})();
