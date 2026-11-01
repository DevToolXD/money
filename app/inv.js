/* 인벤토리: 상점에서 사거나 사장이 회사 창고에서 꺼낸 음식이 들어가요.
   골라서 먹기를 누르면 한 개가 줄고 상태(배고픔·목마름·에너지)가 올라가요 (서버 규칙이 지켜요: app/firebase.src.js 의 eatFromInventory). */
(() => {
  "use strict";

  const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
  // 음식 종류별로 올라가는 상태와 양 (서버 규칙과 같아요)
  // 올라가는 양은 회사 식품 성능으로 정해져요 (기본 20). 종류별로 어떤 상태가 오르는지만 여기서 정해요
  const EFFECT = { water: ["t", "목마름"], snack: ["h", "배고픔"], meal: ["h", "배고픔"], energy: ["e", "에너지"] };
  const gains = {}; // 회사별 올라가는 양 (성능이 바뀌면 다시 읽어요)

  let ctx = null;
  let root = null;
  let state = "loading"; // loading · ok · error · out (로그인 안 함)
  let items = [];
  let selected = null; // 고른 칸 번호
  let busy = false;
  let seq = 0;

  const art = (it) => {
    try {
      return ctx.pkv(it, false, { name: it.coName || "", logo: "" });
    } catch (_) {
      return "";
    }
  };
  const cardHTML = (it, i) => `<article class="card shop-card inv-card${it.id === selected ? " is-on" : ""}" style="--i:${i}" data-key="${esc(it.id)}">
      <div class="shop-art">${art(it)}</div>
      <div class="shop-body">
        <b class="shop-name">${esc(it.name)}</b>
        <span class="shop-co">${esc(it.coName || "")}</span>
        <span class="shop-stock">${it.n}개</span>
      </div>
    </article>`;
  const detailHTML = (it) => {
    const e = EFFECT[it.group];
    const g = gains[it.cid];
    return `<div class="card inv-detail">
      <div class="shop-art">${art(it)}</div>
      <div class="inv-info">
        <b class="shop-name">${esc(it.name)}</b>
        <span class="shop-co">${esc(it.coName || "")}</span>
        <span class="shop-stock">${it.n}개 가지고 있어요</span>
        <span class="inv-effect">${e ? `${e[1]} +${g ?? "…"}` : "먹을 수 없어요"}</span>
        <button type="button" class="btn primary inv-eat" data-eat="1"${e ? "" : " disabled"}>먹기</button>
      </div>
    </div>`;
  };
  const noteHTML = (eyebrow, title, body, extra = "", pic = "") =>
    `<div class="card shop-note">${pic}<div class="art-text"><p class="eyebrow">${esc(eyebrow)}</p><h2 class="card-title">${esc(title)}</h2><p class="card-body">${esc(body)}</p>${extra}</div></div>`;

  const paint = () => {
    if (!root) return;
    if (state === "loading") {
      root.innerHTML = '<div class="card skeletons" style="--rows: 4" aria-hidden="true"></div>';
      return;
    }
    if (state === "out") {
      root.innerHTML = noteHTML("로그인이 필요해요", "로그인하면 볼 수 있어요", "상점에서 사거나 사장님이 창고에서 꺼낸 음식이 여기에 들어가요.");
      return;
    }
    if (state === "error") {
      root.innerHTML = noteHTML("잠깐만요", "인벤토리를 불러오지 못했어요", "인터넷을 확인하고 다시 해 주세요.", '<button type="button" class="btn inv-retry">다시 해 보기</button>');
      return;
    }
    if (!items.length) {
      root.innerHTML = '<p class="inv-empty-text">아무것도 없어요</p>';
      return;
    }
    const sel = items.find((x) => x.id === selected);
    root.innerHTML = `${sel ? detailHTML(sel) : ""}<h2 class="section-title">내 음식</h2><div class="shop-grid">${items.map(cardHTML).join("")}</div>`;
  };

  const load = async () => {
    const my = ++seq;
    if (!ctx?.cloud()?.user) {
      state = "out";
      items = [];
      paint();
      return;
    }
    try {
      const list = await ctx.cloud().listInventory();
      if (my !== seq) return;
      items = list.sort((a, b) => String(a.name).localeCompare(String(b.name), "ko"));
      state = "ok";
    } catch (err) {
      if (my !== seq) return;
      console.warn("인벤토리를 불러오지 못했어요:", err?.code || err);
      state = "error";
    }
    if (selected && !items.some((x) => x.id === selected)) selected = null;
    paint();
  };

  const msgOf = (err) => {
    if (err?.code === "permission-denied") return "지금은 먹을 수 없어요. 잠시 뒤에 다시 해 주세요";
    return err?.message || "먹지 못했어요. 잠시 뒤에 다시 해 주세요";
  };

  // 먹기: 칸이 한 개 줄고 상태가 올라가요. 올라간 상태는 화면에도 바로 반영해요
  const eat = async (it, btn) => {
    const e = EFFECT[it.group];
    if (!e) return ctx.toast("먹을 수 없는 음식이에요");
    if (busy) return;
    const vit = ctx.vitals?.get();
    if (!vit) return ctx.toast("상태를 아직 불러오지 못했어요");
    busy = true;
    btn.classList.add("busy");
    try {
      const v = await ctx.cloud().eatFromInventory(it.cid, it.pid, it.group, vit);
      ctx.vitals.set(v);
      gains[it.cid] = v.gained;
      ctx.toast(`${it.name} 한 개를 먹었어요 · ${e[1]} +${v.gained}`);
    } catch (err) {
      ctx.toast(msgOf(err));
    } finally {
      busy = false;
      btn.classList.remove("busy");
      await load(); // 개수를 서버에서 다시 읽어요
    }
  };

  const init = (c) => {
    ctx = c;
    root = document.getElementById("inv-root");
    if (!root) return;
    root.addEventListener("click", (e) => {
      if (e.target.closest(".inv-retry")) return load();
      const eatBtn = e.target.closest("[data-eat]");
      if (eatBtn) {
        const it = items.find((x) => x.id === selected);
        if (it && !eatBtn.disabled) eat(it, eatBtn);
        return;
      }
      const card = e.target.closest(".inv-card");
      if (card) {
        selected = card.dataset.key === selected ? null : card.dataset.key;
        paint();
        const it = items.find((x) => x.id === selected);
        if (it && gains[it.cid] === undefined) {
          ctx.cloud().foodGain(it.cid).then((v) => {
            gains[it.cid] = v;
            paint();
          }).catch(() => {});
        }
      }
    });
  };

  window.Inventory = { init, refresh: load };
})();
