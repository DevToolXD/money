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
    const rows = Number(getComputedStyle(box).getPropertyValue("--rows")) || 8;
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

  // 기기에서 이상하게 보일 때 원인을 가르려고 둔 스위치예요.
  //   ?glass=off  뒤 화면 굴절 없이, 흐린 유리 판(CSS)만 써요
  //   ?gl=off     WebGL을 아예 안 쓰고 평범한 DOM 아이콘과 알약으로 보여줘요
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
    backdrop?.setActive(name);
    tabbar.requestRender();
  });
  // 이미 선택된 탭을 다시 누르면 맨 위로
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
