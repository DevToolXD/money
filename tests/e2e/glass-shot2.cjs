// 눈으로 확인하는 사진: CSS 유리 (위쪽 유리 버튼, 누른 모습, 로그인 화면). 시험이 아니에요
const { chromium } = require('playwright');
const path = require('path');
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
  const page = await (await b.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2 })).newPage();
  const logs = [];
  page.on('pageerror', (e) => logs.push('ERR ' + e.message));
  await page.goto('http://127.0.0.1:8765/index.html' + (process.env.Q || '')); await page.waitForTimeout(2000);
  await page.evaluate(() => {
    const s = document.querySelector('.screen.is-active');
    const d = document.createElement('div');
    d.innerHTML = Array.from({ length: 30 }, (_, i) => `<div style="height:36px;margin:6px 16px;border-radius:10px;background:hsl(${i * 37} 80% 60%);color:#fff;font:700 20px system-ui;padding:6px">줄 ${i} 글자 ABC</div>`).join('');
    s.prepend(d);
    s.scrollTop = 0;
    const band = document.createElement('div');
    band.style.cssText = 'position:absolute;left:0;right:0;top:0;height:130px;background:repeating-linear-gradient(90deg,#ff3b30 0 30px,#ffcc00 30px 60px,#34c759 60px 90px,#0a7cff 90px 120px);z-index:0';
    band.innerHTML = '<b style="font:800 34px system-ui;color:#fff;position:absolute;top:40px;left:20px">글자 TEXT 123</b>';
    s.prepend(band);
  });
  await page.waitForTimeout(800);
  const out = process.env.OUT;
  console.log('lens', await page.evaluate(() => window.LiquidGlass && [window.LiquidGlass.lens, window.LiquidGlass.count()]));
  await page.screenshot({ path: path.join(out, 'top.png'), clip: { x: 0, y: 0, width: 390, height: 120 } });
  const bx = await page.locator('#edit-chats').boundingBox();
  await page.mouse.move(bx.x + bx.width / 2, bx.y + bx.height / 2); await page.mouse.down(); await page.waitForTimeout(150);
  await page.mouse.move(bx.x + bx.width / 2 + 25, bx.y + bx.height / 2 + 10, { steps: 6 }); await page.waitForTimeout(350);
  await page.screenshot({ path: path.join(out, 'top-press.png'), clip: { x: 0, y: 0, width: 390, height: 120 } });
  await page.mouse.up(); await page.waitForTimeout(900);
  await page.screenshot({ path: path.join(out, 'top-after.png'), clip: { x: 0, y: 0, width: 390, height: 120 } });
  await page.evaluate(() => document.querySelector('.js-open-login, #open-login').click()); await page.waitForTimeout(1500);
  await page.screenshot({ path: path.join(out, 'login.png') });
  console.log(logs.join('\n'));
  await b.close();
})();
