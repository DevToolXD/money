// 서버+화면: 월급. 앱을 켜 두고 본 시간 5분마다 20 멸공이 지갑에 들어와요 (본 시간은 1분마다 쌓여요).
// 시험을 빠르게 하려고 본 시간을 4분 59초(299,000ms)로 미리 적어 두고, 앱을 켜 둔 채 1분 뒤 10분을 넘으면 20 멸공이 들어오는지 봐요.
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const H = { 'content-type': 'application/json', authorization: 'Bearer owner' };
const raw = async (p) => (await (await fetch(`${DB}/${p}`, { headers: H })).json());
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const open = async () => {
    const p = await (await b.newContext({ viewport: { width: 390, height: 844 } })).newPage(); p.errs = []; p.on('pageerror', (e) => p.errs.push(e.message));
    await p.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
    await p.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
    await p.goto('http://127.0.0.1:8765/index.html'); await p.waitForTimeout(3500);
    return p;
  };
  const id = 'sl' + Date.now().toString(36).slice(-5);
  const P0 = await open();
  await P0.evaluate((u) => window.cloud.signUp(u, 'abc123', '월급쟁이'), id);
  await P0.waitForTimeout(2500);
  const uid = await P0.evaluate(() => window.cloud.uid);
  await P0.context().close();
  // 본 시간을 9분 59초로 적어 둬요
  await fetch(`${DB}/presence/${uid}`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { at: { timestampValue: new Date().toISOString() }, play: { integerValue: '299000' }, paid: { integerValue: '0' } } }) });
  const bal = async () => Number(((await raw(`wallets/${uid}`)).fields.bal || {}).integerValue || 0);
  const bal0 = await bal();
  const P = await open();
  await P.evaluate(([u]) => window.cloud.signIn(u, 'abc123'), [id]);
  await P.reload(); await P.waitForTimeout(6000);
  ok('켜자마자는 월급이 없어요 (아직 5분이 안 됐어요)', (await bal()) === bal0);
  let got = false, toast = '';
  for (let i = 0; i < 100 && !got; i++) { await P.waitForTimeout(1000); got = (await bal()) - bal0 === 20; }
  toast = (await P.textContent('#toast').catch(() => '')) || '';
  ok('1분쯤 뒤 5분을 넘기면 20 멸공이 들어와요', got);
  const pr = (await raw(`presence/${uid}`)).fields;
  ok('받은 횟수(paid)가 1이에요', pr.paid && Number(pr.paid.integerValue) === 1);
  ok('본 시간이 5분을 넘었어요', Number(pr.play.integerValue) >= 300000);
  ok('알림이 떠요', toast.includes('월급 20 멸공') || (await P.evaluate(() => document.body.textContent)).includes('월급 20 멸공'));
  await P.waitForTimeout(4000);
  ok('바로 또 들어오지 않아요 (한 번에 20만)', (await bal()) - bal0 === 20);
  ok('화면 오류 없음', P.errs.length === 0); if (P.errs.length) console.log(P.errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close();
  process.exit(res.every(Boolean) ? 0 : 1);
})().catch((e) => { console.log('오류', e); process.exit(1); });
