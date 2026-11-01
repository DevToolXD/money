// 진짜 번들 + 진짜 규칙 (에뮬레이터): 공장 두 대가 한 개씩 만들어요 (60초에 두 개). 재료값은 만든 만큼, 재고는 정산할 때 바로 늘어요
const { chromium } = require('playwright');
const fs = require('fs');
const emu = fs.readFileSync(require('path').join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = [];
const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const open = async (b) => {
  const page = await (await b.newContext({ viewport: { width: 390, height: 844 } })).newPage();
  await page.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await page.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await page.goto('http://127.0.0.1:8765/index.html'); await page.waitForTimeout(3500);
  return page;
};
const rawDoc = async (path) => (await (await fetch(`${DB}/${path}`, { headers: { authorization: 'Bearer owner' } })).json());
const setWallet = (uid, bal) => fetch(`${DB}/wallets/${uid}?updateMask.fieldPaths=bal`, { method: 'PATCH', headers: { 'content-type': 'application/json', authorization: 'Bearer owner' }, body: JSON.stringify({ fields: { bal: { integerValue: String(bal) } } }) });
const cashOf = async (cid) => Number((await rawDoc(`companies/${cid}/lab/main`)).fields.cash.integerValue);
const stockOf = async (cid, pid) => { const d = await rawDoc(`companies/${cid}/products/${pid}`); return d.fields && d.fields.stock ? Number(d.fields.stock.integerValue) : 0; };
const call = (p, fn, args) => p.evaluate(([f, a]) => window.cloud[f](...a).then((r) => (r === undefined ? 'ok' : r), (e) => 'ERR ' + (e.code || e.message)), [fn, args]);
const wait = (p, ms) => p.waitForTimeout(ms);
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--disable-web-security'] });
  const p = await open(b);
  await p.evaluate(() => window.cloud.signUp('facowner' + Date.now().toString(36).slice(-4), 'abc123', '공장장'));
  await p.reload(); await wait(p, 6000);
  const uid = await p.evaluate(() => window.cloud.uid);
  await setWallet(uid, 1000); await wait(p, 2300);
  const cid = await call(p, 'foundCompany', ['공장 테스트', '', 'food']);
  await wait(p, 2300);
  await call(p, 'labCash', [cid, 600, 1]); // 600 토큰 넣기 (50000 센트 이상)
  await wait(p, 2300);
  const pid = await call(p, 'createProduct', [cid, { cat: 'food', group: 'water', item: 'water', name: '공장 생수', shape: 1, cap: 3, capColor: 5, color: -1, costCents: 100, priceCents: 300, paused: false, active: true }]);
  ok('회사·상품 만들기', typeof cid === 'string' && typeof pid === 'string' && !String(cid).startsWith('ERR'));
  const c0 = await cashOf(cid);
  ok('금고 600 토큰 (60000 센트)', c0 === 60000);
  ok('공장 추가: 500토큰을 내고 두 대가 돼요', (await call(p, 'labBuyFactory', [cid])) === 2 && (await cashOf(cid)) === 10000);
  const started = await call(p, 'labStartFactory', [cid, pid, 2]);
  ok('돌리기: 2개 시작', started === 2);
  ok('돌리고 있으면 또 못 돌려요', String(await call(p, 'labStartFactory', [cid, pid, 2])).startsWith('ERR'));
  ok('시작할 때는 재고도 금고도 그대로예요', (await stockOf(cid, pid)) === 0 && (await cashOf(cid)) === 10000);
  ok('바로 정산하면 아무것도 안 나와요', (await call(p, 'labSettleFactory', [cid])).units === 0);
  await wait(p, 35000); // 두 대라서 30초마다 한 개; 시계 여유 3초
  const r1 = await call(p, 'labSettleFactory', [cid]);
  ok('35초 뒤 한 개 정산 → 재고 1, 재료값 1 토큰', r1.units === 1 && (await stockOf(cid, pid)) === 1 && (await cashOf(cid)) === 9900);
  await wait(p, 32000);
  const r2 = await call(p, 'labSettleFactory', [cid]);
  console.log('  r2 =', JSON.stringify(r2), 'stock', await stockOf(cid, pid), 'cash', await cashOf(cid));
  ok('또 한 개 → 재고 2, 다 만들어서 공장이 쉬어요', r2.units === 1 && r2.left === 0 && (await stockOf(cid, pid)) === 2 && (await cashOf(cid)) === 9800);
  ok('다 만든 뒤에는 다시 돌릴 수 있어요', (await call(p, 'labStartFactory', [cid, pid, 1])) === 1);
  const errs = [];
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close();
  process.exit(res.every(Boolean) ? 0 : 1);
})();
