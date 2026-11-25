// 화면: 놀기 탭에서 약속을 올리고(모두에게·고른 사람만), 다른 사람이 참여·취소하고, 주인이 지워요. 접속한 사람은 초록 점이 떠요
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
  const mk = async (name, id) => {
    const p = await (await b.newContext({ viewport: { width: 390, height: 844 } })).newPage(); p.errs = []; p.on('pageerror', (e) => p.errs.push(e.message));
    await p.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
    await p.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
    await p.goto('http://127.0.0.1:8765/index.html'); await p.waitForTimeout(3500);
    await p.evaluate(([u, n]) => window.cloud.signUp(u, 'abc123', n), [id, name]);
    await p.reload(); await p.waitForTimeout(6000);
    p.uid = await p.evaluate(() => window.cloud.uid);
    return p;
  };
  const play = async (p) => { await p.evaluate(() => window.tabbar.flipPage()); await p.waitForTimeout(600); await p.evaluate(() => document.querySelector('.tabbar .tab[data-tab="play"]').click()); await p.waitForTimeout(3000); };
  const toast = async (p, t) => { for (let i = 0; i < 40; i++) { if (((await p.textContent('.toast').catch(() => '')) || '').includes(t)) return true; await p.waitForTimeout(250); } return false; };
  const t = Date.now().toString(36).slice(-4);
  const A = await mk('약속왕', 'pa' + t);
  const B = await mk('참여러', 'pb' + t);
  const C = await mk('구경꾼', 'pc' + t);
  await play(A);
  ok('놀기 탭: 빈 안내와 + 버튼', (await A.textContent('#play-feed')).includes('아직 약속이 없어요') && await A.isVisible('#sns-fab'));
  await A.click('#sns-fab'); await A.waitForTimeout(700);
  ok('새 약속 시트', await A.isVisible('#play-compose .sns-panel'));
  await A.fill('#pl-title', '한강 치맥'); await A.fill('#pl-place', '여의도'); await A.fill('#pl-text', '7시에 봐요');
  await A.click('#pl-post');
  ok('올렸어요', await toast(A, '약속을 올렸어요'));
  await A.waitForTimeout(2000);
  ok('카드: 제목·장소·내일·1명 참여', (await A.textContent('#play-feed')).includes('한강 치맥') && (await A.textContent('#play-feed')).includes('여의도') && (await A.textContent('#play-feed')).includes('내일') && (await A.textContent('#play-feed')).includes('1명 참여'));
  // 고른 사람만
  await A.click('#sns-fab'); await A.waitForTimeout(700);
  await A.fill('#pl-title', '비밀 보드게임'); await A.click('#pl-who-pick'); await A.waitForTimeout(1500);
  ok('사람 고르기 목록', (await A.textContent('#pl-people')).includes('참여러'));
  ok('아무도 안 고르면 올리기 막힘', await A.isDisabled('#pl-post'));
  await A.click(`#pl-people [data-pick="${B.uid}"]`);
  await A.click('#pl-post'); await toast(A, '약속을 올렸어요'); await A.waitForTimeout(1500);
  await A.screenshot({ path: path.join(__dirname, 'shots', 'play.png') });
  await play(B);
  const bf = await B.textContent('#play-feed');
  ok('부른 사람: 둘 다 보여요', bf.includes('한강 치맥') && bf.includes('비밀 보드게임') && bf.includes('부른 사람만'));
  await play(C);
  const cf = await C.textContent('#play-feed');
  ok('안 부른 사람: 공개 약속만', cf.includes('한강 치맥') && !cf.includes('비밀 보드게임'));
  // 참여
  await B.evaluate(() => [...document.querySelectorAll('.pl-card')].find((c) => c.textContent.includes('한강')).querySelector('[data-act="join"]').click());
  ok('참여했어요', await toast(B, '참여했어요'));
  await B.waitForTimeout(2000);
  const card = async (p) => p.evaluate(() => [...document.querySelectorAll('.pl-card')].find((c) => c.textContent.includes('한강')).textContent);
  ok('2명 참여 · 참여 중', (await card(B)).includes('2명 참여') && (await card(B)).includes('참여 중'));
  const plans = (await raw('plans')).documents || [];
  const pub = plans.find((d) => d.fields.title.stringValue === '한강 치맥');
  ok('서버: 명단에 둘', pub.fields.going.arrayValue.values.length === 2);
  await B.evaluate(() => [...document.querySelectorAll('.pl-card')].find((c) => c.textContent.includes('한강')).querySelector('[data-act="leave"]').click());
  ok('참여 취소', await toast(B, '참여를 취소했어요'));
  await B.waitForTimeout(1500);
  // 접속 점: B는 A가 접속 중으로 보여요
  ok('접속 중 초록 점', (await B.$$('#play-feed .on-dot')).length > 0);
  // 지우기
  await A.reload(); await A.waitForTimeout(6000); await play(A);
  await A.evaluate(() => [...document.querySelectorAll('.pl-card')].find((c) => c.textContent.includes('한강')).querySelector('[data-act="del"]').click());
  await A.waitForTimeout(600); await A.click('#alert-ok');
  ok('지웠어요', await toast(A, '약속을 지웠어요'));
  await A.waitForTimeout(1500);
  ok('서버: 공개 약속이 없어졌어요', !((await raw('plans')).documents || []).some((d) => d.fields.title.stringValue === '한강 치맥'));
  const errs = [...A.errs, ...B.errs, ...C.errs];
  ok('화면 오류 없음', errs.length === 0); if (errs.length) console.log(errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close(); process.exit(res.every(Boolean) ? 0 : 1);
})();
