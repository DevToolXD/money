/* 하단 탭 바: 막대 유리, 아이콘·글자, 선택 알약과 렌즈를 캔버스 하나에 WebGL로 직접 그려요.

   - 막대(알약형 탭 줄, 둥근 검색 버튼)는 그 자체가 리퀴드 글라스예요. Backdrop이 찍어 둔 뒤 화면 사진을 스크롤 위치만큼 밀어 읽어서
     확대하고, 가장자리에서 안쪽 내용을 끌어와 휘어 보이게 해요. 사진이 아직 없으면 CSS 유리 판(backdrop-filter)이 대신해요.
   - 선택 알약과 렌즈는 같은 물체예요. 누르면 알약이 떠올라 렌즈가 되고, 움직이다가 도착하면 다시 알약으로 내려앉아요.
   - 렌즈 가장자리는 굴절 모양을 직접 설계했고(안쪽 내용을 길게 끌어오는 번짐), 빨강·초록·파랑이 보는 자리를 다르게 줘서 프리즘 색이 생겨요.
   - 움직임은 해석해로 푸는 스프링이라 프레임 간격이 들쭉날쭉해도 떨리지 않아요.
   - WebGL을 못 쓰면 DOM 아이콘과 CSS 알약으로 대신 보여줘요. */
(() => {
  "use strict";

  const { icons: ICONS, size: ICON_BOX } = window.TabIcons;

  const NS = "http://www.w3.org/2000/svg";
  const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));
  const lerp = (a, b, t) => a + (b - a) * t;

  // 캔버스가 막대 밖으로 더 나가는 여백(렌즈 그림자 때문에)
  const PAD_X = 36;
  const PAD_TOP = 30;
  const PAD_BOTTOM = 48;

  // 아이폰 26에서 잰 스프링 값을 (진동수 rad/s, 감쇠비)로 옮겼어요. stiffness k, damping c → w = sqrt(k), z = c / (2w)
  const SPRING = {
    lift: { w: 25.14, z: 1.0 }, // 떠오르고 내려앉을 때: 임계 감쇠
    move: { w: 15.52, z: 0.85 }, // 탭 사이를 움직일 때
    trail: { w: 24.0, z: 0.75 }, // 손가락을 끌고 갈 때
    stretch: { w: 15.0, z: 0.7 }, // 빨리 움직일 때 길쭉해지는 정도
    press: { w: 25.14, z: 1.0 }, // 누르는 동안 살짝 부푸는 정도
  };
  const HANG_MS = 150; // 도착한 뒤 렌즈를 잠깐 들고 있다가 내려놓는 시간
  const GLASS_FADE = 0.28; // 막대 유리가 처음 나타날 때 CSS 유리 판에서 넘어오는 시간(초). app.css의 지연과 맞춰요

  /* ---------- 스프링 ---------- */

  // 매 프레임 dt만큼을 정확히 풀어요. 목표가 바뀌어도 위치와 속도는 이어져요.
  class Spring {
    constructor(value, preset, eps = 0.01) {
      this.x = value;
      this.v = 0;
      this.target = value;
      this.w = preset.w;
      this.z = preset.z;
      this.eps = eps;
    }

    use(preset) {
      this.w = preset.w;
      this.z = preset.z;
    }

    snap(value) {
      this.x = this.target = value;
      this.v = 0;
    }

    step(dt) {
      const y0 = this.x - this.target;
      const v0 = this.v;
      const w = this.w;
      const z = this.z;
      let y;
      let v;
      if (z < 1) {
        const wd = w * Math.sqrt(1 - z * z);
        const e = Math.exp(-z * w * dt);
        const c = Math.cos(wd * dt);
        const s = Math.sin(wd * dt);
        y = e * (y0 * c + ((v0 + z * w * y0) / wd) * s);
        v = e * (v0 * c - ((z * w * v0 + w * w * y0) / wd) * s);
      } else {
        const e = Math.exp(-w * dt);
        y = e * (y0 + (v0 + w * y0) * dt);
        v = e * (v0 - w * (v0 + w * y0) * dt);
      }
      this.x = this.target + y;
      this.v = v;
    }

    get settled() {
      return Math.abs(this.x - this.target) < this.eps && Math.abs(this.v) < this.eps * 20;
    }
  }

  /* ---------- 색 ---------- */

  const colorCanvas = document.createElement("canvas");
  colorCanvas.width = colorCanvas.height = 1;
  const colorCtx = colorCanvas.getContext("2d", { willReadFrequently: true });

  // "#3182f6", "rgb(...)", "rgba(...)" 를 [r, g, b, a] (0~1)로 바꿔요. 낯선 표기는 캔버스에 맡겨요.
  function parseColor(text) {
    const t = text.trim();
    let m = /^#([0-9a-f]{3,8})$/i.exec(t);
    if (m) {
      let h = m[1];
      if (h.length === 3 || h.length === 4) h = [...h].map((c) => c + c).join("");
      const n = (i) => parseInt(h.slice(i, i + 2), 16) / 255;
      return [n(0), n(2), n(4), h.length === 8 ? n(6) : 1];
    }
    m = /^rgba?\(([^)]+)\)$/i.exec(t);
    if (m) {
      const v = m[1].split(/[\s,/]+/).filter(Boolean).map((x) => (x.endsWith("%") ? parseFloat(x) / 100 : parseFloat(x)));
      return [v[0] / 255, v[1] / 255, v[2] / 255, v.length > 3 ? v[3] : 1];
    }
    colorCtx.clearRect(0, 0, 1, 1);
    colorCtx.fillStyle = "#000";
    colorCtx.fillStyle = t;
    colorCtx.fillRect(0, 0, 1, 1);
    const d = colorCtx.getImageData(0, 0, 1, 1).data;
    return [d[0] / 255, d[1] / 255, d[2] / 255, d[3] / 255];
  }

  /* ---------- 셰이더 ---------- */

  const VERT = `
attribute vec2 aPos;
void main() { gl_Position = vec4(aPos, 0.0, 1.0); }
`;

  const FRAG = `
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif

uniform sampler2D uMask;    // 아이콘·글자 덮개
uniform sampler2D uBack;    // 뒤 화면 사진
uniform sampler2D uBlur;    // 뒤 화면 사진 (흐린 버전)
uniform vec2 uPx;           // 캔버스 크기 (기기 픽셀)
uniform float uDpr;
uniform vec2 uSize;         // 캔버스 크기 (css px)

// 막대 유리
uniform float uGlass;       // 1이면 뒤 화면 사진으로 유리를 그려요
uniform float uGlassMix;    // 막대 유리가 얼마나 나타났나 (0 = CSS 유리 판, 1 = 캔버스 유리)
uniform vec2 uPageOff;      // 캔버스 좌표 → 화면 사진 좌표
uniform vec2 uPageSize;
uniform vec4 uBarA;         // 알약형 막대: 중심 x, y, 반폭, 반높이 (css px)
uniform vec4 uBarB;         // 둥근 검색 버튼
uniform vec2 uPress;        // 눌림으로 부푼 정도 (막대, 검색 버튼)
uniform vec4 uGlassOpt;     // 가장자리 폭, 가장자리에서 끌어오는 거리, 감쇠, 확대
uniform vec4 uGlassLook;    // 디테일 대비 남기는 정도, 밝게/어둡게 덮는 정도, 막대 흐림, 알약 흐림
uniform vec3 uGlassBase;    // 유리가 밝게/어둡게 덮는 색
uniform vec4 uPlatLine;     // 바깥 가는 선 (색 + 알파)
uniform vec4 uPlatRim;      // 안쪽 하이라이트, 가장자리 빛, 색 번짐, 0
uniform vec3 uPlatShadow;   // 진하기, 아래로 내려간 정도, 번짐

// 선택 알약/렌즈
uniform vec4 uLens;         // 중심 x, y, 반폭, 반높이 (css px)
uniform float uExp;         // 모양 지수: 2 = 타원, 클수록 네모에 가까운 둥근 사각형
uniform float uLift;        // 0 = 알약, 1 = 렌즈
uniform float uZoom;
uniform vec4 uOptics;       // 가장자리 폭, 안쪽 내용 끌어오는 거리, 감쇠 지수, 흐림
uniform vec4 uDisp;         // 색 번짐(세로), 색 번짐(가로), 빛 쪽 줄이기, 가로 방향 비율
uniform vec4 uLook;         // 반짝임, 둘레 빛, 무지개, 속 밝기
uniform vec3 uInk;
uniform vec3 uAccent;
uniform vec3 uBase;         // 뒤 사진이 없을 때 막대 유리의 대략적인 색
uniform vec4 uPill;
uniform vec4 uTint;
uniform vec4 uEdge;
uniform vec2 uLight;        // 빛이 오는 쪽 (y 아래가 +)
uniform vec3 uShadow;       // 진하기, 아래로 내려간 정도, 번짐

// 둥근 사각형(알약·원)의 거리와 바깥쪽 방향
float sdRound(vec2 q, vec2 b, float r) {
  vec2 a = abs(q) - b + r;
  return min(max(a.x, a.y), 0.0) + length(max(a, 0.0)) - r;
}
vec2 gradRound(vec2 q, vec2 b, float r) {
  vec2 a = abs(q) - b + r;
  vec2 sg = vec2(q.x < 0.0 ? -1.0 : 1.0, q.y < 0.0 ? -1.0 : 1.0);
  if (a.x > 0.0 && a.y > 0.0) return sg * normalize(a);
  if (a.x > a.y) return vec2(sg.x, 0.0);
  return vec2(0.0, sg.y);
}

// 초타원(superellipse)의 근사 거리와 바깥쪽 방향. 가장자리 가까이에서 정확해요.
float seDist(vec2 q, vec2 ab, float n, out vec2 grad) {
  vec2 r = max(abs(q) / ab, vec2(0.0001));
  float g = pow(r.x, n) + pow(r.y, n);
  float m = pow(g, 1.0 / n);
  vec2 sg = vec2(q.x < 0.0 ? -1.0 : 1.0, q.y < 0.0 ? -1.0 : 1.0);
  vec2 dm = pow(m, 1.0 - n) * pow(r, vec2(n - 1.0)) / ab * sg;
  float len = max(length(dm), 0.00001);
  grad = dm / len;
  return (m - 1.0) / len;
}

float cover(vec2 css) {
  return texture2D(uMask, css / uSize).a;
}

// 한 색깔 채널이 보는 곳을 약간 흐려서 모아요 (가장자리 가까울수록 더 흐려져요)
float tap(vec2 pos, vec2 e, vec2 t, float rt, float re) {
  float c = cover(pos) * 0.4;
  c += cover(pos + t * rt) * 0.15;
  c += cover(pos - t * rt) * 0.15;
  c += cover(pos + e * re) * 0.15;
  c += cover(pos - e * re) * 0.15;
  return c;
}

// ---- 뒤 화면 사진
// 큰 덩어리 색(분홍 카드 같은)은 그대로 비치고, 글자처럼 가는 디테일은 대비를 줄여 회색으로 비쳐요.
// 흐린 사진(lo)이 큰 덩어리, 선명한 사진과의 차이(hi - lo)가 디테일이에요.
vec3 pageColor(vec2 cp, float soft) {
  vec2 uv = (cp + uPageOff) / uPageSize;
  vec3 hi = texture2D(uBack, uv).rgb;
  vec3 lo = texture2D(uBlur, uv).rgb;
  vec3 detail = (hi - lo) * (1.0 - soft);
  float dl = dot(detail, vec3(0.299, 0.587, 0.114));
  detail = mix(detail, vec3(dl), 0.55) * uGlassLook.x;
  return mix(lo + detail, uGlassBase, uGlassLook.y);
}

// 막대(알약·원) 유리 안의 한 점이 보여주는 색: 중심에서 확대하고 가장자리 쪽에서는 안쪽 내용을 끌어와 휘어 보여요
vec3 barSample(vec2 pos, vec4 pl, float soft, float shiftScale) {
  vec2 q = pos - pl.xy;
  float d = sdRound(q, pl.zw, pl.w);
  vec2 e = -gradRound(q, pl.zw, pl.w);
  float x = clamp(max(-d, 0.0) / uGlassOpt.x, 0.0, 1.0);
  float shift = uGlassOpt.y * pow(1.0 - x, uGlassOpt.z) * shiftScale;
  vec2 sp = pl.xy + q / uGlassOpt.w + e * shift;
  return pageColor(sp, soft);
}

void main() {
  vec2 p = vec2(gl_FragCoord.x, uPx.y - gl_FragCoord.y) / uDpr;
  vec2 q = p - uLens.xy;
  vec2 hb = uLens.zw;
  vec2 g;
  float d = seDist(q, hb, uExp, g);

  // ====== 1. 막대 유리 (뒤 화면 사진이 있을 때만)
  vec4 outside = vec4(0.0);
  vec4 barA = vec4(uBarA.xy, uBarA.zw * (1.0 + 0.035 * uPress.x));   // 반지름은 반높이(.w)와 같아요
  vec4 barB = vec4(uBarB.xy, uBarB.zw * (1.0 + 0.035 * uPress.y));
  if (uGlass > 0.5) {
    float dA = sdRound(p - barA.xy, barA.zw, barA.w);
    float dB = sdRound(p - barB.xy, barB.zw, barB.w);
    bool useA = dA < dB;
    vec4 pl = useA ? barA : barB;
    float dP = useA ? dA : dB;

    float dsA = sdRound(p - barA.xy - vec2(0.0, uPlatShadow.y), barA.zw, barA.w);
    float dsB = sdRound(p - barB.xy - vec2(0.0, uPlatShadow.y), barB.zw, barB.w);
    float shadow = uPlatShadow.x * exp(-max(min(dsA, dsB), 0.0) / uPlatShadow.z);
    outside = vec4(0.0, 0.0, 0.0, shadow * uGlassMix);

    if (dP < 1.5) {
      float inP = clamp(0.5 - dP * uDpr, 0.0, 1.0) * uGlassMix;
      vec2 qp = p - pl.xy;
      vec2 ep = -gradRound(qp, pl.zw, pl.w);
      float sP = max(-dP, 0.0);
      float ch = uPlatRim.z;
      vec3 c = vec3(
        barSample(p, pl, uGlassLook.z, 1.0 + ch).r,
        barSample(p, pl, uGlassLook.z, 1.0).g,
        barSample(p, pl, uGlassLook.z, 1.0 - ch).b
      );
      float fac = 0.5 + 0.5 * dot(-ep, uLight);
      // 안쪽 가장자리를 따라 도는 하이라이트와, 가장자리에서 퍼지는 은은한 빛
      float hu = (sP - 2.1) / 0.9;   // pow()는 음수 밑이 정의되지 않아서 직접 제곱해요
      c += vec3(exp(-hu * hu) * (0.25 + 0.75 * fac) * uPlatRim.x);
      c += vec3(exp(-sP / 4.5) * (0.2 + 0.8 * fac) * uPlatRim.y);
      // 바깥 가는 선
      float line = (1.0 - smoothstep(0.0, 1.0, abs(dP + 0.55))) * uPlatLine.a;
      c = mix(c, uPlatLine.rgb, line);
      outside = outside * (1.0 - inP) + vec4(clamp(c, 0.0, 1.0), 1.0) * inP;
    }
  }

  // ====== 2. 검은 글자·아이콘
  float aOut = cover(p);
  outside = outside * (1.0 - aOut) + vec4(uInk * aOut, aOut);

  // 렌즈가 드리우는 그림자
  vec2 gs;
  float ds = seDist(q - vec2(0.0, uShadow.y), hb, uExp, gs);
  float sh = uShadow.x * uLift * exp(-max(ds, 0.0) / uShadow.z);
  outside = outside * (1.0 - sh) + vec4(0.0, 0.0, 0.0, sh);

  if (d > 1.5) {
    gl_FragColor = outside;
    return;
  }

  // ====== 3. 선택 알약 / 렌즈 안쪽
  float s = max(-d, 0.0);                       // 가장자리에서 안쪽으로 몇 px 들어왔나
  vec2 e = -g;                                  // 안쪽을 향하는 방향
  vec2 t = vec2(-e.y, e.x);                     // 가장자리를 따라가는 방향
  float bezel = uOptics.x;
  float x = clamp(s / bezel, 0.0, 1.0);
  float w = 1.0 - x;
  float facing = 0.5 + 0.5 * dot(-e, uLight);   // 빛이 오는 쪽 가장자리일수록 1
  // 맨 가장자리에서는 안쪽 내용을 길게 끌어와요(글자가 줄무늬로 늘어나요).
  // 아이폰 사진에서는 위아래 가장자리가 가장 많이 번지고 양옆은 거의 안 밀려서, 가로 방향은 덜 밀어요.
  float shift = uOptics.y * pow(w, uOptics.z) * (1.0 - uDisp.z * facing) * mix(1.0, uDisp.w, abs(e.x)) * uLift;

  vec2 base = uLens.xy + q / uZoom;
  float rt = uOptics.w * w * uLift;
  float re = rt * 0.5;
  float kr = 1.0 + uDisp.x;
  float kb = 1.0 - uDisp.x;
  vec2 posR = base + e * (shift * kr) - t * (shift * uDisp.y);
  vec2 posG = base + e * shift;
  vec2 posB = base + e * (shift * kb) + t * (shift * uDisp.y);
  vec3 cov;
  if (x >= 1.0) {
    cov = vec3(cover(base));
  } else {
    cov = vec3(tap(posR, e, t, rt, re), tap(posG, e, t, rt, re), tap(posB, e, t, rt, re));
  }

  vec4 fill = mix(uPill, uTint, uLift);
  float fa = fill.a;
  vec3 prem;
  float alpha;
  if (uGlass > 0.5) {
    // 렌즈는 막대 유리 위에 얹혀 있어서, 막대 유리가 보여주는 색을 다시 굴절해서 봐요
    vec4 pl = abs(uLens.x - barA.x) < abs(uLens.x - barB.x) ? barA : barB;
    float soft = mix(uGlassLook.w, uGlassLook.z * 0.5, uLift);
    vec3 bg = vec3(
      barSample(posR, pl, soft, 1.0).r,
      barSample(posG, pl, soft, 1.0).g,
      barSample(posB, pl, soft, 1.0).b
    );
    prem = (bg * (1.0 - fa) + fill.rgb * fa) * (1.0 - cov) + uAccent * cov;
    alpha = 1.0;
  } else {
    float am = max(cov.r, max(cov.g, cov.b));
    prem = uBase * (1.0 - fa) * (am - cov) + fill.rgb * fa * (1.0 - cov) + uAccent * cov;
    alpha = fa + (1.0 - fa) * am;
  }

  // ---- 빛
  float tilt = pow(w, uOptics.z * 0.5) * 1.5;
  vec3 n = normalize(vec3(-e * tilt, 1.0));
  vec3 lv = normalize(vec3(uLight * 0.85, 0.5));
  vec3 hv = normalize(lv + vec3(0.0, 0.0, 1.0));
  vec3 nr = normalize(vec3(-e * tilt * 1.12, 1.0));
  vec3 nb = normalize(vec3(-e * tilt * 0.88, 1.0));
  vec3 sp = vec3(
    pow(max(dot(nr, hv), 0.0), 30.0),
    pow(max(dot(n, hv), 0.0), 30.0),
    pow(max(dot(nb, hv), 0.0), 30.0)
  );
  vec3 light = sp * uLook.x * (0.3 + 0.7 * facing);

  // 안쪽 가장자리를 따라 도는 은은한 빛 (빛 쪽이 더 밝고 반대쪽에도 약하게)
  float rim = exp(-s / 3.2) * (0.35 + 0.65 * facing) + exp(-s / 5.0) * 0.18 * (1.0 - facing);
  light += vec3(rim * uLook.y);

  // 얇은 막에서 보이는 것 같은 무지개 선 두 줄: 가장자리에서 2.5px, 5px 안쪽
  float ang = atan(e.y, e.x);
  vec3 film = 0.5 + 0.5 * cos(6.2831853 * (vec3(0.0, 0.33, 0.67) + ang * 0.17 + 0.1));
  float lu1 = (s - 2.6) / 1.1;
  float lu2 = (s - 5.4) / 1.3;
  float lines = exp(-lu1 * lu1) + 0.7 * exp(-lu2 * lu2);
  light += film * lines * uLook.z * (0.25 + 0.75 * facing);

  // 속 밝기: 위쪽이 살짝 더 밝아요
  light += vec3(uLook.w * (0.5 + 0.5 * dot(q / max(hb, vec2(1.0)), -uLight)));

  light *= uLift;
  float la = clamp(max(light.r, max(light.g, light.b)), 0.0, 1.0);
  prem = prem * (1.0 - la) + light;
  alpha = alpha + (1.0 - alpha) * la;

  // 테두리 가는 선
  float line = 1.0 - smoothstep(0.0, 1.0, abs(d + 0.55));
  float la2 = line * uEdge.a * uLift * (0.45 + 0.55 * (1.0 - facing));
  prem = prem * (1.0 - la2) + uEdge.rgb * la2;
  alpha = alpha + (1.0 - alpha) * la2;

  prem = min(prem, vec3(alpha));
  vec4 inside = vec4(prem, alpha);

  float inMask = clamp(0.5 - d * uDpr, 0.0, 1.0);
  gl_FragColor = outside * (1.0 - inMask) + inside * inMask;
}
`;

  /* ---------- 탭 바 ---------- */

  class TabBar {
    constructor(nav, options = {}) {
      this.nav = nav;
      this.backdrop = options.backdrop || null; // 뒤 화면 사진을 찍어 주는 곳
      this.pageRoot = options.pageRoot || nav.closest(".app") || document.body;
      this.shots = new Map(); // 화면 이름 → { tex, blur, cssW, cssH }
      this.platA = nav.querySelector(".platter-main");
      this.platB = nav.querySelector(".platter-search");
      this.grabGroup = "main";
      this.canvas = nav.querySelector(".tabbar-canvas");
      this.tabs = [...nav.querySelectorAll(":scope > .tab")];
      this.pillEl = nav.querySelector(".pill");
      this.reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");

      // 튜닝 값 (CSS 픽셀)
      this.optics = {
        bezel: 13, // 굴절이 일어나는 가장자리 폭
        shift: 16, // 맨 가장자리에서 안쪽으로 가져오는 거리
        power: 2.2, // 안쪽으로 갈수록 줄어드는 정도
        blur: 1.6, // 가장자리에서의 흐림
        chroma: 0.16, // 빨강·파랑이 갈라지는 정도 (세로)
        tangent: 0.1, // 빨강·파랑이 갈라지는 정도 (가로)
        asym: 0.5, // 빛 쪽 가장자리에서 굴절을 얼마나 줄일지
        sideShift: 0.3, // 양옆 가장자리의 굴절 비율 (위아래를 1로 봤을 때)
        zoom: 1.2,
        ...options.optics,
      };
      this.look = { specular: 0.2, rim: 0.08, film: 0.05, glow: 0.0, ...options.look };
      // 막대 유리: 뒤 화면을 확대해서 보여주고 가장자리에서 휘어요
      this.glass = {
        bezel: 12, // 휘는 가장자리 폭
        shift: 9, // 맨 가장자리에서 안쪽으로 끌어오는 거리
        power: 1.5,
        zoom: 1.75, // 확대
        chroma: 0.05, // 색 번짐
        keep: null, // 뒤 화면의 글자 같은 디테일 대비를 얼마나 남길지 (null이면 테마 값)
        whiten: null, // 유리가 밝게(다크에서는 어둡게) 덮는 정도 (null이면 테마 값)
        rim: null, // 안쪽 가장자리 하이라이트 세기 (null이면 테마 값)
        lineGain: 1.0, // 바깥 가는 선 진하기 배율
        barBlur: 0.1, // 막대 유리가 뒤 화면을 얼마나 흐리게 보여줄지 (0 = 또렷, 1 = 흐린 사진)
        pillBlur: 0.88, // 선택 알약은 훨씬 뿌옇게
        ...options.glass,
      };

      this.current = Math.max(0, this.tabs.findIndex((t) => t.hasAttribute("aria-current")));
      this.press = null;
      this.releasing = false;
      this.arrivedAt = 0;
      this.lastPointerAt = -Infinity;
      this.geo = [];
      this.running = false;
      this.lastTime = 0;
      this.ready = false;
      this.glassOn = false;
      this.glassMix = 0;

      this.x = new Spring(0, SPRING.move, 0.05);
      this.lift = new Spring(0, SPRING.lift, 0.002);
      this.grab = new Spring(0, SPRING.press, 0.002);
      this.stretch = new Spring(0, SPRING.stretch, 0.002);

      this.buildIcons();
      this.bind();

      this.useGL(options.gl === false ? null : this.initGL());

      // 막대 크기가 달라질 때뿐 아니라 앱 틀 높이가 달라질 때도(주소창이 접히고 펴질 때) 막대의 화면 안 위치가 바뀌어요
      this.resizeObserver = new ResizeObserver(() => this.layout());
      this.resizeObserver.observe(nav);
      this.resizeObserver.observe(this.pageRoot);
      this.layout();
      // 글꼴이 늦게 도착하면 글자를 다시 그려요
      if (document.fonts?.ready) document.fonts.ready.then(() => this.layout());
      document.fonts?.addEventListener?.("loadingdone", () => this.layout());
    }

    /* ----- DOM 아이콘 (대체 화면용이면서 접근성용) ----- */

    buildIcons() {
      this.tabs.forEach((tab) => {
        const holder = tab.querySelector(".tab-icon");
        const parts = ICONS[tab.dataset.tab];
        if (!holder || !parts) return;
        const svg = document.createElementNS(NS, "svg");
        svg.setAttribute("viewBox", `0 0 ${ICON_BOX} ${ICON_BOX}`);
        svg.setAttribute("aria-hidden", "true");
        for (const part of parts) {
          const path = document.createElementNS(NS, "path");
          path.setAttribute("d", part.d);
          path.setAttribute("fill", part.fill ? "currentColor" : "none");
          if (part.evenodd) path.setAttribute("fill-rule", "evenodd");
          if (part.stroke) {
            path.setAttribute("stroke", "currentColor");
            path.setAttribute("stroke-width", part.stroke);
            path.setAttribute("stroke-linecap", "round");
            path.setAttribute("stroke-linejoin", "round");
          }
          if (part.transform) path.setAttribute("transform", `matrix(${part.transform.join(" ")})`);
          svg.appendChild(path);
        }
        holder.replaceChildren(svg);
      });
    }

    /* ----- WebGL ----- */

    initGL() {
      let gl = null;
      try {
        gl =
          this.canvas.getContext("webgl", {
            alpha: true,
            premultipliedAlpha: true,
            antialias: false,
            depth: false,
            stencil: false,
            powerPreference: "low-power",
          }) || null;
      } catch (_) {
        gl = null;
      }
      if (!gl) return null;

      const compile = (type, src) => {
        const sh = gl.createShader(type);
        gl.shaderSource(sh, src);
        gl.compileShader(sh);
        if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
          console.warn("셰이더 컴파일 실패:", gl.getShaderInfoLog(sh));
          return null;
        }
        return sh;
      };
      const vs = compile(gl.VERTEX_SHADER, VERT);
      const fs = compile(gl.FRAGMENT_SHADER, FRAG);
      if (!vs || !fs) return null;
      const prog = gl.createProgram();
      gl.attachShader(prog, vs);
      gl.attachShader(prog, fs);
      gl.linkProgram(prog);
      if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
        console.warn("셰이더 연결 실패:", gl.getProgramInfoLog(prog));
        return null;
      }
      gl.useProgram(prog);

      const buf = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, buf);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
      const loc = gl.getAttribLocation(prog, "aPos");
      gl.enableVertexAttribArray(loc);
      gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);

      this.u = {};
      for (const name of [
        "uMask", "uBack", "uBlur", "uPx", "uDpr", "uSize", "uGlass", "uGlassMix", "uPageOff", "uPageSize", "uBarA", "uBarB", "uPress",
        "uGlassOpt", "uGlassLook", "uGlassBase", "uPlatLine", "uPlatRim", "uPlatShadow",
        "uLens", "uExp", "uLift", "uZoom", "uOptics", "uDisp", "uLook",
        "uInk", "uAccent", "uBase", "uPill", "uTint", "uEdge", "uLight", "uShadow",
      ]) {
        this.u[name] = gl.getUniformLocation(prog, name);
      }

      this.tex = this.makeTexture(gl, null);
      gl.disable(gl.BLEND);
      gl.clearColor(0, 0, 0, 0);
      this.emptyTex = this.makeTexture(gl, null);
      return gl;
    }

    // WebGL을 쓰는지에 따라 보이는 것이 달라져요 (못 쓰면 DOM 아이콘, CSS 유리 판, CSS 알약)
    useGL(gl) {
      this.gl = gl;
      this.nav.classList.toggle("is-gl", Boolean(gl));
      this.nav.classList.toggle("is-fallback", !gl);
      if (!gl) {
        this.glassOn = false;
        this.glassMix = 0;
        this.nav.classList.remove("has-glass");
      }
    }

    makeTexture(gl, source) {
      const tex = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
      gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
      if (source) gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);
      else gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array([0, 0, 0, 0]));
      return tex;
    }

    // Backdrop이 화면 사진을 새로 찍을 때마다 불려요
    setShot(name, shot) {
      const gl = this.gl;
      if (!gl) return;
      const max = gl.getParameter(gl.MAX_TEXTURE_SIZE);
      if (shot.canvas.width > max || shot.canvas.height > max) return;
      const old = this.shots.get(name);
      if (old) {
        gl.deleteTexture(old.tex);
        gl.deleteTexture(old.blur);
      }
      this.shots.set(name, {
        tex: this.makeTexture(gl, shot.canvas),
        blur: this.makeTexture(gl, shot.blur),
        cssW: shot.cssW,
        cssH: shot.cssH,
      });
      this.requestRender();
    }

    // 낡은 화면 사진을 모두 버려요 (테마가 바뀌었을 때처럼 사진이 틀린 색일 때). 새 사진이 올 때까지는 CSS 유리 판이 대신해요.
    dropShots() {
      const gl = this.gl;
      if (gl) {
        for (const shot of this.shots.values()) {
          gl.deleteTexture(shot.tex);
          gl.deleteTexture(shot.blur);
        }
      }
      this.shots.clear();
      this.requestRender();
    }

    // 알약이 떠 있거나 움직이는 동안은 무거운 일(화면 사진 찍기)을 미뤄요
    busy() {
      return Boolean(this.press || this.releasing || this.lift.x > 0.02 || this.running);
    }

    /* ----- 배치와 그리기 재료 ----- */

    readTheme() {
      const cs = getComputedStyle(this.nav);
      const get = (name) => parseColor(cs.getPropertyValue(name) || "#000");
      this.theme = {
        ink: get("--tab-ink"),
        accent: get("--tab-accent"),
        base: get("--tab-base"),
        pill: get("--tab-pill"),
        tint: get("--tab-tint"),
        edge: get("--tab-edge"),
        shadow: parseFloat(cs.getPropertyValue("--tab-shadow")) || 0.2,
        glassBase: get("--tab-glass-base"),
        glassKeep: parseFloat(cs.getPropertyValue("--tab-glass-keep")) || 0.4,
        glassDesat: parseFloat(cs.getPropertyValue("--tab-glass-desat")) || 0.2,
        line: get("--tab-line"),
        platShadow: parseFloat(cs.getPropertyValue("--tab-plat-shadow")) || 0.14,
        rim: parseFloat(cs.getPropertyValue("--tab-plat-rim")) || 0.5,
      };
    }

    layout() {
      const nav = this.nav;
      const nr = nav.getBoundingClientRect();
      if (nr.width < 10 || nr.height < 10) return;
      const dpr = clamp(window.devicePixelRatio || 1, 1, 3);
      this.dpr = dpr;
      this.cw = nr.width + PAD_X * 2;
      this.ch = nr.height + PAD_TOP + PAD_BOTTOM;
      this.readTheme();

      this.geo = this.tabs.map((tab) => {
        const r = tab.getBoundingClientRect();
        const ir = tab.querySelector(".tab-icon").getBoundingClientRect();
        const lab = tab.querySelector(".tab-label");
        const lr = lab ? lab.getBoundingClientRect() : null;
        return {
          cx: r.left - nr.left + r.width / 2 + PAD_X,
          cy: r.top - nr.top + r.height / 2 + PAD_TOP,
          w: r.width,
          h: r.height,
          icon: { x: ir.left - nr.left + PAD_X, y: ir.top - nr.top + PAD_TOP, size: ir.width },
          label: lab && lr && lab.textContent.trim()
            ? {
                text: lab.textContent.trim(),
                cx: lr.left - nr.left + lr.width / 2 + PAD_X,
                cy: lr.top - nr.top + lr.height / 2 + PAD_TOP,
                font: (() => {
                  const c = getComputedStyle(lab);
                  return `${c.fontWeight} ${c.fontSize} ${c.fontFamily}`;
                })(),
              }
            : null,
        };
      });

      // 탭마다 알약과 렌즈의 크기: 알약은 칸 크기 그대로, 렌즈는 훨씬 넓고 높이는 막대와 거의 같아요
      const barH = nr.height;
      this.sizes = this.geo.map((g) => {
        const round = g.w >= g.h - 1; // 검색 버튼처럼 가로세로가 같은 칸
        return {
          pillHW: round ? g.w / 2 : g.w / 2 + 2,
          pillHH: g.h / 2,
          lensHW: round ? Math.min(g.w / 2 + 4, barH * 0.5) : barH * 0.72,
          lensHH: barH * 0.45,
          pillExp: round ? 2 : 2.7,
          lensExp: round ? 2 : 2.1,
        };
      });

      if (this.gl) {
        this.canvas.style.width = `${this.cw}px`;
        this.canvas.style.height = `${this.ch}px`;
        this.canvas.style.left = `${-PAD_X}px`;
        this.canvas.style.top = `${-PAD_TOP}px`;
        // 크기를 같은 값으로 다시 넣어도 캔버스가 지워져서 깜빡이니, 달라졌을 때만 바꿔요
        const w = Math.round(this.cw * dpr);
        const h = Math.round(this.ch * dpr);
        if (this.canvas.width !== w) this.canvas.width = w;
        if (this.canvas.height !== h) this.canvas.height = h;
        this.drawMask();
        this.gl.viewport(0, 0, w, h);
      }

      // 멈춰 있을 때는 선택된 탭 위로 맞춰요
      if (!this.press && !this.releasing) {
        this.x.snap(this.geo[this.current].cx);
        this.lift.snap(0);
        this.grab.snap(0);
        this.stretch.snap(0);
      } else {
        this.x.target = this.geo[this.press ? this.press.tabIndex : this.current].cx;
      }
      // 막대 유리 모양 (캔버스 좌표). offset* 값은 눌림 때 CSS가 키우는 배율에 영향을 받지 않아요
      const plat = (el) =>
        el
          ? {
              cx: el.offsetLeft + el.offsetWidth / 2 + PAD_X,
              cy: el.offsetTop + el.offsetHeight / 2 + PAD_TOP,
              hw: el.offsetWidth / 2,
              hh: el.offsetHeight / 2,
            }
          : null;
      this.barA = plat(this.platA);
      this.barB = plat(this.platB);
      const app = this.pageRoot.getBoundingClientRect();
      this.appOff = { x: nr.left - app.left, y: nr.top - app.top };

      this.placeFallbackPill();
      this.ready = true;
      // 캔버스가 지워졌을 수 있으니 같은 프레임 안에서 바로 다시 그려요 (다음 프레임까지 비어 보이지 않게)
      this.render();
      this.requestRender();
    }

    // 글자와 아이콘의 "덮개"를 흰색으로 그려요. 색은 셰이더가 입혀요.
    drawMask() {
      const dpr = this.dpr;
      if (!this.maskCanvas) this.maskCanvas = document.createElement("canvas");
      const mc = this.maskCanvas;
      mc.width = this.canvas.width;
      mc.height = this.canvas.height;
      const ctx = mc.getContext("2d");
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.clearRect(0, 0, mc.width, mc.height);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.fillStyle = "#fff";
      ctx.strokeStyle = "#fff";
      ctx.lineCap = "round";
      ctx.lineJoin = "round";

      this.tabs.forEach((tab, i) => {
        const g = this.geo[i];
        const parts = ICONS[tab.dataset.tab];
        if (parts) {
          const k = g.icon.size / ICON_BOX;
          for (const part of parts) {
            ctx.save();
            ctx.translate(g.icon.x, g.icon.y);
            ctx.scale(k, k);
            if (part.transform) ctx.transform(...part.transform);
            const path = new Path2D(part.d);
            if (part.fill) ctx.fill(path, part.evenodd ? "evenodd" : "nonzero");
            if (part.stroke) {
              ctx.lineWidth = part.stroke;
              ctx.stroke(path);
            }
            ctx.restore();
          }
        }
        if (g.label) {
          ctx.font = g.label.font;
          ctx.textAlign = "center";
          ctx.textBaseline = "middle";
          ctx.fillText(g.label.text, g.label.cx, g.label.cy);
        }
      });

      const gl = this.gl;
      gl.bindTexture(gl.TEXTURE_2D, this.tex);
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
      gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, mc);
    }

    /* ----- 입력 ----- */

    bind() {
      const nav = this.nav;
      nav.addEventListener("contextmenu", (e) => e.preventDefault());
      nav.addEventListener("pointerdown", (e) => this.onDown(e));
      nav.addEventListener("pointermove", (e) => this.onMove(e));
      nav.addEventListener("pointerup", (e) => this.onUp(e, true));
      nav.addEventListener("pointercancel", (e) => this.onUp(e, false));
      nav.addEventListener("click", (e) => this.onClick(e), true);
      nav.addEventListener("keydown", (e) => this.onKey(e));
      // 그래픽 메모리가 모자라 컨텍스트를 잃으면 대체 화면으로 돌아가고, 돌아오면 다시 올려요
      this.canvas.addEventListener("webglcontextlost", (e) => {
        e.preventDefault();
        this.useGL(null);
      });
      this.canvas.addEventListener("webglcontextrestored", () => {
        this.shots.clear();
        this.useGL(this.initGL());
        this.layout();
        if (this.backdrop) for (const [name, shot] of this.backdrop.shots) if (shot.canvas) this.setShot(name, shot);
      });
      // 화면이 스크롤되면 막대 유리가 보여주는 자리도 따라가요
      this.pageRoot.addEventListener("scroll", () => this.requestRender(), { capture: true, passive: true });
      matchMedia("(prefers-color-scheme: dark)").addEventListener?.("change", () => this.refreshTheme());
      new MutationObserver(() => this.refreshTheme()).observe(document.documentElement, {
        attributes: true,
        attributeFilter: ["data-theme"],
      });
    }

    refreshTheme() {
      this.readTheme();
      this.requestRender();
    }

    tabOf(e) {
      const tab = e.target.closest?.(".tab");
      return tab && this.nav.contains(tab) ? tab : null;
    }

    onDown(e) {
      if (e.pointerType === "mouse" && e.button !== 0) return;
      const tab = this.tabOf(e);
      if (!tab || this.press) return;
      const index = this.tabs.indexOf(tab);
      this.lastPointerAt = performance.now();
      try {
        tab.setPointerCapture(e.pointerId);
      } catch (_) {
        /* 이미 놓았어도 괜찮아요 */
      }
      this.press = {
        id: e.pointerId,
        x0: e.clientX,
        tab,
        tabIndex: index,
        dragging: false,
        wasCurrent: index === this.current,
      };
      this.nav.dataset.press = tab.dataset.group || "";
      this.grabGroup = tab.dataset.group || "main";
      this.nav.classList.add("is-pressed");
      this.releasing = false;
      this.arrivedAt = 0;
      this.grab.target = 1;
      this.lift.target = this.reducedMotion.matches ? 0 : 1;
      this.x.use(SPRING.move);
      this.x.target = this.geo[index].cx;
      if (this.reducedMotion.matches) this.x.snap(this.geo[index].cx);
      this.requestRender();
    }

    onMove(e) {
      const p = this.press;
      if (!p || e.pointerId !== p.id) return;
      const dx = e.clientX - p.x0;
      if (!p.dragging && Math.abs(dx) > 6) {
        p.dragging = true;
        this.x.use(SPRING.trail);
      }
      if (p.dragging) {
        const lo = this.geo[0].cx;
        const hi = this.geo[this.geo.length - 1].cx;
        const raw = this.geo[p.tabIndex].cx + dx;
        const inside = clamp(raw, lo, hi);
        // 끝을 넘어 끌면 고무줄처럼 조금만 따라와요
        this.x.target = inside + clamp((raw - inside) * 0.22, -16, 16);
        this.requestRender();
      }
    }

    onUp(e, commit) {
      const p = this.press;
      if (!p || e.pointerId !== p.id) return;
      this.lastPointerAt = performance.now();
      this.press = null;
      this.nav.classList.remove("is-pressed");
      try {
        p.tab.releasePointerCapture(e.pointerId);
      } catch (_) {
        /* 이미 풀렸어요 */
      }
      this.grab.target = 0;

      let index = p.tabIndex;
      if (p.dragging) {
        const ahead = this.x.target + this.x.v * 0.1;
        index = this.nearest(ahead);
      }
      if (!commit) index = this.current;

      this.x.use(SPRING.move);
      this.x.target = this.geo[index].cx;
      this.releasing = true;
      this.arrivedAt = 0;
      if (commit && index === this.current && !p.dragging && p.wasCurrent) {
        this.nav.dispatchEvent(new CustomEvent("tabreselect", { detail: { tab: this.tabs[index] }, bubbles: true }));
      }
      if (commit) this.select(index);
      this.requestRender();
    }

    // 키보드나 보조 기기로 누른 경우만 여기서 처리해요 (포인터로 누른 클릭은 이미 처리했어요)
    onClick(e) {
      const tab = this.tabOf(e);
      if (!tab) return;
      if (performance.now() - this.lastPointerAt < 700) return;
      const index = this.tabs.indexOf(tab);
      if (index === this.current) {
        this.nav.dispatchEvent(new CustomEvent("tabreselect", { detail: { tab }, bubbles: true }));
        return;
      }
      this.go(index);
    }

    onKey(e) {
      const dir = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
      if (!dir) return;
      const i = this.tabs.indexOf(document.activeElement);
      if (i < 0) return;
      const next = clamp(i + dir, 0, this.tabs.length - 1);
      this.tabs[next].focus();
      e.preventDefault();
    }

    nearest(x) {
      let best = 0;
      this.geo.forEach((g, i) => {
        if (Math.abs(g.cx - x) < Math.abs(this.geo[best].cx - x)) best = i;
      });
      return best;
    }

    /* ----- 선택 ----- */

    select(index) {
      if (index === this.current) return;
      this.current = index;
      this.tabs.forEach((t, i) => {
        if (i === index) t.setAttribute("aria-current", "page");
        else t.removeAttribute("aria-current");
      });
      this.placeFallbackPill();
      this.nav.dispatchEvent(new CustomEvent("tabchange", { detail: { tab: this.tabs[index], index }, bubbles: true }));
    }

    // 코드나 키보드로 옮길 때: 렌즈가 떠올라 미끄러지고 내려앉아요
    go(index) {
      this.x.use(SPRING.move);
      this.x.target = this.geo[index].cx;
      if (this.reducedMotion.matches) {
        this.x.snap(this.geo[index].cx);
      } else {
        this.lift.target = 1;
        this.releasing = true;
        this.arrivedAt = 0;
      }
      this.select(index);
      this.requestRender();
    }

    placeFallbackPill() {
      const pill = this.pillEl;
      if (!pill) return;
      const t = this.tabs[this.current];
      if (!t) return;
      const nr = this.nav.getBoundingClientRect();
      const r = t.getBoundingClientRect();
      pill.hidden = false;
      pill.style.width = `${r.width}px`;
      pill.style.height = `${r.height}px`;
      pill.style.transform = `translate(${r.left - nr.left}px, ${r.top - nr.top}px)`;
    }

    /* ----- 매 프레임 ----- */

    requestRender() {
      if (this.running) return;
      this.running = true;
      this.lastTime = 0;
      requestAnimationFrame((t) => this.frame(t));
    }

    frame(now) {
      if (!this.running) return;
      const dt = this.lastTime ? clamp((now - this.lastTime) / 1000, 0, 0.05) : 1 / 60;
      this.lastTime = now;

      this.x.step(dt);
      this.lift.step(dt);
      this.grab.step(dt);

      // 빨리 움직일수록 가로로 길쭉해져요
      this.stretch.target = clamp(Math.abs(this.x.v) / 650, 0, 1);
      this.stretch.step(dt);

      // 막대 유리가 처음 나타날 때는 CSS 유리 판에서 서서히 넘어가요
      if (this.glassOn && this.glassMix < 1) {
        this.glassMix = this.reducedMotion.matches ? 1 : Math.min(1, this.glassMix + dt / GLASS_FADE);
      }

      // 도착하면 잠깐 들고 있다가 내려놓아요
      if (this.releasing) {
        const arrived = Math.abs(this.x.x - this.x.target) < 0.6 && Math.abs(this.x.v) < 25;
        if (arrived) {
          if (!this.arrivedAt) this.arrivedAt = now;
          else if (now - this.arrivedAt > HANG_MS) {
            this.lift.target = 0;
            this.releasing = false;
          }
        } else {
          this.arrivedAt = 0;
        }
      }

      this.render();

      const idle =
        !this.press &&
        !this.releasing &&
        this.x.settled &&
        this.lift.settled &&
        this.grab.settled &&
        this.stretch.settled &&
        (!this.glassOn || this.glassMix >= 1);
      if (idle) {
        this.x.snap(this.x.target);
        this.lift.snap(this.lift.target);
        this.grab.snap(this.grab.target);
        this.stretch.snap(0);
        this.render();
        this.running = false;
      } else {
        requestAnimationFrame((t) => this.frame(t));
      }
    }

    // 현재 위치에서 알약/렌즈의 크기를 탭 사이로 이어서 계산해요
    shape() {
      const x = this.x.x;
      const g = this.geo;
      let i = 0;
      while (i < g.length - 2 && x > g[i + 1].cx) i++;
      const span = g[i + 1].cx - g[i].cx || 1;
      const t = clamp((x - g[i].cx) / span, 0, 1);
      const a = this.sizes[i];
      const b = this.sizes[i + 1];
      const pillHW = lerp(a.pillHW, b.pillHW, t);
      const pillHH = lerp(a.pillHH, b.pillHH, t);
      const lensHW = lerp(a.lensHW, b.lensHW, t);
      const lensHH = lerp(a.lensHH, b.lensHH, t);
      const pillExp = lerp(a.pillExp, b.pillExp, t);
      const lensExp = lerp(a.lensExp, b.lensExp, t);
      const cy = lerp(g[i].cy, g[i + 1].cy, t);

      const lift = clamp(this.lift.x, 0, 1.2);
      const bulge = 1 + 0.05 * clamp(this.grab.x, 0, 1.2);
      const s = clamp(this.stretch.x, 0, 1.3);
      const hw = lerp(pillHW, lensHW, lift) * bulge * (1 + 0.16 * s);
      const hh = lerp(pillHH, lensHH, lift) * bulge * (1 - 0.07 * s);
      return { cx: x, cy: cy - 0.5 * lift, hw, hh, exp: lerp(pillExp, lensExp, lift), lift };
    }

    render() {
      const gl = this.gl;
      if (!gl || !this.ready) return;
      const sh = this.shape();
      const o = this.optics;
      const th = this.theme;
      const u = this.u;
      const lift = clamp(sh.lift, 0, 1);

      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, this.tex);
      gl.uniform1i(u.uMask, 0);

      // 뒤 화면 사진 (지금 보이는 화면의 것). 없으면 CSS 유리 판이 대신해요
      const entry = this.backdrop ? this.shots.get(this.backdrop.active) : null;
      const glassOn = Boolean(entry && this.barA && this.barB);
      if (glassOn !== this.glassOn) {
        this.glassOn = glassOn;
        if (!glassOn) this.glassMix = 0; // CSS 유리 판이 바로 다시 보여요
        this.nav.classList.toggle("has-glass", glassOn);
      }
      gl.activeTexture(gl.TEXTURE1);
      gl.bindTexture(gl.TEXTURE_2D, entry ? entry.tex : this.emptyTex);
      gl.uniform1i(u.uBack, 1);
      gl.activeTexture(gl.TEXTURE2);
      gl.bindTexture(gl.TEXTURE_2D, entry ? entry.blur : this.emptyTex);
      gl.uniform1i(u.uBlur, 2);
      gl.activeTexture(gl.TEXTURE0);
      gl.uniform1f(u.uGlass, glassOn ? 1 : 0);
      gl.uniform1f(u.uGlassMix, this.glassMix);
      if (glassOn) {
        const scroll = this.backdrop.activeEl ? this.backdrop.activeEl.scrollTop : 0;
        gl.uniform2f(u.uPageOff, this.appOff.x - PAD_X, this.appOff.y - PAD_TOP + scroll);
        gl.uniform2f(u.uPageSize, entry.cssW, entry.cssH);
        const A = this.barA;
        const B = this.barB;
        gl.uniform4f(u.uBarA, A.cx, A.cy, A.hw, A.hh);
        gl.uniform4f(u.uBarB, B.cx, B.cy, B.hw, B.hh);
        const grab = clamp(this.grab.x, 0, 1.2);
        gl.uniform2f(u.uPress, this.grabGroup === "main" ? grab : 0, this.grabGroup === "search" ? grab : 0);
        const gg = this.glass;
        gl.uniform4f(u.uGlassOpt, gg.bezel, gg.shift, gg.power, gg.zoom);
        gl.uniform4f(u.uGlassLook, gg.keep ?? th.glassKeep, gg.whiten ?? th.glassDesat, gg.barBlur, gg.pillBlur);
        gl.uniform3f(u.uGlassBase, th.glassBase[0], th.glassBase[1], th.glassBase[2]);
        gl.uniform4f(u.uPlatLine, th.line[0], th.line[1], th.line[2], clamp(th.line[3] * gg.lineGain, 0, 1));
        const rimK = gg.rim ?? th.rim;
        gl.uniform4f(u.uPlatRim, rimK, rimK * 0.28, gg.chroma, 0);
        gl.uniform3f(u.uPlatShadow, th.platShadow, 8, 11);
      }
      gl.uniform2f(u.uPx, this.canvas.width, this.canvas.height);
      gl.uniform1f(u.uDpr, this.canvas.width / this.cw);
      gl.uniform2f(u.uSize, this.cw, this.ch);
      gl.uniform4f(u.uLens, sh.cx, sh.cy, sh.hw, sh.hh);
      gl.uniform1f(u.uExp, sh.exp);
      gl.uniform1f(u.uLift, lift);
      gl.uniform1f(u.uZoom, 1 + (o.zoom - 1) * lift);
      gl.uniform4f(u.uOptics, o.bezel, o.shift, o.power, o.blur);
      gl.uniform4f(u.uDisp, o.chroma, o.tangent, o.asym, o.sideShift);
      gl.uniform4f(u.uLook, this.look.specular, this.look.rim, this.look.film, this.look.glow);
      gl.uniform3f(u.uInk, th.ink[0], th.ink[1], th.ink[2]);
      gl.uniform3f(u.uAccent, th.accent[0], th.accent[1], th.accent[2]);
      gl.uniform3f(u.uBase, th.base[0], th.base[1], th.base[2]);
      gl.uniform4f(u.uPill, th.pill[0], th.pill[1], th.pill[2], th.pill[3]);
      gl.uniform4f(u.uTint, th.tint[0], th.tint[1], th.tint[2], th.tint[3]);
      gl.uniform4f(u.uEdge, th.edge[0], th.edge[1], th.edge[2], th.edge[3]);
      gl.uniform2f(u.uLight, -0.55, -0.835);
      gl.uniform3f(u.uShadow, th.shadow, 7, 9);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    }
  }

  window.TabBar = TabBar;
})();
