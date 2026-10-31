/* 다른 사이트가 이 앱을 액자(iframe)에 넣어 눌러 보게 만드는 공격(클릭재킹)을 막아요.
   github pages는 응답 머리글을 못 바꿔서 여기서 막아요. 액자 안에서 열리면 화면을 숨겨요. */
if (window.top !== window.self) {
  document.documentElement.style.display = "none";
  try {
    window.top.location = window.self.location;
  } catch (_) {
    /* 다른 사이트는 막을 수 없어서 숨기기만 해요 */
  }
}
