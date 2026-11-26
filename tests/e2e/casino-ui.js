// 화면: 도박장 (메시지의 멸공 선물 카드를 10초 꾹 누르면 열려요). 짧게 누르면 안 열리고, 떼도 선물 받기가 눌리지 않아요.
// 슬롯 3칸은 칸마다 별 32%, 별 2칸 1.5배, 3칸 10배, 그 밖 0배. 돌린 결과의 지갑 변화가 서버 값과 맞아야 해요.
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const H = { 'content-type': 'application/json', authorization: 'Bearer owner' };
const open = async (b, mobile = true) => {
  const page = await (await b.newContext({ viewport: { width: 390, height: 844 } })).newPage();
  const errs = []; page.on('pageerror', (e) => errs.push(e.message)); page.on('crash', () => errs.push('CRASH')); page.errs = errs;
  await page.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await page.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await page.goto('http://127.0.0.1:8765/index.html'); await page.waitForTimeout(3500);
  return page;
};
const num = (f) => Number(f.integerValue ?? f.doubleValue);
const bal = async (uid) => num((await (await fetch(`${DB}/wallets/${uid}`, { headers: H })).json()).fields.bal);
const setBal = (uid, v) => fetch(`${DB}/wallets/${uid}?updateMask.fieldPaths=bal`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { bal: { integerValue: String(v) } } }) });
const text = async (p, sel) => ((await p.textContent(sel, { timeout: 800 }).catch(() => '')) || '');
const waitText = async (p, sel, t, n = 40) => { for (let i = 0; i < n; i++) { if ((await text(p, sel)).includes(t)) return true; await p.waitForTimeout(250); } return false; };
const isOpen = (p) => p.evaluate(() => document.getElementById('casino').classList.contains('is-open'));
const sleepP = (p, ms) => p.waitForTimeout(ms);
// 선물 카드의 글자 위를 누르고 있어요 (받기 버튼이 아닌 곳). during은 누르는 동안 한 번 불러요
const pressCard = async (p, ms, during) => {
  await p.evaluate(() => [...document.querySelectorAll('#bubbles .cash[data-gift] .cash-tag')].pop()?.scrollIntoView({ block: 'center' }));
  await p.waitForTimeout(300);
  const bx = await p.locator('#bubbles .cash[data-gift] .cash-tag').last().boundingBox();
  await p.mouse.move(bx.x + bx.width / 2, bx.y + bx.height / 2);
  await p.mouse.down();
  if (during) await during();
  await p.waitForTimeout(ms);
  await p.mouse.up();
};
const won = (n) => String(n).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
const stars = (p) => p.evaluate(() => document.querySelectorAll('.cz-reel.hit').length);

(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const a = await open(b), bp = await open(b);
  const sfx = Date.now().toString(36).slice(-4);
  await a.evaluate((u) => window.cloud.signUp(u, 'abc123', '보내는이'), 'ka' + sfx);
  await a.reload(); await a.waitForTimeout(6000);
  await bp.evaluate((u) => window.cloud.signUp(u, 'abc123', '받는이'), 'kb' + sfx);
  await bp.reload(); await bp.waitForTimeout(6000);
  const uA = await a.evaluate(() => window.cloud.uid), uB = await bp.evaluate(() => window.cloud.uid);
  await setBal(uA, 5000); await setBal(uB, 500); await a.waitForTimeout(2500);

  // 대화를 열고 멸공 100 선물을 보내요
  await a.click('#compose'); await a.waitForTimeout(1500);
  await a.fill('#new-q', '받는'); await a.waitForTimeout(1200);
  await a.click('#new-results .res-row'); await a.waitForTimeout(1500);
  await a.fill('#composer-input', '안녕하세요');
  await a.evaluate(() => document.getElementById('composer').requestSubmit());
  await waitText(a, '#bubbles', '안녕하세요');
  await a.fill('#composer-input', '두 번째');
  await a.evaluate(() => document.getElementById('composer').requestSubmit());
  await waitText(a, '#bubbles', '두 번째');
  await a.click('#plus'); await a.waitForTimeout(300);
  await a.click('#attach [data-a="token"]'); await a.waitForTimeout(700);
  await a.click('#g-send');
  // 받는 사람: 대화 목록에서 대화를 열어요 (말풍선은 열린 대화에 보여요)
  for (let i = 0; i < 60 && (await bp.$('#chat-list .chat-row')) === null; i++) await bp.waitForTimeout(250);
  if ((await bp.$('#chat-list .chat-row')) === null) { await bp.reload(); await bp.waitForTimeout(6000); }
  await bp.click('#chat-list .chat-row'); await bp.waitForTimeout(1500);
  ok('선물 카드가 받는 쪽에 와요 (멸공 선물, 데이터 표시)', await waitText(bp, '#bubbles .cash[data-gift]', '멸공 선물'));
  await bp.waitForTimeout(1200);

  // 1) 짧게 누르면 (4초) 도박장은 안 열리고, 선물도 안 받아져요
  await pressCard(bp, 4000);
  await bp.waitForTimeout(500);
  ok('4초만 누르면 도박장이 안 열려요', !(await isOpen(bp)));
  await bp.waitForTimeout(1500);
  ok('짧게 누른 것은 받기로 이어지지 않아요 (받는이 500 그대로)', (await bal(uB)) === 500);

  // 2) 10초 동안 누르면 열려요. 누르는 동안 진행 막대가 차요
  let barSeen = false;
  await pressCard(bp, 10600, async () => {
    await bp.waitForTimeout(1500);
    barSeen = await bp.evaluate(() => !!document.querySelector('.cz-hold'));
  });
  await bp.waitForTimeout(600);
  ok('누르는 동안 아래쪽에 진행 막대', barSeen);
  ok('10초를 채우면 도박장이 열려요', await isOpen(bp) && (await text(bp, '#title-casino')) === '도박장');
  await bp.waitForTimeout(1500);
  ok('떼도 받기는 안 눌려요 (받는이 500 그대로)', (await bal(uB)) === 500 && !(await text(bp, '#bubbles .cash-state')).includes('받았어요'));
  ok('내 멸공 500 표시', (await text(bp, '#cz-bal')) === '500 멸공');
  ok('기본 칩 100, 버튼 문구', (await text(bp, '#cz-spin')) === '100 멸공 걸고 돌리기' && (await bp.getAttribute('[data-bet="0"]', 'class')).includes('on'));
  ok('배율 표: 10배·1.5배·0배', (await text(bp, '.cz-rules')).includes('10배') && (await text(bp, '.cz-rules')).includes('1.5배') && (await text(bp, '.cz-rules')).includes('0배') && (await text(bp, '.cz-rules')).includes('32%'));
  ok('칸은 3개', (await bp.$$('.cz-reel')).length === 3);
  ok('가로로 넘치지 않아요', await bp.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth));
  await bp.screenshot({ path: path.join(__dirname, 'shots', 'casino-open.png') });

  // 3) 칩을 바꾸면 버튼 문구가 바뀌어요
  await bp.click('[data-bet="1"]'); await bp.waitForTimeout(200);
  ok('칩 1,000: 버튼 문구', (await text(bp, '#cz-spin')) === '1,000 멸공 걸고 돌리기' && (await bp.getAttribute('[data-bet="1"]', 'class')).includes('on'));
  await bp.click('[data-bet="0"]'); await bp.waitForTimeout(200);

  // 4) 세 번 돌려요: 결과 글과 맞힌 칸 수, 지갑 변화가 맞아야 해요
  let expect = 500;
  for (let i = 0; i < 3; i++) {
    await bp.click('#cz-spin');
    await bp.waitForTimeout(300);
    ok(`돌리는 동안 버튼이 꺼져요 (${i + 1}번째)`, await bp.evaluate(() => document.getElementById('cz-spin').disabled));
    let r = '';
    for (let k = 0; k < 60; k++) { r = await text(bp, '#cz-result'); if (r && !r.includes('돌리는') && !r.includes('고르고')) break; await bp.waitForTimeout(250); }
    const h = await stars(bp);
    const want = h === 3 ? 3 : h === 2 ? 2 : h === 1 ? 1 : 0;
    const word = ['하나도 안 맞았어요', '1칸만 맞았어요', '2칸 맞았어요', '3칸 모두 맞았어요'][want];
    ok(`결과 글과 맞힌 칸 수가 같아요 (${h}칸): ${r}`, r.startsWith(word));
    // 서버 지갑: 3칸 +900, 2칸 +50, 그 밖 -100 (건 돈 100)
    expect += h === 3 ? 900 : h === 2 ? 50 : -100;
    await bp.waitForTimeout(2600);
    const got = await bal(uB);
    ok(`지갑이 배율대로 바뀌어요 (기대 ${expect}, 서버 ${got})`, got === expect);
    ok(`내 멸공 표시가 맞아요 (${won(expect)})`, (await text(bp, '#cz-bal')).includes(won(expect)) || (await bp.waitForTimeout(800), (await text(bp, '#cz-bal')).includes(won(expect))));
    ok('돌리기가 끝나면 버튼이 다시 켜져요', await bp.evaluate(() => !document.getElementById('cz-spin').disabled));
    ok('거래 기록에 도박장이 남아요', await bp.evaluate(() => (localStorage.getItem('bank-tx') || '').includes('도박장')));
  }

  // 4-b) 세 칸 모두 별이 나오는 경우(10배)를 확인해요. 시험에서만 난수를 정해 넣어요 (칸마다 난수 하나면 별)
  await bp.evaluate(() => { const real = Math.random; window.__seq = [0, 0, 0]; Math.random = () => (window.__seq.length ? window.__seq.shift() : real()); });
  await bp.click('#cz-spin');
  let r3 = '';
  for (let k = 0; k < 60; k++) { r3 = await text(bp, '#cz-result'); if (r3.includes('3칸')) break; await bp.waitForTimeout(250); }
  ok(`세 칸 모두 별: 10배 글 (${r3})`, r3.startsWith('3칸 모두 맞았어요. 10배예요. 지갑에 +900'));
  ok('세 칸이 모두 파란 칸', (await bp.$$('.cz-reel.hit')).length === 3);
  expect += 900;
  await bp.waitForTimeout(2600);
  ok(`10배: 지갑 +900 (기대 ${expect}, 서버 ${await bal(uB)})`, (await bal(uB)) === expect);
  await bp.waitForTimeout(800);
  ok('10배 뒤 내 멸공 표시', (await text(bp, '#cz-bal')).includes(won(expect)));
  await bp.screenshot({ path: path.join(__dirname, 'shots', 'casino-win.png') });

  // 5) 돈이 모자라면 못 돌려요
  await setBal(uB, 50); await bp.waitForTimeout(2500);
  await bp.click('#cz-spin');
  let short = false;
  for (let i = 0; i < 40 && !short; i++) {
    await bp.waitForTimeout(250);
    short = (await text(bp, '#alert-title')).includes('돌릴 수 없어요') && (await text(bp, '#alert-body')).includes('모자라');
  }
  ok('멸공이 모자라면 돌릴 수 없다고 알려요', short);
  if (short) await bp.click('#alert-ok').catch(() => {});
  await bp.waitForTimeout(500);
  ok('모자랄 때 지갑은 그대로 (50)', (await bal(uB)) === 50);

  // 6) 닫기
  await bp.click('#casino-close'); await bp.waitForTimeout(700);
  ok('닫기 버튼으로 닫혀요', !(await isOpen(bp)));
  await bp.screenshot({ path: path.join(__dirname, 'shots', 'casino-closed.png') });

  const errs = [...a.errs, ...bp.errs].filter((e) => !/Firestore|permission|network/i.test(e));
  ok('자바스크립트 오류 없음', errs.length === 0);
  if (errs.length) console.log(errs);
  await b.close();
  const pass = res.filter(Boolean).length;
  console.log(`\n${pass}/${res.length} 통과`);
  process.exit(pass === res.length ? 0 : 1);
})().catch((e) => { console.log('오류', e); process.exit(1); });
