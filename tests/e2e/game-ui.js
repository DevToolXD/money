// 화면: 게임 탭(독소전쟁). 두 사람이 레드팀(소련)·블루팀(독일)을 고르고 누르면, 클릭이 모여서 서버에 올라가고
// 두 팀 비율로 전선이 한 지역씩 옮겨 가요. 각 진영 가운데 클릭 수, 아래 팀별 랭킹. 릴스 탭에 SNS 게시물이 보여요.
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const H = { 'content-type': 'application/json', authorization: 'Bearer owner' };
const raw = async (p) => (await (await fetch(`${DB}/${p}`, { headers: H })).json());
const int = (f) => Number(f?.integerValue ?? f?.doubleValue ?? 0);
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security'] });
  const mk = async (name, id) => {
    const p = await (await b.newContext({ viewport: { width: 390, height: 844 }, hasTouch: true })).newPage(); p.errs = []; p.on('pageerror', (e) => p.errs.push(e.message));
    await p.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
    await p.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
    await p.goto('http://127.0.0.1:8765/index.html'); await p.waitForTimeout(3500);
    await p.evaluate(([u, n]) => window.cloud.signUp(u, 'abc123', n), [id, name]);
    await p.reload(); await p.waitForTimeout(6000);
    return p;
  };
  const go = async (p, t) => {
    for (let i = 0; i < 2; i++) {
      if (await p.evaluate((t) => { const b = document.querySelector(`.tabbar .tab[data-tab="${t}"]`); if (b) b.click(); return !!b; }, t)) break;
      await p.evaluate(() => window.tabbar.flipPage()); await p.waitForTimeout(600);
    }
    await p.waitForTimeout(2500);
  };
  const toast = async (p, t) => { for (let i = 0; i < 40; i++) { if (((await p.textContent('.toast').catch(() => '')) || '').includes(t)) return true; await p.waitForTimeout(250); } return false; };
  const hits = async (p, n) => { for (let i = 0; i < n; i++) { await p.dispatchEvent('#gm-hit', 'pointerdown', { pointerId: 1, isPrimary: true }); await p.waitForTimeout(70); } };
  const blues = (p) => p.evaluate(() => document.querySelectorAll('#gm-regs path.b').length);
  const t = Date.now().toString(36).slice(-4);
  const A = await mk('붉은병사', 'ga' + t);
  const B = await mk('푸른병사', 'gb' + t);

  // 릴스: SNS에 올린 게시물이 한 화면에 하나씩 보여요
  await A.evaluate(() => window.cloud.postCreate('릴스 시험 게시물', '', '', ''));
  await go(B, 'reels'); await B.waitForTimeout(1500);
  ok('릴스: 올린 게시물이 화면 가득', (await B.textContent('#reels')).includes('릴스 시험 게시물') && await B.isVisible('#reels .rl-s .rl-big'));
  ok('릴스: 쓴 사람·좋아요·댓글', (await B.textContent('#reels .rl-who')).includes('붉은병사') && await B.isVisible('#reels [data-act="like"]') && await B.isVisible('#reels [data-act="replies"]'));
  await B.dblclick('#reels .rl-s', { position: { x: 60, y: 120 } }); await B.waitForTimeout(1500);
  ok('릴스: 두 번 누르면 좋아요', await B.evaluate(() => document.querySelector('#reels [data-act="like"]').classList.contains('on')));
  await B.screenshot({ path: path.join(__dirname, 'shots', 'reels.png') });

  // 게임: 처음엔 10:10, 팀 고르기
  await go(A, 'game');
  ok('지도 20개 지역, 처음엔 반반', (await A.evaluate(() => document.querySelectorAll('#gm-regs path').length)) === 20 && (await blues(A)) === 10);
  ok('팀 고르기: 레드팀 소련 / 블루팀 독일', (await A.textContent('.gm-team.r')).includes('소련') && (await A.textContent('.gm-team.b')).includes('독일'));
  await A.click('.gm-team.r'); await A.waitForTimeout(600);
  ok('한 번 고르면 못 바꾼다고 확인', (await A.textContent('#alert-body')).includes('바꿀 수 없어요'));
  await A.click('#alert-ok');
  ok('레드팀 참전', await toast(A, '레드팀(소련)에 들어왔어요'));
  await A.waitForTimeout(800);
  ok('공격 버튼: 소련군', (await A.textContent('#gm-hit')).includes('소련군 공격'));
  await go(B, 'game');
  await B.click('.gm-team.b'); await B.waitForTimeout(600); await B.click('#alert-ok');
  ok('블루팀 참전', await toast(B, '블루팀(독일)에 들어왔어요'));
  await B.waitForTimeout(800);

  // 지도를 0.1초마다 기록해요 (한 번에 둘 이상 바뀌지 않는지 보려고)
  await A.evaluate(() => { window.__seen = []; setInterval(() => window.__seen.push(document.querySelectorAll('#gm-regs path.b').length), 100); });
  await hits(A, 30);
  ok('누르면 바로 내 클릭이 올라가요', (await A.textContent('#gm-my')).trim() === '30');
  await hits(B, 10);
  await A.waitForTimeout(6500);
  const g = (await raw('game/main')).fields || {};
  ok('서버: 소련 30, 독일 10 (모아서 한 번에)', int(g.r) === 30 && int(g.b) === 10);
  const pa = (await raw(`gamePlayers/${await A.evaluate(() => window.cloud.uid)}`)).fields || {};
  ok('서버: 내 기록 30', int(pa.clicks) === 30 && pa.team?.stringValue === 'r');
  // 목표 전선: 20 x 10/40 = 5 지역만 독일. 한 지역씩 점령해 가요 (0.2초마다 보며 한 번에 둘 이상 바뀌지 않는지)
  await A.waitForTimeout(9000);
  const seen = (await A.evaluate(() => window.__seen)).filter((v, i, a) => i === 0 || v !== a[i - 1]);
  console.log('독일 지역 수 흐름', seen.join(','));
  ok('한 지역씩 점령 (한 번에 다 바뀌지 않아요)', seen.every((v, i) => i === 0 || Math.abs(v - seen[i - 1]) <= 1) && seen.length > 2);
  ok('비율대로 전선: 독일 5 지역', (await blues(A)) === 5 && (await blues(B)) === 5);
  ok('점령 소식', (await A.textContent('#gm-news')).includes('점령했어요'));
  console.log('라벨', await A.textContent('#gm-rt'), await A.textContent('#gm-bt'));
  ok('각 진영 가운데 클릭 수', (await A.textContent('#gm-rt')).trim() === '30' && (await A.textContent('#gm-bt')).trim() === '10');
  ok('비율 막대 75% : 25%', (await A.textContent('#gm-rp')) === '75%' && (await A.textContent('#gm-bp')) === '25%');
  // 서버 규칙: 4초 안에 다시 보내거나 200을 넘게 보내면 막혀요 (빠른 클릭은 걸러요)
  await hits(A, 40);
  ok('너무 빠른 클릭은 1초 20번까지만', Number((await A.textContent('#gm-my')).replace(/,/g, '')) <= 30 + 40);
  await go(A, 'trade'); await go(A, 'game'); await A.waitForTimeout(1500);
  await A.waitForTimeout(31000);
  const rk = await A.textContent('#gm-rank');
  ok('랭킹: 팀별 이름과 점수', rk.includes('붉은병사') && rk.includes('푸른병사'));
  ok('내 순위', (await A.textContent('#gm-mine')).includes('소련팀에서 1등'));
  await A.screenshot({ path: path.join(__dirname, 'shots', 'game.png'), fullPage: false });
  const errs = [...A.errs, ...B.errs];
  ok('화면 오류 없음', errs.length === 0); if (errs.length) console.log(errs.slice(0, 3));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close();
  process.exit(res.every(Boolean) ? 0 : 1);
})();
