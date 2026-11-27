// 화면: 지도 탭의 구글 지도. 서버(meta/gmaps)에 키가 있으면 구글 위성 타일(한글 지명), 없으면 Esri.
// 구글 타일이 많이 실패하면(키·세션 문제) Esri로 돌아가요. 세션은 기기에 저장해 다시 써요. (구글은 시험에서 가짜로 대신해요)
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const H = { 'content-type': 'application/json', authorization: 'Bearer owner' };
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==', 'base64');
const KEY = 'AIzaSyFakeKey1234567890abcdefghijklmno';
const setKey = (k) => fetch(`${DB}/meta/gmaps`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { key: { stringValue: k } } }) });
const tab = async (p, t) => {
  for (let i = 0; i < 2; i++) {
    if (await p.evaluate((t) => { const b = document.querySelector(`.tabbar .tab[data-tab="${t}"]`); if (b) b.click(); return !!b; }, t)) return;
    await p.evaluate(() => window.tabbar.flipPage()); await p.waitForTimeout(600);
  }
};
const srcs = (p) => p.evaluate(() => [...document.querySelectorAll('#map-tiles img.map-tile')].map((i) => i.getAttribute('src')));
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const ctx = await b.newContext({ viewport: { width: 390, height: 844 } });
  const p = await ctx.newPage(); p.errs = []; p.on('pageerror', (e) => p.errs.push(e.message));
  let sessions = 0, tilesOk = true, sessBody = null;
  await p.route('https://tile.googleapis.com/**', async (r) => {
    const u = r.request().url();
    const cors = { 'access-control-allow-origin': '*', 'access-control-allow-headers': 'content-type' };
    if (r.request().method() === 'OPTIONS') return r.fulfill({ status: 204, headers: cors });
    if (u.includes('/createSession')) {
      if (!u.includes('key=' + KEY)) return r.fulfill({ status: 400, headers: cors, body: '{}' });
      sessions++; sessBody = JSON.parse(r.request().postData() || '{}');
      return r.fulfill({ headers: { ...cors, 'content-type': 'application/json' }, body: JSON.stringify({ session: 'SESS' + sessions, expiry: String(Math.floor(Date.now() / 1000) + 14 * 86400), tileWidth: 256, tileHeight: 256, imageFormat: 'jpeg' }) });
    }
    if (!tilesOk) return r.fulfill({ status: 403, body: '' });
    return r.fulfill({ contentType: 'image/png', body: PNG });
  });
  await p.route('https://server.arcgisonline.com/**', (r) => r.fulfill({ contentType: 'image/png', body: PNG }));
  await p.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await p.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await setKey('');
  await p.goto('http://127.0.0.1:8765/index.html'); await p.waitForTimeout(3500);
  await p.evaluate((u) => window.cloud.signUp(u, 'abc123', '지도시험'), 'gm' + Date.now().toString(36).slice(-5));
  await p.reload(); await p.waitForTimeout(6000);
  await tab(p, 'map'); await p.waitForTimeout(2000);
  let s = await srcs(p);
  ok('키가 없으면 Esri 타일', s.length > 3 && s.every((u) => u.includes('arcgisonline')) && sessions === 0);
  ok('출처: Esri', (await p.textContent('#map-attr')).includes('Esri'));
  // 키를 넣고 다시 들어오면 구글
  await setKey(KEY);
  await p.reload(); await p.waitForTimeout(6000);
  await tab(p, 'map'); await p.waitForTimeout(2500);
  s = await srcs(p);
  ok('키가 있으면 구글 위성 타일', s.length > 3 && s.every((u) => u.startsWith('https://tile.googleapis.com/v1/2dtiles/') && u.includes('session=SESS1') && u.includes('key=' + KEY)));
  ok('세션: 위성 + 한글 지명', sessBody && sessBody.mapType === 'satellite' && sessBody.language === 'ko-KR' && (sessBody.layerTypes || []).includes('layerRoadmap'));
  ok('출처: Google', (await p.textContent('#map-attr')).trim() === 'Google');
  await p.screenshot({ path: path.join(__dirname, 'shots', 'gmaps.png') });
  // 끌어도 구글
  await p.mouse.move(200, 400); await p.mouse.down(); await p.mouse.move(60, 300, { steps: 8 }); await p.mouse.up(); await p.waitForTimeout(800);
  s = await srcs(p);
  ok('끌고 나서도 구글 타일', s.length > 3 && s.every((u) => u.includes('tile.googleapis.com')));
  // 다시 들어와도 세션을 새로 안 만들어요
  await p.reload(); await p.waitForTimeout(6000);
  await tab(p, 'map'); await p.waitForTimeout(2500);
  ok('다시 들어와도 저장한 세션을 다시 써요 (세션 만들기 1번)', sessions === 1 && (await srcs(p)).every((u) => u.includes('session=SESS1')));
  // 구글이 막히면 Esri로
  tilesOk = false;
  await p.evaluate(() => document.getElementById('map-zout').click()); await p.waitForTimeout(2500);
  s = await srcs(p);
  ok('구글 타일이 막히면 Esri로 돌아가요', s.length > 3 && s.every((u) => u.includes('arcgisonline')));
  ok('출처도 Esri로', (await p.textContent('#map-attr')).includes('Esri'));
  ok('화면 오류 없음', p.errs.length === 0); if (p.errs.length) console.log(p.errs.slice(0, 3));
  await setKey('');
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close();
  process.exit(res.every(Boolean) ? 0 : 1);
})().catch((e) => { console.log('오류', e); process.exit(1); });
