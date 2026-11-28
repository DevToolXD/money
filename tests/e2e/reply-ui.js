// 화면: 말풍선을 오른쪽으로 밀면 답장 (아이폰 메시지처럼). 밀면 "OO 답장" 줄이 입력줄 위에 생기고, 보내면 상대 말풍선 위에 인용이 붙어요.
// 위아래로 넘길 때나 조금만 밀 때는 답장이 안 돼요.
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const open = async (b) => {
  const page = await (await b.newContext({ viewport: { width: 390, height: 844 } })).newPage();
  page.errs = []; page.on('pageerror', (e) => page.errs.push(e.message));
  await page.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await page.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await page.goto('http://127.0.0.1:8765/index.html'); await page.waitForTimeout(3500);
  return page;
};
const text = async (p, sel) => ((await p.textContent(sel, { timeout: 800 }).catch(() => '')) || '');
const waitText = async (p, sel, t, n = 40) => { for (let i = 0; i < n; i++) { if ((await text(p, sel)).includes(t)) return true; await p.waitForTimeout(250); } return false; };
const send = async (p, t) => { await p.fill('#composer-input', t); await p.evaluate(() => document.getElementById('composer').requestSubmit()); };
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const a = await open(b), bp = await open(b);
  const sfx = Date.now().toString(36).slice(-4);
  await a.evaluate((u) => window.cloud.signUp(u, 'abc123', '보내는이'), 'ra' + sfx); await a.reload(); await a.waitForTimeout(6000);
  await bp.evaluate((u) => window.cloud.signUp(u, 'abc123', '받는이'), 'rb' + sfx); await bp.reload(); await bp.waitForTimeout(6000);
  await a.click('#compose'); await a.waitForTimeout(1500);
  await a.fill('#new-q', '받는'); await a.waitForTimeout(1200);
  await a.click('#new-results .res-row'); await a.waitForTimeout(1500);
  await send(a, '안녕하세요 저녁 뭐 먹어요');
  await waitText(a, '#bubbles', '안녕하세요');
  // 받는 쪽이 대화를 열고 말풍선을 밀어요
  for (let i = 0; i < 60 && (await bp.$('#chat-list .chat-row')) === null; i++) await bp.waitForTimeout(250);
  await bp.click('#chat-list .chat-row'); await bp.waitForTimeout(1800);
  ok('받는 쪽에 말풍선이 보여요', await waitText(bp, '#bubbles', '안녕하세요'));
  const box = await bp.evaluate(() => { const r = document.querySelector('#bubbles .bubble:not(.me)').getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; });
  // 조금만 밀면 답장이 안 돼요
  await bp.mouse.move(box.x, box.y); await bp.mouse.down(); await bp.mouse.move(box.x + 30, box.y, { steps: 5 }); await bp.mouse.up(); await bp.waitForTimeout(500);
  ok('조금만 밀면 답장이 안 돼요', !(await bp.$('#reply-bar')));
  // 위아래로 넘기면 답장이 안 돼요
  await bp.mouse.move(box.x, box.y); await bp.mouse.down(); await bp.mouse.move(box.x + 20, box.y + 90, { steps: 8 }); await bp.mouse.up(); await bp.waitForTimeout(500);
  ok('위아래로 넘기면 답장이 안 돼요', !(await bp.$('#reply-bar')));
  // 밀면: 말풍선이 따라 움직여요
  await bp.mouse.move(box.x, box.y); await bp.mouse.down(); await bp.mouse.move(box.x + 70, box.y, { steps: 8 });
  const moved = await bp.evaluate(() => { const l = document.querySelector('#bubbles .line:not(.me)'); return { cls: l.className, rx: getComputedStyle(l).transform, p: l.style.getPropertyValue('--rp') }; });
  ok('미는 동안 말풍선이 손가락을 따라 움직이고 화살표가 나타나요', moved.cls.includes('swp') && moved.rx !== 'none' && Number(moved.p) > 0.5);
  await bp.screenshot({ path: path.join(__dirname, 'shots', 'reply-swipe.png') });
  await bp.mouse.up(); await bp.waitForTimeout(600);
  ok('충분히 밀고 놓으면 답장 줄이 생겨요', await bp.isVisible('#reply-bar') && (await text(bp, '#reply-bar')).includes('안녕하세요'));
  ok('답장 줄에 누구에게인지 보여요', (await text(bp, '#reply-bar')).includes('보내는이') || (await text(bp, '#reply-bar')).includes('답장'));
  ok('놓은 말풍선은 제자리로 돌아와요', await bp.evaluate(() => { const l = document.querySelector('#bubbles .line:not(.me)'); return !l.classList.contains('swp') && !l.style.getPropertyValue('--rx'); }));
  await bp.screenshot({ path: path.join(__dirname, 'shots', 'reply-bar.png') });
  await bp.click('#reply-x'); await bp.waitForTimeout(300);
  ok('X를 누르면 답장이 취소돼요', !(await bp.$('#reply-bar')));
  // 다시 밀어서 보내요
  await bp.mouse.move(box.x, box.y); await bp.mouse.down(); await bp.mouse.move(box.x + 90, box.y, { steps: 8 }); await bp.mouse.up(); await bp.waitForTimeout(500);
  await send(bp, '김치찌개요');
  ok('보내면 답장 줄이 사라져요', await (async () => { await bp.waitForTimeout(800); return !(await bp.$('#reply-bar')); })());
  ok('내 말풍선 위에 인용이 붙어요', await waitText(bp, '#bubbles .bubble.me .rq', '안녕하세요'));
  ok('상대 화면에도 인용이 붙어요', await waitText(a, '#bubbles .bubble .rq', '안녕하세요'));
  await a.screenshot({ path: path.join(__dirname, 'shots', 'reply-quote.png') });
  const errs = [...a.errs, ...bp.errs];
  ok('화면 오류 없음', errs.length === 0); if (errs.length) console.log(errs.slice(0, 4));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close();
  process.exit(res.every(Boolean) ? 0 : 1);
})().catch((e) => { console.log('오류', e); process.exit(1); });
