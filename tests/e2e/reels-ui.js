// 화면: 릴스 댓글 판 (인스타처럼). 댓글을 누르면 게시물이 위로 줄고 어두운 판이 올라와요.
// 댓글 달기, GIF 검색해서 보내기(GIPHY는 가짜로 대신해요), 멸공 선물 → 글쓴이가 SNS 알림에서 받기, 끌어 내려 닫기
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const H = { 'content-type': 'application/json', authorization: 'Bearer owner' };
const raw = async (p) => (await (await fetch(`${DB}/${p}`, { headers: H })).json());
const int = (f) => Number(f?.integerValue ?? f?.doubleValue ?? 0);
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==', 'base64');
const GIFS = { data: [{ id: 'aBcDe12345', images: { fixed_width: { width: '200', height: '150' } } }, { id: 'XyZ9876543', images: { fixed_width: { width: '200', height: '260' } } }, { id: 'Qwert11111', images: { fixed_width: { width: '200', height: '200' } } }] };
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const mk = async (name, id) => {
    const p = await (await b.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, hasTouch: true })).newPage(); p.errs = []; p.on('pageerror', (e) => p.errs.push(e.message));
    p.giphy = [];
    await p.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
    await p.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
    await p.route('https://api.giphy.com/**', (r) => { p.giphy.push(r.request().url()); r.fulfill({ contentType: 'application/json', headers: { 'access-control-allow-origin': '*' }, body: JSON.stringify(GIFS) }); });
    await p.route('https://media.giphy.com/**', (r) => r.fulfill({ contentType: 'image/png', body: PNG }));
    await p.goto('http://127.0.0.1:8765/index.html'); await p.waitForTimeout(3500);
    await p.evaluate(([u, n]) => window.cloud.signUp(u, 'abc123', n), [id, name]);
    await p.reload(); await p.waitForTimeout(6000);
    p.uid = await p.evaluate(() => window.cloud.uid);
    return p;
  };
  const go = async (p, t) => {
    for (let i = 0; i < 2; i++) {
      if (await p.evaluate((t) => { const b = document.querySelector(`.tabbar .tab[data-tab="${t}"]`); if (b) b.click(); return !!b; }, t)) break;
      await p.evaluate(() => window.tabbar.flipPage()); await p.waitForTimeout(600);
    }
    await p.waitForTimeout(2500);
  };
  const toast = async (p, t) => { for (let i = 0; i < 60; i++) { if (((await p.textContent('.toast').catch(() => '')) || '').includes(t)) return true; await p.waitForTimeout(250); } return false; };
  const t = Date.now().toString(36).slice(-4);
  const A = await mk('릴스주인', 'ra' + t);
  const B = await mk('구경꾼', 'rb' + t);
  await fetch(`${DB}/meta/giphy`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { key: { stringValue: 'testkey1234567' } } }) });
  await fetch(`${DB}/wallets/${B.uid}?updateMask.fieldPaths=bal`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { bal: { integerValue: '5000' } } }) });
  await A.evaluate(() => window.cloud.postCreate('오늘 첫 릴스예요', '', '', ''));
  await B.reload(); await B.waitForTimeout(6000);
  await go(B, 'reels');
  await B.click('#reels [data-act="replies"]'); await B.waitForTimeout(1500);
  ok('댓글을 누르면 어두운 판이 올라와요', await B.isVisible('#rc .rc-sheet') && await B.evaluate(() => document.getElementById('rc').classList.contains('on')));
  ok('게시물은 위로 작게 줄어요', await B.evaluate(() => document.documentElement.classList.contains('rc-open') && getComputedStyle(document.getElementById('reels')).transform.startsWith('matrix(0.52')));
  const l1 = await B.textContent('#rc-list');
  ok('맨 위에 글쓴이의 글, 빈 댓글 안내', l1.includes('릴스주인') && l1.includes('오늘 첫 릴스예요') && l1.includes('아직 댓글이 없어요'));
  ok('입력줄: 내 사진·대화에 참여하세요·GIF·선물 (이모지 줄 없음)', await B.isVisible('#rc-me .avatar') && (await B.getAttribute('#rc-in', 'placeholder')) === '대화에 참여하세요...' && await B.isVisible('#rc-gifb') && await B.isVisible('#rc-giftb') && !(await B.$('.rc-emoji')));
  await B.fill('#rc-in', '멋져요'); await B.press('#rc-in', 'Enter'); await B.waitForTimeout(2500);
  ok('댓글이 달려요', (await B.textContent('#rc-list')).includes('멋져요') && (await B.inputValue('#rc-in')) === '');
  await B.screenshot({ path: path.join(__dirname, 'shots', 'reels-comments.png') });
  // GIF
  await B.click('#rc-gifb'); await B.waitForTimeout(1500);
  ok('GIF: GIPHY 검색칸과 인기 GIF 두 줄', await B.isVisible('#rc-q') && (await B.getAttribute('#rc-q', 'placeholder')) === 'GIPHY 검색' && (await B.$$('#rc-grid .rc-g')).length === 3 && B.giphy.some((u) => u.includes('/trending')));
  await B.fill('#rc-q', '고양이'); await B.waitForTimeout(1200);
  ok('GIF 검색어로 찾아요', B.giphy.some((u) => u.includes('/search') && u.includes(encodeURIComponent('고양이'))));
  await B.screenshot({ path: path.join(__dirname, 'shots', 'reels-gif.png') });
  await B.click('#rc-grid .rc-g'); await B.waitForTimeout(2500);
  ok('GIF를 누르면 댓글로 가요', await B.isVisible('#rc-list img.rc-gifimg') && (await B.getAttribute('#rc-list img.rc-gifimg', 'src')).startsWith('https://media.giphy.com/media/aBcDe12345/200w.webp'));
  // 선물
  await B.click('#rc-giftb'); await B.waitForTimeout(600);
  ok('선물 판: 받는 사람과 내 멸공', (await B.textContent('#rc-gt')).includes('릴스주인') && (await B.textContent('#rc-gb')).includes('5,000'));
  await B.click('#rc-chips [data-v="1000"]');
  ok('금액 칩', (await B.inputValue('#rc-amt')) === '1,000');
  await B.click('#rc-send');
  ok('선물했어요', await toast(B, '릴스주인님에게 멸공 1,000을 선물했어요'));
  await B.waitForTimeout(1500);
  ok('서버: 보낸 사람 지갑 4,000', int((await raw(`wallets/${B.uid}`)).fields?.bal) === 4000);
  // 끌어 내려 닫기
  const g = await B.locator('#rc-grab').boundingBox();
  await B.mouse.move(g.x + g.width / 2, g.y + 10); await B.mouse.down();
  for (let i = 1; i <= 10; i++) { await B.mouse.move(g.x + g.width / 2, g.y + 10 + i * 20); await B.waitForTimeout(16); }
  await B.mouse.up(); await B.waitForTimeout(800);
  ok('끌어 내리면 닫혀요', !(await B.evaluate(() => document.getElementById('rc').classList.contains('on'))) && !(await B.evaluate(() => document.documentElement.classList.contains('rc-open'))));
  // 글쓴이: SNS 알림에서 받기
  await go(A, 'untitled3'); await A.waitForTimeout(2000);
  ok('SNS 종에 빨간 점', await A.isVisible('#sn-dot'));
  await A.click('#sns-bell'); await A.waitForTimeout(1500);
  ok('알림: 누가 얼마를 선물했는지', (await A.textContent('#sn-list')).includes('구경꾼') && (await A.textContent('#sn-list')).includes('1,000 멸공'));
  await A.screenshot({ path: path.join(__dirname, 'shots', 'sns-noti.png') });
  await A.click('#sn-list .sn-take');
  ok('받았어요', await toast(A, '멸공 1,000을 받았어요'));
  await A.waitForTimeout(1500);
  ok('서버: 받은 사람 지갑 1,000, 선물은 받음', int((await raw(`wallets/${A.uid}`)).fields?.bal) === 1000);
  ok('알림 줄은 받음으로', (await A.textContent('#sn-list')).includes('받음') && !(await A.isVisible('#sn-dot')));
  const errs = [...A.errs, ...B.errs];
  ok('화면 오류 없음', errs.length === 0); if (errs.length) console.log(errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close();
  process.exit(res.every(Boolean) ? 0 : 1);
})();
