# money

토스 스타일 디자인 시스템(`design-system/`)을 쓰는 모바일 중심 정적 사이트예요. 빌드 도구 없이 HTML 파일을 그대로 열면 돼요.

## 구조

```
index.html          홈 화면 시작본(계좌 목록, 금액, 하단 버튼)
design-system/      디자인 시스템 원본(토큰, 컴포넌트, 서체, 가이드)
  README.md         색·글자·간격·모서리 규칙
  guidelines/       보이스와 톤, 화면 패턴, 모션
  components/       React 컴포넌트 11개(window.TossStyle)와 사용 예시
  example.html      컴포넌트 전체를 쓴 예시
```

## 실행

```bash
python3 -m http.server 8000
```

브라우저에서 `http://localhost:8000`을 열어요. React는 cdnjs에서 불러오므로 인터넷 연결이 필요해요.

## 디자인 규칙

- 화면은 모바일 단일 칼럼(375~430px)으로 만들고, PC에서는 가운데에 같은 칼럼을 둬요.
- 이 스타일로 만든 화면에 토스의 이름, 로고, 앱 아이콘을 넣지 않아요.
- 문구는 해요체로 쓰고, 금융 용어는 일상 말로 바꿔 써요(`design-system/guidelines/10-writing.md`).
