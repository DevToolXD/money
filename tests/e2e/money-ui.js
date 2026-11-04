// 화면: 돈 탭. 잔액, 이자 상품(가입·가격·토큰 부족), 거래내역, 카드 꾹 눌러 순서 바꾸기(이 기기에 저장)
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
const num = (f) => Number(f.integerValue ?? f.doubleValue);
const wallet = async (uid) => (await (await fetch(`${DB}/wallets/${uid}`, { headers: H })).json()).fields;
const setBal = (uid, bal) => fetch(`${DB}/wallets/${uid}?updateMask.fieldPaths=bal`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { bal: { integerValue: String(bal) } } }) });
const tab = async (p, t) => {
  for (let i = 0; i < 2; i++) {
    if (await p.evaluate((t) => { const b = document.querySelector(`.tabbar .tab[data-tab="${t}"]`); if (b) b.click(); return !!b; }, t)) return;
    await p.evaluate(() => window.tabbar.flipPage()); await p.waitForTimeout(600);
  }
};
const text = async (p, sel) => ((await p.textContent(sel).catch(() => '')) || '');
const toastHas = async (p, t) => { for (let i = 0; i < 24; i++) { if ((await text(p, '.toast')).includes(t)) return true; await p.waitForTimeout(250); } return false; };
const order = (p) => p.evaluate(() => [...document.querySelectorAll('#movable > .movable')].map((c) => c.dataset.id).join(','));
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const O = await open(b);
  const p = O.page;
  const sfx = Date.now().toString(36).slice(-4);
  await p.evaluate((u) => window.cloud.signUp(u, 'abc123', '돈주인'), 'mo' + sfx);
  await p.reload(); await p.waitForTimeout(6000);
  const uid = await p.evaluate(() => window.cloud.uid);
  await setBal(uid, 100); await p.waitForTimeout(2500);
  await tab(p, 'money'); await p.waitForTimeout(1200);
  ok('잔액 100 토큰', (await text(p, '#bal-num')).replace(/\s/g, '').includes('100토큰'));
  ok('가입한 상품이 없어요', (await text(p, '#bank-rate')) === '가입한 상품이 없어요');
  ok('5% 상품 가격 안내', (await text(p, '#bank-note')).includes('연 5% 상품 20 토큰부터'));
  ok('거래내역이 비어 있어요', (await text(p, '#tx-top')).includes('아직 거래내역이 없어요'));
  ok('카드 두 장: 이자 상품, 최근 거래내역', (await order(p)) === 'bank,hist');

  // 이자 상품 시트
  await p.click('#open-bank'); await p.waitForTimeout(700);
  ok('이자 상품 시트가 열려요', await p.evaluate(() => document.getElementById('bank-sheet').classList.contains('is-open')));
  ok('내 토큰 100', (await text(p, '#bank-bal')).includes('100 토큰'));
  ok('상품 10개', (await p.$$('#bank-list .prod')).length === 10);
  const labels = await p.$$eval('#bank-list [data-buy]', (bs) => bs.map((b) => b.textContent.trim()));
  ok('5%·10%는 구입, 15%부터 토큰 부족', labels[0] === '구입' && labels[1] === '구입' && labels[2] === '토큰 부족');
  ok('10% 가격 70 토큰', (await text(p, '#bank-list .prod:nth-child(2) .prod-price')) === '70 토큰');
  await p.click('#bank-list [data-buy="5"]');
  await p.waitForSelector('#alert-ok', { state: 'visible' }); await p.click('#alert-ok');
  ok('가입 알림', await toastHas(p, '연 5% 이자 상품에 가입했어요'));
  await p.waitForTimeout(2500);
  const w = await wallet(uid);
  ok('서버 지갑: 5%, 80 토큰', num(w.rate) === 5 && num(w.bal) === 80);
  ok('버튼이 가입 중으로', (await text(p, '#bank-list [data-buy="5"]')).trim() === '가입 중');
  await p.click('#bank-sheet [data-close]').catch(() => p.evaluate(() => document.querySelector('#dim').click()));
  await p.waitForTimeout(800);
  ok('이자율 표시', (await text(p, '#bank-rate')) === '연 5% 복리');
  ok('다음 이자 안내', (await text(p, '#bank-note')).includes('다음 이자까지'));
  ok('잔액 80', (await text(p, '#bal-num')).replace(/\s/g, '').includes('80토큰'));
  ok('최근 거래내역에 구입 기록', (await text(p, '#tx-top')).includes('이자 상품 구입') && (await text(p, '#tx-top')).includes('-20'));

  // 거래내역 시트
  await p.click('#ac-search'); await p.waitForTimeout(700);
  ok('거래내역 시트', (await text(p, '#history-list')).includes('이자 상품 구입'));
  await p.evaluate(() => document.querySelector('#history-sheet [data-close]')?.click() || document.getElementById('dim').click());
  await p.waitForTimeout(800);

  // 꾹 눌러 순서 바꾸기
  const hb = await p.locator('#movable .movable[data-id="hist"] h2').boundingBox();
  const bb = await p.locator('#movable .movable[data-id="bank"] h2').boundingBox();
  await p.mouse.move(hb.x + 40, hb.y + 10); await p.mouse.down(); await p.waitForTimeout(700);
  for (let i = 1; i <= 10; i++) { await p.mouse.move(hb.x + 40, hb.y + 10 - ((hb.y - bb.y + 30) * i) / 10); await p.waitForTimeout(40); }
  await p.waitForTimeout(300); await p.mouse.up(); await p.waitForTimeout(600);
  ok('카드 순서가 바뀌어요', (await order(p)) === 'hist,bank');
  ok('순서를 이 기기에 저장', (await p.evaluate(() => localStorage.getItem('money-order'))) === 'hist,bank');
  await p.reload(); await p.waitForTimeout(5000);
  ok('다시 열어도 같은 순서', (await order(p)) === 'hist,bank');
  ok('다시 열어도 거래내역이 남아요 (이 기기)', (await text(p, '#tx-top')).includes('이자 상품 구입'));

  const errs = O.errs;
  ok('화면 오류 없음', errs.length === 0); if (errs.length) console.log(errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close(); process.exit(res.every(Boolean) ? 0 : 1);
})();
