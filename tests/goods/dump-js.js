// 예전 JS(app/shapes.js, app/biz.js)로 모든 그림·원가·이름을 한 줄씩 적어요. web/crates/goods/examples/dump.rs 와 같은 순서예요.
// 쓰는 법: sh tests/goods/same.sh  (git에서 예전 JS를 꺼내 비교해요)
const fs = require('fs');
const vm = require('vm');
const [shapesSrc, bizSrc] = [process.argv[2], process.argv[3]].map((p) => fs.readFileSync(p, 'utf8'));
const window = {};
const sandbox = { window, document: { getElementById: () => null, querySelector: () => null }, matchMedia: () => ({ matches: false }), console };
vm.createContext(sandbox);
vm.runInContext(shapesSrc, sandbox);
const biz = bizSrc
  .replace('let ctx = null;', 'let ctx = { safeImg: () => "", company: () => null };')
  .replace('window.Business = {', 'window.Business = { costLines, nameOf, thumbHTML,');
vm.runInContext(biz, sandbox);
const S = window.Shapes, B = window.Business;
const out = [];
const n = (v) => String(v);
const fams = [['bottle', S.KINDS.bottle], ['bag', [...S.KINDS.bag.chips, ...S.KINDS.bag.jelly, ...S.KINDS.bag.bread]], ['box', S.KINDS.box], ['meat', S.KINDS.meat], ['can', S.KINDS.can], ['cup', S.KINDS.cup], ['wrap', S.KINDS.wrap]];
for (const [fam, ks] of fams) {
  for (const k of [...ks, 'nope']) {
    for (let s = 0; s < 5; s++) {
      const r = S.body(fam, k, s);
      out.push(`B|${fam}|${k}|${s}|${r.body}|${r.ext}|${r.det}|${r.hl}|${r.win}|${r.xf}|${n(r.cy)}|${n(r.half)}|${n(r.top)}|${n(r.nr)}`);
      out.push(`T|${fam}|${k}|${s}|${B.thumbHTML(fam, k, s)}`);
    }
  }
}
for (const k of [...S.KINDS.cap, 'nope']) {
  for (let s = 0; s < 5; s++) {
    for (const [nr, t] of [[12, 22], [10.4, 30.7], [9, 18.5]]) {
      const c = S.cap(k, nr, t, s);
      out.push(`C|${k}|${s}|${n(nr)}|${n(t)}|${c.body}|${c.det}`);
    }
    out.push(`T|cap|${k}|${s}|${B.thumbHTML('cap', k, s)}`);
  }
}
const opt = (v) => (v == null ? 'None' : `Some(${v})`);
for (const item of ['water', 'jelly', 'chips', 'bread', 'lunchbox', 'meat', 'bar', 'drink', 'coffee']) {
  for (const shape of [null, 0, 7, 23, 49]) {
    for (const [cap, cc, color, content] of [[null, null, null, null], [13, 42, 5, 2], [49, null, 49, 5]]) {
      const p = { item, shape, cap, capColor: cc, color, content };
      const c = B.costLines(p);
      const key = `${item}|${opt(shape)}|${opt(cap)}|${opt(cc)}|${opt(color)}|${opt(content)}`;
      out.push(`P|${key}|${c.lines.map(([a, v]) => `${a}=${v}`).join(',')}|${c.total}|${B.nameOf(p)}`);
      out.push(`V|${key}|${B.pkv(p, shape === 7, { name: '사', logo: '' })}`);
    }
  }
}
console.log(out.join('\n'));
