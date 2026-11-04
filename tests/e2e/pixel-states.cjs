// 러스트로 옮기기 전후 비교용 사진: 탭 바(쉬기·누르고 있기·핫바 넘김·다크), 위쪽 유리 버튼(쉬기·누르기). 시험이 아니에요
// OUT 폴더에 같은 이름으로 찍어서, tests/e2e/pixel-diff.py 로 비교해요
const { chromium } = require('playwright');
const path = require('path');
(async () => {
  const out = process.env.OUT;
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--disable-web-security'] });
  for (const dark of [false, true]) {
    const page = await (await b.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, colorScheme: dark ? 'dark' : 'light', reducedMotion: 'no-preference' })).newPage();
    const errs = []; page.on('pageerror', (e) => errs.push(e.message));
    await page.goto('http://127.0.0.1:8765/index.html'); await page.waitForTimeout(1500);
    await page.evaluate(() => {
      const s = document.querySelector('.screen.is-active');
      const d = document.createElement('div');
      d.innerHTML = Array.from({ length: 30 }, (_, i) => `<div style="height:36px;margin:6px 16px;border-radius:10px;background:hsl(${i * 37} 80% 60%);color:#fff;font:700 20px sans-serif;padding:6px">줄 ${i} 글자 ABC</div>`).join('');
      s.appendChild(d);
      const band = document.createElement('div');
      band.style.cssText = 'position:absolute;left:0;right:0;top:0;height:130px;background:repeating-linear-gradient(90deg,#ff3b30 0 30px,#ffcc00 30px 60px,#34c759 60px 90px,#0a7cff 90px 120px);z-index:0';
      s.prepend(band);
      s.scrollTop = 0;
    });
    // 화면 사진이 다시 찍힐 때까지 기다려요
    for (let i = 0; i < 60; i++) { const ok = await page.evaluate(() => [...(window.backdrop?.shots || [])].some(([k, v]) => k === 'messages' && v.canvas && v.cssH > 1500)); if (ok) break; await page.waitForTimeout(1000); }
    await page.waitForTimeout(1500);
    const tag = dark ? 'dark' : 'light';
    const bar = { x: 0, y: 680, width: 390, height: 164 };
    const top = { x: 0, y: 0, width: 390, height: 110 };
    await page.screenshot({ path: path.join(out, `${tag}-bar-rest.png`), clip: bar });
    await page.screenshot({ path: path.join(out, `${tag}-top-rest.png`), clip: top });
    // 누르고 가만히 (렌즈가 다 떠오른 뒤)
    const t = await page.locator('.tabbar .tab[data-tab="messages"]').boundingBox();
    await page.mouse.move(t.x + t.width / 2, t.y + t.height / 2); await page.mouse.down(); await page.waitForTimeout(2500);
    await page.screenshot({ path: path.join(out, `${tag}-bar-hold.png`), clip: bar });
    await page.mouse.up(); await page.waitForTimeout(2000);
    await page.screenshot({ path: path.join(out, `${tag}-bar-after.png`), clip: bar });
    // 위쪽 유리 버튼 누르고 가만히
    await page.evaluate(() => document.querySelector('.screen.is-active').scrollTop = 0); await page.waitForTimeout(800);
    const e = await page.locator('#edit-chats').boundingBox();
    await page.mouse.move(e.x + e.width / 2, e.y + e.height / 2); await page.mouse.down(); await page.waitForTimeout(1200);
    await page.screenshot({ path: path.join(out, `${tag}-top-hold.png`), clip: top });
    await page.mouse.up(); await page.waitForTimeout(1200);
    // 핫바 넘기기
    await page.evaluate(() => window.tabbar.flipPage()); await page.waitForTimeout(1500);
    await page.screenshot({ path: path.join(out, `${tag}-bar-page2.png`), clip: bar });
    console.log(tag, 'errs', JSON.stringify(errs));
    await page.close();
  }
  await b.close();
})();
