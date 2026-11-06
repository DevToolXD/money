// 코인 탭 채굴 프록시: 브라우저 한 개 연결 = 풀 로그인 한 개. 풀이 주는 일(job)을 브라우저로, 브라우저가 찾은 share를 풀로 보내요.
// 지갑 주소는 WALLET 하나뿐이에요 (채굴한 모네로는 모두 이 주소로 가요). 토큰 보상은 앱의 Firestore 규칙이 따로 정해요.
import net from "node:net";
import http from "node:http";
import { WebSocketServer } from "ws";

const PORT = Number(process.env.PORT || 8080);
const POOL_HOST = process.env.POOL_HOST || "pool.supportxmr.com";
const POOL_PORT = Number(process.env.POOL_PORT || 3333);
const WALLET = process.env.WALLET || "485EQBnoMChJzNdVFNr1CDL1ELdw1QhcgHPLpE7ECpi8ZenhrHCA8hdDM8Fe8512H5TuW48bQV7Aa6Cto3ryJW5t7NkH368";
const ORIGINS = (process.env.ORIGINS || "").split(",").map((s) => s.trim()).filter(Boolean); // 비우면 모두 허락
const MAX = Number(process.env.MAX_CONN || 500);

const server = http.createServer((req, res) => { res.writeHead(200, { "content-type": "text/plain" }); res.end("ok"); });
const wss = new WebSocketServer({ server, maxPayload: 4096 });

wss.on("connection", (ws, req) => {
  if (ORIGINS.length && !ORIGINS.includes(req.headers.origin || "")) return ws.close(1008, "origin");
  if (wss.clients.size > MAX) return ws.close(1013, "busy");
  const send = (o) => ws.readyState === 1 && ws.send(JSON.stringify(o));
  let pool = null, buf = "", seq = 1, minerId = "";
  const toPool = (o) => pool && !pool.destroyed && pool.write(JSON.stringify(o) + "\n");
  const job = (j) => j && send({ type: "job", job_id: j.job_id, blob: j.blob, target: j.target, seed_hash: j.seed_hash, height: j.height });
  const open = (rig) => {
    pool = net.connect(POOL_PORT, POOL_HOST, () => toPool({ id: seq++, method: "login", params: { login: WALLET, pass: rig, rigid: rig, agent: "money-sim-web/1.0" } }));
    pool.setEncoding("utf8");
    pool.on("data", (d) => {
      buf += d;
      let i;
      while ((i = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, i); buf = buf.slice(i + 1);
        let m; try { m = JSON.parse(line); } catch { continue; }
        if (m.method === "job") job(m.params);
        else if (m.result && m.result.id && m.result.job) { minerId = m.result.id; send({ type: "ready" }); job(m.result.job); }
        else if (m.result && m.result.status === "OK") send({ type: "accepted" });
        else if (m.error) send({ type: "rejected", error: String(m.error.message || "error").slice(0, 80) });
      }
    });
    pool.on("error", () => send({ type: "error", error: "pool" }));
    pool.on("close", () => ws.close(1011, "pool closed"));
  };
  ws.on("message", (raw) => {
    let m; try { m = JSON.parse(String(raw)); } catch { return; }
    if (m.type === "hello" && !pool) open(String(m.rig || "web").replace(/[^A-Za-z0-9_-]/g, "").slice(0, 20) || "web");
    else if (m.type === "submit" && minerId && /^[0-9a-f]{8}$/.test(m.nonce || "") && /^[0-9a-f]{64}$/.test(m.result || "")) toPool({ id: seq++, method: "submit", params: { id: minerId, job_id: String(m.job_id), nonce: m.nonce, result: m.result } });
  });
  ws.on("close", () => pool && pool.destroy());
});
server.listen(PORT, () => console.log(`채굴 프록시: ${PORT} → ${POOL_HOST}:${POOL_PORT}`));
