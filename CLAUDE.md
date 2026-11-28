# 이 프로젝트

자본주의 시뮬레이터 (휴대폰 기준 정적 사이트, GitHub Pages, Firebase).

## "자본 디자인"

- 애플의 손글씨 움직임(새 기기에서 hello, 안녕하세요처럼 글씨가 써지는 모션)은 앞으로 **"글씨 모션"**이라고 불러요. 빈 화면의 empty 도 같은 글씨 모션으로 써요.

사용자가 "자본 디자인"이라고 부르는 것은 **실제 Apple Store 앱(iOS 26) 스타일**이에요. 새 화면은 항상 이 디자인으로 만들어요.

- 연한 라벤더 회색 바탕(#f2f2f7), 모서리 10px의 흰 카드 (예전 28px에서 줄였어요), 카드에 그림자 없음
- 굵은 큰 제목(36px, 800), 섹션 제목 26px 800, 파란 강조색(#0a7cff)
- 위쪽 유리 알약 버튼(편집 등), 스크롤하면 접히며 나타나는 유리 상단 바
- 가로로 넘기는 카드 줄, 알약 칩, 두 장씩 나란한 카드, 설정 목록(색 아이콘 + ›)
- 리퀴드 글라스: 하단 탭 바, 유리 버튼, 로그인 입력 카드. 모양은 Kyant Liquid Glass(Android)를 옮긴 것이에요: 굴절 lens(버튼 12·24, 큰 판 24·48), 45도 빛 0.5px 흰 선, 채도 1.5배, 누르면 빛 번짐·부풀기·끌기. 탭 바는 `web/crates/ui/src/tabbar.rs`, 나머지는 `web/crates/ui/src/glass.rs`(유리 목록 `SEL`)가 그려요. 새 유리를 만들면 `SEL`에 넣어요
- 토스 스타일은 쓰지 않아요. 문구는 해요체, 모든 곳에서 꾹 눌러도 글자가 선택되지 않게(입력칸 제외)

## 러스트 원칙 (가장 먼저 지킬 것)

- **앞으로 만드는 모든 것은 러스트로 만들어요.** 새 JS는 쓰지 않아요.
- **남은 JS도 러스트로 옮겼어요.** 남은 것은 `app/guard.js`(화면 그리기 전에 돌아야 해서 WASM으로는 못 해요)와 라이브러리 연결 목록뿐이에요.
- 옮길 때 **품질이 떨어지거나, 버그가 생기거나, 모양·움직임·문구가 조금이라도 달라지면 안 돼요.** 옮기기 전에 예전 JS로 화면 시험과 사진을 먼저 만들어 두고, 러스트로 옮긴 뒤 같은 시험이 통과하고 사진이 같은지 확인해요 (상품 그림처럼 글자까지 비교할 수 있으면 비교해요).
- 구글이 JS로만 주는 Firebase 라이브러리와 외부 라이브러리(html2canvas)는 우리가 쓴 코드가 아니에요. 러스트로 대신할 수 있는지 먼저 확인하고, 실시간 듣기 같은 기능이 떨어지면 옮기지 않고 사용자에게 알려요.

## 철칙

- **이모지를 절대 쓰지 않아요.** 아이콘은 항상 직접 그린 심플하고 모던한 검은 윤곽선(SVG, 선 굵기 1.8~1.9, 둥근 끝)이에요.
- 이 게임의 화폐 단위는 **멸공**이에요 (예: 1,250,000 멸공, 새 계정은 0 멸공으로 시작해요). 계좌번호 같은 은행 개념은 쓰지 않아요.

## 서버·보안 (바꿀 때 지킬 것)

- 지갑·선물·이자는 `firestore.rules`가 서버에서 검사해요. 규칙을 고치면 `cd tests && npm run rules`를 꼭 돌려요.
- `app/firebase.js`는 `app/firebase.src.js`를 묶은 결과물이라 직접 고치지 않아요 (`npm run build`).
- 화면과 앱 틀은 모두 러스트예요: `web/` (기록: `docs/rust-migration-plan.md`). `app.js`는 없어요. 탭 바·뒤 화면 사진·아이콘·리퀴드 글라스·서버 연결도 러스트예요(`web/crates/ui/src/tabbar.rs`, `backdrop.rs`, `icons.rs`, `glass.rs`, `fire.rs`). JS로 남은 우리 코드는 `app/guard.js`(그리기 전에 돌아야 해요), `app/firebase.src.js`(구글 라이브러리를 window.fb로 넘기는 목록뿐), `app/firebase-config.js`, `app/rs/boot.js`예요. `app/rs/`는 `sh web/build.sh`의 결과물이라 직접 고치지 않아요. 러스트를 고치면 빌드해서 `app/rs/`도 같이 올려요.
- 상품 그림(`web/crates/goods`)을 고치면 `cd web && cargo run -p goods --example dump`로 확인하고, 예전 JS와 같아야 하는 동안은 `sh tests/goods/same.sh`를 돌려요.
- `view!` 매크로 안의 속성 닫는 괄호에서 `>`, `>=`는 태그 끝으로 읽혀요. `disabled=move || { a >= b }`처럼 중괄호로 감싸요.
- 화면 시험: `cd tests && npm run build && EMU=1 node build.mjs && sh e2e/run.sh` (화면 하나를 옮길 때마다 돌려요).
- 회원가입 전화번호 인증은 쓰지 않아요 (한국 통신사가 해외 문자 업체의 문자를 막아서 문자가 안 갔어요). 대신 부계정은 "초대한 친구가 3시간 플레이해야 초대 보상 50 멸공을 받는다"로 막아요 (presence/{uid}.play, firestore.rules). `server/phone-verify`와 phones·verified·otp 규칙은 쓰지 않는 채로 남겨 뒀어요.
- 안드로이드 앱(APK)은 `android/` (사이트를 감싼 WebView 하나, 자바라서 러스트 원칙의 예외예요). `sh android/build.sh`가 저장소 맨 위 `capitalism.apk`를 만들고 사이트가 내려받게 해요. 서명 열쇠(`android/capitalism.keystore`)와 받은 도구(`android/.tools`)는 올리지 않아요. 앱은 이름표에 `JabonApp`을 붙여서 `install.rs`(설치 안내)가 알아봐요. 앱 모양을 바꿀 때만 다시 빌드해요 (사이트 내용은 WebView가 매번 불러와요).
- 베타 초기화(관리 탭)는 Firestore를 모두 지우고(`meta/beta`를 done으로), 로그인 계정은 `fire.rs`의 `reset_account`/`sign_in`/`reclaim`이 "초기화 전에 만든 계정"을 사람들이 앱을 열 때·로그인할 때·같은 비밀번호로 가입할 때 스스로 지워요 (브라우저에서는 남의 계정을 지울 수 없어서요. owner는 빼요). 관리자가 다시 누르면 done에서 다시 시작해요.
- 월급: 앱을 켜 두고 본 시간(presence.play) 5분마다 20 멸공 (뒤에 가 있는 시간은 안 세요). 받은 횟수는 presence.paid, 지갑 op 'salary' (`fire.rs` salary_claim, `presence.rs` pay_salary, firestore.rules).
- 사진 조정: 프로필 사진을 고르면 `crop.rs`의 조정 화면(끌고 모아서 정사각형 틀에 맞춰 저장)이 떠요. 밀어서 답장은 `chat/reply.rs`(말풍선을 오른쪽으로 밀기, 메시지 're'), 자판 위치 맞춤은 `viewport.rs`예요.
- 익명 로그인 금지. 사용자 글·사진은 반드시 `esc()`/`safeImg()`를 거쳐요. 인라인 스크립트는 CSP가 막아요.
- 서버를 아끼는 구조(최근 40개만 듣기, 프로필 캐시, 이자 10분 정산)와 backdrop 사진 찍기 간격을 느슨하게 되돌리지 않아요.
- 탭 바 뒤 화면 사진(html2canvas)은 껐어요. 찍을 때마다 화면이 50~430ms 멈춰서(폰처럼 느린 CPU로 쟀어요) 렉의 주범이었어요. 탭 바 유리는 화면 바탕색으로 그려요. 다시 켜지 말아요.
- 2초마다 다시 그리는 곳은 `dom::set_text`·`dom::set_html_if`처럼 바뀐 경우에만 DOM을 건드려요. 매 프레임 `scrollTop` 같은 레이아웃 값을 읽지 않아요.
