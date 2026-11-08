// 화면: 앱 틀. 로그인 화면(가입 전환, 입력 검사, 실명 확인, 화면으로 가입), 계정(이름·아이디·소개 고치기·공지 화면 오가기),
// 상태 버튼(배고픔·목마름·에너지), 창업(시트로), 회사 프로필(업종 바꾸기), 랭킹, 위쪽 바, 탭 오가기, 로그아웃
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
const setBal = (uid, v) => fetch(`${DB}/wallets/${uid}?updateMask.fieldPaths=bal`, { method: 'PATCH', headers: H, body: JSON.stringify({ fields: { bal: { integerValue: String(v) } } }) });
const text = async (p, sel) => ((await p.textContent(sel).catch(() => '')) || '');
const toastHas = async (p, t) => { for (let i = 0; i < 30; i++) { if ((await text(p, '.toast')).includes(t)) return true; await p.waitForTimeout(250); } return false; };
const tab = async (p, t) => {
  if (t === 'account') { await p.evaluate(() => document.querySelector('.tabbar .tab-account').click()); await p.waitForTimeout(450); await p.click('.qm-row[data-go="account"]'); return; }
  for (let i = 0; i < 2; i++) {
    if (await p.evaluate((t) => { const b = document.querySelector(`.tabbar .tab[data-tab="${t}"]`); if (b) b.click(); return !!b; }, t)) return;
    await p.evaluate(() => window.tabbar.flipPage()); await p.waitForTimeout(600);
  }
};
const active = (p) => p.evaluate(() => document.querySelector('.screen.is-active')?.dataset.screen);
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const O = await open(b);
  const p = O.page;
  const sfx = Date.now().toString(36).slice(-4);
  // 손님
  ok('처음 화면은 메시지', (await active(p)) === 'messages');
  ok('손님: 로그인하면 대화를 시작할 수 있어요', (await text(p, '#chat-empty')).includes('로그인하면'));
  await tab(p, 'account'); await p.waitForTimeout(600);
  ok('계정 탭: 로그인 카드', !(await p.isHidden('#auth-card')) && (await p.isHidden('#auth-row')));
  ok('위쪽 제목도 바뀌어요', (await text(p, '#topbar-title')) === (await p.evaluate(() => document.querySelector('.screen.is-active').dataset.title)));
  await p.click('#open-login'); await p.waitForTimeout(900);
  ok('로그인 화면이 열려요', await p.evaluate(() => document.getElementById('login').classList.contains('is-open')) && (await text(p, '#login-title')) === '로그인');
  await p.click('#login-submit'); await p.waitForTimeout(200);
  ok('빈칸이면 안내', (await text(p, '#login-error')) === '아이디와 비밀번호를 입력해 주세요');
  await p.fill('#login-email', 'a!'); await p.fill('#login-pw', 'abc123'); await p.click('#login-submit'); await p.waitForTimeout(200);
  ok('아이디 규칙 안내', (await text(p, '#login-error')).includes('영문, 숫자'));
  await p.click('#login-switch'); await p.waitForTimeout(500);
  ok('계정 만들기로 바뀌어요', (await text(p, '#login-title')) === '계정 만들기' && (await text(p, '#login-submit')) === '계정 만들기');
  await p.click('#login-name'); await p.waitForTimeout(400);
  ok('이름 칸을 누르면 실명 안내', (await text(p, '#alert-title')) === '실명으로 가입해 주세요');
  await p.click('#alert-ok'); await p.waitForTimeout(300);
  const id = 'sh' + sfx;
  await p.fill('#login-name', '홍길동');
  await p.fill('#login-email', id); await p.fill('#login-pw', 'abc123'); await p.fill('#login-pw2', 'abc124');
  await p.click('#login-submit'); await p.waitForTimeout(200);
  ok('비밀번호 확인이 다르면 안내', (await text(p, '#login-error')) === '비밀번호가 서로 달라요');
  await p.fill('#login-pw2', 'abc123');
  await p.click('#login-submit'); await p.waitForTimeout(400);
  ok('실명 확인 질문', (await text(p, '#alert-title')).includes('홍길동'));
  await p.click('#alert-ok');
  let inOk = false;
  for (let i = 0; i < 40 && !inOk; i++) { await p.waitForTimeout(250); inOk = !(await p.evaluate(() => document.getElementById('login').classList.contains('is-open'))); }
  ok('가입하면 로그인 화면이 닫혀요', inOk);
  ok('이 기기에 계정을 만들었다고 기억해요', (await p.evaluate(() => localStorage.getItem('acct-made'))) === id);
  await p.waitForTimeout(2500);
  ok('계정 화면: 이름과 아이디', (await text(p, '#profile-name')) === '홍길동' && (await text(p, '#profile-id')) === '@' + id);
  ok('로그아웃 줄이 보여요', !(await p.isHidden('#auth-row')));
  const uid = await p.evaluate(() => window.cloud.uid);

  // 소개 고치기
  await p.evaluate(() => document.querySelector('.js-edit-profile').click()); await p.waitForTimeout(500);
  ok('편집 시트: 이름을 바꿀 수 있어요', !(await p.isDisabled('#edit-name')) && (await p.inputValue('#edit-name')) === '홍길동');
  await p.fill('#edit-bio', '시험 소개');
  await p.click('#sheet-save'); await p.waitForTimeout(500);
  ok('소개가 바뀌어요', (await text(p, '#profile-bio')) === '시험 소개');
  // 공지 화면 오가기
  await p.click('#open-notices'); await p.waitForTimeout(600);
  ok('공지 화면', (await active(p)) === 'notices');
  await p.click('#notices-back'); await p.waitForTimeout(600);
  ok('뒤로 가면 계정', (await active(p)) === 'account');

  // 새로 만드는 동안: 상태 버튼은 쉬고, 메시지·SNS 말고는 무제 탭이에요
  await p.waitForTimeout(2000);
  ok('상태 버튼은 숨어 있어요', await p.isHidden('#status-fab'));
  const labels = await p.evaluate(() => [...document.querySelectorAll('.tabbar .tab:not(.tab-account) .tab-label')].map((e) => e.textContent));
  ok('첫 쪽: 메시지 + 무제 3개', labels.join(',') === '메시지,홈,무제,무제');
  await tab(p, 'home'); await p.waitForTimeout(1500);
  ok('홈 탭: 지도가 떠요', (await active(p)) === 'home' && await p.isVisible('#map-view') && await p.isVisible('#map-zin'));
  await tab(p, 'blank2'); await p.waitForTimeout(1200);
  ok('무제 탭: 빈 화면 글씨 모션', (await active(p)) === 'blank2' && (await text(p, '#screen-blank2')).includes('empty'));
  await p.evaluate(() => window.tabbar.flipPage()); await p.waitForTimeout(700);
  const labels2 = await p.evaluate(() => [...document.querySelectorAll('.tabbar .tab:not(.tab-account) .tab-label')].map((e) => e.textContent));
  ok('둘째 쪽: 무제 3개 + SNS', labels2.join(',') === '무제,무제,무제,SNS');
  await p.evaluate(() => window.tabbar.flipPage()); await p.waitForTimeout(700);

  // 위쪽 바: 내리면 작은 제목이 나와요
  await tab(p, 'account'); await p.waitForTimeout(600);
  await p.evaluate(() => { const s = document.querySelector('.screen.is-active'); s.scrollTop = 200; s.dispatchEvent(new Event('scroll')); }); await p.waitForTimeout(300);
  ok('내리면 위쪽 유리 바', await p.evaluate(() => document.getElementById('topbar').classList.contains('on')));

  // 로그아웃
  await p.evaluate(() => { const s = document.querySelector('.screen.is-active'); s.scrollTop = 0; });
  await p.click('#logout'); await p.waitForTimeout(5000);
  ok('로그아웃하면 손님 화면', (await p.evaluate(() => window.cloud?.user)) == null && (await p.evaluate(() => localStorage.getItem('profile-name'))) === null);

  // 같은 기기에서는 계정을 또 만들 수 없어요 (임시: 브라우저에 기억)
  ok('로그아웃해도 이 기기 계정 기억은 남아요', (await p.evaluate(() => localStorage.getItem('acct-made'))) === id);
  await p.evaluate(() => window.__shell && 0);
  await p.evaluate(() => { const b = document.querySelector('.js-open-login') || document.getElementById('open-login'); if (b) b.click(); }); await p.waitForTimeout(800);
  if (!(await p.evaluate(() => document.getElementById('login').classList.contains('signup')))) { await p.click('#login-switch'); await p.waitForTimeout(500); }
  await p.fill('#login-name', '김둘째'); await p.waitForTimeout(400);
  if (await p.isVisible('#alert-ok')) { await p.click('#alert-ok'); await p.waitForTimeout(400); }
  await p.fill('#login-email', 'two' + sfx); await p.fill('#login-pw', 'abc123'); await p.fill('#login-pw2', 'abc123');
  await p.click('#login-submit'); await p.waitForTimeout(400);
  if (await p.isVisible('#alert-ok')) { await p.click('#alert-ok'); await p.waitForTimeout(800); }
  ok('같은 기기에서 두 번째 가입은 막혀요', (await text(p, '#login-error')).includes('이 기기에서는 이미 계정을 만들었어요'));

  const errs = O.errs;
  ok('화면 오류 없음', errs.length === 0); if (errs.length) console.log(errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close(); process.exit(res.every(Boolean) ? 0 : 1);
})();
