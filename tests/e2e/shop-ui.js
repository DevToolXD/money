// 화면: 메뉴 → 상점 (애플 스토어 "제품" 탭처럼). 카테고리·칩, 카드를 누르면 화면 가득 커지고 옆으로 넘기고 끌어내려 닫기,
// 멸공으로 사기(같은 것은 한 번만), 인벤에서 산 것만 보이고 여러 개 같이 끼기
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const H = { 'content-type': 'application/json', authorization: 'Bearer owner' };
const raw = async (p) => (await (await fetch(`${DB}/${p}`, { headers: H })).json());
const int = (f) => Number(f?.integerValue ?? f?.doubleValue ?? 0);
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const p = await (await b.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2 })).newPage(); p.errs = []; p.on('pageerror', (e) => p.errs.push(e.message));
  await p.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await p.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await p.goto('http://127.0.0.1:8765/index.html'); await p.waitForTimeout(3500);
  await p.evaluate((u) => window.cloud.signUp(u, 'abc123', '후교'), 'sh' + Date.now().toString(36).slice(-4));
  await p.reload(); await p.waitForTimeout(6000);
  const uid = await p.evaluate(() => window.cloud.uid);
  await fetch(`${DB}/wallets/${uid}?updateMask.fieldPaths=bal`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { bal: { integerValue: '5000' } } }) });
  await p.waitForTimeout(1500);
  const toast = async (t) => { for (let i = 0; i < 60; i++) { if (((await p.textContent('.toast').catch(() => '')) || '').includes(t)) return true; await p.waitForTimeout(250); } return false; };
  const menu = async (go) => { await p.evaluate(() => document.querySelector('.tabbar .tab-account').click()); await p.waitForTimeout(500); await p.click(`.qm-row[data-go="${go}"]`); await p.waitForTimeout(2200); };
  await p.evaluate(() => document.querySelector('.tabbar .tab-account').click()); await p.waitForTimeout(500);
  const rows = await p.evaluate(() => [...document.querySelectorAll('.qm-row')].map((e) => e.textContent));
  ok('메뉴: 상점·인벤 (뽑기 없음)', rows.includes('상점') && rows.includes('인벤') && !rows.includes('뽑기'));
  await p.click('.qm-row[data-go="shop"]'); await p.waitForTimeout(2500);
  ok('상점: 큰 제목, 카테고리 6칸, 칩, 카드 13장', (await p.textContent('#title-shop')) === '상점' && (await p.$$('.sh-cat')).length === 6 && (await p.$$('.sh-chip')).length === 4 && (await p.$$('#sh-row .sh-card')).length === 13);
  await p.click('.sh-cat[data-cat="face"]'); await p.waitForTimeout(400);
  ok('카테고리: 얼굴만 (선글라스·안경)', (await p.$$('#sh-row .sh-card')).length === 2);
  await p.click('.sh-cat[data-cat="all"]'); await p.click('.sh-chip[data-chip="cheap"]'); await p.waitForTimeout(400);
  ok('칩: 1,000 멸공 이하', (await p.$$('#sh-row .sh-card')).length === 6 && (await p.textContent('.sh-h.red')).includes('1,000 멸공 이하'));
  await p.click('.sh-chip[data-chip="cheap"]'); await p.waitForTimeout(400);
  // 크게 보기
  const c = await p.locator('#sh-row .sh-card[data-k="cap"]').boundingBox();
  await p.evaluate(() => document.querySelector('#sh-row').scrollLeft = 0);
  await p.click('#sh-row .sh-card[data-k="lambo"] .sh-pic'); await p.waitForTimeout(700);
  const big = await p.locator('#sh-pager .sh-card[data-k="lambo"]').boundingBox();
  ok('카드를 누르면 화면 가득 커져요', await p.isVisible('#sh-pager') && big.height > 550 && big.x >= 15 && big.x <= 25);
  await p.screenshot({ path: path.join(__dirname, 'shots', 'shop-big.png') });
  await p.mouse.move(330, 400); await p.mouse.down(); for (let i = 1; i <= 8; i++) { await p.mouse.move(330 - i * 30, 400); await p.waitForTimeout(16); } await p.mouse.up(); await p.waitForTimeout(600);
  const g = await p.locator('#sh-pager .sh-card[data-k="goldbar"]').boundingBox();
  ok('옆으로 넘기면 다음 카드가 제자리에', g.x >= 15 && g.x <= 25);
  await p.mouse.move(200, 300); await p.mouse.down(); for (let i = 1; i <= 8; i++) { await p.mouse.move(200, 300 + i * 25); await p.waitForTimeout(16); } await p.mouse.up(); await p.waitForTimeout(800);
  ok('끌어내리면 닫혀요', await p.isHidden('#sh-pager'));
  await p.screenshot({ path: path.join(__dirname, 'shots', 'shop.png') });
  // 사기
  await p.evaluate(() => document.querySelector('#sh-row').scrollLeft = 2000); await p.waitForTimeout(300);
  await p.click('#sh-row [data-buy="cap"]'); await p.waitForTimeout(600);
  ok('사기 전에 값을 물어요', (await p.textContent('#alert-title')).includes('야구모자') && (await p.textContent('#alert-body')).includes('900'));
  await p.click('#alert-ok');
  ok('샀어요', await toast('야구모자를 샀어요'));
  await p.waitForTimeout(1500);
  const bag = ((await raw(`bag/${uid}`)).fields?.items?.arrayValue?.values || []).map((v) => v.stringValue);
  ok('서버: 지갑 4,100, 가방에 모자', int((await raw(`wallets/${uid}`)).fields?.bal) === 4100 && bag.join() === 'cap');
  ok('산 카드는 보유 중', (await p.textContent('#sh-row [data-buy="cap"]')) === '보유 중' && await p.isDisabled('#sh-row [data-buy="cap"]'));
  await p.waitForTimeout(2300);
  await p.click('#sh-row [data-buy="glasses"]'); await p.waitForTimeout(600); await p.click('#alert-ok');
  await toast('안경을 샀어요'); await p.waitForTimeout(2300);
  await p.click('#sh-row .sh-card[data-k="lambo"] .sh-pic').catch(() => {}); await p.waitForTimeout(700);
  await p.click('#sh-pager [data-buy="lambo"]'); await p.waitForTimeout(600); await p.click('#alert-ok');
  let short = false; for (let i = 0; i < 40 && !short; i++) { await p.waitForTimeout(250); short = (await p.textContent('#alert-title')).includes('살 수 없어요') && (await p.textContent('#alert-body')).includes('모자라'); }
  ok('멸공이 모자라면 못 사요', short && int((await raw(`wallets/${uid}`)).fields?.bal) === 3300);
  await p.evaluate(() => document.getElementById('alert-ok').click()); await p.waitForTimeout(600);
  await p.click('#sh-pager [data-close]'); await p.waitForTimeout(800);
  // 인벤
  await menu('inv');
  ok('인벤: 산 것만 (2개)', (await p.$$('#inv-root .iv-card')).length === 2);
  for (const k of ['glasses', 'cap']) { await p.click(`#inv-root [data-act="wear"][data-item="${k}"]`); await toast('꼈어요'); await p.waitForTimeout(1500); }
  const gear = ((await raw(`profiles/${uid}`)).fields?.gear?.arrayValue?.values || []).map((v) => v.stringValue);
  ok('여러 개 같이 끼기', gear.includes('glasses') && gear.includes('cap') && await p.isVisible('#inv-root .iv-hero .dc-glasses') && await p.isVisible('#inv-root .iv-hero .dc-cap'));
  await p.evaluate(() => window.cloud.postCreate('장신구 자랑', '', '', ''));
  await p.evaluate(() => document.querySelector('.tabbar .tab[data-tab="untitled3"]').click()); await p.waitForTimeout(3000);
  ok('SNS 게시물 내 동그라미에 장신구', await p.isVisible('#sns-feed .dc-glasses') && await p.isVisible('#sns-feed .dc-cap'));
  ok('화면 오류 없음', p.errs.length === 0); if (p.errs.length) console.log(p.errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close();
  process.exit(res.every(Boolean) ? 0 : 1);
})();
