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

// ====== Kyant Liquid Glass (github.com/Kyant0/AndroidLiquidGlass, Apache-2.0)의 계산을 그대로 옮겼어요.
// lens: 가장자리에서 refractionHeight 안쪽까지, 원호 모양(circleMap)으로 안쪽 내용을 끌어와요. 색 번짐은 7가지 색을 따로 읽어요.
// highlight: 45도에서 오는 빛. 가장자리 방향과 빛 방향이 나란할수록 밝아요 (0.5px 선, 흰색 50%).
// vibrancy: 채도 1.5배. 막대 표면은 #FAFAFA 40% (다크 #121212 40%).
uniform vec4 uCont;         // 막대 표면 색 (색 + 알파)
uniform float uDark;        // 다크 테마면 1

float circleMap(float x) { return 1.0 - sqrt(max(1.0 - x * x, 0.0)); }

// 둥근 사각형 렌즈: p를 굴절된 위치로 (css px). amt는 끌어오는 거리, h는 휘는 가장자리 폭
vec2 kyLens(vec2 p, vec2 c, vec2 hb, float r, float h, float amt, float depth, out vec2 dg) {
  vec2 q = p - c;
  float sd = sdRound(q, hb, r);
  dg = vec2(0.0);
  if (h <= 0.0 || -sd >= h) return p;
  sd = min(sd, 0.0);
  float d = circleMap(1.0 - -sd / h) * -amt;
  float gr = min(r * 1.5, min(hb.x, hb.y));
  vec2 g = gradRound(q, hb, gr);
  if (depth > 0.0) g = normalize(g + depth * normalize(q + vec2(0.0001)));
  dg = d * g;
  return p + dg;
}
// 초타원 렌즈(선택 알약)도 같은 계산이에요
vec2 kyLensSE(vec2 p, vec2 c, vec2 hb, float n, float h, float amt, out vec2 dg) {
  vec2 g;
  float sd = seDist(p - c, hb, n, g);
  dg = vec2(0.0);
  if (h <= 0.0 || -sd >= h) return p;
  sd = min(sd, 0.0);
  float d = circleMap(1.0 - -sd / h) * -amt;
  dg = d * g;
  return p + dg;
}
vec3 vibrant(vec3 c) {
  float l = dot(c, vec3(0.213, 0.715, 0.072));
  return clamp(vec3(l) + (c - vec3(l)) * 1.5, 0.0, 1.0);
}
// 뒤 화면 사진 (k: 0 또렷, 1 흐림)
vec3 page(vec2 cp, float k) {
  vec2 uv = (cp + uPageOff) / uPageSize;
  return mix(texture2D(uBack, uv).rgb, texture2D(uBlur, uv).rgb, k);
}
// 45도 빛 하이라이트의 세기 (가장자리 방향 g)
float kyHighlight(vec2 g) {
  return abs(dot(g, vec2(0.70710678, 0.70710678)));
}
// ====== TiHxdvt/liquid-glass-webgl (MIT, Copyright 2026 HaodongWen)의 "접힘 변위장"을 옮겼어요.
// 가장자리에서 안쪽으로 dist 만큼 끌어오는데, 감쇠가 가팔라서 고리 안에서 접히는 점이 생겨요:
// 바깥쪽 반 고리는 뒤집힌 상으로 보이고(애플 렌즈 가장자리의 거꾸로 비친 띠), 안쪽은 부드럽게 평평해져요.
float tanh1(float x) { float e = exp(2.0 * clamp(x, -10.0, 10.0)); return (e - 1.0) / (e + 1.0); }
float tiShift(float depth, float B, float dist, float fold, float bendK, out float sPrime) {
  B = max(B, 1.0);
  float t = clamp(depth / B, 0.0, 1.0);
  float eps = 0.004;
  float w = max(1.0 - t, 0.0);
  float wr = w * w / (w + eps);
  float sRaw = depth + dist * pow(wr, fold);
  float wrSlope = w * (w + 2.0 * eps) / ((w + eps) * (w + eps));
  float sRawPrime = 1.0 - dist * fold * pow(max(wr, 1e-5), fold - 1.0) * wrSlope / B;
  float t0 = clamp(1.0 - pow(clamp(B / max(dist * fold, 1e-3), 0.0, 1.0), 1.0 / max(fold - 1.0, 1e-3)), 0.0, 1.0);
  float sMin = t0 * B + dist * pow(max(1.0 - t0, 0.0), fold);
  float d0 = t0 * B;
  float V = max(dist - sMin, 0.5);
  float du0 = max(d0, 1e-3);
  float ue = clamp((d0 - depth) / du0, 0.0, 1.0);
  float ue2 = min(ue * ue, 0.999);
  float sOuter = sMin + V * (1.0 - sqrt(1.0 - ue2));
  float sOuterPrime = -V * ue / (du0 * sqrt(1.0 - ue2));
  float k = max(clamp(bendK, 0.0, 10.0), 1.0);
  float a = 1.0 / k;
  float bend = max(B * (1.0 - t0) / max(fold - 1.0, 1e-3) * k, 0.5);
  float x = sRaw - sMin;
  float ug = x / bend;
  float th = tanh1(ug);
  float sInner = sMin + x * (a + (1.0 - a) * th);
  float soften = a + (1.0 - a) * (th + ug * (1.0 - th * th));
  float sInnerPrime = sRawPrime * soften;
  float sMap = depth < d0 ? sOuter : sInner;
  sPrime = depth < d0 ? sOuterPrime : sInnerPrime;
  return sMap - depth;
}
vec3 chromaExpand(vec3 c, float hi, float lo) {
  float lum = dot(c, vec3(0.299, 0.587, 0.114));
  vec3 dev = c - vec3(lum);
  return vec3(lum) + dev * mix(vec3(lo), vec3(hi), step(0.0, dev));
}
// 애플 유리 투과율 (TiHxdvt가 실제 화면에서 잰 표): 밝은 모드 +, 어두운 모드 −
vec3 glassTint(vec3 c, float tint) {
  vec3 t = tint >= 0.0
    ? chromaExpand(c * 0.748 + vec3(38.0 / 255.0), 1.36, 2.09)
    : chromaExpand(vec3(0.047) + pow(max(c, vec3(0.0)), vec3(0.8)) * 0.643, 1.8, 2.25);
  return mix(c, clamp(t, 0.0, 1.0), clamp(abs(tint), 0.0, 1.0));
}
// 가장자리 빛: 프레넬 + 붙은 가는 선 + 접힘 선 + 위아래 흰 테/양옆 검은 테
vec3 tiRim(vec3 col, float depth, float sPrime, float B, vec2 n, float rim) {
  float dpt = max(depth, 0.0);
  float slope = abs(sPrime);
  float fres = 0.04 + 0.96 * pow(1.0 - 1.0 / sqrt(1.0 + slope * slope), 5.0);
  float win = exp(-2.0 * dpt / 11.0);
  float rimLine = exp(-dpt / 1.6);
  float seam = exp(-slope * 14.0);
  vec3 sheen = vec3(0.25);
  col += sheen * (fres * rim * 0.85) * win;
  col += sheen * seam * rim * 0.4;
  return col;
}

// 막대 유리가 보여주는 색 (굴절 + 흐림 + 채도 + 표면)
vec3 barGlass(vec2 p, vec4 pl, float soft) {
  vec2 q = p - pl.xy;
  float sd = sdRound(q, pl.zw, pl.w);
  vec2 n = gradRound(q, pl.zw, max(pl.w - 0.5, 0.0));
  float sp;
  float sh = tiShift(max(-sd, 0.0), 11.0, 14.0, 2.6, 3.0, sp);
  vec2 inward = -n;
  float dsp = 0.01;
  vec3 c = vec3(page(p + inward * sh * (1.0 - dsp), soft).r, page(p + inward * sh, soft).g, page(p + inward * sh * (1.0 + dsp), soft).b);
  c = glassTint(c, uDark > 0.5 ? -1.0 : 1.0);
  return c; // 막대에는 가장자리 선(프레넬·접힘 선)을 그리지 않아요
}
float rimBand(float sd) {
  // 0.5px 선 + 0.25px 흐림
  return clamp(1.0 - abs(sd + 0.25) / 0.5, 0.0, 1.0);
}

void main() {
  vec2 p = vec2(gl_FragCoord.x, uPx.y - gl_FragCoord.y) / uDpr;
  vec2 q = p - uLens.xy;
  vec2 hb = uLens.zw;
  vec2 g;
  float d = seDist(q, hb, uExp, g);
  float pr = uLift; // 눌린 정도 (Kyant pressProgress)

  // ====== 1. 막대 유리
  vec4 outside = vec4(0.0);
  float scA = 1.0;   // 눌러도 막대는 커지지 않아요
  vec4 barA = vec4(uBarA.xy, uBarA.zw * scA);
  vec4 barB = vec4(uBarB.xy, uBarB.zw * (1.0 + 0.02 * uPress.y));
  float dA = sdRound(p - barA.xy, barA.zw, barA.w);
  float dB = sdRound(p - barB.xy, barB.zw, barB.w);
  float dMin = min(dA, dB);

  // 그림자 (Kyant Shadow: 24px 번짐, 4px 아래, 검정 10%)
  float dsA = sdRound(p - barA.xy - vec2(0.0, 4.0), barA.zw, barA.w);
  float dsB = sdRound(p - barB.xy - vec2(0.0, 4.0), barB.zw, barB.w);
  float sdS = max(min(dsA, dsB), 0.0);
  float shadow = 0.1 * exp(-(sdS * sdS) / (2.0 * 10.0 * 10.0));
  // 뒤 화면 사진이 없으면 막대는 CSS 판(.platter)이 그려요
  // 그림자는 없앴어요 (막대 아래가 깨끗하게)
  shadow = 0.0;

  if (uGlass > 0.5 && dMin < 1.5) {
    bool useA = dA < dB;
    vec4 pl = useA ? barA : barB;
    float dP = useA ? dA : dB;
    float inP = clamp(0.5 - dP * uDpr, 0.0, 1.0);
    vec2 qp = p - pl.xy;
    vec3 c = barGlass(p, pl, 0.12);
    // 아주 얇은 회색 윤곽선 (0.5px 남짓)
    float ln = clamp(1.0 - abs(dP + 0.55) / 0.6, 0.0, 1.0);
    c = mix(c, uDark > 0.5 ? vec3(0.34) : vec3(0.72), ln * 0.6);
    // 누르는 동안: 전체가 살짝 밝아지고, 선택 알약 쪽에서 빛이 번져요 (Kyant InteractiveHighlight)
    float glowR = pl.w * 2.0 * 1.5;
    float glow = smoothstep(glowR, glowR * 0.5, distance(p, vec2(uLens.x, pl.y)));
    c += vec3(0.08 * pr + 0.15 * pr * glow) * (useA ? 1.0 : 0.0);
    float mixIn = inP * uGlassMix;
    outside = outside * (1.0 - mixIn) + vec4(clamp(c, 0.0, 1.0), 1.0) * mixIn;
  }

  // ====== 2. 검은 글자·아이콘
  float aOut = cover(p);
  outside = outside * (1.0 - aOut) + vec4(uInk * aOut, aOut);

  // 선택 알약의 그림자 (누를 때만)
  vec2 gs;
  float ds = seDist(q - vec2(0.0, 4.0), hb, uExp, gs);
  float dsx = max(ds, 0.0);
  float sh = 0.0; // 알약 그림자도 없앴어요
  outside = outside * (1.0 - sh) + vec4(0.0, 0.0, 0.0, sh);

  if (d > 1.5) {
    gl_FragColor = outside;
    return;
  }

  // ====== 3. 선택 알약: 쉬고 있을 때는 맑은 유리 + 검정 10%, 누르면 렌즈(10, 14)와 색 번짐
  float inMask = clamp(0.5 - d * uDpr, 0.0, 1.0);
  vec3 col;
  float cov;
  vec3 covRGB = vec3(0.0);
  if (uGlass > 0.5) {
    vec4 plL = abs(uLens.x - barA.x) < abs(uLens.x - barB.x) ? barA : barB;
    vec2 gL;
    float sdL = seDist(q, hb, uExp, gL);
    float spL;
    float B = 12.0 * pr + 0.001;
    float shL = tiShift(max(-sdL, 0.0), B, 22.0 * pr, 2.6, 3.0, spL) * step(0.001, pr);
    vec2 inw = -gL;
    // 렌즈는 살짝 확대해요 (1.1배)
    vec2 zp = uLens.xy + (p - uLens.xy) / (1.0 + 0.1 * pr);
    float dsp = 0.045 * pr;
    vec2 rp = zp + inw * shL;
    vec2 rR = zp + inw * shL * (1.0 - dsp);
    vec2 rB = zp + inw * shL * (1.0 + dsp);
    rp.y = clamp(rp.y, plL.y - plL.w + 0.5, plL.y + plL.w - 0.5);
    rR.y = clamp(rR.y, plL.y - plL.w + 0.5, plL.y + plL.w - 0.5);
    rB.y = clamp(rB.y, plL.y - plL.w + 0.5, plL.y + plL.w - 0.5);
    // 렌즈 안은 그대로 비치지 않고 살짝 뿌옇게 (흐림 0.55)
    vec3 c7 = vec3(barGlass(rR, plL, 0.55).r, barGlass(rp, plL, 0.55).g, barGlass(rB, plL, 0.55).b);
    c7 = tiRim(c7, -sdL, spL, B, gL, 0.25 * pr);
    col = c7;
    // 강조색 아이콘도 같은 렌즈를 지나요
    cov = max(cover(zp + inw * shL), 0.0) * smoothstep(-0.5, 2.0, -d);
    // 프리즘: 아이콘 가장자리도 빨강·초록·파랑이 따로 휘어서 무지개 띠가 생겨요 (애플 렌즈처럼)
    covRGB = vec3(cov);
    if (pr > 0.01) {
      float ap = smoothstep(-0.5, 2.0, -d);
      vec2 base = zp + inw * shL;
      vec2 off = (inw * shL * 0.35 + (q / max(hb, vec2(1.0))) * 4.5) * pr;
      covRGB = vec3(cover(base + off), cover(base), cover(base - off)) * ap;
    }
  } else {
    // 뒤 화면 사진이 없을 때: 예전처럼 반투명 알약
    vec4 fill = mix(uPill, uTint, pr);
    cov = cover(p);
    vec3 prem = fill.rgb * fill.a * (1.0 - cov) + uAccent * cov;
    float al = fill.a + (1.0 - fill.a) * cov;
    gl_FragColor = outside * (1.0 - inMask) + vec4(prem, al) * inMask;
    return;
  }
  // 표면: 선택된 알약은 한 톤 진한 회색 (다크는 더 까맣게). 움직이는 렌즈도 살짝 진해요
  col = mix(col, vec3(0.0), (uDark > 0.5 ? 0.38 : 0.13) * (1.0 - pr));
  col = mix(col, vec3(0.0), (uDark > 0.5 ? 0.22 : 0.1) * pr);
  col = col * (1.0 - covRGB) + uAccent * covRGB;
  // 속 그림자 (8px, 위에서 아래로, 검정 15%) — 누를 때만
  vec2 gi;
  float di = seDist(q - vec2(0.0, 8.0 * pr), hb, uExp, gi);
  float inner = 0.15 * pr * smoothstep(-8.0 * pr - 0.001, 8.0 * pr + 0.001, di);
  col = mix(col, vec3(0.0), inner);
  // 45도 빛 (누를 때만)
  vec4 inside = vec4(clamp(col, 0.0, 1.0), 1.0);
  gl_FragColor = outside * (1.0 - inMask) + inside * inMask;
}
