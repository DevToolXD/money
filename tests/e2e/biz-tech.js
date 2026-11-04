// 화면: 컴퓨터 회사. 팔 상품 정하기는 자기 업종(컴퓨터)만 나오고, 노트북을 골라 이름·가격을 정해 팔아요
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
const num = (f) => Number(f.integerValue ?? f.doubleValue);
// 탭 바는 두 쪽이에요 (사업 탭은 넘긴 쪽). 없으면 넘겨서 찾아요
const tab = async (p, t) => {
  for (let i = 0; i < 2; i++) {
    if (await p.evaluate((t) => { const b = document.querySelector(`.tabbar .tab[data-tab="${t}"]`); if (b) b.click(); return !!b; }, t)) return;
    await p.evaluate(() => window.tabbar.flipPage()); await p.waitForTimeout(600);
  }
};
const text = async (p, sel) => ((await p.textContent(sel).catch(() => '')) || '');
const shot = (p, n) => (process.env.SHOTS ? p.screenshot({ path: path.join(__dirname, 'shots', `biz-ui-${n}.png`) }) : null);
const okAlert = async (p) => { await p.waitForSelector('#alert-ok', { state: 'visible', timeout: 5000 }); await p.click('#alert-ok'); await p.waitForTimeout(500); };
const toastHas = async (p, t) => { for (let i = 0; i < 20; i++) { if ((await text(p, '.toast')).includes(t)) return true; await p.waitForTimeout(250); } return false; };
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const O = await open(b);
  const p = O.page;
  const sfx = Date.now().toString(36).slice(-4);
  await p.evaluate((u) => window.cloud.signUp(u, 'abc123', '사장'), 'tc' + sfx);
  await p.reload(); await p.waitForTimeout(6000);
  const uid = await p.evaluate(() => window.cloud.uid);
  await setBal(uid, 5000); await p.waitForTimeout(2300);
  await p.evaluate(() => window.cloud.foundCompany('테크 회사', '', 'pc'));
  await p.reload(); await p.waitForTimeout(6000);
  await tab(p, 'business'); await p.waitForTimeout(3000);
  ok('컴퓨터 회사도 팔 상품 정하기 버튼이 보여요', await p.isVisible('#biz-pick'));
  await p.click('#biz-pick'); await p.waitForTimeout(1200);
  ok('컴퓨터 칸부터 열려요 (식품·인공지능은 없어요)', (await text(p, '.pk-title b')) === '컴퓨터' && (await p.$$('.pk-stage [data-tech]')).length === 3 && !(await text(p, '.pk-stage')).includes('식품'));
  await shot(p, 'tech-list');
  await p.click('.pk-stage [data-tech="laptop"]'); await p.waitForTimeout(1200);
  ok('이름과 가격: 노트북, 원가 12 토큰', (await p.inputValue('#pk-name')) === '노트북' && (await text(p, '.pk-stage')).includes('12 토큰'));
  await shot(p, 'tech-sum');
  await p.click('#pk-sell');
  ok('팔기 시작 알림', await toastHas(p, '노트북을(를) 팔기 시작했어요'));
  await p.waitForTimeout(2500);
  ok('판매 목록에 노트북', (await text(p, '#biz-list')).includes('노트북'));
  ok('공장도 보여요', await p.isVisible('#biz-factory'));
  ok('화면 오류 없음', O.errs.length === 0); if (O.errs.length) console.log(O.errs);
  console.log(`${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close(); process.exit(res.every(Boolean) ? 0 : 1);
})();
