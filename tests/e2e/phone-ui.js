// 화면: 회원가입 직후 전화번호 인증. 새 계정은 인증 전에는 앱을 못 쓰고, 번호 → 인증번호 → 확인 → 인증 끝.
// Zavu 서버 함수(fn.zavu.dev)는 시험에서 가짜로 대신해요 (인증번호는 123456). 기존(2026-11-26 12:30 UTC 전)에 만든 계정은 묻지 않아요.
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const H = { 'content-type': 'application/json', authorization: 'Bearer owner' };
const text = async (p, sel) => ((await p.textContent(sel, { timeout: 800 }).catch(() => '')) || '');
const waitText = async (p, sel, t, n = 40) => { for (let i = 0; i < n; i++) { if ((await text(p, sel)).includes(t)) return true; await p.waitForTimeout(250); } return false; };
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const p = await (await b.newContext({ viewport: { width: 390, height: 844 } })).newPage(); p.errs = []; p.on('pageerror', (e) => p.errs.push(e.message));
  await p.addInitScript(() => { try { localStorage.setItem('pg-force', '1'); } catch (e) {} });
  const calls = []; let sent = 0; let uidNow = '';
  await p.route('https://fn.zavu.dev/**', async (r) => {
    const body = JSON.parse(r.request().postData() || '{}');
    calls.push(body.op + ':' + (body.phone || '') + ':' + (body.code || ''));
    const cors = { 'access-control-allow-origin': '*', 'content-type': 'application/json' };
    if (!body.idToken) return r.fulfill({ status: 401, headers: cors, body: JSON.stringify({ ok: false, code: 'not_signed_in' }) });
    if (body.op === 'send') {
      if (!/^010-\d{4}-\d{4}$/.test(body.phone)) return r.fulfill({ headers: cors, body: JSON.stringify({ ok: false, code: 'bad_phone' }) });
      sent++; return r.fulfill({ headers: cors, body: JSON.stringify({ ok: true }) });
    }
    if (body.op === 'check') {
      if (body.code !== '123456') return r.fulfill({ headers: cors, body: JSON.stringify({ ok: false, code: 'wrong_code' }) });
      await fetch(`${DB}/verified/${uidNow}`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { at: { integerValue: String(Date.now()) }, tail: { stringValue: '5678' } } }) });
      return r.fulfill({ headers: cors, body: JSON.stringify({ ok: true }) });
    }
    r.fulfill({ headers: cors, body: JSON.stringify({ ok: false, code: 'bad_op' }) });
  });
  await p.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await p.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await p.goto('http://127.0.0.1:8765/index.html'); await p.waitForTimeout(3500);
  await p.evaluate((u) => window.cloud.signUp(u, 'abc123', '인증시험'), 'ph' + Date.now().toString(36).slice(-5));
  await p.reload(); await p.waitForTimeout(6500);
  uidNow = await p.evaluate(() => window.cloud.uid);
  ok('가입하고 들어오면 전화번호 인증 화면이 떠요', await p.isVisible('#phone-gate') && (await text(p, '#pg-title')) === '전화번호 인증');
  ok('처음에는 번호 칸과 "인증번호 받기"만 (인증번호 칸은 숨김)', await p.isVisible('#pg-phone') && await p.isVisible('#pg-send') && !(await p.isVisible('#pg-code')));
  await p.screenshot({ path: path.join(__dirname, 'shots', 'phone-1.png') });
  // 틀린 번호
  await p.fill('#pg-phone', '0212345678'); await p.dispatchEvent('#pg-phone', 'input'); await p.click('#pg-send'); await p.waitForTimeout(900);
  ok('010이 아닌 번호는 안내가 떠요', (await text(p, '#pg-msg')).includes('010으로 시작하는'));
  // 번호는 쓰는 대로 010-1234-5678 모양으로
  await p.fill('#pg-phone', ''); await p.type('#pg-phone', '01012345678'); await p.waitForTimeout(200);
  ok('번호가 010-1234-5678 모양으로 정리돼요', (await p.inputValue('#pg-phone')) === '010-1234-5678');
  await p.click('#pg-send'); await p.waitForTimeout(1200);
  ok('받기를 누르면 인증번호 칸·확인·다시 받기가 나와요', await p.isVisible('#pg-code') && await p.isVisible('#pg-check') && await p.isVisible('#pg-resend') && !(await p.isVisible('#pg-send')) && sent === 1);
  ok('안내: 문자로 6자리를 보냈어요', (await text(p, '#pg-msg')).includes('6자리'));
  ok('다시 받기는 60초 동안 막혀요 (남은 초가 보여요)', (await p.isDisabled('#pg-resend')) && /\(\d+초\)/.test(await text(p, '#pg-resend')));
  await p.screenshot({ path: path.join(__dirname, 'shots', 'phone-2.png') });
  // 틀린 인증번호
  await p.fill('#pg-code', '111111'); await p.dispatchEvent('#pg-code', 'input'); await p.waitForTimeout(1000);
  ok('틀린 인증번호는 안내가 떠요 (화면은 그대로)', (await text(p, '#pg-msg')).includes('맞지 않아요') && await p.isVisible('#phone-gate'));
  // 맞는 인증번호: 6자리를 넣으면 바로 확인돼요
  await p.fill('#pg-code', '123456'); await p.dispatchEvent('#pg-code', 'input'); await p.waitForTimeout(1800);
  ok('맞는 인증번호를 넣으면 인증 화면이 닫혀요', !(await p.isVisible('#phone-gate')));
  ok('인증 완료 알림', await waitText(p, '.toast', '전화번호 인증을 마쳤어요', 12));
  ok('서버 호출: 보내기 두 번(틀린 번호·맞는 번호)과 확인 두 번', calls.filter((c) => c.startsWith('send')).length === 2 && calls.filter((c) => c.startsWith('check')).length === 2);
  // 다시 들어와도 안 물어요
  await p.reload(); await p.waitForTimeout(6500);
  ok('다시 들어와도 인증 화면이 안 떠요', !(await p.isVisible('#phone-gate')));
  // 인증 안 한 새 계정은 다시 들어와도 또 떠요
  await p.evaluate(() => window.cloud.signOut()); await p.waitForTimeout(1500);
  await p.evaluate((u) => window.cloud.signUp(u, 'abc123', '두번째'), 'pi' + Date.now().toString(36).slice(-5));
  await p.reload(); await p.waitForTimeout(6500);
  ok('인증을 안 한 새 계정은 들어올 때마다 인증 화면이 떠요', await p.isVisible('#phone-gate'));
  // 로그아웃 버튼
  await p.click('#pg-logout'); await p.waitForTimeout(2500);
  ok('인증 화면의 로그아웃 버튼', !(await p.isVisible('#phone-gate')) && (await p.evaluate(() => !window.cloud.uid)));
  ok('화면 오류 없음', p.errs.length === 0); if (p.errs.length) console.log(p.errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close();
  process.exit(res.every(Boolean) ? 0 : 1);
})().catch((e) => { console.log('오류', e); process.exit(1); });
