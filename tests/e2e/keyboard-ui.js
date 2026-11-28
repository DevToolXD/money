// 화면: 자판이 올라왔을 때 글쓰기 판. 쓰려는 칸이 자판에 가리지 않고 보여야 해요.
// (실제 자판 대신 화면 높이를 줄여서 자판이 올라온 것처럼 해요. 아이폰 실기기에서는 따로 봐야 해요)
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const p = await (await b.newContext({ viewport: { width: 390, height: 844 } })).newPage(); p.errs = []; p.on('pageerror', (e) => p.errs.push(e.message));
  await p.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await p.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await p.goto('http://127.0.0.1:8765/index.html'); await p.waitForTimeout(3500);
  await p.evaluate((u) => window.cloud.signUp(u, 'abc123', '자판시험'), 'kb' + Date.now().toString(36).slice(-5));
  await p.reload(); await p.waitForTimeout(6000);
  await p.evaluate(() => window.tabbar.flipPage()); await p.waitForTimeout(600);
  await p.evaluate(() => document.querySelector('.tabbar .tab[data-tab="trade"]').click()); await p.waitForTimeout(1500);
  await p.click('#sns-fab'); await p.waitForTimeout(800);
  const vis = (id) => p.evaluate((i) => { const r = document.getElementById(i).getBoundingClientRect(); return { top: r.top, bottom: r.bottom, vh: window.innerHeight }; }, id);
  // 자판 없이: 맨 아래 칸(설명)은 판 안에서 스크롤해야 보일 수도 있어요
  await p.focus('#tr-price');
  await p.setViewportSize({ width: 390, height: 430 }); // 자판이 올라온 높이
  await p.waitForTimeout(900);
  ok('자판이 올라오면 kb-open이 켜져요', await p.evaluate(() => document.documentElement.classList.contains('kb-open')));
  const panel = await p.evaluate(() => { const r = document.querySelector('#trade-compose .sns-panel').getBoundingClientRect(); return { top: r.top, bottom: r.bottom, h: r.height, vh: window.innerHeight }; });
  ok(`판이 보이는 높이를 거의 다 써요 (판 ${Math.round(panel.h)}px / 화면 ${panel.vh}px)`, panel.h > panel.vh * 0.8);
  const m = await vis('tr-price');
  ok(`쓰려는 칸(멸공)이 자판 위 보이는 곳에 있어요 (${Math.round(m.top)}~${Math.round(m.bottom)} / ${m.vh})`, m.top >= 0 && m.bottom <= m.vh - 4);
  await p.screenshot({ path: path.join(__dirname, 'shots', 'kb-trade.png') });
  await p.focus('#tr-want'); await p.waitForTimeout(900);
  const w = await vis('tr-want');
  ok('다른 칸(바꿀 것)으로 옮겨도 보여요', w.top >= 0 && w.bottom <= w.vh - 4);
  // 설명 칸 (맨 아래)
  const last = await p.evaluate(() => { const t = document.querySelector('#trade-compose textarea, #trade-compose .pl-in:last-of-type'); return t ? t.id : ''; });
  if (last) { await p.focus('#' + last); await p.waitForTimeout(900); const d = await vis(last); ok('맨 아래 칸도 보여요', d.top >= 0 && d.bottom <= d.vh - 4); }
  ok('화면 오류 없음', p.errs.length === 0); if (p.errs.length) console.log(p.errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close();
  process.exit(res.every(Boolean) ? 0 : 1);
})().catch((e) => { console.log('오류', e); process.exit(1); });
