// 화면: 일 탭 (러스트). 시장 목록 → 회사를 눌러 지원 → 내 회사 화면(배합 다듬기) → 그만두기 → 다시 시장.
// 원자력 회사 사장은 배합 카드가 보여요
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const H = { 'content-type': 'application/json', authorization: 'Bearer owner' };
const open = async (b) => {
  const page = await (await b.newContext({ viewport: { width: 390, height: 844 } })).newPage();
  const errs = []; page.on('pageerror', (e) => errs.push(e.message));
  await page.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await page.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await page.goto('http://127.0.0.1:8765/index.html'); await page.waitForTimeout(3500);
  return { page, errs };
};
const setBal = (uid, bal) => fetch(`${DB}/wallets/${uid}?updateMask.fieldPaths=bal`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { bal: { integerValue: String(bal) } } }) });
const tab = (p, t) => p.evaluate((t) => document.querySelector(`.tabbar .tab[data-tab="${t}"]`).click(), t);
const text = async (p, sel) => ((await p.textContent(sel).catch(() => '')) || '');
const okAlert = async (p) => { await p.waitForSelector('#alert-ok', { state: 'visible', timeout: 5000 }); await p.click('#alert-ok'); };
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const O = await open(b), S = await open(b);
  const sfx = Date.now().toString(36).slice(-4);
  await O.page.evaluate((u) => window.cloud.signUp(u, 'abc123', '사장'), 'wo' + sfx);
  await O.page.reload(); await O.page.waitForTimeout(6000);
  const uO = await O.page.evaluate(() => window.cloud.uid);
  await setBal(uO, 1000); await O.page.waitForTimeout(2300);
  const cid = await O.page.evaluate(() => window.cloud.foundCompany('화면 식품', '', 'food'));
  await O.page.waitForTimeout(2300);
  await O.page.evaluate((c) => window.cloud.setOpenings(c, 1), cid);

  await S.page.evaluate((u) => window.cloud.signUp(u, 'abc123', '직원'), 'ws' + sfx);
  await S.page.reload(); await S.page.waitForTimeout(6000);
  await tab(S.page, 'work'); await S.page.waitForTimeout(3000);
  ok('시장: 회사 목록에 새 회사가 보여요', (await text(S.page, '#wk-list')).includes('화면 식품'));
  ok('시장: 자리 1이 보여요', (await text(S.page, `#wk-list .wk-row[data-id="${cid}"] .wk-tag`)).includes('자리 1'));
  await S.page.fill('#wk-q', '없는회사이름'); await S.page.waitForTimeout(1500);
  ok('검색: 없는 이름이면 목록에 안 보여요', !(await text(S.page, '#wk-list')).includes('화면 식품'));
  await S.page.fill('#wk-q', '화면'); await S.page.waitForTimeout(1500);
  ok('검색: 이름으로 찾으면 다시 보여요', (await text(S.page, '#wk-list')).includes('화면 식품'));
  await S.page.click(`#wk-list .wk-row[data-id="${cid}"]`); await S.page.waitForTimeout(700);
  ok('회사를 누르면 지원 시트가 열려요', await S.page.isVisible('.wk-sheet .wk-apply'));
  await S.page.click('.wk-sheet .wk-apply');
  await okAlert(S.page); await S.page.waitForTimeout(4000);
  ok('지원하면 알림이 떠요', (await text(S.page, '.toast')).includes('화면 식품에 입사했어요'));
  ok('입사하면 내 회사 화면(직원 · 식품)이 보여요', (await text(S.page, '#work-root .wk-role')).includes('직원'));
  ok('배합 다듬기 버튼이 보여요', await S.page.isVisible('#wk-tune'));
  const fp0 = Number((await text(S.page, '#lab-fp')).replace(/\D/g, '')) || 0;
  await S.page.click('#wk-tune'); await S.page.waitForTimeout(3500);
  const fp1 = Number((await text(S.page, '#lab-fp')).replace(/\D/g, '')) || 0;
  ok(`배합 다듬기를 누르면 식품 성능이 올라요 (${fp0} → ${fp1})`, fp1 === fp0 + 1);
  await S.page.click('.wk-quit');
  await okAlert(S.page); await S.page.waitForTimeout(600);
  await okAlert(S.page); await S.page.waitForTimeout(4000);
  ok('그만두면 시장 화면으로 돌아와요', await S.page.isVisible('#wk-list'));

  // 원자력 회사 사장: 배합 카드
  const N = await open(b);
  await N.page.evaluate((u) => window.cloud.signUp(u, 'abc123', '원장'), 'wn' + sfx);
  await N.page.reload(); await N.page.waitForTimeout(6000);
  const uN = await N.page.evaluate(() => window.cloud.uid);
  await setBal(uN, 1000); await N.page.waitForTimeout(2300);
  await N.page.evaluate(() => window.cloud.foundCompany('화면 원자력', '', 'nuclear'));
  await N.page.reload(); await N.page.waitForTimeout(6000);
  await tab(N.page, 'work'); await N.page.waitForTimeout(3500);
  ok('원자력 사장: 사장 · 원자력 화면', (await text(N.page, '#work-root .wk-role')).includes('사장'));
  ok('원자력 사장: 플라스크 배합 카드가 보여요', (await N.page.$('#work-root .lab-liquid')) !== null);

  const errs = [...O.errs, ...S.errs, ...N.errs];
  ok('화면 오류 없음', errs.length === 0); if (errs.length) console.log(errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close(); process.exit(res.every(Boolean) ? 0 : 1);
})();
