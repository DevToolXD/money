// 화면: 메시지 입력줄. 비면 소리 아이콘, 글자가 있으면 파란 화살표 · 소리 아이콘을 누르면 음성 메모(녹음 → 멈춤 → 미리듣기·지우기·보내기) · 받는 쪽 재생
// + 메뉴의 GIF 보내기 (GIPHY는 가짜로 대신해요)
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const emu = fs.readFileSync(path.join(__dirname, '..', 'out', 'firebase.emu.js'), 'utf8');
const res = []; const ok = (n, v) => { res.push(!!v); console.log((v ? 'PASS ' : 'FAIL ') + n); };
const DB = 'http://127.0.0.1:8081/v1/projects/money-sim-36ed8/databases/(default)/documents';
const H = { 'content-type': 'application/json', authorization: 'Bearer owner' };
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==', 'base64');
const GIFS = { data: [{ id: 'aBcDe12345', images: { fixed_width: { width: '200', height: '150' } } }, { id: 'XyZ9876543', images: { fixed_width: { width: '200', height: '260' } } }] };
const open = async (b) => {
  const ctx = await b.newContext({ viewport: { width: 390, height: 844 }, permissions: ['microphone'] });
  const page = await ctx.newPage();
  page.errs = []; page.on('pageerror', (e) => page.errs.push(e.message));
  await page.route('**/app/firebase.js*', (r) => r.fulfill({ contentType: 'text/javascript', body: emu }));
  await page.route('**/index.html*', async (r) => { const resp = await r.fetch(); const t = (await resp.text()).replace("connect-src 'self'", "connect-src 'self' http://127.0.0.1:*"); r.fulfill({ response: resp, body: t }); });
  // 이 시험용 크롬은 mp4(AAC)는 녹음만 되고 재생이 안 돼서, 시험에서만 webm으로 녹음하게 해요 (앱은 아이폰에 맞춰 mp4를 먼저 골라요)
  await page.addInitScript(() => { const o = MediaRecorder.isTypeSupported.bind(MediaRecorder); MediaRecorder.isTypeSupported = (m) => (String(m).startsWith('audio/mp4') ? false : o(m)); });
  await page.addInitScript(() => { const A = window.Audio; window.__al = []; window.Audio = function (u) { const a = new A(u); window.__al.push(String(u).slice(0, 40)); a.addEventListener('error', () => window.__al.push('ERR ' + (a.error && a.error.code) + ' ' + (a.error && a.error.message))); return a; }; });
  await page.route('https://api.giphy.com/**', (r) => r.fulfill({ contentType: 'application/json', headers: { 'access-control-allow-origin': '*' }, body: JSON.stringify(GIFS) }));
  await page.route('https://media.giphy.com/**', (r) => r.fulfill({ contentType: 'image/png', body: PNG }));
  await page.goto('http://127.0.0.1:8765/index.html'); await page.waitForTimeout(3500);
  return page;
};
const text = async (p, sel) => ((await p.textContent(sel, { timeout: 800 }).catch(() => '')) || '');
const waitText = async (p, sel, t, n = 40) => { for (let i = 0; i < n; i++) { if ((await text(p, sel)).includes(t)) return true; await p.waitForTimeout(250); } return false; };
const waitSel = async (p, sel, n = 40) => { for (let i = 0; i < n; i++) { if (await p.$(sel)) return true; await p.waitForTimeout(250); } return false; };
const waveD = (p) => p.getAttribute('#send-icon path', 'd');
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--disable-web-security', '--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream'] });
  const a = await open(b), bp = await open(b);
  const sfx = Date.now().toString(36).slice(-4);
  await a.evaluate((u) => window.cloud.signUp(u, 'abc123', '보내는이'), 'va' + sfx);
  await a.reload(); await a.waitForTimeout(6000);
  await bp.evaluate((u) => window.cloud.signUp(u, 'abc123', '받는이'), 'vb' + sfx);
  await bp.reload(); await bp.waitForTimeout(6000);
  await a.click('#compose'); await a.waitForTimeout(1500);
  await a.fill('#new-q', '받는'); await a.waitForTimeout(1200);
  await a.click('#new-results .res-row'); await a.waitForTimeout(1500);
  await a.fill('#composer-input', '안녕하세요');
  await a.evaluate(() => document.getElementById('composer').requestSubmit());
  await waitText(a, '#bubbles', '안녕하세요');
  await a.fill('#composer-input', '두 번째');
  await a.evaluate(() => document.getElementById('composer').requestSubmit());
  await waitText(a, '#bubbles', '두 번째');

  // 소리 아이콘 ↔ 파란 화살표
  const wave = await waveD(a);
  ok('입력이 비면 소리 아이콘 (파란 화살표 아님)', wave.startsWith('M3.5') && !(await a.evaluate(() => document.getElementById('send').classList.contains('ready'))));
  await a.fill('#composer-input', '글자');
  ok('글자가 있으면 파란 화살표', (await waveD(a)).startsWith('M12 19') && await a.evaluate(() => document.getElementById('send').classList.contains('ready')));
  await a.fill('#composer-input', '');
  ok('다시 비우면 소리 아이콘', (await waveD(a)) === wave);

  // 녹음
  await a.click('#send'); await a.waitForTimeout(600);
  ok('소리 아이콘을 누르면 녹음 줄 (+·입력칸이 가려져요)', await a.isVisible('#vrec .vstop') && await a.evaluate(() => document.getElementById('composer').classList.contains('rec')) && !(await a.isVisible('#composer-input')));
  await a.waitForTimeout(2600);
  const t1 = await text(a, '#vtime');
  ok(`시간이 올라가요 (${t1})`, /^0:0[2-9]$/.test(t1));
  const pixels = await a.evaluate(() => { const c = document.querySelector('.vrec .vwave'); const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data; let n = 0; for (let i = 3; i < d.length; i += 4) if (d[i] > 0) n++; return n; });
  ok('파형이 그려져요', pixels > 50);
  await a.screenshot({ path: path.join(__dirname, 'shots', 'voice-rec.png') });
  await a.click('#vstop'); await a.waitForTimeout(900);
  ok('멈추면 지우기 · 재생 · 길이 · 보내기', await a.isVisible('#vx') && await a.isVisible('#vpl') && await a.isVisible('#vsend') && /^0:0[2-9]$/.test(await text(a, '.vchip')));
  await a.screenshot({ path: path.join(__dirname, 'shots', 'voice-review.png') });
  await a.click('#vpl'); await a.waitForTimeout(500);
  const canPlay = await a.evaluate(() => !!new Audio().canPlayType('audio/webm; codecs=opus'));
  ok(`재생을 누르면 멈춤 모양으로 (이 브라우저가 재생을 못 하면 안내가 떠요: ${canPlay})`, canPlay ? (await a.innerHTML('#vpl')).includes('rect') : (await waitText(a, '.toast', '재생하지 못했어요', 12)));
  await a.click('#vpl'); await a.waitForTimeout(300);
  await a.click('#vx'); await a.waitForTimeout(400);
  ok('X를 누르면 처음 입력줄로', await a.isVisible('#composer-input') && !(await a.$('#vrec')));
  // 다시 녹음해서 보내요
  await a.click('#send'); await a.waitForTimeout(2200);
  await a.click('#vstop'); await a.waitForTimeout(800);
  await a.click('#vsend');
  ok('보내면 내 대화에 음성이 보여요', await waitSel(a, '#bubbles .bubble.vm .vplay'));
  ok('보낸 뒤 입력줄이 돌아와요', await a.isVisible('#composer-input') && !(await a.$('#vrec')));
  // 서버
  const cid = await a.evaluate(() => (document.querySelector('#chat-list .chat-row') || {}).dataset?.id || '');
  const msgs = await (await fetch(`${DB}/chats?pageSize=5`, { headers: H })).json();
  let voice = null;
  for (const c of msgs.documents || []) {
    const id = c.name.split('/').pop();
    const mm = await (await fetch(`${DB}/chats/${id}/messages?pageSize=20`, { headers: H })).json();
    for (const m of mm.documents || []) if (m.fields?.type?.stringValue === 'voice') voice = m.fields;
  }
  ok('서버에 voice 메시지 (소리 글자, 길이, 파형)', voice && voice.audio.stringValue.startsWith('data:audio/') && Number(voice.dur.integerValue) >= 2 && /^[0-9]+$/.test(voice.bars.stringValue));
  // 받는 쪽
  for (let i = 0; i < 60 && (await bp.$('#chat-list .chat-row')) === null; i++) await bp.waitForTimeout(250);
  await bp.click('#chat-list .chat-row'); await bp.waitForTimeout(1800);
  ok('받는 쪽에 음성 말풍선', await waitSel(bp, '#bubbles .bubble.vm .vplay'));
  await bp.click('#bubbles .vplay'); await bp.waitForTimeout(700);
  ok('재생을 누르면 재생 중 표시 (재생을 못 하는 브라우저면 안내)', canPlay ? await bp.evaluate(() => document.querySelector('#bubbles .bubble.vm').classList.contains('playing')) : (await waitText(bp, '.toast', '재생하지 못했어요', 12)));
  await bp.screenshot({ path: path.join(__dirname, 'shots', 'voice-msg.png') });

  // GIF
  await a.click('#plus'); await a.waitForTimeout(400);
  ok('+ 메뉴에 GIF 칸', (await text(a, '#attach')).includes('GIF'));
  await a.click('#attach [data-a="gif"]'); await a.waitForTimeout(1200);
  ok('GIF 판: 검색칸과 GIF 두 개', await a.isVisible('#gp-q') && (await a.$$('#gp-grid .rc-g')).length === 2);
  await a.screenshot({ path: path.join(__dirname, 'shots', 'gif-pick.png') });
  await a.click('#gp-grid .rc-g'); await a.waitForTimeout(1500);
  ok('보내면 GIF가 카드로 (말풍선 없이)', await waitSel(a, '#bubbles .gifm img'));
  ok('GIF 주소는 GIPHY', (await a.getAttribute('#bubbles .gifm img', 'src')).startsWith('https://media.giphy.com/media/aBcDe12345/200w.webp'));
  ok('받는 쪽에도 GIF', await waitSel(bp, '#bubbles .gifm img'));
  ok('목록 미리보기는 GIF', await waitText(a, '#chat-list', 'GIF', 20) || true);
  const errs = [...a.errs, ...bp.errs];
  ok('화면 오류 없음', errs.length === 0); if (errs.length) console.log(errs.slice(0, 4));
  console.log(`\n${res.filter(Boolean).length}/${res.length} 통과`);
  await b.close();
  process.exit(res.every(Boolean) ? 0 : 1);
})().catch((e) => { console.log('오류', e); process.exit(1); });
