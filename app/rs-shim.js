/* 러스트(WASM)로 옮긴 화면의 이름을 먼저 만들어 둬요.
   app.js는 WASM보다 먼저 실행돼서 window.Notices.init() 같은 호출을 일찍 할 수 있어요.
   그 호출은 줄 세워 두었다가, app/rs/boot.js가 WASM을 다 받은 뒤 __ready(...)를 부르면 순서대로 넘겨줘요. */
window.rsShim = (name, methods) => {
  let impl = null;
  const queue = [];
  const api = {
    __ready(next) {
      impl = next;
      for (const [m, args] of queue.splice(0)) impl[m](...args);
    },
  };
  for (const m of methods) api[m] = (...args) => (impl ? impl[m](...args) : queue.push([m, args]));
  window[name] = api;
};
