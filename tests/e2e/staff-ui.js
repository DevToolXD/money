// 화면+서버: 사장이 직원 목록을 보고 시급을 정하고 급여 금고를 채우고, 직원이 임금을 받고, 해고하고, 회사 이름·마크를 바꿔요
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const open = async (b) => {
  const page = await (await b.newContext({ viewport: { width: 390, height: 844 } })).newPage();
  page.errs = []; page.on('pageerror', (e) => page.errs.push(e.message));
  await page.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await page.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await page.goto('http://127.0.0.1:8765/index.html'); await page.waitForTimeout(3500);
  return page;
};
const patch = (p, mask, fields) => fetch(`${DB}/${p}?${mask.map((m) => 'updateMask.fieldPaths=' + m).join('&')}`, { method: 'PATCH', headers: { 'content-type': 'application/json', authorization: 'Bearer owner' }, body: JSON.stringify({ fields }) });
const raw = async (p) => (await (await fetch(`${DB}/${p}`, { headers: { authorization: 'Bearer owner' } })).json());
const call = (p, fn, args) => p.evaluate(([f, a]) => window.cloud[f](...a).then((r) => (r === undefined ? 'ok' : r), (e) => 'ERR ' + (e.code || e.message)), [fn, args]);
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--disable-web-security'] });
  const sfx = Date.now().toString(36).slice(-4);
  const O = await open(b); await O.evaluate((u) => window.cloud.signUp(u, 'abc123', '대표'), 'stb' + sfx); await O.reload(); await O.waitForTimeout(6000);
  const S = await open(b); await S.evaluate((u) => window.cloud.signUp(u, 'abc123', '일꾼'), 'sts' + sfx); await S.reload(); await S.waitForTimeout(6000);
  const uO = await O.evaluate(() => window.cloud.uid), uS = await S.evaluate(() => window.cloud.uid);
  await patch(`wallets/${uO}`, ['bal'], { bal: { integerValue: '1000' } }); await O.waitForTimeout(2500);
  const cid = await call(O, 'foundCompany', ['월급회사', '', 'food']); await O.waitForTimeout(2500);
  await call(O, 'setOpenings', [cid, 2]); await call(S, 'applyJob', [cid, 'food']); await S.waitForTimeout(2500);
  await O.reload(); await O.waitForTimeout(6000);
  await O.evaluate(() => window.tabbar.flipPage()); await O.waitForTimeout(500);
  await O.evaluate(() => document.querySelector('.tabbar .tab[data-tab="business"]').click()); await O.waitForTimeout(3500);
  ok('직원 관리 칸이 보여요', await O.isVisible('#biz-staff'));
  ok('직원이 목록에 있어요', (await O.textContent('#biz-staff')).includes('일꾼'));
  ok('발전도가 보여요', (await O.textContent('#biz-staff')).includes('회사 발전'));
  // 시급 정하기
  await O.fill('#biz-staff .stf-wage input', '3'); await O.press('#biz-staff .stf-wage input', 'Tab'); await O.waitForTimeout(1500);
  ok('시급이 서버에 적혀요', Number((await raw(`jobs/${uS}`)).fields.wage.integerValue) === 3);
  // 급여 금고 채우기
  await O.fill('#stf-fund-n', '50'); await O.click('#biz-staff [data-act="fund"]'); await O.waitForTimeout(2500);
  ok('급여 금고가 5000센트', Number((await raw(`companies/${cid}`)).fields.pay.integerValue) === 5000);
  ok('금고 숫자가 화면에 보여요', (await O.textContent('#biz-staff')).includes('50 토큰'));
  await O.evaluate(() => document.querySelector('#biz-staff').scrollIntoView({ block: 'start' })); await O.waitForTimeout(500);
  await O.screenshot({ path: path.join(__dirname, 'shots', 'staff.png') });
  // 3시간 전에 입사한 것으로 바꾸고 직원이 일 탭을 열면 임금을 받아요
  await patch(`jobs/${uS}`, ['at'], { at: { timestampValue: new Date(Date.now() - 3.5 * 3600e3).toISOString() } });
  const bal0 = Number((await raw(`wallets/${uS}`)).fields.bal.integerValue);
  await S.reload(); await S.waitForTimeout(6000);
  await S.evaluate(() => document.querySelector('.tabbar .tab[data-tab="work"]').click()); await S.waitForTimeout(4500);
  ok('직원 임금 칸', (await S.textContent('#wk-wage')).includes('시급 3 토큰'));
  const bal1 = Number((await raw(`wallets/${uS}`)).fields.bal.integerValue);
  ok('3시간 × 3토큰 = 9 토큰을 받아요', bal1 - bal0 === 9);
  ok('급여 금고에서 9 토큰이 빠져요', Number((await raw(`companies/${cid}`)).fields.pay.integerValue) === 5000 - 900);
  // 일한 횟수 (사장 화면)
  await S.evaluate(() => window.cloud.trackWork(40)).catch(() => {});
  await O.evaluate(() => document.querySelector('#biz-staff')).catch(() => {});
  // 해고 (두 번 확인)
  await O.evaluate(() => { window.__fireTest = true; });
  await O.click('#biz-staff [data-act="fire"]'); await O.waitForTimeout(700);
  await O.click('.alert-btn:has-text("해고하기")'); await O.waitForTimeout(700);
  await O.click('.alert-btn:has-text("네, 해고할게요")'); await O.waitForTimeout(2500);
  ok('해고하면 직원이 사라져요', !(await O.textContent('#biz-staff')).includes('일꾼'));
  ok('직원 수가 0', Number((await raw(`companies/${cid}`)).fields.staff.integerValue) === 0);
  ok('일자리 기록도 지워져요', (await raw(`jobs/${uS}`)).error !== undefined);
  // 회사 이름 편집
  await O.click('#biz-logo'); await O.waitForTimeout(800);
  await O.fill('#co-name-in', '새 월급회사'); await O.click('#co-edit-save'); await O.waitForTimeout(2000);
  ok('회사 이름이 바뀌어요', (await O.textContent('#biz-name')).includes('새 월급회사'));
  ok('서버에도 적혀요', (await raw(`companies/${cid}`)).fields.name.stringValue === '새 월급회사');
  ok('화면 오류 없음', O.errs.length + S.errs.length === 0); if (O.errs.length + S.errs.length) console.log(O.errs.concat(S.errs).slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close(); process.exit(res.every(Boolean) ? 0 : 1);
})();
