/* 탭 바 뒤의 화면을 사진으로 찍어 두는 곳.
   막대 유리는 뒤의 화면을 확대하고 가장자리에서 휘게 보여줘야 하는데, 브라우저는 "지금 이 자리 뒤에 뭐가 있는지"를
   픽셀로 알려주지 않아요. 그래서 화면마다 전체 길이를 한 번 찍어 두고(html2canvas), 스크롤 위치만큼 밀어서 읽어요.
   찍은 사진은 확대되고 흐려진 채로만 보이기 때문에 조금 달라도 티가 나지 않아요.

   - 화면이 바뀌거나 내용이 변하면 잠깐 뒤에 다시 찍어요 (탭 바를 만지는 동안은 미뤄요)
   - 사진은 작게 줄인 흐림 버전도 같이 만들어요 (알약이 뿌옇게 보이는 데 써요) */
(() => {
  "use strict";

  const MAX_TEXTURE = 4096;
  const SCALE = 2;
  const DEBOUNCE_MS = 260;

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
          for (const [name, el] of this.screens) {
            // 화면 자체의 class/style 변화는 탭을 옮길 때 켜고 끄는 것이라 내용 변화가 아니에요
            if (el.contains(r.target) && !(r.target === el && r.type === "attributes")) dirty.add(name);
          }
        }
        dirty.forEach((name) => this.invalidate(name));
      });
      for (const el of this.screens.values()) {
        this.mutations.observe(el, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ["class", "style"] });
      }
    }

    get activeEl() {
      return this.active ? this.screens.get(this.active) : null;
    }

    setActive(name) {
      this.active = name;
      this.ensure(name, true);
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
      this.pump(DEBOUNCE_MS);
      if (!this.queue.includes(name)) this.queue.push(name);
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
      // 탭 바가 움직이는 동안에는 메인 스레드를 붙잡지 않아요
      if (this.isBusy()) return this.pump(200);
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
      if (this.queue.length) this.pump(60);
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

      return { canvas, blur, cssW, cssH, dirty: false };
    }
  }

  window.Backdrop = Backdrop;
})();
