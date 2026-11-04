// 눈으로 확인하는 사진: 모든 화면과 시트 (사장 계정, 회사·상품·재고 있음). 시험이 아니에요
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const H = { 'content-type': 'application/json', authorization: 'Bearer owner' };
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--disable-web-security'] });
  const page = await (await b.newContext({ viewport: { width: 390, height: 1700 }, deviceScaleFactor: 1 })).newPage();
  const errs = []; page.on('pageerror', (e) => errs.push(e.message));
  await page.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await page.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await page.goto('http://127.0.0.1:8765/index.html'); await page.waitForTimeout(3500);
  await page.evaluate((u) => window.cloud.signUp(u, 'abc123', '사장님'), 'all' + Date.now().toString(36).slice(-4));
  await page.reload(); await page.waitForTimeout(6000);
  const uid = await page.evaluate(() => window.cloud.uid);
  await fetch(`${DB}/wallets/${uid}?updateMask.fieldPaths=bal`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { bal: { integerValue: '3000' } } }) });
  await page.waitForTimeout(2500);
  const cid = await page.evaluate(() => window.cloud.foundCompany('시험 식품', '', 'food'));
  await page.waitForTimeout(2300);
  await page.evaluate((c) => window.cloud.labCash(c, 600, 1), cid); await page.waitForTimeout(2300);
  for (const [name, shape] of [['시험 생수', 1], ['시험 과자', 7]]) {
    const pid = await page.evaluate(([c, n, sh]) => window.cloud.createProduct(c, { cat: 'food', group: 'water', item: 'water', name: n, shape: sh, cap: 3, capColor: 5, color: -1, costCents: 50, priceCents: 150, paused: false, active: true }), [cid, name, shape]);
    await fetch(`${DB}/companies/${cid}/products/${pid}?updateMask.fieldPaths=stock`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { stock: { integerValue: '5' } } }) });
  }
  await page.reload(); await page.waitForTimeout(6000);
  const out = process.env.OUT;
  const tab = async (t) => { for (let i = 0; i < 2; i++) { if (await page.evaluate((t) => { const x = document.querySelector(`.tabbar .tab[data-tab="${t}"]`); if (x) x.click(); return !!x; }, t)) return; await page.evaluate(() => window.tabbar.flipPage()); await page.waitForTimeout(600); } };
  for (const t of ['messages', 'money', 'work', 'shop', 'account', 'ranking', 'inventory', 'business']) {
    await tab(t); await page.waitForTimeout(2500);
    await page.screenshot({ path: path.join(out, `all-${t}.png`) });
  }
  // 넘침 찾기: 화면보다 넓은 요소
  const wide = await page.evaluate(() => [...document.querySelectorAll('.screen *')].filter((e) => { const r = e.getBoundingClientRect(); return r.width > 0 && (r.right > innerWidth + 1 || r.left < -1) && getComputedStyle(e).position !== 'fixed'; }).slice(0, 15).map((e) => e.className + ' ' + Math.round(e.getBoundingClientRect().left) + '..' + Math.round(e.getBoundingClientRect().right)));
  console.log('wide', JSON.stringify(wide));
  console.log('errs', JSON.stringify(errs.slice(0, 5)));
  await b.close();
})();
