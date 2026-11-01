// 진짜 번들 + 진짜 규칙 (에뮬레이터): 공지 올리기(투표), 사람마다 한 번 전체 화면, 투표 한 번, 오너만 집계
const { chromium } = require('playwright');
const fs = require('fs');
const emu = fs.readFileSync(require('path').join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = [];
const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const open = async (b) => {
  const ctx = await b.newContext({ viewport: { width: 390, height: 844 } });
  const page = await ctx.newPage();
  const errs = [];
  page.on('pageerror', (e) => errs.push(e.message));
  await page.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await page.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  await page.goto('http://127.0.0.1:8765/index.html'); await page.waitForTimeout(3500);
  return { page, errs };
};
const rawDoc = async (path) => (await (await fetch(`${DB}/${path}`, { headers: { authorization: 'Bearer owner' } })).json());
const call = (p, fn, args) => p.evaluate(([f, a]) => window.cloud[f](...a).then((r) => (r === undefined ? 'ok' : r), (e) => 'ERR ' + (e.code || e.message)), [fn, args]);
const wait = (p, ms) => p.waitForTimeout(ms);
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--disable-web-security'] });
  const O = await open(b), M = await open(b), N = await open(b);
  const pO = O.page, pM = M.page, pN = N.page;
  await pO.evaluate(() => window.cloud.signUp('owner', 'abc123', '관리자'));
  await pO.reload(); await wait(pO, 6000);
  const uO = await pO.evaluate(() => window.cloud.uid);
  await pM.evaluate(() => window.cloud.signUp('noticemem1', 'abc123', '공지받이'));
  await pM.reload(); await wait(pM, 6000);
  const uM = await pM.evaluate(() => window.cloud.uid);
  await pN.evaluate(() => window.cloud.signUp('noticenew1', 'abc123', '새사람'));
  await pN.reload(); await wait(pN, 6000);
  await wait(pO, 2300);

  ok('일반 사람은 공지를 올릴 수 없어요', (await call(pM, 'adminNotice', [{ title: '나쁜', body: '공지' }])).startsWith('ERR'));
  await wait(pO, 300);
  ok('제목 없이는 못 올려요', (await call(pO, 'adminNotice', [{ title: '  ', body: '내용' }])).startsWith('ERR'));
  ok('투표 항목이 1개면 못 올려요', (await call(pO, 'adminNotice', [{ title: '투표', body: '고르세요', options: ['하나'] }])).startsWith('ERR'));
  await wait(pO, 2300);
  const id1 = await call(pO, 'adminNotice', [{ title: '점검 안내', body: '오늘 밤 점검해요' }]);
  ok('오너: 투표 없는 공지 올리기', typeof id1 === 'string' && !id1.startsWith('ERR'));
  await wait(pO, 2300);
  const id2 = await call(pO, 'adminNotice', [{ title: '저녁 메뉴', body: '무엇을 먹을까요?', options: ['짜장', '짬뽕', '볶음밥'] }]);
  ok('오너: 투표 있는 공지 올리기', typeof id2 === 'string' && !id2.startsWith('ERR'));
  const idx = await rawDoc('meta/noticeIndex');
  ok('목록 맨 앞이 새 공지예요', idx.fields && idx.fields.ids.arrayValue.values[0].stringValue === id2);

  // 기존 회원: 올라온 공지 두 개를 차례로 한 번씩 봐요 (오래된 것부터)
  await wait(pM, 2300);
  const first = await call(pM, 'unseenNotice', []);
  ok('회원: 가장 오래된 공지부터 한 개', first && first.title === '점검 안내');
  ok('회원: 확인 누르면 기록에 남아요', (await call(pM, 'markNoticeSeen', [first.id])) === 'ok');
  const second = await call(pM, 'unseenNotice', []);
  ok('회원: 다음 공지는 투표가 있어요', second && second.title === '저녁 메뉴' && second.options.length === 3);
  ok('회원: 골라요 (짬뽕=1)', (await call(pM, 'voteNotice', [second.id, 1])) === 'ok');
  ok('회원: 같은 투표를 또 고칠 수 없어요', (await call(pM, 'voteNotice', [second.id, 2])).startsWith('ERR'));
  ok('회원: 닫으면 본 것으로 적혀요', (await call(pM, 'markNoticeSeen', [second.id])) === 'ok');
  await wait(pM, 2300);
  ok('회원: 다 본 뒤에는 아무것도 안 떠요', (await call(pM, 'unseenNotice', [])) === null);
  await wait(pM, 2300);
  ok('회원: 목록은 두 개 다 보여요', (await call(pM, 'noticeList', [await call(pM, 'noticeIds', [])])).length === 2);
  ok('회원은 목록을 다 못 봐요 (집계는 오너만)', (await call(pM, 'noticeTally', [{ id: id2, options: ['a', 'b', 'c'] }])).startsWith('ERR'));
  await wait(pO, 2300);
  const tally = await call(pO, 'noticeTally', [{ id: id2, options: ['짜장', '짬뽕', '볶음밥'] }]);
  ok('오너: 투표 집계 (짬뽕 1표)', Array.isArray(tally) && tally[1] === 1 && tally[0] === 0);

  // 공지보다 먼저 가입한 사람은 아직 안 본 공지를 봐요. 공지 뒤에 가입한 사람은 예전 공지를 띄우지 않아요
  await wait(pN, 2300);
  const nFirst = await call(pN, 'unseenNotice', []);
  ok('공지 전에 가입한 사람은 점검 안내를 봐요', nFirst && nFirst.title === '점검 안내');
  const X = await open(b);
  await X.page.evaluate(() => window.cloud.signUp('noticelate1', 'abc123', '늦은사람'));
  await X.page.reload(); await wait(X.page, 6000);
  ok('공지 뒤에 가입한 사람은 예전 공지를 띄우지 않아요', (await call(X.page, 'unseenNotice', [])) === null);

  // 화면: 로그인하면 전체 화면 공지가 한 번 떠요
  await pM.reload(); await wait(pM, 5000);
  const overlayOn = await pM.evaluate(() => !!document.querySelector('.notice-full'));
  ok('회원: 다시 열어도 이미 본 공지는 안 떠요', !overlayOn);
  await pN.evaluate(() => window.cloud.voteNotice ? 0 : 0);
  await pO.reload(); await wait(pO, 5000);
  ok('오너 화면: 이미 쓴 관리 화면에 오류가 없어요', O.errs.length === 0);
  const errs = [...O.errs, ...M.errs, ...N.errs, ...X.errs];
  ok('화면 오류 없음', errs.length === 0);
  if (errs.length) console.log('  errors:', errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close();
  process.exit(res.every(Boolean) ? 0 : 1);
})();
