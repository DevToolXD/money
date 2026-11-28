#!/bin/sh
# 안드로이드 앱(APK) 만들기: 사이트를 감싼 WebView 앱 하나예요. 결과는 저장소 맨 위 capitalism.apk (사이트에서 내려받게 해요).
# 안드로이드 스튜디오 없이 만들어요: aapt2(apktool 안에 들어 있어요)·dx·안드로이드 클래스 모음을 메이븐 센트럴에서 받아 써요.
# 서명 열쇠(capitalism.keystore)는 저장소에 올리지 않아요(.gitignore). 잃어버리면 새 열쇠로 만든 앱은 옛 앱 위에 덮어 설치되지 않아요.
set -eu
cd "$(dirname "$0")"
T=.tools
B=build
M=https://repo.maven.apache.org/maven2
mkdir -p "$T" "$B"
fetch() {
  [ -s "$2" ] && return 0
  for i in 1 2 3 4 5; do
    curl -sf -m 900 -o "$2.part" "$1" && mv "$2.part" "$2" && return 0
    sleep 4
  done
  echo "받지 못했어요: $1" >&2
  exit 1
}
fetch "$M/org/robolectric/android-all/14-robolectric-10818077/android-all-14-robolectric-10818077.jar" "$T/android.jar"
fetch "$M/org/apktool/apktool-lib/3.0.3/apktool-lib-3.0.3.jar" "$T/apktool-lib.jar"
fetch "$M/com/jakewharton/android/repackaged/dalvik-dx/16.0.1/dalvik-dx-16.0.1.jar" "$T/dx.jar"
if [ ! -x "$T/aapt2" ]; then
  unzip -p "$T/apktool-lib.jar" prebuilt/linux/aapt2 > "$T/aapt2"
  chmod +x "$T/aapt2"
fi
KS=capitalism.keystore
PASS=capitalism-sim
if [ ! -f "$KS" ]; then
  keytool -genkeypair -keystore "$KS" -storepass "$PASS" -keypass "$PASS" -alias key -keyalg RSA -keysize 2048 -validity 36500 -dname "CN=Capitalism Simulator, O=Jabon, C=KR"
fi
rm -rf "$B"/*
"$T/aapt2" compile --dir res -o "$B/res.zip"
"$T/aapt2" link -o "$B/base.apk" -I "$T/android.jar" --manifest AndroidManifest.xml \
  --min-sdk-version 24 --target-sdk-version 29 --version-code "${VERSION_CODE:-1}" --version-name "${VERSION_NAME:-1.0}" "$B/res.zip"
mkdir -p "$B/classes"
javac --release 8 -encoding UTF-8 -Xlint:-options -cp "$T/android.jar" -d "$B/classes" $(find src -name '*.java')
java -cp "$T/dx.jar" com.android.dx.command.Main --dex --output="$B/classes.dex" "$B/classes"
cp "$B/base.apk" "$B/unsigned.apk"
(cd "$B" && zip -q -j unsigned.apk classes.dex)
jarsigner -keystore "$KS" -storepass "$PASS" -sigalg SHA256withRSA -digestalg SHA-256 -signedjar ../capitalism.apk "$B/unsigned.apk" key >/dev/null
jarsigner -verify ../capitalism.apk | head -1
ls -la ../capitalism.apk
