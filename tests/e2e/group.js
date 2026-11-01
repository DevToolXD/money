// 화면: 여러 명을 골라 단톡방을 만들고, 글을 보내고, 받는 사람 화면에 보내는 사람 이름이 보여요. 대화 안에서 더 초대해요
const { chromium } = require('playwright');
const fs = require('fs');
const emu = fs.readFileSync(require('path').join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const open = async (b) => {
  const page = await (await b.newContext({ viewport: { width: 390, height: 844 } })).newPage();
  const errs = []; page.on('pageerror', (e) => errs.push(e.message));
  await page.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await page.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await page.goto('http://127.0.0.1:8765/index.html'); await page.waitForTimeout(3500);
  return { page, errs };
};
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const sfx = Date.now().toString(36).slice(-4);
  const people = [['ga' + sfx, '가나다'], ['gb' + sfx, '라마바'], ['gc' + sfx, '사아자'], ['gd' + sfx, '차카타']];
  const S2 = [];
  for (const [id, name] of people) {
    const x = await open(b); S2.push(x);
    await x.page.evaluate(([u, n]) => window.cloud.signUp(u, 'abc123', n), [id, name]);
    await x.page.reload(); await x.page.waitForTimeout(6000);
  }
  const [A, B, C, D] = S2.map((x) => x.page);
  await A.waitForTimeout(2000);
  await A.evaluate(() => document.getElementById('compose').click()); await A.waitForTimeout(1200);
  await A.click('#new-multi'); await A.waitForTimeout(300);
  const rows = await A.$$('#new-results .res-row');
  for (const r of rows) {
    const t = (await r.textContent()) || '';
    if (t.includes('라마바') || t.includes('사아자')) await r.click();
  }
  await A.waitForTimeout(200);
  ok('두 명을 골랐어요', (await A.textContent('#new-go')).includes('2명'));
  await A.click('#new-go'); await A.waitForTimeout(2500);
  const title = (await A.textContent('#thread-name')) || '';
  ok('단톡방이 열리고 이름이 보여요', title.includes('라마바') && title.includes('사아자'));
  ok('대화 안에서 초대 버튼이 보여요', await A.isVisible('#thread-invite'));
  await A.evaluate(() => document.getElementById('composer').requestSubmit());
  await A.fill('#composer-input', '단톡 안녕');
  await A.evaluate(() => document.getElementById('composer').requestSubmit()); await A.waitForTimeout(2500);
  await B.reload(); await B.waitForTimeout(6000);
  await B.evaluate(() => document.querySelector('[data-tab="messages"]')?.click()); await B.waitForTimeout(1200);
  const list = (await B.textContent('#chat-list')) || '';
  ok('받는 사람 목록에 단톡방이 있어요', list.includes('가나다') || list.includes('라마바'));
  await B.evaluate(() => document.querySelector('#chat-list [data-id^="g_"]').click()); await B.waitForTimeout(2500);
  const bub = (await B.textContent('#bubbles')) || '';
  ok('보낸 사람 이름 가나다가 보여요', bub.includes('가나다') && bub.includes('단톡 안녕'));
  // A가 D를 초대
  await A.evaluate(() => document.getElementById('thread-invite').click()); await A.waitForTimeout(1500);
  const rows2 = await A.$$('#new-results .res-row');
  for (const r of rows2) { if ((await r.textContent()).includes('차카타')) await r.click(); }
  await A.click('#new-go'); await A.waitForTimeout(2000);
  const members = await A.evaluate(async () => (await window.cloud.inviteMembers(document.getElementById('thread-name').dataset.cid || '', [])) ?? []).catch(() => []);
  ok('초대 뒤 확인 메시지가 나와요', (await A.textContent('#toast, .toast').catch(() => '')).includes('초대'));
  const errs = S2.flatMap((x) => x.errs);
  ok('화면 오류 없음', errs.length === 0); if (errs.length) console.log(errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close(); process.exit(res.every(Boolean) ? 0 : 1);
})();
