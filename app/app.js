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
    { id: "minji", name: "민지", color: "#ff9500", last: "내일 점심 뭐 먹을까?", time: "오후 2:14", unread: true, msgs: [["them", "오늘 뭐 해?"], ["me", "집에서 쉬고 있어"], ["them", "내일 점심 뭐 먹을까?"]], reply: "좋아! 그럼 그때 봐" },
    { id: "mom", name: "엄마", color: "#ff375f", last: "밥은 먹었니?", time: "오전 11:02", unread: true, msgs: [["them", "밥은 먹었니?"]], reply: "그래, 조심히 다녀" },
    { id: "apple", name: "Apple 지원", color: "#0a84ff", last: "주문하신 제품이 출고 준비 중이에요", time: "어제", unread: false, msgs: [["them", "주문하신 제품이 출고 준비 중이에요"]], reply: "문의해 주셔서 감사해요" },
    { id: "junho", name: "준호", color: "#30d158", last: "ㅋㅋㅋ 그거 봤어?", time: "어제", unread: false, msgs: [["them", "ㅋㅋㅋ 그거 봤어?"], ["me", "아직 못 봤어"]], reply: "꼭 봐봐" },
    { id: "team", name: "회사 동료들", color: "#5e5ce6", last: "회의 시간 3시로 바뀌었어요", time: "월요일", unread: false, msgs: [["them", "회의 시간 3시로 바뀌었어요"]], reply: "확인했어요" },
  ];
  const list = document.getElementById("chat-list");
  const emptyMsg = document.getElementById("chat-empty");
  const esc = (t) => t.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
  const renderChats = (q = "") => {
    const shown = chats.filter((c) => c.name.includes(q) || c.last.includes(q));
    list.innerHTML = shown
      .map(
        (c) => `<li><button class="row chat-row" type="button" data-id="${c.id}"><span class="avatar" style="width:48px;height:48px;background:${c.color}">${esc(c.name[0])}</span><span class="row-main"><b>${esc(c.name)}</b><span>${esc(c.last)}</span></span><span class="chat-side"><span class="meta">${c.time}</span>${c.unread ? '<i class="dot"></i>' : ""}</span></button></li>`
      )
      .join("");
    emptyMsg.hidden = shown.length > 0;
  };
  renderChats();
  document.getElementById("chat-search").addEventListener("input", (e) => renderChats(e.target.value.trim()));

  const thread = document.getElementById("thread");
  const bubbles = document.getElementById("bubbles");
  const input = document.getElementById("composer-input");
  const sendBtn = document.getElementById("send");
  let openChat = null;
  const addBubble = (who, text) => {
    const b = document.createElement("div");
    b.className = "bubble" + (who === "me" ? " me" : "");
    b.textContent = text;
    bubbles.appendChild(b);
    bubbles.scrollTo({ top: bubbles.scrollHeight, behavior: reduceMotion.matches ? "auto" : "smooth" });
  };
  const open = (id) => {
    openChat = chats.find((c) => c.id === id);
    if (!openChat) return;
    openChat.unread = false;
    document.getElementById("thread-who").innerHTML = `<span class="avatar" style="background:${openChat.color}">${esc(openChat.name[0])}</span>${esc(openChat.name)}`;
    bubbles.innerHTML = "";
    openChat.msgs.forEach(([w, t]) => addBubble(w, t));
    app.classList.add("in-thread");
    thread.classList.add("is-open");
    renderChats(document.getElementById("chat-search").value.trim());
  };
  const close = () => {
    thread.classList.remove("is-open");
    app.classList.remove("in-thread");
    input.blur();
    tabbar.requestRender();
  };
  list.addEventListener("click", (e) => {
    const row = e.target.closest(".chat-row");
    if (row) open(row.dataset.id);
  });
  document.getElementById("compose").addEventListener("click", () => open("minji"));
  document.getElementById("thread-back").addEventListener("click", close);
  input.addEventListener("input", () => (sendBtn.disabled = !input.value.trim()));
  document.getElementById("composer").addEventListener("submit", (e) => {
    e.preventDefault();
    const text = input.value.trim();
    if (!text || !openChat) return;
    const chat = openChat;
    input.value = "";
    sendBtn.disabled = true;
    chat.msgs.push(["me", text]);
    chat.last = text;
    chat.time = "방금";
    addBubble("me", text);
    setTimeout(() => {
      chat.msgs.push(["them", chat.reply]);
      chat.last = chat.reply;
      if (openChat === chat) addBubble("them", chat.reply);
      else chat.unread = true;
      renderChats(document.getElementById("chat-search").value.trim());
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
  document.getElementById("edit-profile").addEventListener("click", () => {
    editName.value = nameEl.textContent;
    editBio.value = store.get("profile-bio", "");
    sheetOpen(true);
  });
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
})();
