// 화면: 상점에 식품·장신구·거래 칸이 있어요. 칸을 바꾸면 목록이 바뀌어요
const { chromium } = require('playwright');
const fs = require('fs');
const emu = fs.readFileSync(require('path').join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const page = await (await b.newContext({ viewport: { width: 390, height: 844 } })).newPage();
  const errs = []; page.on('pageerror', (e) => errs.push(e.message));
  await page.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await page.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await page.goto('http://127.0.0.1:8765/index.html'); await page.waitForTimeout(3500);
  const id = 'cat' + Date.now().toString(36).slice(-4);
  await page.evaluate((u) => window.cloud.signUp(u, 'abc123', '손님'), id);
  await page.reload(); await page.waitForTimeout(6000);
  await page.evaluate(() => document.querySelector('.tabbar .tab[data-tab="shop"]').click()); await page.waitForTimeout(2000);
  const labels = await page.$$eval('.shop-cats [data-cat]', (els) => els.map((e) => e.textContent.trim()));
  ok('칸은 식품·장신구·거래 세 개예요', JSON.stringify(labels) === JSON.stringify(['식품', '장신구', '거래']));
  await page.click('.shop-cats [data-cat="acc"]'); await page.waitForTimeout(1500);
  ok('장신구 칸은 곧 들어와요', (await page.textContent('#shop-root')).includes('곧 들어와요'));
  await page.click('.shop-cats [data-cat="trade"]'); await page.waitForTimeout(1500);
  ok('거래 칸을 고르면 거래 안내가 나와요', (await page.textContent('#shop-root')).includes('거래'));
  ok('화면 오류 없음', errs.length === 0); if (errs.length) console.log(errs.slice(0, 2));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close(); process.exit(res.every(Boolean) ? 0 : 1);
})();
