// 코인 탭 채굴 프록시 (Cloudflare Workers, 무료): 브라우저(WebSocket) ↔ 모네로 풀(TCP).
// 대시보드: Workers & Pages → Create application → Hello World 시작 → 이름 money-miner → Deploy → Edit code → 이 파일 전체를 붙여 넣고 Deploy.
// 채굴한 모네로는 모두 WALLET로 가요. 토큰 보상은 앱의 Firestore 규칙이 정해요.
import { connect } from "cloudflare:sockets";

const WALLET = "485EQBnoMChJzNdVFNr1CDL1ELdw1QhcgHPLpE7ECpi8ZenhrHCA8hdDM8Fe8512H5TuW48bQV7Aa6Cto3ryJW5t7NkH368";
// 앞에서부터 차례로 붙어 봐요 (한 곳이 막히면 다음 곳)
const POOLS = [
  { hostname: "pool.supportxmr.com", port: 3333 },
  { hostname: "gulf.moneroocean.stream", port: 10128 },
  { hostname: "xmr-asia1.nanopool.org", port: 10343 },
  { hostname: "pool.hashvault.pro", port: 3333 },
];
const ORIGINS = ["https://www.xn--sn3bu3q0e19c.com", "https://xn--sn3bu3q0e19c.com", "https://devtoolxd.github.io"];

export default {
  async fetch(req) {
    if (req.headers.get("Upgrade") !== "websocket") return new Response("ok");
    const origin = req.headers.get("Origin") || "";
    const [client, ws] = Object.values(new WebSocketPair());
    ws.accept();
    // 허락하지 않은 사이트는 이유를 알려 주고 닫아요
    if (!ORIGINS.includes(origin)) { ws.close(1008, "origin"); return new Response(null, { status: 101, webSocket: client }); }
    const send = (o) => { try { ws.send(JSON.stringify(o)); } catch {} };
    let sock = null, writer = null, minerId = "", seq = 1, started = false;
    const toPool = (o) => writer && writer.write(new TextEncoder().encode(JSON.stringify(o) + "\n")).catch(() => {});
    const job = (j) => j && send({ type: "job", job_id: j.job_id, blob: j.blob, target: j.target, seed_hash: j.seed_hash, height: j.height });
    const open = async (rig) => {
      let last = "pool";
      for (const P of POOLS) {
        let gotJob = false;
        try {
          sock = connect(P);
          writer = sock.writable.getWriter();
          await toPool({ id: seq++, method: "login", params: { login: WALLET, pass: rig, rigid: rig, agent: "money-sim-web/1.0" } });
          const reader = sock.readable.pipeThrough(new TextDecoderStream()).getReader();
          let buf = "";
          for (;;) {
            const { value, done } = await reader.read();
            if (done) break;
            buf += value;
            let i;
            while ((i = buf.indexOf("\n")) >= 0) {
              const line = buf.slice(0, i); buf = buf.slice(i + 1);
              let m; try { m = JSON.parse(line); } catch { continue; }
              if (m.method === "job") job(m.params);
              else if (m.result && m.result.id && m.result.job) { gotJob = true; minerId = m.result.id; send({ type: "ready", pool: P.hostname }); job(m.result.job); }
              else if (m.result && m.result.status === "OK") send({ type: "accepted" });
              else if (m.error) { last = "pool " + String(m.error.message || "error").slice(0, 60); send({ type: "rejected", error: last }); }
            }
          }
          last = "pool closed " + P.hostname;
        } catch (e) { last = "pool " + P.hostname + " " + String(e && e.message || e).slice(0, 50); }
        try { sock && sock.close(); } catch {}
        sock = null; writer = null;
        if (gotJob) break; // 일을 받았다가 끊긴 거면 다른 풀로 옮기지 않아요
      }
      try { ws.close(1011, last.slice(0, 120)); } catch {}
    };
    ws.addEventListener("message", (e) => {
      let m; try { m = JSON.parse(String(e.data)); } catch { return; }
      if (m.type === "hello" && !started) { started = true; open(String(m.rig || "web").replace(/[^A-Za-z0-9_-]/g, "").slice(0, 20) || "web"); }
      else if (m.type === "submit" && minerId && /^[0-9a-f]{8}$/.test(m.nonce || "") && /^[0-9a-f]{64}$/.test(m.result || "")) toPool({ id: seq++, method: "submit", params: { id: minerId, job_id: String(m.job_id), nonce: m.nonce, result: m.result } });
    });
    ws.addEventListener("close", () => { try { sock && sock.close(); } catch {} });
    return new Response(null, { status: 101, webSocket: client });
  },
};
