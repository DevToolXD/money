// 화면: 프로필 사진 조정. 사진을 고르면 까만 화면에 사진이 뜨고, 끌어서 옮기고 모아서(휠) 크기를 맞춘 뒤 "저장"하면 틀 안만 프로필이 돼요. 뒤로 가면 그대로예요.
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const WIDE = fs.readFileSync(path.join(__dirname, 'fixtures', 'wide.png')); // 왼쪽 반 빨강, 오른쪽 반 파랑 (800x400)
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
// 저장된 사진의 왼쪽·가운데·오른쪽 색 (R, B 크기로 빨강/파랑을 가려요)
const probe = (p) => p.evaluate(async () => {
  const src = localStorage.getItem('profile-photo') || '';
  if (!src) return null;
  const img = new Image(); img.src = src; await img.decode();
  const c = document.createElement('canvas'); c.width = img.width; c.height = img.height;
  const x = c.getContext('2d'); x.drawImage(img, 0, 0);
  const at = (fx) => { const d = x.getImageData(Math.floor(img.width * fx), Math.floor(img.height / 2), 1, 1).data; return d[0] > d[2] ? 'R' : 'B'; };
  return { w: img.width, h: img.height, l: at(0.1), m: at(0.5), r: at(0.9) };
});
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const p = await (await b.newContext({ viewport: { width: 390, height: 844 } })).newPage(); p.errs = []; p.on('pageerror', (e) => p.errs.push(e.message));
  await p.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await p.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await p.goto('http://127.0.0.1:8765/index.html'); await p.waitForTimeout(3500);
  await p.evaluate((u) => window.cloud.signUp(u, 'abc123', '사진시험'), 'cr' + Date.now().toString(36).slice(-5));
  await p.reload(); await p.waitForTimeout(6000);
  const pick = async () => { await p.setInputFiles('#file-profile', { name: 'wide.png', mimeType: 'image/png', buffer: WIDE }); await p.waitForTimeout(1200); };
  const frame = () => p.evaluate(() => { const r = document.getElementById('crop-frame').getBoundingClientRect(); const i = document.querySelector('.crop-img').getBoundingClientRect(); return { fx: r.left, fy: r.top, f: r.width, ix: i.left, iy: i.top, iw: i.width, ih: i.height }; });

  // 1) 열려요: 사진을 고르면 조정 화면
  await pick();
  ok('사진을 고르면 조정 화면(까만 화면, 저장, 뒤로)이 떠요', await p.isVisible('#crop') && await p.isVisible('#crop-save') && await p.isVisible('#crop-back'));
  const f0 = await frame();
  ok('틀은 정사각형이고 화면 폭만큼 커요', Math.abs(f0.f - 390) < 2);
  ok('처음에는 틀이 사진으로 가득 차요 (가운데 맞춤)', f0.ix <= f0.fx + 1 && f0.iy <= f0.fy + 1 && f0.ix + f0.iw >= f0.fx + f0.f - 1 && f0.iy + f0.ih >= f0.fy + f0.f - 1);
  await p.screenshot({ path: path.join(__dirname, 'shots', 'crop-open.png') });
  // 2) 뒤로: 아무것도 안 바뀌어요
  await p.click('#crop-back'); await p.waitForTimeout(400);
  ok('뒤로 가면 조정 화면이 닫히고 사진은 그대로예요', !(await p.$('#crop')) && !(await p.evaluate(() => localStorage.getItem('profile-photo'))));
  // 3) 그대로 저장: 가운데 부분 (왼쪽 빨강 · 오른쪽 파랑)
  await pick(); await p.click('#crop-save'); await p.waitForTimeout(1500);
  const a = await probe(p);
  ok('그대로 저장하면 가운데 정사각형이 저장돼요 (왼쪽 빨강, 오른쪽 파랑)', a && a.l === 'R' && a.r === 'B' && Math.abs(a.w - a.h) <= 1);
  ok('저장된 사진은 작아요 (프로필 한도 안)', (await p.evaluate(() => localStorage.getItem('profile-photo').length)) <= 36000);
  ok('저장하면 조정 화면이 닫혀요', !(await p.$('#crop')));
  // 4) 끌어서 옮기기: 사진을 오른쪽으로 끌면 왼쪽(빨강)이 더 보여요
  await p.evaluate(() => localStorage.removeItem('profile-photo'));
  await pick();
  await p.mouse.move(200, 400); await p.mouse.down(); await p.mouse.move(380, 400, { steps: 10 }); await p.mouse.up(); await p.waitForTimeout(300);
  const f1 = await frame();
  ok('끌면 사진이 따라 움직여요 (틀 밖으로 사진 끝이 들어오진 않아요)', f1.ix > f0.ix && f1.ix <= f1.fx + 1);
  await p.screenshot({ path: path.join(__dirname, 'shots', 'crop-pan.png') });
  await p.click('#crop-save'); await p.waitForTimeout(1500);
  const b1 = await probe(p);
  ok('옮긴 대로 저장돼요 (왼쪽 빨강이 더 많이 들어와요)', b1 && b1.l === 'R' && b1.m === 'R' && b1.r === 'R');
  // 5) 크기 맞추기(휠): 확대하면 틀 안에 더 작은 부분이 들어와요
  await p.evaluate(() => localStorage.removeItem('profile-photo'));
  await pick();
  const s0 = (await frame()).iw;
  await p.mouse.move(195, 420); for (let i = 0; i < 6; i++) await p.mouse.wheel(0, -120);
  await p.waitForTimeout(300);
  const f2 = await frame();
  ok('확대하면 사진이 커져요', f2.iw > s0 * 1.5);
  ok('확대해도 틀은 사진으로 가득 차 있어요', f2.ix <= f2.fx + 1 && f2.iy <= f2.fy + 1 && f2.ix + f2.iw >= f2.fx + f2.f - 1 && f2.iy + f2.ih >= f2.fy + f2.f - 1);
  await p.screenshot({ path: path.join(__dirname, 'shots', 'crop-zoom.png') });
  for (let i = 0; i < 30; i++) await p.mouse.wheel(0, 120);
  await p.waitForTimeout(300);
  ok('아무리 줄여도 틀보다 작아지지 않아요', Math.abs((await frame()).iw - s0) < 2);
  await p.click('#crop-back'); await p.waitForTimeout(300);
  ok('화면 오류 없음', p.errs.length === 0); if (p.errs.length) console.log(p.errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close();
  process.exit(res.every(Boolean) ? 0 : 1);
})().catch((e) => { console.log('오류', e); process.exit(1); });
