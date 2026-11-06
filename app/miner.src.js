/* 채굴 일꾼 (Web Worker): 모네로 RandomX 해시(randomx.js 라이브러리)만 계산해요. 켜고 끄기·보상·서버 연결은 러스트(web/crates/ui/src/coin.rs)가 해요.
   러스트(WASM)는 워커 안에서 이 라이브러리를 직접 부를 수 없어서, 계산 고리만 여기 두어요. `npm run build`(tests 폴더)가 app/miner.js 한 개로 묶어요. */
import { randomx_init_cache, randomx_create_vm } from "randomx-web";

let vm = null, seed = "", job = null, run = false, nonce = 0;
const hex2b = (h) => Uint8Array.from(h.match(/../g).map((x) => parseInt(x, 16)));
const b2hex = (b) => Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
// 풀이 주는 목표(4바이트면 짧은 꼴): 해시 끝 8바이트(작은 쪽 먼저)가 이보다 작으면 share예요
function target64(t) {
  const b = hex2b(t);
  let v = 0n;
  for (let i = b.length - 1; i >= 0; i--) v = (v << 8n) | BigInt(b[i]);
  return b.length === 4 ? 0xffffffffffffffffn / (0xffffffffn / (v || 1n)) : v;
}
function loop() {
  if (!run || !job || !vm) { run = false; return; }
  const blob = hex2b(job.blob), t = target64(job.target), t0 = performance.now();
  let n = 0;
  while (performance.now() - t0 < 400) {
    nonce = (nonce + 1) >>> 0;
    blob[39] = nonce & 255; blob[40] = (nonce >>> 8) & 255; blob[41] = (nonce >>> 16) & 255; blob[42] = (nonce >>> 24) & 255;
    const h = vm.calculate_hash(blob);
    n++;
    let v = 0n;
    for (let i = 31; i >= 24; i--) v = (v << 8n) | BigInt(h[i]);
    if (v < t) postMessage({ type: "share", job_id: job.job_id, nonce: b2hex(blob.slice(39, 43)), result: b2hex(h) });
  }
  postMessage({ type: "rate", n, ms: performance.now() - t0 });
  setTimeout(loop, 0);
}
onmessage = (e) => {
  const m = e.data;
  if (m.type === "job") {
    job = m;
    nonce = (Math.random() * 0xffffffff) >>> 0;
    if (m.seed_hash && m.seed_hash !== seed) {
      seed = m.seed_hash;
      postMessage({ type: "status", s: "init" });
      vm = randomx_create_vm(randomx_init_cache(hex2b(seed)));
      postMessage({ type: "status", s: "ready" });
    }
    if (!run) { run = true; setTimeout(loop, 0); }
  } else if (m.type === "stop") {
    run = false; job = null;
  }
};
