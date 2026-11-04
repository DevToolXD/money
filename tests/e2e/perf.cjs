// 렉 재기: 휴대폰처럼 CPU를 4배 느리게 하고, 탭 끌기·스크롤·화면 바꾸기 동안 프레임 간격과 긴 작업(50ms 넘는 것)을 재요. 시험이 아니에요
const { chromium } = require('playwright');
(async () => {
  const b = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--disable-web-security'] });
  const page = await (await b.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2 })).newPage();
  const errs = []; page.on('pageerror', (e) => errs.push(e.message));
  await page.goto(process.env.URL || 'http://127.0.0.1:8765/index.html'); await page.waitForTimeout(4000);
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Emulation.setCPUThrottlingRate', { rate: Number(process.env.CPU || 4) });
  await page.evaluate(() => {
    window.__perf = { gaps: [], long: [] };
    let last = 0;
    const f = (t) => { if (last) window.__perf.gaps.push(t - last); last = t; requestAnimationFrame(f); };
    requestAnimationFrame(f);
    new PerformanceObserver((l) => l.getEntries().forEach((e) => window.__perf.long.push([e.duration, e.name]))).observe({ type: 'longtask', buffered: false });
  });
  const phase = async (name, fn) => {
    await page.evaluate(() => { window.__perf.gaps = []; window.__perf.long = []; });
    await fn();
    const r = await page.evaluate(() => {
      const g = window.__perf.gaps.slice().sort((a, b) => a - b);
      const q = (p) => g[Math.min(g.length - 1, Math.floor(g.length * p))] || 0;
      return { frames: g.length, p50: q(0.5).toFixed(1), p95: q(0.95).toFixed(1), max: (g[g.length - 1] || 0).toFixed(1), over50: g.filter((x) => x > 50).length, long: window.__perf.long.map((x) => Math.round(x[0])) };
    });
    console.log(name.padEnd(10), JSON.stringify(r));
  };
  const tab = async (n) => page.locator(`.tabbar .tab[data-tab="${n}"]`).boundingBox();
  await phase('drag', async () => {
    const a = await tab('messages'); const z = await tab('shop');
    for (let k = 0; k < 3; k++) {
      await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2); await page.mouse.down();
      for (let i = 0; i <= 30; i++) { await page.mouse.move(a.x + a.width / 2 + ((z.x - a.x) * i) / 30, a.y + a.height / 2); await page.waitForTimeout(16); }
      for (let i = 30; i >= 0; i--) { await page.mouse.move(a.x + a.width / 2 + ((z.x - a.x) * i) / 30, a.y + a.height / 2); await page.waitForTimeout(16); }
      await page.mouse.up(); await page.waitForTimeout(400);
    }
  });
  await phase('scroll', async () => {
    await page.mouse.move(195, 400);
    for (let i = 0; i < 40; i++) { await page.mouse.wheel(0, i < 20 ? 60 : -60); await page.waitForTimeout(16); }
    await page.waitForTimeout(500);
  });
  await phase('tabs', async () => {
    for (const n of ['money', 'work', 'shop', 'messages', 'money', 'messages']) { const t = await tab(n); if (!t) continue; await page.mouse.click(t.x + t.width / 2, t.y + t.height / 2); await page.waitForTimeout(700); }
  });
  await phase('idle', async () => { await page.waitForTimeout(8000); });
  console.log('errs', JSON.stringify(errs));
  await b.close();
})();
