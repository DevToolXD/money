// 화면+서버: 코인 탭. 켤 때 허락을 받고, 가짜 풀(진짜 프록시를 거쳐)에서 일을 받아 RandomX로 계산하고 share가 인정되며,
// 100초마다 1 토큰 보상은 계정마다 한 번만 (두 탭이 동시에 받아도 늘지 않아요), 끄면 멈춰요
const { chromium } = require('playwright');
const fs = require('fs'); const path = require('path'); const net = require('net'); const { spawn } = require('child_process');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const H = { 'content-type': 'application/json', authorization: 'Bearer owner' };
const raw = async (p) => (await (await fetch(`${DB}/${p}`, { headers: H })).json());
(async () => {
  // 가짜 풀: 로그인하면 아주 쉬운 일을 주고, 제출은 모두 OK
  let submits = 0;
  const pool = net.createServer((s) => {
    s.setEncoding('utf8'); let buf = '';
    s.on('data', (d) => { buf += d; let i; while ((i = buf.indexOf('\n')) >= 0) { const m = JSON.parse(buf.slice(0, i)); buf = buf.slice(i + 1);
      if (m.method === 'login') s.write(JSON.stringify({ id: m.id, result: { id: 'm1', status: 'OK', job: { job_id: 'j1', blob: '0c0c' + 'ab'.repeat(74), target: 'ffffffff', seed_hash: '11'.repeat(32), height: 1 } } }) + '\n');
      if (m.method === 'submit') { submits++; s.write(JSON.stringify({ id: m.id, result: { status: 'OK' } }) + '\n'); } } });
  }).listen(3334);
  let proxy = spawn('node', [path.join(__dirname, '../../server/miner-proxy/index.js')], { env: { ...process.env, PORT: '8090', POOL_HOST: '127.0.0.1', POOL_PORT: '3334' }, stdio: 'inherit' });
  await new Promise((r) => setTimeout(r, 800));
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--disable-web-security'] });
  const ctx = await b.newContext({ viewport: { width: 390, height: 844 } });
  const open = async () => { const p = await ctx.newPage(); p.errs = []; p.on('pageerror', (e) => p.errs.push(e.message));
    await p.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
    await p.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:* ws://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
    await p.goto('http://127.0.0.1:8765/index.html'); await p.waitForTimeout(3500); return p; };
  const p = await open();
  await p.evaluate((u) => window.cloud.signUp(u, 'abc123', '채굴자'), 'cn' + Date.now().toString(36).slice(-4));
  await p.evaluate(() => localStorage.setItem('coin-proxy', 'ws://127.0.0.1:8090'));
  await p.reload(); await p.waitForTimeout(6000);
  const uid = await p.evaluate(() => window.cloud.uid);
  await p.evaluate(() => document.querySelector('.tabbar .tab-account').click()); await p.waitForTimeout(500);
  await p.click('.qm-row[data-go="coin"]'); await p.waitForTimeout(800);
  ok('코인 탭이 열려요', await p.isVisible('#screen-coin #coin-toggle'));
  ok('처음엔 꺼져 있어요', (await p.getAttribute('#coin-toggle', 'aria-checked')) === 'false');
  await p.click('#coin-toggle'); await p.waitForTimeout(500);
  ok('켤 때 허락을 물어요', (await p.textContent('#alert')).includes('채굴을 시작할까요?'));
  await p.click('#alert-cancel'); await p.waitForTimeout(500);
  ok('취소하면 안 켜져요', (await p.getAttribute('#coin-toggle', 'aria-checked')) === 'false');
  await p.click('#coin-toggle'); await p.waitForTimeout(500); await p.click('#alert-ok');
  await p.waitForTimeout(9000);
  ok('켜졌어요', (await p.getAttribute('#coin-toggle', 'aria-checked')) === 'true');
  const hashes = Number((await p.textContent('#coin-hashes')).replace(/\D/g, ''));
  console.log('state', await p.textContent('#coin-state'), 'rate', await p.textContent('#coin-rate'), 'hashes', hashes, 'shares', await p.textContent('#coin-shares'), 'pool submits', submits);
  ok('RandomX로 해시를 계산해요', hashes > 0);
  ok('찾은 share를 풀로 보내고 인정받아요', submits > 0 && Number(await p.textContent('#coin-shares')) > 0);
  ok('채굴 기록이 생겨요', !!(await raw(`mining/${uid}`)).fields);
  // 보상: 250초 전에 시작한 것으로 바꾸고 두 탭에서 동시에 받아도 2 토큰만
  const p2 = await open(); await p2.waitForTimeout(9000); // 두 번째 탭도 (허락을 기억해서) 켜져요
  const at0 = new Date(Date.now() - 250000).toISOString();
  await fetch(`${DB}/mining/${uid}?updateMask.fieldPaths=at`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { at: { timestampValue: at0 } } }) });
  const bal0 = Number((await raw(`wallets/${uid}`)).fields.bal.integerValue);
  await Promise.all([p.evaluate(() => window.cloud.mineClaim()).catch(() => 0), p2.evaluate(() => window.cloud.mineClaim()).catch(() => 0)]);
  await p.waitForTimeout(1500);
  const bal1 = Number((await raw(`wallets/${uid}`)).fields.bal.integerValue);
  ok(`250초면 2 토큰, 두 탭이어도 한 번만 (${bal1 - bal0})`, bal1 - bal0 === 2);
  await p.screenshot({ path: path.join(__dirname, 'shots', 'coin.png') });
  // 허락을 기억해요: 다시 열면 묻지 않고 켜져요
  await p2.close();
  await p.reload(); await p.waitForTimeout(9000);
  await p.evaluate(() => document.querySelector('.tabbar .tab-account').click()); await p.waitForTimeout(500);
  await p.click('.qm-row[data-go="coin"]'); await p.waitForTimeout(800);
  ok('다시 열면 묻지 않고 켜져 있어요', (await p.getAttribute('#coin-toggle', 'aria-checked')) === 'true' && !(await p.isVisible('#alert.is-open')));
  // 풀이 끊기면 끄지 않고 다시 붙어요
  proxy.kill(); await p.waitForTimeout(3000);
  const st = await p.textContent('#coin-state');
  console.log('끊긴 뒤:', st);
  ok('끊겨도 켜져 있고 다시 연결한다고 알려요', (await p.getAttribute('#coin-toggle', 'aria-checked')) === 'true' && st.includes('다시 연결'));
  ok('끊긴 동안은 보상 시간이 멈춰요', (await p.textContent('#coin-next')) === '멈춤');
  proxy = spawn('node', [path.join(__dirname, '../../server/miner-proxy/index.js')], { env: { ...process.env, PORT: '8090', POOL_HOST: '127.0.0.1', POOL_PORT: '3334' }, stdio: 'inherit' });
  await p.waitForTimeout(12000);
  console.log('다시 붙은 뒤:', await p.textContent('#coin-state'));
  ok('서버가 돌아오면 다시 채굴해요', (await p.textContent('#coin-state')).includes('채굴 중') || (await p.textContent('#coin-state')).includes('연결했어요'));
  await p.click('#coin-toggle'); await p.waitForTimeout(1500);
  ok('끄면 멈춰요', (await p.getAttribute('#coin-toggle', 'aria-checked')) === 'false' && (await p.textContent('#coin-rate')) === '—');
  ok('끄면 허락도 잊어요', (await p.evaluate(() => localStorage.getItem('coin-on'))) === '');
  ok('화면 오류 없음', p.errs.length === 0); if (p.errs.length) console.log(p.errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close(); proxy.kill(); pool.close(); process.exit(res.every(Boolean) ? 0 : 1);
})();
