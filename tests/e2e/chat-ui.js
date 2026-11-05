// 화면: 메시지. 새 대화(사람 고르기) → 글 보내기 → 받는 사람 목록(안 읽음 점) → 열기, 찾기, 꾹 눌러 고정, 안 읽음만 보기,
// 토큰 선물(받기)과 요청(보내기), 사진 보내기와 크게 보기, 투표 만들고 투표하기, 그리기, 배경화면, 편집에서 삭제
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const H = { 'content-type': 'application/json', authorization: 'Bearer owner' };
const open = async (b) => {
  const page = await (await b.newContext({ viewport: { width: 390, height: 844 } })).newPage();
  const errs = []; page.on('pageerror', (e) => errs.push(e.message));
  await page.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await page.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await page.goto('http://127.0.0.1:8765/index.html'); await page.waitForTimeout(3500);
  return { page, errs };
};
const num = (f) => Number(f.integerValue ?? f.doubleValue);
const bal = async (uid) => num((await (await fetch(`${DB}/wallets/${uid}`, { headers: H })).json()).fields.bal);
const setBal = (uid, v) => fetch(`${DB}/wallets/${uid}?updateMask.fieldPaths=bal`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { bal: { integerValue: String(v) } } }) });
const text = async (p, sel) => ((await p.textContent(sel).catch(() => '')) || '');
const toastHas = async (p, t) => { for (let i = 0; i < 30; i++) { if ((await text(p, '.toast')).includes(t)) return true; await p.waitForTimeout(250); } return false; };
const waitText = async (p, sel, t, n = 40) => { for (let i = 0; i < n; i++) { if ((await text(p, sel)).includes(t)) return true; await p.waitForTimeout(250); } return false; };
const okAlert = async (p) => { await p.waitForSelector('#alert-ok', { state: 'visible', timeout: 5000 }); await p.click('#alert-ok'); await p.waitForTimeout(400); };
const press = async (p, sel) => { await p.evaluate((s) => [...document.querySelectorAll(s)].pop()?.scrollIntoView({ block: 'center' }), sel); await p.waitForTimeout(300); const bx = await p.locator(sel).last().boundingBox(); await p.mouse.move(bx.x + bx.width / 2, bx.y + bx.height / 2); await p.mouse.down(); await p.waitForTimeout(650); await p.mouse.up(); await p.waitForTimeout(400); };
// 1×1 빨간 PNG
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==', 'base64');
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const A = await open(b), B = await open(b);
  const sfx = Date.now().toString(36).slice(-4);
  await A.page.evaluate((u) => window.cloud.signUp(u, 'abc123', '보내는이'), 'ca' + sfx);
  await A.page.reload(); await A.page.waitForTimeout(6000);
  await B.page.evaluate((u) => window.cloud.signUp(u, 'abc123', '받는이'), 'cb' + sfx);
  await B.page.reload(); await B.page.waitForTimeout(6000);
  const [a, bp] = [A.page, B.page];
  const uA = await a.evaluate(() => window.cloud.uid), uB = await bp.evaluate(() => window.cloud.uid);
  await setBal(uA, 500); await setBal(uB, 500); await a.waitForTimeout(2500);

  ok('처음에는 메시지가 없어요', (await text(a, '#chat-empty')).includes('메시지가 없어요'));
  // 새 대화
  await a.click('#compose'); await a.waitForTimeout(1500);
  ok('새 메시지 시트: 사람 목록', (await text(a, '#new-title')) === '새 메시지' && (await text(a, '#new-results')).includes('받는이'));
  await a.fill('#new-q', '받는'); await a.waitForTimeout(1200);
  ok('이름으로 찾기', (await text(a, '#new-results')).includes('받는이'));
  await a.click('#new-results .res-row'); await a.waitForTimeout(1500);
  ok('대화가 열려요', (await a.evaluate(() => document.getElementById('thread').classList.contains('is-open'))) && (await text(a, '#thread-name')) === '받는이');
  await a.fill('#composer-input', '안녕하세요');
  ok('글을 쓰면 보내기 버튼이 켜져요', await a.evaluate(() => document.getElementById('send').classList.contains('ready')));
  await a.evaluate(() => document.getElementById('composer').requestSubmit());
  ok('내 말풍선', await waitText(a, '#bubbles', '안녕하세요'));
  ok('전송됨 표시', await waitText(a, '#bubbles .read', '전송됨'));
  await a.fill('#composer-input', '두 번째');
  await a.evaluate(() => document.getElementById('composer').requestSubmit());
  await waitText(a, '#bubbles', '두 번째');

  // 받는 사람
  ok('받는 사람 목록에 대화와 안 읽음 점', await waitText(bp, '#chat-list', '보내는이') && (await bp.$('#chat-list .chat-row .dot')) !== null);
  ok('미리보기 글은 마지막 글', (await text(bp, '#chat-list .chat-prev')) === '두 번째');
  await bp.click('#filter-unread'); await bp.waitForTimeout(300);
  ok('안 읽음만 보기에도 보여요', (await text(bp, '#chat-list')).includes('보내는이'));
  await bp.click('#filter-unread'); await bp.waitForTimeout(300);
  await bp.fill('#chat-search', '없는이름'); await bp.waitForTimeout(300);
  ok('찾기: 없으면 결과가 없어요', (await text(bp, '#chat-empty')) === '결과가 없어요' && !(await bp.isHidden('#chat-empty')));
  await bp.fill('#chat-search', ''); await bp.waitForTimeout(300);
  // 꾹 눌러 고정
  await press(bp, '#chat-list .chat-row');
  ok('꾹 누르면 메뉴', await bp.evaluate(() => document.getElementById('ctx-back').classList.contains('is-open')) && (await text(bp, '#ctx-menu2')).includes('고정'));
  await bp.click('#ctx-menu2 [data-k="0"]'); await bp.waitForTimeout(500);
  ok('고정하면 위쪽 고정 칸으로', (await text(bp, '#pinned')).includes('보내는이') && !(await bp.isHidden('#pinned')));
  await bp.click('#pinned .pin'); await bp.waitForTimeout(2000);
  ok('열면 두 말풍선', (await text(bp, '#bubbles')).includes('안녕하세요') && (await text(bp, '#bubbles')).includes('두 번째'));

  // 토큰 선물: 보내는이 → 받는이
  await a.click('#plus'); await a.waitForTimeout(300);
  ok('+ 메뉴', await a.evaluate(() => document.getElementById('attach-back').classList.contains('is-open')));
  await a.click('#attach [data-a="token"]'); await a.waitForTimeout(700);
  ok('선물 시트: 받는이님에게, 100', (await text(a, '#gift-to')) === '받는이님에게' && (await text(a, '#g-num')) === '100');
  await a.dispatchEvent('#g-plus', 'pointerdown'); await a.dispatchEvent('#g-plus', 'pointerup');
  ok('+ 를 누르면 110', (await text(a, '#g-num')) === '110');
  await a.click('#g-amt'); await a.waitForTimeout(200);
  await a.click('#keypad [data-k="5"]'); await a.click('#keypad [data-k="0"]'); await a.waitForTimeout(200);
  ok('키패드로 50', (await text(a, '#g-num')) === '50');
  await a.click('#g-send');
  ok('선물 알림', await toastHas(a, '받는이님에게 토큰 50을 보냈어요'));
  ok('받는 쪽에 선물 카드와 받기 버튼', await waitText(bp, '#bubbles .cash', '토큰 선물') && (await bp.$('#bubbles [data-act="accept"]')) !== null);
  await bp.evaluate(() => document.querySelector('#bubbles [data-act="accept"]').click());
  ok('받았어요 알림', await toastHas(bp, '토큰 50을 받았어요'));
  await bp.waitForTimeout(2500);
  ok('지갑: 보내는이 450, 받는이 550', (await bal(uA)) === 450 && (await bal(uB)) === 550);
  ok('선물 카드가 받았어요로', await waitText(bp, '#bubbles .cash-state', '받았어요'));
  // 요청: 받는이 → 보내는이 에게 30 요청
  await bp.click('#plus'); await bp.waitForTimeout(300);
  await bp.click('#attach [data-a="token"]'); await bp.waitForTimeout(700);
  await bp.click('#g-amt'); await bp.click('#keypad [data-k="3"]'); await bp.click('#keypad [data-k="0"]');
  await bp.click('#g-request');
  ok('요청 알림', await toastHas(bp, '보내는이님에게 토큰 30을 요청했어요'));
  ok('보내는 쪽에 요청 카드', await waitText(a, '#bubbles', '토큰 요청'));
  await a.waitForTimeout(800);
  await a.evaluate(() => document.querySelector('#bubbles [data-act="pay"]').click());
  await okAlert(a);
  const balIs = async (x, y) => { for (let i = 0; i < 40; i++) { if ((await bal(uA)) === x && (await bal(uB)) === y) return true; await a.waitForTimeout(250); } return false; };
  ok('요청을 보내면 선물이 돼요: 보내는이 420', await balIs(420, 550));
  // 요청한 사람이 선물을 받아요
  const acc = async () => { for (let i = 0; i < 40; i++) { if (await bp.$('#bubbles [data-act="accept"]')) return true; await bp.waitForTimeout(250); } return false; };
  ok('요청한 쪽에 받기 버튼', await acc());
  await bp.evaluate(() => document.querySelector('#bubbles [data-act="accept"]').click());
  ok('지갑: 보내는이 420, 받는이 580', await balIs(420, 580));
  ok('보냈어요 알림', (await text(a, '.toast')) === '보냈어요');

  // 사진 보내기와 크게 보기
  await a.setInputFiles('#file-photo', { name: 'r.png', mimeType: 'image/png', buffer: PNG });
  ok('사진 말풍선', await waitText(bp, '#bubbles', '') && (await (async () => { for (let i = 0; i < 40; i++) { if (await bp.$('#bubbles .media img')) return true; await bp.waitForTimeout(250); } return false; })()));
  await bp.evaluate(() => document.querySelector('#bubbles .media').click()); await bp.waitForTimeout(500);
  ok('사진 크게 보기', !(await bp.isHidden('#viewer')) && (await bp.getAttribute('#viewer-img', 'src')).startsWith('data:image/jpeg'));
  await bp.evaluate(() => document.getElementById('viewer').click()); await bp.waitForTimeout(500);

  // 투표
  await a.click('#plus'); await a.waitForTimeout(300);
  await a.click('#attach [data-a="poll"]'); await a.waitForTimeout(600);
  ok('투표 시트: 선택지 두 개', (await a.$$('#poll-opts .poll-edit')).length === 2);
  await a.click('#poll-send');
  ok('질문이 없으면 안내', (await text(a, '#poll-err')) === '질문을 적어 주세요');
  await a.fill('#poll-q', '점심 뭐 먹을까');
  await a.fill('#poll-opts [data-t="0"]', '김밥');
  await a.fill('#poll-opts [data-t="1"]', '라면');
  await a.click('#poll-add'); await a.waitForTimeout(200);
  ok('선택지 더하기', (await a.$$('#poll-opts .poll-edit')).length === 3);
  await a.click('#poll-send'); await a.waitForTimeout(1500);
  ok('받는 쪽에 투표 카드', await waitText(bp, '#bubbles .poll', '점심 뭐 먹을까'));
  await bp.evaluate(() => document.querySelector('#bubbles [data-vote="1"]').click());
  ok('투표하면 1명 투표했어요', await waitText(bp, '#bubbles .poll-meta', '1명 투표했어요'));
  ok('보낸 쪽에도 반영', await waitText(a, '#bubbles .poll-meta', '1명 투표했어요'));

  // 그리기
  await a.click('#plus'); await a.waitForTimeout(300);
  await a.click('#attach [data-a="draw"]'); await a.waitForTimeout(700);
  ok('그리기 화면', await a.evaluate(() => document.getElementById('draw').classList.contains('is-open')));
  const cb = await a.locator('#draw-canvas').boundingBox();
  await a.mouse.move(cb.x + 60, cb.y + 120); await a.mouse.down();
  for (let i = 0; i < 12; i++) await a.mouse.move(cb.x + 60 + i * 15, cb.y + 120 + i * 8);
  await a.mouse.up();
  await a.click('#draw-ok'); await a.waitForTimeout(1500);
  ok('그림 카드', await (async () => { for (let i = 0; i < 40; i++) { if (await bp.$('#bubbles .media.drawn img')) return true; await bp.waitForTimeout(250); } return false; })());

  // 배경화면
  await a.click('#thread-name-btn'); await a.waitForTimeout(500);
  await a.click('#wall-sheet [data-pick="mint"]'); await a.waitForTimeout(300);
  ok('배경화면 바꾸기', (await a.getAttribute('#thread', 'data-wall')) === 'mint');
  await a.click('#wall-done'); await a.waitForTimeout(400);
  ok('배경 저장 (이 기기)', JSON.parse(await a.evaluate(() => localStorage.getItem('walls')) || '{}')[await a.evaluate(() => document.querySelector('#chat-list .chat-row, #pinned .pin')?.dataset.id || '')] === 'mint' || (await a.evaluate(() => localStorage.getItem('walls'))).includes('mint'));
  // 말풍선 꾹 눌러 복사 메뉴
  await press(a, '#bubbles .bubble');
  ok('말풍선 메뉴: 복사', (await text(a, '#ctx-menu')).includes('복사'));
  await a.evaluate(() => document.getElementById('ctx-back').classList.remove('is-open', 'mode-stack'));
  // 뒤로 → 목록
  await a.click('#thread-back'); await a.waitForTimeout(600);
  ok('뒤로 가면 목록', !(await a.evaluate(() => document.getElementById('thread').classList.contains('is-open'))) && (await text(a, '#chat-list')).includes('받는이'));
  // 편집 → 삭제
  await a.click('#edit-chats'); await a.waitForTimeout(300);
  ok('편집 버튼이 완료로', (await text(a, '#edit-chats')) === '완료');
  await a.click('#chat-list [data-del]'); await a.waitForTimeout(400);
  ok('삭제: 한 번 물어봐요', (await text(a, '.alert-card')).includes('삭제할까요'));
  await okAlert(a);
  ok('삭제: 한 번 더 물어봐요', (await text(a, '.alert-card')).includes('정말 삭제할까요'));
  await okAlert(a); await a.waitForTimeout(400);
  ok('삭제하면 목록에서 빠져요', !(await text(a, '#chat-list')).includes('받는이'));
  await a.click('#edit-chats');

  const errs = [...A.errs, ...B.errs];
  ok('화면 오류 없음', errs.length === 0); if (errs.length) console.log(errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close(); process.exit(res.every(Boolean) ? 0 : 1);
})();
