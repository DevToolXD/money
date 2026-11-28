// 화면: 앱 설치 안내. 안드로이드는 들어올 때마다 안내 창(APK 받기)과 아래 작은 회색 안내, 아이폰은 사파리 홈 화면 추가 안내.
// 이미 앱(APK의 JabonApp 이름표)이거나 홈 화면 앱이면, 컴퓨터면 아무것도 안 떠요.
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const UA = {
  android: 'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36',
  app: 'Mozilla/5.0 (Linux; Android 14; Pixel 8; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/130.0.0.0 Mobile Safari/537.36 JabonApp/1',
  ios: 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1',
  pc: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
};
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const open = async (ua, init) => {
    const p = await (await b.newContext({ viewport: { width: 390, height: 844 }, userAgent: ua })).newPage(); p.errs = []; p.on('pageerror', (e) => p.errs.push(e.message));
    if (init) await p.addInitScript(init);
    await p.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
    await p.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
    await p.goto('http://127.0.0.1:8765/index.html'); await p.waitForTimeout(1200);
    return p;
  };
  const has = (p, sel) => p.evaluate((s) => !!document.querySelector(s), sel);
  const txt = (p, sel) => p.evaluate((s) => (document.querySelector(s) || {}).textContent || '', sel);

  // 안드로이드
  const A = await open(UA.android);
  ok('안드로이드: 아래 작은 회색 안내가 바로 있어요', await has(A, '#inst-bar') && (await txt(A, '#inst-bar')).includes('APK 설치'));
  ok('안내는 설치 파일 capitalism.apk를 받게 해요', (await A.getAttribute('#inst-bar', 'href')) === 'capitalism.apk' && (await A.getAttribute('#inst-bar', 'download')) === 'capitalism.apk');
  const bar = await A.evaluate(() => { const r = document.getElementById('inst-bar').getBoundingClientRect(); return { h: r.height, fs: parseFloat(getComputedStyle(document.getElementById('inst-bar')).fontSize), bottom: r.bottom }; });
  ok(`안내는 작아요 (높이 ${bar.h}px, 글자 ${bar.fs}px)`, bar.h <= 28 && bar.fs <= 12);
  ok('처음에는 안내 창이 아직 없어요', !(await has(A, '#inst-sheet')));
  await A.waitForTimeout(2600);
  ok('잠시 뒤 안내 창이 떠요 (APK 설치 / 나중에)', await has(A, '#inst-sheet') && (await txt(A, '#inst-sheet')).includes('앱으로 설치하세요') && (await has(A, '#inst-sheet a.inst-go[href="capitalism.apk"]')));
  await A.screenshot({ path: path.join(__dirname, 'shots', 'install-android.png') });
  await A.click('#inst-sheet .inst-later'); await A.waitForTimeout(300);
  ok('나중에를 누르면 창만 닫히고 아래 안내는 그대로', !(await has(A, '#inst-sheet')) && await has(A, '#inst-bar'));
  await A.reload(); await A.waitForTimeout(3800);
  ok('다시 들어오면 안내 창이 또 떠요 (들어올 때마다)', await has(A, '#inst-sheet') && await has(A, '#inst-bar'));

  // 아이폰
  const I = await open(UA.ios);
  ok('아이폰: 아래 작은 회색 안내 (홈 화면에 추가)', await has(I, '#inst-bar') && (await txt(I, '#inst-bar')).includes('홈 화면에 추가'));
  await I.waitForTimeout(2600);
  const steps = await txt(I, '#inst-sheet');
  ok('아이폰 안내 창: 공유 → 홈 화면에 추가 → 추가', steps.includes('공유') && steps.includes('홈 화면에 추가') && steps.includes('추가를 누르면 끝'));
  await I.screenshot({ path: path.join(__dirname, 'shots', 'install-ios.png') });
  await I.click('#inst-sheet .inst-go'); await I.waitForTimeout(300);
  ok('알겠어요를 누르면 닫혀요', !(await has(I, '#inst-sheet')));
  await I.click('#inst-bar'); await I.waitForTimeout(400);
  ok('아래 안내를 누르면 방법이 다시 떠요', await has(I, '#inst-sheet'));

  // 이미 앱이거나 홈 화면 앱이거나 컴퓨터
  const W = await open(UA.app); await W.waitForTimeout(2800);
  ok('설치한 앱(APK) 안에서는 아무것도 안 떠요', !(await has(W, '#inst-bar')) && !(await has(W, '#inst-sheet')));
  const S = await open(UA.ios, () => { Object.defineProperty(navigator, 'standalone', { get: () => true }); }); await S.waitForTimeout(2800);
  ok('아이폰 홈 화면 앱으로 열면 아무것도 안 떠요', !(await has(S, '#inst-bar')) && !(await has(S, '#inst-sheet')));
  const P = await open(UA.pc); await P.waitForTimeout(2800);
  ok('컴퓨터에서는 아무것도 안 떠요', !(await has(P, '#inst-bar')) && !(await has(P, '#inst-sheet')));
  const errs = [...A.errs, ...I.errs, ...W.errs, ...S.errs, ...P.errs];
  ok('화면 오류 없음', errs.length === 0); if (errs.length) console.log(errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close();
  process.exit(res.every(Boolean) ? 0 : 1);
})().catch((e) => { console.log('오류', e); process.exit(1); });
