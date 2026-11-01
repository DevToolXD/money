/* 공지 (vote 리포의 공지 방식을 가져왔어요, 화면만 우리 것이에요):
   - 설정의 공지 화면에 올라온 공지가 모두 있어요 (누르면 펼쳐져요)
   - 로그인하면 아직 안 본 공지를 한 개씩 전체 화면으로 한 번 보여줘요. 투표가 있으면 골라야 닫을 수 있어요
   서버 쪽은 app/firebase.src.js 의 unseenNotice · markNoticeSeen · voteNotice 가 맡아요. */
(() => {
  "use strict";

  const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
  const dateOf = (ms) => (ms ? new Date(ms).toLocaleDateString("ko-KR", { year: "numeric", month: "long", day: "numeric" }) : "");

  let ctx = null;
  let listRoot = null;
  let list = [];
  let openId = null;
  let showing = false;

  const itemHTML = (n) => {
    const on = openId === n.id;
    return `<article class="card notice-item${on ? " is-open" : ""}">
      <button type="button" class="notice-head" data-id="${esc(n.id)}" aria-expanded="${on ? "true" : "false"}">
        <span class="notice-head-text"><b>${esc(n.title)}</b><span>${esc(dateOf(n.at))}</span></span>
        ${n.options ? '<span class="notice-pill">투표</span>' : ""}
        <svg class="notice-chev" viewBox="0 0 8 14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M1 1l6 6-6 6"/></svg>
      </button>
      ${on ? `<div class="notice-body"><p>${esc(n.body)}</p>${n.options ? `<ol class="notice-opts">${n.options.map((o) => `<li>${esc(o)}</li>`).join("")}</ol>` : ""}</div>` : ""}
    </article>`;
  };

  const paintList = () => {
    listRoot.innerHTML = list.length
      ? `<div class="notice-list">${list.map(itemHTML).join("")}</div>`
      : '<div class="card notice-empty"><p class="card-body">아직 올라온 공지가 없어요</p></div>';
  };

  // 공지 화면: 올라온 공지를 모두 보여줘요 (새로 열 때 한 번 읽어요)
  const refresh = async () => {
    if (!listRoot) return;
    if (!ctx?.cloud()?.user) {
      listRoot.innerHTML = '<div class="card notice-empty"><p class="card-body">로그인하면 공지를 볼 수 있어요</p></div>';
      return;
    }
    listRoot.innerHTML = '<div class="card skeletons" style="--rows: 3" aria-hidden="true"></div>';
    try {
      const ids = await ctx.cloud().noticeIds();
      list = await ctx.cloud().noticeList(ids);
      paintList();
    } catch (err) {
      console.warn("공지를 불러오지 못했어요:", err?.code || err);
      listRoot.innerHTML = '<div class="card notice-empty"><p class="card-body">공지를 불러오지 못했어요. 잠시 뒤 다시 열어 주세요</p></div>';
    }
  };

  // 한 번만 보여주는 전체 화면. 투표가 있으면 하나를 골라야 "투표하고 닫기"가 켜져요
  const fullScreen = (n, done) => {
    const el = document.createElement("div");
    el.className = "notice-full";
    el.setAttribute("role", "dialog");
    el.setAttribute("aria-modal", "true");
    el.setAttribute("aria-labelledby", "notice-full-title");
    const poll = n.options && n.options.length >= 2 ? n.options : null;
    el.innerHTML = `<div class="notice-full-card">
        <span class="notice-pill">공지</span>
        <h2 id="notice-full-title" class="notice-full-title">${esc(n.title)}</h2>
        <p class="notice-full-date">${esc(dateOf(n.at))}</p>
        <p class="notice-full-text">${esc(n.body)}</p>
        ${poll ? `<div class="notice-poll" role="radiogroup" aria-label="투표">
            <span class="notice-poll-title">투표 · 하나를 골라 주세요</span>
            ${poll.map((o, i) => `<button type="button" class="notice-opt" role="radio" aria-checked="false" data-i="${i}">${esc(o)}</button>`).join("")}
          </div>` : ""}
        <div class="notice-full-foot">
          <button type="button" class="btn primary notice-ok"${poll ? " disabled" : ""}>${poll ? "투표하고 닫기" : "확인했어요"}</button>
          ${poll ? '<button type="button" class="notice-skip">투표 안 하고 닫기</button>' : ""}
          <p class="notice-hint">지난 공지는 설정의 공지에서 다시 볼 수 있어요</p>
        </div>
      </div>`;
    let pick = null;
    const close = async (vote) => {
      el.querySelectorAll("button").forEach((b) => (b.disabled = true));
      try {
        if (vote && pick !== null) await ctx.cloud().voteNotice(n.id, pick).catch(() => {});
        await ctx.cloud().markNoticeSeen(n.id).catch(() => {});
      } finally {
        el.remove();
        done();
      }
    };
    el.addEventListener("click", (e) => {
      const opt = e.target.closest(".notice-opt");
      if (opt) {
        pick = Number(opt.dataset.i);
        el.querySelectorAll(".notice-opt").forEach((b) => {
          const on = b === opt;
          b.classList.toggle("is-on", on);
          b.setAttribute("aria-checked", on ? "true" : "false");
        });
        el.querySelector(".notice-ok").disabled = false;
        return;
      }
      if (e.target.closest(".notice-ok")) close(true);
      else if (e.target.closest(".notice-skip")) close(false);
    });
    return el;
  };

  // 아직 안 본 공지를 하나씩 보여줘요 (끝나면 다음 것을 찾아요)
  const check = async () => {
    if (showing || !ctx?.cloud()?.user) return;
    showing = true;
    try {
      const n = await ctx.cloud().unseenNotice();
      if (n) {
        document.body.appendChild(fullScreen(n, () => {
          showing = false;
          check();
        }));
        return;
      }
    } catch (err) {
      console.warn("공지를 확인하지 못했어요:", err?.code || err);
    }
    showing = false;
  };

  const init = (c) => {
    ctx = c;
    listRoot = document.getElementById("notices-root");
    if (!listRoot) return;
    listRoot.addEventListener("click", (e) => {
      const head = e.target.closest(".notice-head");
      if (!head) return;
      openId = openId === head.dataset.id ? null : head.dataset.id;
      paintList();
    });
  };

  window.Notices = { init, refresh, check };
})();
