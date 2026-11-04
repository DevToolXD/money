// 눈으로 확인하는 사진: 가입 → 돈·메시지·사업·일·계정 탭을 차례로 찍어요 (시험이 아니에요. run.sh 목록에 안 들어가요)
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const page = await (await b.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: Number(process.env.DSF || 1) })).newPage();
  page.on('pageerror', (e) => console.log('ERR', e.message));
  await page.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await page.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await page.goto('http://127.0.0.1:8765/index.html'); await page.waitForTimeout(3500);
  await page.evaluate((u) => window.cloud.signUp(u, 'abc123', '구경꾼'), 'tour' + Date.now().toString(36).slice(-4));
  await page.reload(); await page.waitForTimeout(6000);
  const uid = await page.evaluate(() => window.cloud.uid);
  await fetch(`${DB}/wallets/${uid}?updateMask.fieldPaths=bal`, { method: 'PATCH', headers: { 'content-type': 'application/json', authorization: 'Bearer owner' }, body: JSON.stringify({ fields: { bal: { integerValue: '1250' } } }) });
  await page.waitForTimeout(2500);
  const out = process.env.OUT || path.join(__dirname, 'shots');
  const tab = async (t) => { for (let i = 0; i < 2; i++) { if (await page.evaluate((t) => { const x = document.querySelector(`.tabbar .tab[data-tab="${t}"]`); if (x) x.click(); return !!x; }, t)) return; await page.evaluate(() => window.tabbar.flipPage()); await page.waitForTimeout(600); } };
  for (const t of ['messages', 'money', 'work', 'shop', 'account', 'business']) {
    await tab(t); await page.waitForTimeout(2200);
    await page.screenshot({ path: path.join(out, `tour-${t}.png`) });
  }
  await b.close();
})();
