// 진짜 번들 (에뮬레이터): 아이디 앞부분·이름 앞부분으로 찾고, 이름으로 토큰을 줄 수 있어요
const { chromium } = require('playwright');
const fs = require('fs');
const emu = fs.readFileSync(require('path').join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const open = async (b) => {
  const page = await (await b.newContext({ viewport: { width: 390, height: 844 } })).newPage();
  await page.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await page.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await page.goto('http://127.0.0.1:8765/index.html'); await page.waitForTimeout(3500);
  return page;
};
const call = (p, fn, args) => p.evaluate(([f, a]) => window.cloud[f](...a).then((r) => (r === undefined ? 'ok' : r), (e) => 'ERR ' + (e.code || e.message)), [fn, args]);
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const sfx = Date.now().toString(36).slice(-4);
  const O = await open(b), T = await open(b);
  await O.evaluate(() => 0);
  await O.evaluate(() => window.cloud.signUp('owner', 'abc123', '관리자'));
  await O.reload(); await O.waitForTimeout(6000);
  const name = '김민준' + sfx.replace(/[a-z0-9]/g, '');
  const id = 'minjun' + sfx;
  await T.evaluate(([u, n]) => window.cloud.signUp(u, 'abc123', n), [id, name]);
  await T.reload(); await T.waitForTimeout(6000);
  await O.waitForTimeout(2300);
  const byId = await call(O, 'searchProfiles', ['minj']);
  ok('아이디 앞부분 minj로 찾아요', Array.isArray(byId) && byId.some((p) => p.id === id));
  const byName = await call(O, 'searchProfiles', [name.slice(0, 2)]);
  ok('이름 앞 두 글자로 찾아요', Array.isArray(byName) && byName.some((p) => p.id === id));
  const found = await call(O, 'searchProfiles', ['zzqqxx']);
  ok('없는 이름은 빈 목록', Array.isArray(found) && found.length === 0);
  await O.waitForTimeout(2300);
  const g = await call(O, 'adminGrant', [name, 5]);
  ok('이름으로 토큰을 줄 수 있어요', typeof g === 'string' && g.length > 5);
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close(); process.exit(res.every(Boolean) ? 0 : 1);
})();
