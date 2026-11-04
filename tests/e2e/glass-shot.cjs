// 눈으로 확인하는 사진: 탭 바 유리 (쉬는 모습, 누른 모습). 시험이 아니에요
const { chromium } = require('playwright');
const path = require('path');
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--enable-webgl'] });
  const page = await (await b.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, colorScheme: process.env.DARK ? 'dark' : 'light' })).newPage();
  const logs = [];
  page.on('console', (m) => { if (m.type() !== 'log') logs.push(m.text()); });
  page.on('pageerror', (e) => logs.push('ERR ' + e.message));
  await page.goto('http://127.0.0.1:8765/index.html?' + (process.env.Q || '')); await page.waitForTimeout(1500);
  // 뒤에 비칠 색 있는 내용
  await page.evaluate(() => {
    const s = document.querySelector('.screen.is-active');
    const d = document.createElement('div');
    d.innerHTML = Array.from({ length: 30 }, (_, i) => `<div style="height:36px;margin:6px 16px;border-radius:10px;background:hsl(${i * 37} 80% 60%);color:#fff;font:700 20px system-ui;padding:6px">줄 ${i} 글자 ABC</div>`).join('');
    s.appendChild(d);
    s.scrollTop = 300;
  });
  await page.waitForTimeout(Number(process.env.WAIT || 18000)); console.log("shots", await page.evaluate(() => [...(window.backdrop?.shots || [])].map(([k, v]) => k + ":" + (v.canvas ? v.cssH : "none") + (v.dirty ? "*" : "")).join(" ")));
  const out = process.env.OUT;
  const clip = { x: 0, y: 680, width: 390, height: 164 };
  await page.screenshot({ path: path.join(out, 'bar-rest.png'), clip });
  const tab = await page.locator('.tabbar .tab[data-tab="money"]').boundingBox();
  await page.mouse.move(tab.x + tab.width / 2, tab.y + tab.height / 2); await page.mouse.down(); await page.waitForTimeout(700);
  await page.mouse.move(tab.x + tab.width / 2 + 30, tab.y + tab.height / 2, { steps: 5 }); await page.waitForTimeout(400);
  await page.screenshot({ path: path.join(out, 'bar-press.png'), clip });
  await page.mouse.up(); await page.waitForTimeout(800);
  await page.screenshot({ path: path.join(out, 'full.png') });
  console.log(logs.slice(0, 8).join('\n'));
  await b.close();
})();
