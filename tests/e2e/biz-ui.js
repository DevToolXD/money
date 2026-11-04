// 화면: 사업 탭. 팔 상품 정하기(물: 병 → 뚜껑 → 뚜껑 색 → 이름·가격 → 팔기), 더보기(이름 바꾸기, 판매 멈추기),
// 직원 채용 숫자, 창고에서 꺼내기, 금고에 넣기, 원자력 발전 연구실
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
  await p.evaluate((u) => window.cloud.signUp(u, 'abc123', '사장'), 'bu' + sfx);
  await p.reload(); await p.waitForTimeout(6000);
  const uid = await p.evaluate(() => window.cloud.uid);
  await setBal(uid, 5000); await p.waitForTimeout(2300);
  const cid = await p.evaluate(() => window.cloud.foundCompany('사업 화면', '', 'food'));
  await p.reload(); await p.waitForTimeout(6000);
  await tab(p, 'business'); await p.waitForTimeout(3000);

  // 팔 상품 정하기
  ok('처음에는 파는 상품이 없어요', await p.isVisible('#biz-empty'));
  await p.click('#biz-pick'); await p.waitForTimeout(900);
  // 식품 회사는 식품 안의 것부터 바로 열려요 (다른 업종은 보이지 않아요)
  ok('정하기 화면이 식품부터 열려요', (await p.isVisible('.pk')) && (await text(p, '.pk-title b')) === '식품');
  await shot(p, '1-root');
  ok('식품 회사: 업종 동그라미는 없어요', (await p.$$('.pk-orb')).length === 0);
  ok('물·음식·에너지 식품', (await text(p, '.pk-stage')).includes('에너지 식품'));
  await p.click('.pk-view:last-child [data-node="water"]'); await p.waitForTimeout(900);
  ok('물: 페트병 모양 칸, 그림 50개', (await text(p, '.pk-title b')) === '페트병 모양' && (await p.$$('.pk-view:last-child .pk-tile')).length === 50);
  ok('고르기 전에는 확인을 못 눌러요', await p.isDisabled('.pk-ok'));
  await p.click('.pk-view:last-child .pk-tile[data-v="7"]'); await p.waitForTimeout(400);
  ok('하나 고르면 표시되고 미리보기가 나와요', (await p.$('.pk-view:last-child .pk-tile[data-v="7"].is-on')) !== null && (await p.$('.pk-prev.on .pkv')) !== null);
  await shot(p, '2-tiles');
  await p.click('.pk-ok'); await p.waitForTimeout(900);
  ok('확인: 뚜껑 모양으로 넘어가요', (await text(p, '.pk-title b')) === '뚜껑 모양' && (await text(p, '.pk-title span')).includes('물'));
  await p.click('.pk-view:last-child .pk-tile[data-v="12"]'); await p.waitForTimeout(300);
  await p.click('.pk-ok'); await p.waitForTimeout(900);
  ok('뚜껑 색 칸: 색 50개', (await text(p, '.pk-title b')) === '뚜껑 색' && (await p.$$('.pk-view:last-child .pk-sw')).length === 50);
  await p.click('.pk-view:last-child .pk-sw[data-v="3"]'); await p.waitForTimeout(300);
  await p.click('.pk-ok'); await p.waitForTimeout(900);
  ok('이름과 가격 정하기: 이름이 미리 채워져요', (await text(p, '.pk-title b')) === '이름과 가격 정하기' && (await p.inputValue('#pk-name')) === '생수 1L');
  await shot(p, '3-summary');
  const cost = await text(p, '.pk-cost-total');
  ok('원가 합계가 보여요', cost.includes('토큰'));
  ok('이익 글이 보여요', (await text(p, '#pk-profit')).includes('이익률'));
  await p.click('[data-step="100"]'); await p.waitForTimeout(200);
  const price1 = await p.inputValue('#pk-input');
  await p.click('[data-step="-100"]'); await p.waitForTimeout(200);
  ok('+1 / −1 이 가격을 바꿔요', Math.abs(Number(price1) - Number(await p.inputValue('#pk-input')) - 1) < 1e-9);
  await p.fill('#pk-name', '시험 생수');
  await p.fill('#pk-input', '0.01'); await p.waitForTimeout(200);
  ok('원가보다 낮으면 손해 글', (await text(p, '#pk-profit')).includes('낮아요'));
  await p.click('[data-suggest]'); await p.waitForTimeout(200);
  await p.click('#pk-sell');
  ok('팔기: 알림', await toastHas(p, '시험 생수을(를) 팔기 시작했어요'));
  await p.waitForTimeout(1500);
  ok('정하기 화면이 닫혀요', !(await p.isVisible('.pk')));
  ok('판매 목록에 새 상품', (await text(p, '#biz-list')).includes('시험 생수') && !(await p.isVisible('#biz-empty')));

  await shot(p, '4-list');
  // 더보기: 이름 바꾸기
  await p.click('#biz-list [data-more]'); await p.waitForTimeout(500);
  ok('더보기 시트: 세 가지', (await p.$$('.bz-sheet .bz-act')).length === 4);
  await p.click('.bz-sheet [data-id="rename"]'); await p.waitForTimeout(500);
  await p.fill('.bz-input', '새 생수');
  await p.click('.bz-card [data-k="ok"]');
  ok('이름을 바꿨어요', await toastHas(p, '이름을 바꿨어요'));
  ok('목록에도 새 이름', (await text(p, '#biz-list')).includes('새 생수'));
  await p.waitForTimeout(600);
  // 판매 멈추기 (두 번 물어봐요)
  await p.click('#biz-list [data-more]'); await p.waitForTimeout(500);
  await p.click('.bz-sheet [data-id="pause"]'); await p.waitForTimeout(500);
  await okAlert(p); await okAlert(p);
  ok('판매를 멈췄어요', await toastHas(p, '판매를 멈췄어요'));
  ok('목록에 판매 멈춤 표시', (await text(p, '#biz-list')).includes('판매 멈춤'));

  // 직원 채용
  const n0 = Number(await text(p, '#hire-num'));
  await p.click('#hire-plus'); await p.click('#hire-plus'); await p.waitForTimeout(200);
  ok('+ 두 번이면 2 늘어요', Number(await text(p, '#hire-num')) === n0 + 2);
  ok('맞추기 버튼 글', (await text(p, '#hire-go')).includes(`모집 인원 ${n0 + 2}명으로 맞추기`));
  await p.click('#hire-go');
  ok('모집 인원 알림', await toastHas(p, `모집 인원을 ${n0 + 2}명으로 맞췄어요`));
  await p.waitForTimeout(500);
  ok('모집 줄이 바뀌어요', (await text(p, '#hire-stat')).includes(`모집 중 ${n0 + 2}명`));

  // 창고: 재고를 넣고 꺼내기
  const pid = await p.evaluate((c) => window.cloud.listProducts(c).then((l) => l[0].id), cid);
  await fetch(`${DB}/companies/${cid}/products/${pid}?updateMask.fieldPaths=stock`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { stock: { integerValue: '4' } } }) });
  await tab(p, 'money'); await p.waitForTimeout(800); await tab(p, 'business'); await p.waitForTimeout(3000);
  ok('창고에 재고 4개', (await text(p, '#dash-list')).includes('재고 4개') && (await p.isVisible('#biz-dash-card')));
  await p.fill('#dash-list .dash-q', '3');
  await p.click('#dash-list [data-take]');
  ok('꺼내기 알림', await toastHas(p, '새 생수 3개를 인벤토리에 넣었어요'));
  await p.waitForTimeout(1500);
  ok('재고 1개로 줄어요', (await text(p, '#dash-list')).includes('재고 1개'));

  // 금고에 넣기
  ok('식품 회사 금고 줄', (await text(p, '#biz-lab')).includes('식품 회사 · 금고'));
  await p.fill('#cash-amt', '25');
  await p.click('#biz-lab [data-cash="in"]');
  ok('금고에 넣었어요', await toastHas(p, '25 토큰을 금고에 넣었어요'));
  await p.waitForTimeout(2500);
  ok('금고 숫자가 25', (await text(p, '#biz-lab .lab-cash')).includes('25'));
  const lab = await (await fetch(`${DB}/companies/${cid}/lab/main`, { headers: H })).json();
  ok('서버 금고도 2500센트', num(lab.fields.cash) === 2500);

  // 원자력 회사: 발전 연구실
  await p.evaluate(() => window.cloud.foundCompany('사업 원자력', '', 'nuclear')); await p.waitForTimeout(2500);
  await p.reload(); await p.waitForTimeout(6000);
  await tab(p, 'business'); await p.waitForTimeout(2500);
  const nucBtn = await p.$('#biz-switch [data-co]:not([aria-pressed="true"])');
  if (nucBtn && !(await text(p, '#biz-name')).includes('원자력')) { await nucBtn.click(); await p.waitForTimeout(3000); }
  await shot(p, '5-nuclear');
  ok('원자력: 상품 목록은 숨어요', !(await p.isVisible('#biz-prod-title')));
  ok('원자력: 전기 연구실', (await text(p, '#biz-lab')).includes('원자력 발전 · 상품 1개: 전기'));
  await p.fill('#lab-amt', '50'); await p.waitForTimeout(200);
  ok('만들 전기를 적으면 비용이 바뀌어요', (await text(p, '#lab-est')).includes('비용'));
  await p.fill('#lab-amt', '9999999'); await p.waitForTimeout(200);
  ok('숫자는 6자리까지만', (await p.inputValue('#lab-amt')).length <= 6);
  ok('인공지능 사용 칸', (await text(p, '#lab-aiuse')).length > 0);

  const errs = O.errs;
  ok('화면 오류 없음', errs.length === 0); if (errs.length) console.log(errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close(); process.exit(res.every(Boolean) ? 0 : 1);
})();
