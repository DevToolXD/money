// 서버+화면: 베타 초기화(meta/beta done) 뒤에는 그 전에 만든 계정이 모두 지워져요.
// - 로그인해 둔 채로 앱을 열면 계정이 지워지고 로그아웃돼요
// - 지워진 계정으로 로그인하면 "초기화되어 지워졌어요", 같은 아이디로 새로 가입하면 아이디를 비워 줘요
// - 초기화 뒤에 만든 계정과 owner는 그대로예요
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const H = { 'content-type': 'application/json', authorization: 'Bearer owner' };
const done = (at) => fetch(`${DB}/meta/beta`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { state: { stringValue: 'done' }, at: { timestampValue: new Date(at).toISOString() } } }) });
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const open = async () => {
    const p = await (await b.newContext({ viewport: { width: 390, height: 844 } })).newPage(); p.errs = []; p.on('pageerror', (e) => p.errs.push(e.message));
    await p.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
    await p.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
    await p.goto('http://127.0.0.1:8765/index.html'); await p.waitForTimeout(3500);
    return p;
  };
  const sfx = Date.now().toString(36).slice(-4);
  const uid = (p) => p.evaluate(() => window.cloud.uid);
  const call = (p, fn, args) => p.evaluate(([f, a]) => window.cloud[f](...a).then(() => 'OK', (e) => 'ERR ' + (e.message || e.code)), [fn, args]);

  // 1) 로그인해 둔 계정
  const A = await open();
  await A.evaluate((u) => window.cloud.signUp(u, 'abc123', '옛계정'), 'ra' + sfx);
  await A.reload(); await A.waitForTimeout(6000);
  const uA = await uid(A);
  ok('초기화 전: 로그인돼 있어요', !!uA);
  await A.waitForTimeout(1500);
  await done(Date.now() + 1000); // 초기화가 끝난 시각 (계정 만든 뒤)
  await A.reload(); await A.waitForTimeout(6500);
  ok('초기화 뒤 앱을 열면 로그아웃돼요', !(await uid(A)));
  const r1 = await call(A, 'signIn', ['ra' + sfx, 'abc123']);
  ok('지워진 계정으로는 로그인이 안 돼요 (계정이 없어요)', r1.startsWith('ERR'));

  // 2) 로그아웃해 둔 옛 계정: 로그인하면 그 자리에서 지워져요
  await fetch('http://127.0.0.1:9099/emulator/v1/projects/money-sim-36ed8/accounts', { method: 'DELETE' });
  await fetch(`${DB}/meta/beta`, { method: 'DELETE', headers: H });
  const B = await open();
  await B.evaluate((u) => window.cloud.signUp(u, 'abc123', '로그아웃계정'), 'rb' + sfx);
  await B.waitForTimeout(1500);
  await B.evaluate(() => window.cloud.signOut()); await B.waitForTimeout(1200);
  await B.waitForTimeout(1500);
  await done(Date.now());
  const r2 = await call(B, 'signIn', ['rb' + sfx, 'abc123']);
  ok('초기화 전에 만든 계정으로 로그인하면 안내가 떠요', r2.includes('초기화되어'));
  ok('그리고 로그아웃 상태예요', !(await uid(B)));
  const r3 = await call(B, 'signIn', ['rb' + sfx, 'abc123']);
  ok('계정은 이미 지워졌어요', r3.startsWith('ERR') && !r3.includes('초기화되어'));

  // 3) 옛 아이디를 새로 가입 (같은 비밀번호면 비워 줘요, 다르면 안 돼요)
  await fetch('http://127.0.0.1:9099/emulator/v1/projects/money-sim-36ed8/accounts', { method: 'DELETE' });
  await fetch(`${DB}/meta/beta`, { method: 'DELETE', headers: H });
  const C = await open();
  await C.evaluate((u) => window.cloud.signUp(u, 'abc123', '옛주인'), 'rc' + sfx);
  await C.waitForTimeout(1500);
  await C.evaluate(() => window.cloud.signOut()); await C.waitForTimeout(1500);
  await done(Date.now());
  const r4 = await call(C, 'signUp', ['rc' + sfx, 'zzz999', '새사람']);
  ok('다른 비밀번호로는 옛 아이디를 가져갈 수 없어요', r4.startsWith('ERR') && r4.includes('이미 있는 아이디'));
  const r5 = await call(C, 'signUp', ['rc' + sfx, 'abc123', '새주인']);
  ok('초기화 전 계정과 같은 비밀번호로 가입하면 아이디를 비워 줘요', r5 === 'OK' && !!(await uid(C)));

  // 4) 초기화 뒤에 만든 계정은 그대로
  await C.reload(); await C.waitForTimeout(6000);
  ok('초기화 뒤에 만든 계정은 지워지지 않아요', !!(await uid(C)));

  // 5) owner는 그대로
  const O = await open();
  await O.evaluate(() => window.cloud.signUp('owner', 'abc123', '관리자').catch(() => window.cloud.signIn('owner', 'abc123')));
  await O.waitForTimeout(1500);
  await done(Date.now() + 60000);
  await O.reload(); await O.waitForTimeout(6000);
  ok('owner 계정은 초기화해도 남아요', !!(await uid(O)));
  const errs = [...A.errs, ...B.errs, ...C.errs, ...O.errs];
  ok('화면 오류 없음', errs.length === 0); if (errs.length) console.log(errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close();
  process.exit(res.every(Boolean) ? 0 : 1);
})().catch((e) => { console.log('오류', e); process.exit(1); });
