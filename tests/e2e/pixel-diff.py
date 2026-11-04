# 두 폴더의 같은 이름 사진을 비교해요: 다른 픽셀 비율과 최대 차이 (러스트로 옮기기 전후 확인용)
import sys, os
from PIL import Image, ImageChops
a, b = sys.argv[1], sys.argv[2]
bad = 0
for n in sorted(os.listdir(a)):
    if not n.endswith('.png'):
        continue
    A = Image.open(os.path.join(a, n)).convert('RGB')
    B = Image.open(os.path.join(b, n)).convert('RGB')
    d = ImageChops.difference(A, B)
    px = d.getdata()
    diff = sum(1 for p in px if max(p) > 12)
    mx = max(max(p) for p in px)
    ratio = diff / (A.width * A.height)
    flag = 'OK ' if ratio < 0.002 else 'DIF'
    if ratio >= 0.002: bad += 1
    print(f'{flag} {n}: {ratio*100:.3f}% 다름, 최대 {mx}')
sys.exit(1 if bad else 0)
