// 화면: 메뉴 → 상점 (프레임 11종(vote 저장소의 일반 프레임), 값은 모두 100 멸공). 카테고리, 카드를 누르면 화면 가득 커지고 옆으로 넘기고 끌어내려 닫기,
// 멸공으로 사기(같은 것은 한 번만), 인벤에서 산 것만 보이고 프레임은 한 번에 하나만 걸기
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
  await fetch(`${DB}/wallets/${uid}?updateMask.fieldPaths=bal`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { bal: { integerValue: '450' } } }) });
  await p.waitForTimeout(1500);
  const toast = async (t) => { for (let i = 0; i < 60; i++) { if (((await p.textContent('.toast').catch(() => '')) || '').includes(t)) return true; await p.waitForTimeout(250); } return false; };
  const menu = async (go) => { await p.evaluate(() => document.querySelector('.tabbar .tab-account').click()); await p.waitForTimeout(500); await p.click(`.qm-row[data-go="${go}"]`); await p.waitForTimeout(2200); };
  await p.evaluate(() => document.querySelector('.tabbar .tab-account').click()); await p.waitForTimeout(500);
  const rows = await p.evaluate(() => [...document.querySelectorAll('.qm-row')].map((e) => e.textContent));
  ok('메뉴: 상점·인벤', rows.includes('상점') && rows.includes('인벤'));
  await p.click('.qm-row[data-go="shop"]'); await p.waitForTimeout(2500);
  ok('상점: 큰 제목, 카테고리 4칸, 프레임 11장', (await p.textContent('#title-shop')) === '상점' && (await p.$$('.sh-cat')).length === 4 && (await p.$$('#sh-row .sh-card')).length === 11);
  const names = await p.evaluate(() => [...document.querySelectorAll('#sh-row .sh-card')].map((c) => c.querySelector('b').textContent + '|' + c.querySelector('.sh-foot span').textContent));
  ok('이름과 값(모두 100 멸공)', names.join(' / ') === ['네온', '별빛', '벚꽃', '불꽃', '파도', '나비', '왕관', '고양이', '토끼', '천사', '악마'].map((n) => n + ' 프레임|100 멸공').join(' / '));
  ok('프레임 그림 11장이 모두 있어요 (vote 저장소의 그림)', await p.evaluate(() => { const d = [...document.querySelectorAll('#sh-row .fr-demo')]; return d.length === 11 && d.every((x) => x.querySelector('.da-b').innerHTML.length > 100); }));
  ok('미리보기는 내 프로필 (사진이 없으면 내 이름 글자)', await p.evaluate(() => [...document.querySelectorAll('#sh-row .fr-ava')].length === 11 && [...document.querySelectorAll('#sh-row .fr-ava')].every((e) => e.textContent === '후교')));
  ok('프레임이 움직여요 (애니메이션)', await p.evaluate(() => [...document.querySelectorAll('#sh-row .fr-demo *')].some((e) => getComputedStyle(e).animationName.startsWith('av'))));
  await p.click('.sh-cat[data-cat="ring"]'); await p.waitForTimeout(400);
  ok('카테고리: 링 2개', (await p.$$('#sh-row .sh-card')).length === 2);
  await p.click('.sh-cat[data-cat="nature"]'); await p.waitForTimeout(400);
  ok('카테고리: 자연 4개', (await p.$$('#sh-row .sh-card')).length === 4);
  await p.click('.sh-cat[data-cat="head"]'); await p.waitForTimeout(400);
  ok('카테고리: 머리 장식 5개', (await p.$$('#sh-row .sh-card')).length === 5);
  await p.click('.sh-cat[data-cat="all"]'); await p.waitForTimeout(400);
  // 크게 보기
  await p.evaluate(() => document.querySelector('#sh-row').scrollLeft = 0);
  await p.click('#sh-row .sh-card[data-k="neon"] .sh-pic'); await p.waitForTimeout(700);
  const big = await p.locator('#sh-pager .sh-card[data-k="neon"]').boundingBox();
  ok('카드를 누르면 화면 가득 커져요', await p.isVisible('#sh-pager') && big.height > 550 && big.x >= 15 && big.x <= 25);
  await p.screenshot({ path: path.join(__dirname, 'shots', 'shop-big.png') });
  await p.mouse.move(330, 400); await p.mouse.down(); for (let i = 1; i <= 8; i++) { await p.mouse.move(330 - i * 30, 400); await p.waitForTimeout(16); } await p.mouse.up(); await p.waitForTimeout(600);
  const g = await p.locator('#sh-pager .sh-card[data-k="stars"]').boundingBox();
  ok('옆으로 넘기면 다음 카드가 제자리에', g.x >= 15 && g.x <= 25);
  await p.mouse.move(200, 300); await p.mouse.down(); for (let i = 1; i <= 8; i++) { await p.mouse.move(200, 300 + i * 25); await p.waitForTimeout(16); } await p.mouse.up(); await p.waitForTimeout(800);
  ok('끌어내리면 닫혀요', await p.isHidden('#sh-pager'));
  await p.screenshot({ path: path.join(__dirname, 'shots', 'shop.png') });
  for (const k of ['flame', 'ocean']) {
    await p.evaluate((k) => document.querySelector(`#sh-row .sh-card[data-k="${k}"]`).scrollIntoView({ inline: 'center', block: 'nearest' }), k); await p.waitForTimeout(300);
    await p.click(`#sh-row .sh-card[data-k="${k}"] .sh-pic`); await p.waitForTimeout(900);
    await p.screenshot({ path: path.join(__dirname, 'shots', `shop-${k}.png`) });
    await p.click('#sh-pager [data-close]'); await p.waitForTimeout(800);
  }
  const buy = async (k) => { await p.evaluate((k) => document.querySelector(`#sh-row [data-buy="${k}"]`).scrollIntoView({ inline: 'center', block: 'nearest' }), k); await p.waitForTimeout(300); await p.click(`#sh-row [data-buy="${k}"]`); await p.waitForTimeout(600); };
  // 사기
  await buy('devil');
  ok('사기 전에 값을 물어요', (await p.textContent('#alert-title')).includes('악마 프레임') && (await p.textContent('#alert-body')).includes('100'));
  await p.click('#alert-ok');
  ok('샀어요', await toast('악마 프레임을 샀어요'));
  await p.waitForTimeout(1500);
  const bag = ((await raw(`bag/${uid}`)).fields?.items?.arrayValue?.values || []).map((v) => v.stringValue);
  ok('서버: 지갑 350, 가방에 악마', int((await raw(`wallets/${uid}`)).fields?.bal) === 350 && bag.join() === 'devil');
  ok('산 카드는 보유 중', (await p.textContent('#sh-row [data-buy="devil"]')) === '보유 중' && await p.isDisabled('#sh-row [data-buy="devil"]'));
  for (const [k, n] of [['cat', '고양이'], ['neon', '네온'], ['stars', '별빛']]) { await p.waitForTimeout(2300); await buy(k); await p.click('#alert-ok'); await toast(n + ' 프레임을 샀어요'); }
  await p.waitForTimeout(2300);
  await buy('flame');
  await p.click('#alert-ok');
  let short = false; for (let i = 0; i < 40 && !short; i++) { await p.waitForTimeout(250); short = (await p.textContent('#alert-title')).includes('살 수 없어요') && (await p.textContent('#alert-body')).includes('모자라'); }
  ok('멸공이 모자라면 못 사요 (남은 50)', short && int((await raw(`wallets/${uid}`)).fields?.bal) === 50);
  await p.evaluate(() => document.getElementById('alert-ok').click()); await p.waitForTimeout(600);
  // 인벤
  await p.evaluate(() => window.cloud.setPhoto('data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==')); await p.waitForTimeout(2500);
  await menu('inv');
  ok('인벤 미리보기도 내 사진', await p.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--me-photo').includes('data:image/png') && !!document.querySelector('#inv-root .fr-ava.photo')));
  ok('인벤: 산 것만 (4개)', (await p.$$('#inv-root .iv-card')).length === 4);
  await p.click('#inv-root [data-act="wear"][data-item="devil"]'); await toast('걸었어요'); await p.waitForTimeout(1500);
  await p.click('#inv-root [data-act="wear"][data-item="stars"]'); await p.waitForTimeout(2300);
  const gear = ((await raw(`profiles/${uid}`)).fields?.gear?.arrayValue?.values || []).map((v) => v.stringValue);
  ok('프레임은 한 번에 하나 (별빛으로 바뀌어요)', gear.join() === 'stars' && (await p.$$('#inv-root .iv-hero .deco-art')).length === 2);
  await p.screenshot({ path: path.join(__dirname, 'shots', 'shop-inv.png') });
  await p.evaluate(() => window.cloud.postCreate('프레임 자랑', '', '', ''));
  await p.evaluate(() => document.querySelector('.tabbar .tab[data-tab="untitled3"]').click()); await p.waitForTimeout(3000);
  ok('SNS 게시물 내 동그라미에 프레임', (await p.$$('#sns-feed .deco-art')).length >= 2);
  await p.screenshot({ path: path.join(__dirname, 'shots', 'shop-sns.png') });
  ok('화면 오류 없음', p.errs.length === 0); if (p.errs.length) console.log(p.errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close();
  process.exit(res.every(Boolean) ? 0 : 1);
})();
