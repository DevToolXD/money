(() => {
  "use strict";

  const { LiquidGlass, GlassTabBar, LOOKS } = window.LiquidGlass;

  const root = document.documentElement;
  const app = document.querySelector(".app");
  const nav = document.querySelector(".dock .lg-tabs");
  const screens = new Map([...document.querySelectorAll(".screen")].map((el) => [el.dataset.screen, el]));
  const scheme = matchMedia("(prefers-color-scheme: dark)");
  const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)");

  // UIKit에서 잰 스프링 값 (stiffness / damping)
  const LIFT = { stiffness: 632, damping: 50.3 }; // 렌즈가 떠오를 때: 임계 감쇠
  const MOVE = { stiffness: 241, damping: 26.4 }; // 렌즈가 탭 사이를 움직일 때: 0.405초 / 0.85
  const TRAIL = { stiffness: 576, damping: 36 }; // 손가락을 끌고 갈 때: 0.26초 / 0.75

  const setSpring = ({ stiffness, damping }) => {
    const { x, y } = bar.lens;
    x.stiffness = y.stiffness = stiffness;
    x.damping = y.damping = damping;
  };

  let bar = null;

  // 탭 바 유리는 가장 가까운 [data-theme]을 보고 밝은/어두운 모양을 골라요
  const applyTheme = () => {
    root.dataset.theme = scheme.matches ? "dark" : "light";
  };

  const showScreen = (name) => {
    for (const [key, el] of screens) {
      const on = key === name;
      el.classList.toggle("is-active", on);
      el.inert = !on;
    }
  };

  // 라이브러리의 탭 바는 자기 자신(탭 줄)만 렌즈에 복사해요. 그러면 렌즈 아래의 실제 화면이 비치지 않으니
  // 렌즈의 원본을 앱 전체로 바꿔서, 눌렀을 때 뒤의 화면이 렌즈에 굴절돼 보이게 해요.
  const mount = () => {
    const theme = root.dataset.theme;
    bar = new GlassTabBar(nav, { look: LOOKS.tabs[theme] });

    // 주소 끝에 ?lens=bar 를 붙이면 라이브러리 기본 동작(탭 줄만 비추기)으로 돌아가요.
    // 기기에서 렌즈가 이상하게 보일 때 원인을 가르려고 둔 스위치예요.
    if (new URLSearchParams(location.search).get("lens") === "bar") return;

    bar.lens.destroy();
    // 앱 전체를 통째로 복사하니, 스크롤이나 변화가 연달아 와도 32ms에 한 번만 다시 복사해요
    const lens = new LiquidGlass(app, { container: nav, throttle: 32, ...LOOKS.tabs[theme] });
    lens.scale.stiffness = LIFT.stiffness;
    lens.scale.damping = LIFT.damping;
    bar.lens = lens;
    setSpring(MOVE);
    // 화면마다 스크롤한 위치가 달라서, 복사본에도 같은 위치를 알려줘요
    for (const screen of screens.values()) lens.scrolled.add(screen);
  };

  const unmount = () => {
    bar?.destroy();
    bar = null;
  };

  nav.addEventListener("change", (e) => {
    showScreen(e.detail.tab.dataset.tab);
  });

  const scrollToTop = (tab) =>
    screens.get(tab.dataset.tab)?.scrollTo({ top: 0, behavior: reduceMotion.matches ? "auto" : "smooth" });

  // 이미 선택된 탭을 다시 누르면 맨 위로: 누른 순간에 선택돼 있었고 손가락이 거의 안 움직였을 때만이에요.
  // (클릭 이벤트로 판단하면 탭 바가 선택을 바꾼 뒤에 오는 진짜 클릭이 다시 누름으로 보여서 어긋나요)
  let press = null;
  let lastPointer = -Infinity;
  let sliding = null;

  nav.addEventListener(
    "pointerdown",
    (e) => {
      const tab = e.target.closest("button");
      if (!tab) return;
      lastPointer = performance.now();
      press = { tab, wasCurrent: tab === bar?.current, x: e.clientX };
      // 어느 판을 눌렀는지 알려줘서 그 판만 부풀게 해요
      nav.dataset.press = tab.dataset.group;

      // 다른 탭을 누르면 렌즈가 지금 선택된 탭 위에서 먼저 떠올라 눌린 탭으로 미끄러져 가요.
      // (캡처 단계라서 탭 바가 렌즈를 눌린 탭에 바로 올려놓기 전에 먼저 실행돼요)
      if (e.button !== 0 || !bar || bar.lens.visible || !bar.current || tab === bar.current) return;
      bar.lift(bar.current);
      sliding = { x: e.clientX };
      // 라이브러리가 눌린 동안은 손가락을 바짝 따라가는 스프링(TRAIL)으로 바꿔요. 미끄러지는 동안만 이동 스프링으로 돌려놔요
      setTimeout(() => sliding && setSpring(MOVE), 0);
    },
    true
  );

  // 손가락이 끌기 시작하면 다시 바짝 따라가는 스프링으로
  nav.addEventListener("pointermove", (e) => {
    if (sliding && Math.abs(e.clientX - sliding.x) > 6) {
      setSpring(TRAIL);
      sliding = null;
    }
  });

  nav.addEventListener("pointerup", (e) => {
    if (press?.wasCurrent && Math.abs(e.clientX - press.x) < 8) scrollToTop(press.tab);
    press = null;
    sliding = null;
  });
  nav.addEventListener("pointercancel", () => {
    press = null;
    sliding = null;
  });

  // 키보드로 이미 선택된 탭을 누를 때 (포인터 없이 온 클릭)
  nav.addEventListener(
    "click",
    (e) => {
      const tab = e.target.closest("[data-tab]");
      if (tab?.hasAttribute("aria-current") && performance.now() - lastPointer > 800) scrollToTop(tab);
    },
    true
  );

  window.addEventListener("resize", () => bar?.placePill());

  scheme.addEventListener("change", () => {
    unmount();
    applyTheme();
    mount();
  });

  applyTheme();
  const initial = nav.querySelector("[aria-current]");
  showScreen(initial ? initial.dataset.tab : "messages");
  mount();
})();
