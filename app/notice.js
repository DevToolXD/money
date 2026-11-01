/* 공지는 러스트(WASM)로 옮겼어요: web/crates/ui/src/notices.rs
   이 파일은 이름(window.Notices)만 먼저 만들어 두고, WASM이 다 받아지기 전에 온 호출을 줄 세워 두었다가 넘겨줘요.
   (app.js는 WASM보다 먼저 실행돼서 init·refresh·check를 일찍 부를 수 있어요) */
(() => {
  "use strict";
  let impl = null;
  const queue = [];
  const call = (name, args) => (impl ? impl[name](...args) : queue.push([name, args]));
  window.Notices = {
    init: (...a) => call("init", a),
    refresh: (...a) => call("refresh", a),
    check: (...a) => call("check", a),
    // app/rs/boot.js가 WASM을 다 받은 뒤 불러요
    __ready(next) {
      impl = next;
      for (const [name, args] of queue.splice(0)) impl[name](...args);
    },
  };
})();
