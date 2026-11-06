// 코인 탭 채굴 프록시 (Cloudflare Workers, 무료): 브라우저(WebSocket) ↔ 모네로 풀(TCP).
// 대시보드: Workers & Pages → Create application → Hello World 시작 → 이름 money-miner → Deploy → Edit code → 이 파일 전체를 붙여 넣고 Deploy.
// 채굴한 모네로는 모두 WALLET로 가요. 토큰 보상은 앱의 Firestore 규칙이 정해요.
import { connect } from "cloudflare:sockets";

const WALLET = "485EQBnoMChJzNdVFNr1CDL1ELdw1QhcgHPLpE7ECpi8ZenhrHCA8hdDM8Fe8512H5TuW48bQV7Aa6Cto3ryJW5t7NkH368";
const POOL = { hostname: "pool.supportxmr.com", port: 3333 };
const ORIGINS = ["https://www.xn--sn3bu3q0e19c.com", "https://xn--sn3bu3q0e19c.com", "https://devtoolxd.github.io"];

export default {
  async fetch(req) {
    if (req.headers.get("Upgrade") !== "websocket") return new Response("ok");
    const origin = req.headers.get("Origin") || "";
    if (!ORIGINS.includes(origin)) return new Response("origin", { status: 403 });
    const [client, ws] = Object.values(new WebSocketPair());
    ws.accept();
    const send = (o) => { try { ws.send(JSON.stringify(o)); } catch {} };
    let sock = null, writer = null, minerId = "", seq = 1;
    const toPool = (o) => writer && writer.write(new TextEncoder().encode(JSON.stringify(o) + "\n")).catch(() => {});
    const job = (j) => j && send({ type: "job", job_id: j.job_id, blob: j.blob, target: j.target, seed_hash: j.seed_hash, height: j.height });
    const open = async (rig) => {
      try {
        sock = connect(POOL);
        writer = sock.writable.getWriter();
        toPool({ id: seq++, method: "login", params: { login: WALLET, pass: rig, rigid: rig, agent: "money-sim-web/1.0" } });
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
            else if (m.result && m.result.id && m.result.job) { minerId = m.result.id; send({ type: "ready" }); job(m.result.job); }
            else if (m.result && m.result.status === "OK") send({ type: "accepted" });
            else if (m.error) send({ type: "rejected", error: String(m.error.message || "error").slice(0, 80) });
          }
        }
      } catch { send({ type: "error", error: "pool" }); }
      try { ws.close(1011, "pool closed"); } catch {}
    };
    ws.addEventListener("message", (e) => {
      let m; try { m = JSON.parse(String(e.data)); } catch { return; }
      if (m.type === "hello" && !sock) open(String(m.rig || "web").replace(/[^A-Za-z0-9_-]/g, "").slice(0, 20) || "web");
      else if (m.type === "submit" && minerId && /^[0-9a-f]{8}$/.test(m.nonce || "") && /^[0-9a-f]{64}$/.test(m.result || "")) toPool({ id: seq++, method: "submit", params: { id: minerId, job_id: String(m.job_id), nonce: m.nonce, result: m.result } });
    });
    ws.addEventListener("close", () => { try { sock && sock.close(); } catch {} });
    return new Response(null, { status: 101, webSocket: client });
  },
};
