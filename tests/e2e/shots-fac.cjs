// 눈으로 확인: 사업 탭 아래쪽(공장·창고)과 상점 줄. 시험이 아니에요
const { chromium } = require('playwright');
const fs = require('fs'); const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const H = { 'content-type': 'application/json', authorization: 'Bearer owner' };
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const page = await (await b.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2 })).newPage();
  await page.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await page.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await page.goto('http://127.0.0.1:8765/index.html'); await page.waitForTimeout(3500);
  await page.evaluate((u) => window.cloud.signUp(u, 'abc123', '사장님'), 'fac' + Date.now().toString(36).slice(-4));
  await page.reload(); await page.waitForTimeout(6000);
  const uid = await page.evaluate(() => window.cloud.uid);
  await fetch(`${DB}/wallets/${uid}?updateMask.fieldPaths=bal`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { bal: { integerValue: '3000' } } }) });
  await page.waitForTimeout(2500);
  const cid = await page.evaluate(() => window.cloud.foundCompany('시험 식품', '', 'food'));
  await page.waitForTimeout(2300);
  for (const n of ['시험 생수', '시험 과자']) {
    const pid = await page.evaluate(([c, n]) => window.cloud.createProduct(c, { cat: 'food', group: 'water', item: 'water', name: n, shape: 1, cap: 3, capColor: 5, color: -1, costCents: 50, priceCents: 150, paused: false, active: true }), [cid, n]);
    await fetch(`${DB}/companies/${cid}/products/${pid}?updateMask.fieldPaths=stock`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { stock: { integerValue: '5' } } }) });
  }
  await page.reload(); await page.waitForTimeout(6000);
  await page.evaluate(() => window.tabbar.flipPage()); await page.waitForTimeout(600);
  await page.evaluate(() => document.querySelector('.tabbar .tab[data-tab="business"]').click()); await page.waitForTimeout(3000);
  for (const [i, sel] of [[1, '#biz-factory'], [2, '#biz-dash-card'], [3, '#biz-hire']]) {
    await page.evaluate((s) => document.querySelector(s)?.scrollIntoView({ block: 'center' }), sel); await page.waitForTimeout(500);
    await page.screenshot({ path: path.join(process.env.OUT, `fac${i}.png`) });
  }
  await b.close();
})();
