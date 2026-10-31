/* 탭 바 뒤의 화면을 사진으로 찍어 두는 곳.
   막대 유리는 뒤의 화면을 확대하고 가장자리에서 휘게 보여줘야 하는데, 브라우저는 "지금 이 자리 뒤에 뭐가 있는지"를
   픽셀로 알려주지 않아요. 그래서 화면마다 전체 길이를 한 번 찍어 두고(html2canvas), 스크롤 위치만큼 밀어서 읽어요.
   찍은 사진은 확대되고 흐려진 채로만 보이기 때문에 조금 달라도 티가 나지 않아요.

   - 화면이 바뀌거나 내용이 변하면 잠깐 뒤에 다시 찍어요 (탭 바를 만지는 동안은 미뤄요)
   - 사진은 작게 줄인 흐림 버전도 같이 만들어요 (알약이 뿌옇게 보이는 데 써요) */
(() => {
  "use strict";

  const MAX_TEXTURE = 4096;
  // 화면 사진은 막대 유리 뒤에서 흐리게 보일 뿐이라 1.5배면 충분해요 (2배보다 가볍고 모양은 같아요)
  const SCALE = 1.5;
  const DEBOUNCE_MS = 700;
  // 같은 화면을 다시 찍는 건 아무리 자주 바뀌어도 이만큼은 간격을 둬요. 화면 사진 찍기가 가장 무거운 일이에요
  const MIN_GAP_MS = 6000;
  const idle = (fn) => (window.requestIdleCallback ? window.requestIdleCallback(fn, { timeout: 2500 }) : setTimeout(fn, 60));

  class Backdrop {
    constructor({ root, screens, isBusy = () => false, onUpdate = () => {} }) {
      this.root = root; // 화면들이 놓인 앱 틀(.app)
      this.screens = screens; // Map 이름 → 화면 요소
      this.isBusy = isBusy;
      this.onUpdate = onUpdate;
      this.active = null;
      this.shots = new Map(); // 이름 → { canvas, blur, cssW, cssH, dirty }
      this.queue = [];
      this.working = false;
      this.timer = 0;
      this.supported = typeof window.html2canvas === "function";

      if (!this.supported) return;

      this.mutations = new MutationObserver((records) => {
        const dirty = new Set();
        for (const r of records) {
          // 매초 바뀌는 숫자(data-volatile)나 끌고 있는 카드의 움직임은 내용이 바뀐 게 아니에요
          const node = r.target.nodeType === 1 ? r.target : r.target.parentElement;
          if (!node || node.closest("[data-volatile], .lifting")) continue;
          for (const [name, el] of this.screens) {
            // 화면 자체의 class 변화는 탭을 옮길 때 켜고 끄는 것이라 내용 변화가 아니에요
            if (el.contains(r.target) && !(r.target === el && r.type === "attributes")) dirty.add(name);
          }
        }
        dirty.forEach((name) => this.invalidate(name));
      });
      for (const el of this.screens.values()) {
        this.mutations.observe(el, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ["class"] });
      }
    }

    get activeEl() {
      return this.active ? this.screens.get(this.active) : null;
    }

    setActive(name) {
      this.active = name;
      const shot = this.shots.get(name);
      // 이미 찍어 둔 사진이 있으면 그대로 쓰고, 낡았을 때만 간격을 두고 다시 찍어요
      if (shot?.canvas && shot.dirty && Date.now() - (shot.takenAt || 0) < MIN_GAP_MS) this.pump(MIN_GAP_MS - (Date.now() - shot.takenAt));
      else this.ensure(name, !shot?.canvas);
    }

    // 사진이 없거나 낡았으면 찍도록 줄을 세워요
    ensure(name, urgent = false) {
      if (!this.supported) return;
      const shot = this.shots.get(name);
      if (shot && !shot.dirty) return;
      if (!this.queue.includes(name)) {
        if (urgent) this.queue.unshift(name);
        else this.queue.push(name);
      }
      this.pump(urgent ? 0 : 40);
    }

    invalidate(name) {
      if (!this.supported) return;
      const shot = this.shots.get(name);
      if (shot) shot.dirty = true;
      else this.shots.set(name, { dirty: true });
      if (!this.queue.includes(name)) this.queue.push(name);
      // 보이지 않는 화면은 바로 찍지 않고, 그 화면이 보일 때(setActive) 찍어요
      if (name !== this.active) return;
      const wait = Math.max(DEBOUNCE_MS, MIN_GAP_MS - (Date.now() - (shot?.takenAt || 0)));
      this.pump(shot?.canvas ? wait : DEBOUNCE_MS);
    }

    invalidateAll() {
      for (const name of this.screens.keys()) this.invalidate(name);
    }

    pump(delay) {
      clearTimeout(this.timer);
      this.timer = setTimeout(() => this.run(), delay);
    }

    async run() {
      if (this.working || !this.queue.length) return;
      // 탭 바가 움직이는 동안이나 화면이 꺼져 있을 때는 메인 스레드를 붙잡지 않아요
      if (this.isBusy() || document.hidden) return this.pump(400);
      // 폰이 다른 일을 안 하는 틈에 찍어요
      if (!this.idleDone) {
        if (!this.idleWaiting) {
          this.idleWaiting = true;
          idle(() => {
            this.idleWaiting = false;
            this.idleDone = true;
            this.run();
          });
        }
        return;
      }
      this.idleDone = false;
      this.working = true;
      const name = this.queue.shift();
      try {
        const shot = await this.shoot(name);
        if (shot) {
          this.shots.set(name, shot);
          this.onUpdate(name, shot);
        }
      } catch (err) {
        console.warn("화면 사진을 찍지 못했어요:", name, err);
      }
      this.working = false;
      // 다음 화면은 한참 뒤에 찍어요 (여러 화면을 한꺼번에 찍으면 폰이 멈칫해요)
      if (this.queue.length) this.pump(1200);
    }

    async shoot(name) {
      const el = this.screens.get(name);
      if (!el) return null;
      if (document.fonts?.ready) await document.fonts.ready;
      const cssW = el.clientWidth;
      const cssH = Math.max(el.scrollHeight, el.clientHeight);
      if (cssW < 10 || cssH < 10) return null;
      const scale = Math.min(SCALE, window.devicePixelRatio || 1, MAX_TEXTURE / cssH, MAX_TEXTURE / cssW);
      // 앱 바탕색이 투명하면(그라디언트·이미지일 때) 사진이 검게 나오니 문서 바탕색으로 대신해요
      let bg = getComputedStyle(this.root).backgroundColor;
      if (/rgba?\([^)]*,\s*0\)$|transparent/.test(bg)) {
        bg = getComputedStyle(document.documentElement).getPropertyValue("--bg-canvas").trim() || "#ffffff";
      }

      const canvas = await window.html2canvas(el, {
        scale,
        backgroundColor: bg,
        logging: false,
        width: cssW,
        height: cssH,
        windowWidth: cssW,
        windowHeight: cssH,
        scrollX: 0,
        scrollY: 0,
        onclone: (doc, clone) => {
          // 숨겨져 있던 화면도 보이게 하고, 스크롤 상자가 아니라 전체 길이로 펼쳐서 찍어요
          clone.style.cssText += `;position:relative;inset:auto;height:auto;overflow:visible;visibility:visible;opacity:1;width:${cssW}px;`;
          const wrap = clone.parentElement;
          if (wrap) wrap.style.cssText += ";position:relative;inset:auto;height:auto;";
          const app = doc.querySelector(".app");
          if (app) app.style.cssText += ";height:auto;overflow:visible;";
        },
      });

      // 흐린 버전: 작게 줄였다가 늘려서 보여요 (샘플링이 부드럽게 이어져서 뿌옇게 보여요)
      const blur = document.createElement("canvas");
      const k = 1 / 6;
      blur.width = Math.max(2, Math.round(canvas.width * k));
      blur.height = Math.max(2, Math.round(canvas.height * k));
      const bctx = blur.getContext("2d");
      bctx.imageSmoothingEnabled = true;
      bctx.imageSmoothingQuality = "high";
      bctx.drawImage(canvas, 0, 0, blur.width, blur.height);

      return { canvas, blur, cssW, cssH, dirty: false, takenAt: Date.now() };
    }
  }

  window.Backdrop = Backdrop;
})();
