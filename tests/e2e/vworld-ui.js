// 화면: 지도 탭의 브이월드 지도. 서버(meta/vworld)에 키가 있으면 한국 안은 브이월드 위성 + 한글 지명(Hybrid), 한국 밖·키 없음은 Esri.
// 브이월드가 많이 실패하면(키·사이트 주소 문제) Esri로 돌아가요. (브이월드는 시험에서 가짜로 대신해요)
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const H = { 'content-type': 'application/json', authorization: 'Bearer owner' };
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==', 'base64');
const KEY = '1A2B3C4D-1234-5678-9ABC-DEF012345678';
const setKey = (k) => fetch(`${DB}/meta/vworld`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { key: { stringValue: k } } }) });
const tab = async (p, t) => {
  for (let i = 0; i < 2; i++) {
    if (await p.evaluate((t) => { const b = document.querySelector(`.tabbar .tab[data-tab="${t}"]`); if (b) b.click(); return !!b; }, t)) return;
    await p.evaluate(() => window.tabbar.flipPage()); await p.waitForTimeout(600);
  }
};
const srcs = (p, cls = '') => p.evaluate((c) => [...document.querySelectorAll('#map-tiles img.map-tile' + c)].filter((i) => !i.classList.contains('map-tile-h') || c).map((i) => i.getAttribute('src')), cls);
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const ctx = await b.newContext({ viewport: { width: 390, height: 844 } });
  const p = await ctx.newPage(); p.errs = []; p.on('pageerror', (e) => p.errs.push(e.message));
  let vok = true;
  await p.route('https://api.vworld.kr/**', async (r) => (vok ? r.fulfill({ contentType: 'image/png', body: PNG }) : r.fulfill({ status: 403, body: '' })));
  await p.route('https://server.arcgisonline.com/**', (r) => r.fulfill({ contentType: 'image/png', body: PNG }));
  await p.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await p.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await setKey('');
  await p.goto('http://127.0.0.1:8765/index.html'); await p.waitForTimeout(3500);
  await p.evaluate((u) => window.cloud.signUp(u, 'abc123', '지도시험'), 'vw' + Date.now().toString(36).slice(-5));
  await p.reload(); await p.waitForTimeout(6000);
  await tab(p, 'map'); await p.waitForTimeout(2000);
  let s = await srcs(p);
  ok('키가 없으면 Esri 타일', s.length > 3 && s.every((u) => u.includes('arcgisonline')));
  ok('출처: Esri', (await p.textContent('#map-attr')).includes('Esri'));
  // 키를 넣고 다시 들어오면 (서울) 브이월드 위성 + 한글 지명
  await setKey(KEY);
  await p.reload(); await p.waitForTimeout(6000);
  await tab(p, 'map'); await p.waitForTimeout(2500);
  s = await srcs(p);
  const h = await p.evaluate(() => [...document.querySelectorAll('#map-tiles img.map-tile-h')].map((i) => i.getAttribute('src')));
  ok('서울은 브이월드 위성 타일', s.length > 3 && s.every((u) => u.startsWith('https://api.vworld.kr/req/wmts/1.0.0/' + KEY + '/Satellite/') && u.endsWith('.jpeg')));
  ok('한글 지명 겹침 (Hybrid)', h.length === s.length && h.every((u) => u.includes('/Hybrid/') && u.endsWith('.png')));
  ok('출처에 VWorld', (await p.textContent('#map-attr')).includes('VWorld'));
  await p.screenshot({ path: path.join(__dirname, 'shots', 'vworld.png') });
  // 한국 밖(도쿄)은 Esri
  await p.evaluate(() => localStorage.setItem('map-last', '35.68,139.76,12'));
  await p.reload(); await p.waitForTimeout(6000);
  await tab(p, 'map'); await p.waitForTimeout(2500);
  s = await srcs(p);
  ok('한국 밖(도쿄)은 Esri 타일, 지명 겹침 없음', s.length > 3 && s.every((u) => u.includes('arcgisonline')) && (await p.$$('#map-tiles img.map-tile-h')).length === 0);
  // 서울로 돌아와 끌어 보기
  await p.evaluate(() => localStorage.setItem('map-last', '37.56650,126.97800,12'));
  await p.reload(); await p.waitForTimeout(6000);
  await tab(p, 'map'); await p.waitForTimeout(2500);
  await p.mouse.move(200, 400); await p.mouse.down(); await p.mouse.move(60, 300, { steps: 8 }); await p.mouse.up(); await p.waitForTimeout(800);
  s = await srcs(p);
  ok('끌고 나서도 브이월드', s.length > 3 && s.every((u) => u.includes('api.vworld.kr')));
  // 브이월드가 막히면 Esri로
  vok = false;
  await p.evaluate(() => document.getElementById('map-zout').click()); await p.waitForTimeout(2500);
  s = await srcs(p);
  ok('브이월드가 막히면 Esri로 돌아가요', s.length > 3 && s.every((u) => u.includes('arcgisonline')));
  ok('출처도 Esri로', !(await p.textContent('#map-attr')).includes('VWorld'));
  ok('화면 오류 없음', p.errs.length === 0); if (p.errs.length) console.log(p.errs.slice(0, 3));
  await setKey('');
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close();
  process.exit(res.every(Boolean) ? 0 : 1);
})().catch((e) => { console.log('오류', e); process.exit(1); });
