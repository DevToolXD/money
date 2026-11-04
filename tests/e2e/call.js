// 화면: 1:1 대화에서 전화 걸기 → 상대 화면에 전화가 와요 → 받으면 둘 다 통화 중(시간이 흘러요) → 끊으면 둘 다 닫혀요.
// 새 메시지 알림 배너도 여기서 봐요
const { chromium } = require('playwright');
const fs = require('fs');
const emu = fs.readFileSync(require('path').join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const open = async (b) => {
  const page = await (await b.newContext({ viewport: { width: 390, height: 844 }, permissions: ['microphone'] })).newPage();
  const errs = []; page.on('pageerror', (e) => errs.push(e.message));
  await page.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await page.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await page.goto('http://127.0.0.1:8765/index.html'); await page.waitForTimeout(3500);
  return { page, errs };
};
const text = async (p, sel) => ((await p.textContent(sel).catch(() => '')) || '');
const until = async (p, fn, n = 40) => { for (let i = 0; i < n; i++) { if (await p.evaluate(fn).catch(() => false)) return true; await p.waitForTimeout(250); } return false; };
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security', '--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream'] });
  const sfx = Date.now().toString(36).slice(-4);
  const X = await open(b); const Y = await open(b);
  const A = X.page; const B = Y.page;
  await A.evaluate((u) => window.cloud.signUp(u, 'abc123', '가나다'), 'ca' + sfx);
  await B.evaluate((u) => window.cloud.signUp(u, 'abc123', '라마바'), 'cb' + sfx);
  await A.reload(); await B.reload(); await A.waitForTimeout(6000); await B.waitForTimeout(3000);
  const bUid = await B.evaluate(() => window.cloud.uid);
  // A가 B에게 글을 보내면 B 화면 위에 알림이 내려와요
  const cid = await A.evaluate((u) => window.cloud.openChat(u), bUid);
  await A.evaluate(([c, u]) => window.cloud.send(c, u, { type: 'text', text: '전화할게' }, '전화할게'), [cid, bUid]);
  ok('새 메시지 알림이 위에 떠요', await until(B, () => { const e = document.querySelector('.note-banner'); return e && !e.hidden && e.textContent.includes('전화할게') && e.textContent.includes('가나다'); }));
  // A가 대화를 열고 전화를 걸어요
  await A.reload(); await A.waitForTimeout(6000);
  await A.evaluate((c) => document.querySelector(`#chat-list [data-id="${c}"]`)?.click(), cid); await A.waitForTimeout(2000);
  ok('대화 위쪽에 전화 버튼', await A.isVisible('#thread-call'));
  ok('1:1 대화에는 초대 버튼이 없어요', !(await A.isVisible('#thread-invite')));
  await A.screenshot({ path: __dirname + '/shots/call-head.png', clip: { x: 0, y: 0, width: 390, height: 160 } });
  await A.click('#thread-call');
  ok('거는 쪽: 통화 화면', await until(A, () => { const e = document.getElementById('call'); return e && !e.hidden && e.textContent.includes('라마바'); }));
  ok('받는 쪽: 전화가 왔어요', await until(B, () => { const e = document.getElementById('call'); return e && !e.hidden && e.textContent.includes('전화가 왔어요'); }));
  await B.click('#call [data-call="answer"]');
  ok('받는 쪽: 통화 중 (시간)', await until(B, () => /\d\d:\d\d/.test(document.getElementById('call-time')?.textContent || '') && !!document.querySelector('#call [data-call="end"]')));
  ok('거는 쪽: 통화 중 (시간)', await until(A, () => /\d\d:\d\d/.test(document.getElementById('call-time')?.textContent || '')));
  ok('버튼 여섯 개 (스피커·녹음·소리 끔·더 보기·종료·키패드)', (await A.$$('#call .call-btn')).length === 6);
  await A.screenshot({ path: __dirname + '/shots/call-talk.png' });
  ok('키패드는 처음엔 숨어 있어요', await A.isHidden('#call-pad'));
  await A.click('#call [data-call="pad"]'); await A.waitForTimeout(300);
  await A.screenshot({ path: __dirname + '/shots/call-pad.png' });
  await A.click('#call [data-call="pad"]'); await A.waitForTimeout(300);
  await A.click('#call [data-call="mute"]');
  ok('소리 끔이 켜져요', (await A.getAttribute('#call [data-call="mute"]', 'aria-pressed')) === 'true');
  await A.click('#call [data-call="end"]');
  ok('끊으면 거는 쪽 화면이 닫혀요', await until(A, () => document.getElementById('call').hidden));
  ok('상대 화면도 닫혀요', await until(B, () => document.getElementById('call').hidden));
  ok('오류 없음', X.errs.length === 0 && Y.errs.length === 0);
  if (X.errs.length || Y.errs.length) console.log(X.errs, Y.errs);
  await b.close();
  console.log(`${res.filter(Boolean).length}/${res.length} 통과`);
  process.exit(res.every(Boolean) ? 0 : 1);
})();
