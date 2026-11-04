/* 러스트(WASM)로 옮긴 화면의 이름을 먼저 만들어 둬요.
   app.js는 WASM보다 먼저 실행돼서 window.Notices.init() 같은 호출을 일찍 할 수 있어요.
   그 호출은 줄 세워 두었다가, app/rs/boot.js가 WASM을 다 받은 뒤 __ready(...)를 부르면 순서대로 넘겨줘요.
   줄 서 있는 동안에는 약속(Promise)을 돌려줘요. 나중에 진짜 결과로 풀려요 (await walletOp(...) 같은 곳이 그대로 동작해요). */
window.rsShim = (name, methods) => {
  let impl = null;
  const queue = [];
  const api = {
    __ready(next) {
      impl = next;
      for (const [m, args, ok, fail] of queue.splice(0)) {
        try {
          ok(impl[m](...args));
        } catch (err) {
          fail(err);
        }
      }
    },
  };
  for (const m of methods) api[m] = (...args) => (impl ? impl[m](...args) : new Promise((ok, fail) => queue.push([m, args, ok, fail])));
  window[name] = api;
};
