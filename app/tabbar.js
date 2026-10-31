/* 하단 탭 바: 막대 유리, 아이콘·글자, 선택 알약과 렌즈를 캔버스 하나에 WebGL로 직접 그려요.

   - 막대(알약형 탭 줄, 둥근 검색 버튼)는 그 자체가 리퀴드 글라스예요. Backdrop이 찍어 둔 뒤 화면 사진을 스크롤 위치만큼 밀어 읽어서
     확대하고, 가장자리에서 안쪽 내용을 끌어와 휘어 보이게 해요. 사진이 아직 없으면 CSS 유리 판(backdrop-filter)이 대신해요.
   - 선택 알약과 렌즈는 같은 물체예요. 누르면 알약이 떠올라 렌즈가 되고, 움직이다가 도착하기 직전에 다시 알약으로 내려앉아요.
     렌즈는 늘 막대 유리 안에 머물러요(끝 탭에서는 좁아지고, 그림자도 막대 안에만 드리워요).
   - 렌즈 가장자리에서는 뒤 화면(유리)만 휘어요. 빨강·초록·파랑이 보는 자리를 다르게 줘서 프리즘 색이 생기고, 글자와 아이콘은 확대만 해요.
   - 렌즈가 떠 있는 동안에는 막대 전체가 더 투명해지고 가장자리에 프리즘 선이 돌아요(excite).
   - 움직임은 감쇠비 1인 스프링을 해석해로 풀어서, 되튀지 않고 프레임 간격이 들쭉날쭉해도 떨리지 않아요.
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

  // 움직임은 모두 (진동수 rad/s, 감쇠비)로 정해요. 감쇠비 1(임계 감쇠)이라 되튀거나 흔들리지 않고, 값이 클수록 짧게 끝나요.
  // 90%까지 가는 데 걸리는 시간은 대략 3.9 / w 초예요.
  const SPRING = {
    lift: { w: 26, z: 1.0 }, // 렌즈가 떠오르고 내려앉아요 (0.15초)
    move: { w: 21, z: 1.0 }, // 탭 사이를 옮겨 가요 (0.19초)
    trail: { w: 30, z: 1.0 }, // 손가락을 끌고 갈 때 바짝 따라와요
    stretch: { w: 18, z: 1.0 }, // 빨리 움직일 때 살짝 길쭉해져요
    press: { w: 26, z: 1.0 }, // 누르는 동안 막대가 살짝 부풀어요
    excite: { w: 13, z: 1.0 },
    page: { w: 36, z: 0.5 }, // 핫바가 넘어갈 때: 짧고 굵게 "뜨끙" 하고 한 번 튕겨요 // 렌즈가 떠 있는 동안 막대 전체에 프리즘이 켜졌다 꺼져요 (0.3초)
  };
  // 핫바: 옆으로 넘기면 바 안의 내용만 다음 쪽으로 바뀌어요 (프로필 버튼은 그대로)
  const PAGES = [
    [{ id: "messages", label: "메시지" }, { id: "money", label: "돈" }, { id: "work", label: "일" }, { id: "shop", label: "상점" }],
    [{ id: "ranking", label: "랭킹" }, { id: "untitled2", label: "무제" }, { id: "business", label: "사업" }, { id: "untitled3", label: "무제" }],
  ];
  const LAND_PX = 8; // 렌즈가 목표까지 이만큼 남으면 내려앉기 시작해요. 도착할 때까지 들고 있지 않아요
  const LENS_INSET = 4; // 렌즈가 막대 유리 가장자리에서 떨어져 있는 최소 거리 (막대 안쪽 여백과 같아요)
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

uniform sampler2D uMask;    // 아이콘·글자 덮개 (계정 버튼처럼 늘 그 자리에 있는 것)
uniform sampler2D uMaskA;   // 핫바 첫 쪽의 아이콘·글자
uniform sampler2D uMaskB;   // 핫바 둘째 쪽의 아이콘·글자
uniform float uSlide;       // 핫바가 넘어간 정도 (0 = 첫 쪽, 1 = 둘째 쪽, 튕길 때는 0~1 밖으로 나가요)
uniform float uSlideW;      // 한 쪽을 넘기는 거리 (css px)
uniform vec2 uClip;         // 핫바 안쪽 왼쪽·오른쪽 끝 (밖으로 나간 내용은 서서히 지워요)
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
uniform float uExcite;      // 렌즈가 떠 있는 동안 막대 전체가 얼마나 들떠 있나 (0~1): 더 투명해지고 프리즘이 켜져요
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
uniform vec3 uOptics;       // 가장자리 폭, 안쪽 내용 끌어오는 거리, 감쇠 지수
uniform vec4 uDisp;         // 색 번짐(세로), 색 번짐(가로), 빛 쪽 줄이기, 가로 방향 비율
uniform vec4 uLook;         // 반짝임, 둘레 빛, 무지개, 속 밝기
uniform vec3 uInk;
uniform vec3 uAccent;
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
  float fixedInk = texture2D(uMask, css / uSize).a;
  float edge = smoothstep(uClip.x, uClip.x + 10.0, css.x) * (1.0 - smoothstep(uClip.y - 10.0, uClip.y, css.x));
  float a = texture2D(uMaskA, vec2(css.x + uSlide * uSlideW, css.y) / uSize).a;
  float b = texture2D(uMaskB, vec2(css.x - (1.0 - uSlide) * uSlideW, css.y) / uSize).a;
  return max(fixedInk, max(a, b) * edge);
}

// 프리즘 색: 하늘색 → 연보라 → 분홍 → 노랑 → 하늘색으로 부드럽게 돌아요 (h가 1 늘면 한 바퀴)
vec3 prism(float h) {
  float k = fract(h) * 4.0;
  vec3 r = mix(vec3(0.40, 0.78, 1.00), vec3(0.72, 0.60, 1.00), smoothstep(0.0, 1.0, k));
  r = mix(r, vec3(1.00, 0.62, 0.82), smoothstep(1.0, 2.0, k));
  r = mix(r, vec3(1.00, 0.88, 0.50), smoothstep(2.0, 3.0, k));
  return mix(r, vec3(0.40, 0.78, 1.00), smoothstep(3.0, 4.0, k));
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
vec3 barSample(vec2 pos, vec4 pl, float soft, float shiftScale, float zoomScale) {
  vec2 q = pos - pl.xy;
  float d = sdRound(q, pl.zw, pl.w);
  vec2 e = -gradRound(q, pl.zw, pl.w);
  float x = clamp(max(-d, 0.0) / uGlassOpt.x, 0.0, 1.0);
  float shift = uGlassOpt.y * pow(1.0 - x, uGlassOpt.z) * shiftScale;
  vec2 sp = pl.xy + q / (uGlassOpt.w * zoomScale) + e * shift;
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
  vec4 barA = vec4(uBarA.xy, uBarA.zw * (1.0 + 0.02 * uPress.x));   // 반지름은 반높이(.w)와 같아요
  vec4 barB = vec4(uBarB.xy, uBarB.zw * (1.0 + 0.02 * uPress.y));
  float dA = sdRound(p - barA.xy, barA.zw, barA.w);
  float dB = sdRound(p - barB.xy, barB.zw, barB.w);
  float platIn = clamp(0.5 - min(dA, dB) * uDpr, 0.0, 1.0);   // 막대 안이면 1. 렌즈 그림자를 막대 안에만 드리우는 데 써요
  float ex = uExcite;
  if (uGlass > 0.5) {
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
      // 들떠 있는 동안에는 빨강은 더 크게, 파랑은 더 작게 확대해서 막대 전체에 프리즘 색 번짐이 생겨요 (바깥쪽일수록 많이 벌어져요)
      float zk = ex * 0.02;
      vec3 c = vec3(
        barSample(p, pl, uGlassLook.z, 1.0 + ch, 1.0 + zk).r,
        barSample(p, pl, uGlassLook.z, 1.0, 1.0).g,
        barSample(p, pl, uGlassLook.z, 1.0 - ch, 1.0 - zk).b
      );
      float fac = 0.5 + 0.5 * dot(-ep, uLight);
      // 안쪽 가장자리를 따라 도는 하이라이트와, 가장자리에서 퍼지는 은은한 빛
      float hu = (sP - 2.1) / 0.9;   // pow()는 음수 밑이 정의되지 않아서 직접 제곱해요
      c += vec3(exp(-hu * hu) * (0.25 + 0.75 * fac) * uPlatRim.x);
      c += vec3(exp(-sP / 4.5) * (0.2 + 0.8 * fac) * uPlatRim.y);
      if (ex > 0.002) {
        // 막대 가장자리를 따라 얇은 막 같은 무지개 선이 돌아요. 색은 렌즈 위치를 따라 흘러요
        float angP = atan(ep.y, ep.x);
        vec3 hue = prism(angP * 0.17 + (uLens.x - pl.x) * 0.0045 + 0.1);
        float l1 = (sP - 2.4) / 1.2;
        float l2 = (sP - 5.0) / 1.5;
        float l3 = (sP - 9.0) / 3.5;
        float film = exp(-l1 * l1) + 0.6 * exp(-l2 * l2) + 0.22 * exp(-l3 * l3);
        c = mix(c, hue, clamp(film * uPlatRim.w * ex * (0.3 + 0.7 * fac), 0.0, 0.75));
        // 렌즈 둘레로 빛이 갈라져 퍼지는 은은한 색 번짐
        float dx = (p.x - uLens.x) / max(uLens.z * 2.4, 1.0);
        c = mix(c, prism(dx * 0.4 + 0.15), 0.1 * ex * exp(-dx * dx));
      }
      // 바깥 가는 선
      float line = (1.0 - smoothstep(0.0, 1.0, abs(dP + 0.55))) * uPlatLine.a;
      c = mix(c, uPlatLine.rgb, line);
      outside = outside * (1.0 - inP) + vec4(clamp(c, 0.0, 1.0), 1.0) * inP;
    }
  }

  // ====== 2. 검은 글자·아이콘
  float aOut = cover(p);
  outside = outside * (1.0 - aOut) + vec4(uInk * aOut, aOut);

  // 렌즈가 드리우는 그림자: 막대 유리 안에만 드리워요 (막대 밖으로 새지 않게)
  vec2 gs;
  float ds = seDist(q - vec2(0.0, uShadow.y), hb, uExp, gs);
  float sh = uShadow.x * uLift * exp(-max(ds, 0.0) / uShadow.z) * platIn;
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
  float kr = 1.0 + uDisp.x;
  float kb = 1.0 - uDisp.x;
  vec2 posR = base + e * (shift * kr) - t * (shift * uDisp.y);
  vec2 posG = base + e * shift;
  vec2 posB = base + e * (shift * kb) + t * (shift * uDisp.y);
  // 글자와 아이콘은 휘지 않고 확대만 해요. 가장자리로 끌어와 늘이면 글자 아랫부분이 렌즈 테두리 쪽으로 흘러내려 보여서,
  // 휘는 것은 뒤 화면(유리)만 맡고 글자는 늘 또렷하게 렌즈 안에 머물러요. 맨 가장자리에서는 서서히 지워져요.
  float cov = cover(base) * smoothstep(0.0, 0.3, x);

  vec4 fill = mix(uPill, uTint, uLift);
  float fa = fill.a;
  vec3 prem;
  float alpha;
  if (uGlass > 0.5) {
    // 렌즈는 막대 유리 위에 얹혀 있어서, 막대 유리가 보여주는 색을 다시 굴절해서 봐요
    vec4 pl = abs(uLens.x - barA.x) < abs(uLens.x - barB.x) ? barA : barB;
    float soft = mix(uGlassLook.w, uGlassLook.z * 0.5, uLift);
    vec3 bg = vec3(
      barSample(posR, pl, soft, 1.0, 1.0).r,
      barSample(posG, pl, soft, 1.0, 1.0).g,
      barSample(posB, pl, soft, 1.0, 1.0).b
    );
    prem = (bg * (1.0 - fa) + fill.rgb * fa) * (1.0 - cov) + uAccent * cov;
    alpha = 1.0;
  } else {
    prem = fill.rgb * fa * (1.0 - cov) + uAccent * cov;
    alpha = fa + (1.0 - fa) * cov;
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
  vec3 film = prism(ang * 0.17 + 0.1);
  float lu1 = (s - 2.6) / 1.1;
  float lu2 = (s - 5.4) / 1.3;
  float lines = exp(-lu1 * lu1) + 0.7 * exp(-lu2 * lu2);

  // 속 밝기: 위쪽이 살짝 더 밝아요
  light += vec3(uLook.w * (0.5 + 0.5 * dot(q / max(hb, vec2(1.0)), -uLight)));

  light *= uLift;
  float la = clamp(max(light.r, max(light.g, light.b)), 0.0, 1.0);
  prem = prem * (1.0 - la) + light;
  alpha = alpha + (1.0 - alpha) * la;

  // 무지개 선은 더하지 않고 섞어서, 하얀 유리 위에서도 색이 보이게 해요
  float fa2 = clamp(lines * uLook.z * (0.25 + 0.75 * facing), 0.0, 0.85) * uLift;
  prem = prem * (1.0 - fa2) + film * fa2;
  alpha = alpha + (1.0 - alpha) * fa2;

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
      this.platB = nav.querySelector(".platter-side");
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
        chroma: 0.16, // 빨강·파랑이 갈라지는 정도 (세로)
        tangent: 0.1, // 빨강·파랑이 갈라지는 정도 (가로)
        asym: 0.5, // 빛 쪽 가장자리에서 굴절을 얼마나 줄일지
        sideShift: 0.3, // 양옆 가장자리의 굴절 비율 (위아래를 1로 봤을 때)
        zoom: 1.2,
        ...options.optics,
      };
      this.look = { specular: 0.2, rim: 0.08, film: 0.22, glow: 0.0, ...options.look };
      // 막대 유리: 뒤 화면을 확대해서 보여주고 가장자리에서 휘어요
      this.glass = {
        bezel: 12, // 휘는 가장자리 폭
        shift: 5, // 맨 가장자리에서 안쪽으로 끌어오는 거리
        power: 1.5,
        zoom: 1.35, // 확대
        chroma: 0.05, // 색 번짐
        keep: null, // 뒤 화면의 글자 같은 디테일 대비를 얼마나 남길지 (null이면 테마 값)
        whiten: null, // 유리가 밝게(다크에서는 어둡게) 덮는 정도 (null이면 테마 값)
        rim: null, // 안쪽 가장자리 하이라이트 세기 (null이면 테마 값)
        lineGain: 1.0, // 바깥 가는 선 진하기 배율
        barBlur: 0.35, // 막대 유리가 뒤 화면을 얼마나 흐리게 보여줄지 (0 = 또렷, 1 = 흐린 사진)
        pillBlur: 0.88, // 선택 알약은 훨씬 뿌옇게
        // 렌즈가 떠 있는 동안 막대 전체가 들뜨는 정도 (excite 0 → 1)
        film: 0.46, // 가장자리를 도는 무지개 선의 진하기
        exciteChroma: 1.5, // 가장자리 색 번짐이 늘어나는 배율 (1 + 이 값 × excite)
        exciteKeep: 0.3, // 뒤 화면 디테일이 더 또렷해지는 정도. 너무 크면 뒤 글자가 탭 글자와 겹쳐 읽기 어려워요
        exciteWhiten: 0.35, // 하얗게 덮는 정도가 줄어드는 비율 → 뒤 화면 색이 더 비쳐서 투명해 보여요
        ...options.glass,
      };

      this.current = Math.max(0, this.tabs.findIndex((t) => t.hasAttribute("aria-current")));
      this.press = null;
      this.releasing = false;
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
      this.excite = new Spring(0, SPRING.excite, 0.002);
      this.page = 0; // 지금 보이는 핫바
      this.selPage = 0; // 선택된 탭이 있는 핫바
      this.pageS = new Spring(0, SPRING.page, 0.001);

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
          if (part.alpha) path.setAttribute("fill-opacity", part.alpha);
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
        "uMask", "uMaskA", "uMaskB", "uSlide", "uSlideW", "uClip", "uBack", "uBlur", "uPx", "uDpr", "uSize", "uGlass", "uGlassMix", "uPageOff", "uPageSize", "uBarA", "uBarB", "uPress", "uExcite",
        "uGlassOpt", "uGlassLook", "uGlassBase", "uPlatLine", "uPlatRim", "uPlatShadow",
        "uLens", "uExp", "uLift", "uZoom", "uOptics", "uDisp", "uLook",
        "uInk", "uAccent", "uPill", "uTint", "uEdge", "uLight", "uShadow",
      ]) {
        this.u[name] = gl.getUniformLocation(prog, name);
      }

      this.tex = this.makeTexture(gl, null);
      this.texA = this.makeTexture(gl, null);
      this.texB = this.makeTexture(gl, null);
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
      return Boolean(this.press || this.releasing || this.lift.x > 0.02 || this.excite.x > 0.02 || !this.pageS.settled || this.running);
    }

    /* ----- 배치와 그리기 재료 ----- */

    readTheme() {
      const cs = getComputedStyle(this.nav);
      const get = (name) => parseColor(cs.getPropertyValue(name) || "#000");
      this.theme = {
        ink: get("--tab-ink"),
        accent: get("--tab-accent"),
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

      // 탭마다 알약과 렌즈의 크기: 알약은 칸 크기 그대로(막대 안쪽 여백 4px), 렌즈는 조금 더 넓고 높이는 알약과 거의 같아요.
      // 막대 끝에서는 렌즈가 막대 유리 밖으로 나가지 못해서 shape()가 폭을 줄여요.
      const barH = nr.height;
      this.sizes = this.geo.map((g) => {
        const round = g.w >= g.h - 1; // 검색 버튼처럼 가로세로가 같은 칸
        // 선택 알약은 칸 폭에 맞춘 동그라미예요 (칸이 좁은 폰에서도 타원이 되지 않게)
        const r = Math.min(g.w, g.h) / 2;
        return {
          pillHW: r,
          pillHH: r,
          lensHW: round ? Math.min(g.w / 2 + 2, barH * 0.47) : Math.min(barH * 0.62, g.w * 0.8),
          lensHH: barH * 0.45,
          pillExp: 2,
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
        this.excite.snap(0);
        this.pageS.snap(this.page);
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
    // 늘 그 자리에 있는 것(계정 버튼), 핫바 첫 쪽, 핫바 둘째 쪽을 따로 그려서 셰이더가 옆으로 밀어 보여줘요.
    drawMask() {
      const dpr = this.dpr;
      const gl = this.gl;
      if (!this.masks) this.masks = [0, 1, 2].map(() => document.createElement("canvas"));
      const paint = (mc, items) => {
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
        for (const { g, parts, label } of items) {
          if (parts) {
            const k = g.icon.size / ICON_BOX;
            for (const part of parts) {
              ctx.save();
              ctx.translate(g.icon.x, g.icon.y);
              ctx.scale(k, k);
              if (part.transform) ctx.transform(...part.transform);
              const path = new Path2D(part.d);
              if (part.fill) {
                ctx.globalAlpha = part.alpha ?? 1;
                ctx.fill(path, part.evenodd ? "evenodd" : "nonzero");
                ctx.globalAlpha = 1;
              }
              if (part.stroke) {
                ctx.lineWidth = part.stroke;
                ctx.stroke(path);
              }
              ctx.restore();
            }
          }
          if (label) {
            ctx.font = label.font;
            ctx.textAlign = "center";
            ctx.textBaseline = "middle";
            ctx.fillText(label.text, label.cx, label.cy);
          }
        }
      };
      const last = this.tabs.length - 1;
      const pageItems = (p) =>
        PAGES[p].map((e, i) => ({ g: this.geo[i], parts: ICONS[e.id], label: this.geo[i].label ? { ...this.geo[i].label, text: e.label } : null }));
      paint(this.masks[0], [{ g: this.geo[last], parts: ICONS[this.tabs[last].dataset.tab], label: null }]);
      paint(this.masks[1], pageItems(0));
      paint(this.masks[2], pageItems(1));
      [this.tex, this.texA, this.texB].forEach((tex, i) => {
        gl.bindTexture(gl.TEXTURE_2D, tex);
        gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
        gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, this.masks[i]);
      });
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
        wasCurrent: this.isSelected(index),
        t0: performance.now(),
        lx: e.clientX,
        lt: performance.now(),
        flick: false,
      };
      this.nav.dataset.press = tab.dataset.group || "";
      this.grabGroup = tab.dataset.group || "main";
      this.nav.classList.add("is-pressed");
      this.releasing = false;
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
      const now = performance.now();
      const v = (e.clientX - p.lx) / Math.max(now - p.lt, 1); // px/ms
      p.lx = e.clientX;
      p.lt = now;
      // 빠르게 휙 넘기면 선택을 옮기는 게 아니라 핫바를 넘겨요. 렌즈는 제자리로 돌아가요
      if (!p.flick && p.tabIndex < 4 && Math.abs(dx) > 10 && Math.abs(v) > 0.4) {
        p.flick = true;
        this.x.use(SPRING.move);
        this.x.target = this.geo[p.tabIndex].cx;
      }
      if (p.flick) return;
      if (!p.dragging && Math.abs(dx) > 6) {
        p.dragging = true;
        this.x.use(SPRING.trail);
      }
      if (p.dragging) {
        // 첫 탭과 마지막 칸 밖으로는 끌려가지 않아요 (렌즈가 막대 밖으로 나가지 않게)
        const lo = this.geo[0].cx;
        const hi = this.geo[this.geo.length - 1].cx;
        this.x.target = clamp(this.geo[p.tabIndex].cx + dx, lo, hi);
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

      const dx = e.clientX - p.x0;
      const flipped = commit && p.tabIndex < 4 && (p.flick || (Math.abs(dx) >= 40 && performance.now() - p.t0 < 420));
      let index = p.tabIndex;
      if (p.dragging && !flipped) {
        const ahead = this.x.target + this.x.v * 0.1;
        index = this.nearest(ahead);
      }
      if (!commit || flipped) index = this.current;

      this.x.use(SPRING.move);
      this.x.target = this.geo[index].cx;
      this.releasing = true;
      if (flipped) {
        this.flipPage();
        this.requestRender();
        return;
      }
      if (commit && this.isSelected(index) && !p.dragging && p.wasCurrent) {
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
      if (this.isSelected(index)) {
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

    // 이 칸이 지금 선택돼 있나 (같은 칸이어도 핫바가 다르면 다른 탭이에요)
    isSelected(index) {
      return index === this.current && (index >= 4 || this.selPage === this.page);
    }

    markCurrent() {
      this.tabs.forEach((t, i) => {
        if (this.isSelected(i)) t.setAttribute("aria-current", "page");
        else t.removeAttribute("aria-current");
      });
    }

    select(index) {
      if (this.isSelected(index)) return;
      const crossing = index < 4 && this.selPage !== this.page;
      this.current = index;
      if (index < 4) this.selPage = this.page;
      if (crossing) this.x.snap(this.geo[index].cx);
      this.markCurrent();
      this.placeFallbackPill();
      this.nav.dispatchEvent(new CustomEvent("tabchange", { detail: { tab: this.tabs[index], index }, bubbles: true }));
    }

    // 핫바를 다음 쪽으로 넘겨요: 바 안의 내용만 짧고 굵게 튕기며 옆으로 밀려요
    flipPage() {
      this.page = 1 - this.page;
      this.applyPageDOM();
      this.markCurrent();
      this.placeFallbackPill();
      this.pageS.target = this.page;
      if (this.reducedMotion.matches) this.pageS.snap(this.page);
      else {
        navigator.vibrate?.(24); // 뜨끙
        setTimeout(() => navigator.vibrate?.(12), 110);
      }
      this.requestRender();
    }

    // 버튼의 이름과 아이콘을 지금 보이는 쪽 것으로 바꿔요 (눌림·접근성·대체 화면용)
    applyPageDOM() {
      PAGES[this.page].forEach((e, i) => {
        const tab = this.tabs[i];
        tab.dataset.tab = e.id;
        const lab = tab.querySelector(".tab-label");
        if (lab) lab.textContent = e.label;
      });
      this.buildIcons();
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
      pill.hidden = this.current < 4 && this.selPage !== this.page;
      const d = Math.min(r.width, r.height);
      pill.style.width = pill.style.height = `${d}px`;
      pill.style.transform = `translate(${r.left - nr.left + (r.width - d) / 2}px, ${r.top - nr.top + (r.height - d) / 2}px)`;
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

      // 도착하기 조금 전부터 렌즈를 내려놓기 시작해요. 도착할 때까지 들고 있지 않아서 한 번에 매끄럽게 이어져요.
      if (this.releasing && Math.abs(this.x.x - this.x.target) < LAND_PX) {
        this.lift.target = 0;
        this.releasing = false;
      }
      // 막대 전체의 프리즘은 렌즈를 따라 켜지고, 더 느긋하게 꺼져요
      this.excite.target = this.lift.target;

      this.x.step(dt);
      this.lift.step(dt);
      this.grab.step(dt);
      this.excite.step(dt);
      this.pageS.step(dt);

      // 빨리 움직일수록 살짝 길쭉해져요
      this.stretch.target = clamp(Math.abs(this.x.v) / 800, 0, 1);
      this.stretch.step(dt);

      // 막대 유리가 처음 나타날 때는 CSS 유리 판에서 서서히 넘어가요
      if (this.glassOn && this.glassMix < 1) {
        this.glassMix = this.reducedMotion.matches ? 1 : Math.min(1, this.glassMix + dt / GLASS_FADE);
      }

      this.render();

      const idle =
        !this.press &&
        !this.releasing &&
        this.x.settled &&
        this.lift.settled &&
        this.grab.settled &&
        this.stretch.settled &&
        this.excite.settled &&
        this.pageS.settled &&
        (!this.glassOn || this.glassMix >= 1);
      if (idle) {
        this.x.snap(this.x.target);
        this.lift.snap(this.lift.target);
        this.grab.snap(this.grab.target);
        this.stretch.snap(0);
        this.excite.snap(this.excite.target);
        this.pageS.snap(this.pageS.target);
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

      const lift = clamp(this.lift.x, 0, 1);
      const bulge = 1 + 0.03 * clamp(this.grab.x, 0, 1);
      const s = clamp(this.stretch.x, 0, 1);
      let hw = lerp(pillHW, lensHW, lift) * bulge * (1 + 0.08 * s);
      let hh = lerp(pillHH, lensHH, lift) * bulge * (1 - 0.04 * s);
      // 렌즈는 막대 유리 안에 머물러요. 끝 탭에서는 좁아지고 가운데에서는 넓어지며, 알약보다 작아지지는 않아요.
      hw = Math.min(hw, Math.max(pillHW, this.reach(x)));
      hh = Math.min(hh, Math.max(pillHH, this.barA.hh - LENS_INSET));
      // 선택된 탭이 다른 쪽 핫바에 있으면 알약도 내용과 함께 밀려나고 작아져요
      let cx = x;
      if (this.current < 4) {
        const d = this.selPage - this.pageS.x;
        const vis = clamp(1 - Math.abs(d) * 1.8, 0, 1);
        cx = vis < 0.03 ? -1000 : x + d * this.barA.hw * 2;
        hw *= vis;
        hh *= vis;
      }
      return { cx, cy: cy - 0.5 * lift, hw: Math.max(hw, 0.5), hh: Math.max(hh, 0.5), exp: lerp(pillExp, lensExp, lift), lift };
    }

    // 렌즈 중심이 x일 때 막대 유리 가장자리까지 쓸 수 있는 반폭이에요 (두 막대 유리 중 더 넉넉한 쪽)
    reach(x) {
      let room = -Infinity;
      for (const bar of [this.barA, this.barB]) {
        room = Math.max(room, Math.min(x - (bar.cx - bar.hw), bar.cx + bar.hw - x));
      }
      return room - LENS_INSET;
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
      gl.activeTexture(gl.TEXTURE3);
      gl.bindTexture(gl.TEXTURE_2D, this.texA);
      gl.uniform1i(u.uMaskA, 3);
      gl.activeTexture(gl.TEXTURE4);
      gl.bindTexture(gl.TEXTURE_2D, this.texB);
      gl.uniform1i(u.uMaskB, 4);
      gl.activeTexture(gl.TEXTURE0);
      gl.uniform1f(u.uSlide, this.pageS.x);
      gl.uniform1f(u.uSlideW, this.barA.hw * 2);
      gl.uniform2f(u.uClip, this.barA.cx - this.barA.hw, this.barA.cx + this.barA.hw);

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
      // 막대 유리 모양은 렌즈 그림자를 막대 안으로 가두는 데도 쓰이니 항상 알려줘요
      const A = this.barA;
      const B = this.barB;
      gl.uniform4f(u.uBarA, A.cx, A.cy, A.hw, A.hh);
      gl.uniform4f(u.uBarB, B.cx, B.cy, B.hw, B.hh);
      const excite = clamp(this.excite.x, 0, 1);
      gl.uniform1f(u.uExcite, excite);
      if (glassOn) {
        const scroll = this.backdrop.activeEl ? this.backdrop.activeEl.scrollTop : 0;
        gl.uniform2f(u.uPageOff, this.appOff.x - PAD_X, this.appOff.y - PAD_TOP + scroll);
        gl.uniform2f(u.uPageSize, entry.cssW, entry.cssH);
        const grab = clamp(this.grab.x, 0, 1);
        gl.uniform2f(u.uPress, this.grabGroup === "main" ? grab : 0, this.grabGroup === "side" ? grab : 0);
        const gg = this.glass;
        gl.uniform4f(u.uGlassOpt, gg.bezel, gg.shift, gg.power, gg.zoom);
        // 렌즈가 떠 있는 동안에는 하얗게 덮는 정도를 줄이고 디테일을 더 남겨서 막대 전체가 더 투명해져요
        const keep = (gg.keep ?? th.glassKeep) * (1 + gg.exciteKeep * excite);
        const whiten = (gg.whiten ?? th.glassDesat) * (1 - gg.exciteWhiten * excite);
        gl.uniform4f(u.uGlassLook, keep, whiten, gg.barBlur * (1 - excite), gg.pillBlur);
        gl.uniform3f(u.uGlassBase, th.glassBase[0], th.glassBase[1], th.glassBase[2]);
        gl.uniform4f(u.uPlatLine, th.line[0], th.line[1], th.line[2], clamp(th.line[3] * gg.lineGain, 0, 1));
        const rimK = gg.rim ?? th.rim;
        gl.uniform4f(u.uPlatRim, rimK, rimK * 0.28, gg.chroma * (1 + gg.exciteChroma * excite), gg.film);
        gl.uniform3f(u.uPlatShadow, th.platShadow, 8, 11);
      }
      gl.uniform2f(u.uPx, this.canvas.width, this.canvas.height);
      gl.uniform1f(u.uDpr, this.canvas.width / this.cw);
      gl.uniform2f(u.uSize, this.cw, this.ch);
      gl.uniform4f(u.uLens, sh.cx, sh.cy, sh.hw, sh.hh);
      gl.uniform1f(u.uExp, sh.exp);
      gl.uniform1f(u.uLift, lift);
      gl.uniform1f(u.uZoom, 1 + (o.zoom - 1) * lift);
      gl.uniform3f(u.uOptics, o.bezel, o.shift, o.power);
      gl.uniform4f(u.uDisp, o.chroma, o.tangent, o.asym, o.sideShift);
      gl.uniform4f(u.uLook, this.look.specular, this.look.rim, this.look.film, this.look.glow);
      gl.uniform3f(u.uInk, th.ink[0], th.ink[1], th.ink[2]);
      gl.uniform3f(u.uAccent, th.accent[0], th.accent[1], th.accent[2]);
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
