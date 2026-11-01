/* 관리 (오너만): 토큰 지급, 공지, 계정 데이터 지우기.
   행동마다 인증토큰 3개를 앱이 만들어 서버에 올리고, 같은 3개를 행동과 함께 보내요. 서버(규칙)가 맞는지, 5분 안인지 확인해요.
   공지는 메시지 탭 맨 위에도 보여요. */
(() => {
  "use strict";

  const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
  const when = (ms) => (ms ? new Date(ms).toLocaleString("ko-KR", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit" }) : "");

  let ctx = null;
  let root = null;
  let noticeRoot = null;
  let busy = false;

  const msgOf = (err) => {
    if (err?.code === "nouser") return "그런 아이디가 없어요";
    if (err?.code === "permission-denied") return "확인이 맞지 않거나 시간이 지났어요. 다시 해 주세요";
    return err?.message || "처리하지 못했어요. 잠시 뒤 다시 해 주세요";
  };

  const noticesHTML = (list) =>
    list.length
      ? list.map((n) => `<div class="mg-notice"><b>${esc(n.text)}</b><span>${esc(when(n.at))}</span></div>`).join("")
      : '<p class="card-body">아직 공지가 없어요</p>';

  const paint = (notices) => {
    root.innerHTML = `
      <h2 class="section-title">토큰 지급</h2>
      <div class="card mg-card">
        <label class="lab-field"><span>아이디</span><input id="mg-grant-id" class="lab-input" type="text" autocomplete="off" autocapitalize="none" maxlength="20" placeholder="받을 사람 아이디"></label>
        <label class="lab-field"><span>토큰</span><input id="mg-grant-amt" class="lab-input" type="text" inputmode="numeric" maxlength="7" value="100"></label>
        <button type="button" class="btn primary" id="mg-grant">지급하기</button>
      </div>
      <h2 class="section-title">공지</h2>
      <div class="card mg-card">
        <label class="lab-field"><span>내용 (300자까지)</span><textarea id="mg-notice-text" class="lab-input mg-text" maxlength="300" placeholder="모두에게 보일 공지"></textarea></label>
        <button type="button" class="btn primary" id="mg-notice">공지 올리기</button>
      </div>
      <div class="card mg-card" id="mg-notices">${noticesHTML(notices)}</div>
      <h2 class="section-title">계정 데이터 지우기</h2>
      <div class="card mg-card">
        <p class="card-body">그 사람의 프로필, 지갑, 설정, 상태를 지워요. 로그인 계정 자체는 웹에서 지울 수 없어요.</p>
        <label class="lab-field"><span>아이디</span><input id="mg-del-id" class="lab-input" type="text" autocomplete="off" autocapitalize="none" maxlength="20" placeholder="지울 사람 아이디"></label>
        <button type="button" class="btn mg-danger" id="mg-del">지우기</button>
      </div>`;
  };

  const refresh = async () => {
    if (!root || !ctx?.cloud()?.user) return;
    try {
      paint(await ctx.cloud().listNotices());
    } catch (err) {
      console.warn("공지를 불러오지 못했어요:", err?.code || err);
      paint([]);
    }
  };

  // 메시지 탭 맨 위: 가장 최근 공지 한 개
  const refreshNotice = async () => {
    if (!noticeRoot) return;
    if (!ctx?.cloud()?.user) {
      noticeRoot.hidden = true;
      return;
    }
    try {
      const [n] = await ctx.cloud().listNotices();
      noticeRoot.hidden = !n;
      noticeRoot.innerHTML = n ? `<div class="card notice-card"><p class="eyebrow">공지 · ${esc(when(n.at))}</p><p class="card-body">${esc(n.text)}</p></div>` : "";
    } catch (_) {
      noticeRoot.hidden = true;
    }
  };

  // 행동 하나: 누르는 동안 막고, 끝나면 알려주고 목록을 다시 읽어요
  const act = async (btn, fn, okMsg) => {
    if (busy) return;
    busy = true;
    btn.classList.add("busy");
    try {
      const out = await fn();
      ctx.toast(typeof okMsg === "function" ? okMsg(out) : okMsg);
      await refresh();
      refreshNotice();
    } catch (err) {
      ctx.toast(msgOf(err));
    } finally {
      busy = false;
      btn.classList.remove("busy");
    }
  };

  const init = (c) => {
    ctx = c;
    root = document.getElementById("manage-root");
    noticeRoot = document.getElementById("notice-root");
    if (root) {
      root.addEventListener("click", (e) => {
        const b = e.target.closest("button");
        if (!b) return;
        if (b.id === "mg-grant") {
          const id = document.getElementById("mg-grant-id").value.trim();
          const amt = Number(String(document.getElementById("mg-grant-amt").value).replace(/\D/g, ""));
          if (!id) return ctx.toast("받을 사람 아이디를 적어 주세요");
          if (!(amt >= 1 && amt <= 1000000)) return ctx.toast("토큰은 1부터 100만까지 적어 주세요");
          act(b, () => ctx.cloud().adminGrant(id, amt), `${id} 님에게 ${amt} 토큰을 지급했어요`);
        } else if (b.id === "mg-notice") {
          const text = document.getElementById("mg-notice-text").value.trim();
          if (!text) return ctx.toast("공지 내용을 적어 주세요");
          act(b, () => ctx.cloud().adminNotice(text), "공지를 올렸어요");
        } else if (b.id === "mg-del") {
          const id = document.getElementById("mg-del-id").value.trim();
          if (!id) return ctx.toast("지울 사람 아이디를 적어 주세요");
          if (!window.confirm(`${id} 님의 프로필, 지갑, 설정, 상태를 지울까요? 되돌릴 수 없어요.`)) return;
          act(b, () => ctx.cloud().adminDelete(id), `${id} 님의 데이터를 지웠어요`);
        }
      });
    }
  };

  window.Manage = { init, refresh, refreshNotice };
})();
