# html2canvas

[html2canvas](https://github.com/niklasvh/html2canvas) 1.4.1 (MIT, `LICENSE` 참고). 배포본 `html2canvas.min.js`를 그대로 넣었어요.

탭 바의 막대 유리가 뒤 화면을 굴절해서 보여주려고, 화면을 한 번 캔버스로 찍는 데만 써요(`app/backdrop.js`).
인터넷 연결 없이도 돌도록 CDN을 쓰지 않고 파일로 넣었어요.

없거나 불러오지 못해도 사이트는 돌아가요. 그때는 막대 유리가 CSS 유리 판(`backdrop-filter`)으로 보여요.
