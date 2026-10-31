/* 파이어베이스 연결: 익명으로 로그인하고, 내 상태(프로필·배경화면·대화)를 Firestore의 users/{uid} 문서 하나에 저장해요.
   설정값이 비어 있거나 연결에 실패하면 window.cloud는 null이고, 앱은 이 기기(localStorage)에만 저장해요. */
import { firebaseConfig } from "./firebase-config.js";

const BASE = "https://www.gstatic.com/firebasejs/11.0.2";

window.cloudReady = (async () => {
  if (!firebaseConfig.apiKey || !firebaseConfig.projectId) return null;
  try {
    const [{ initializeApp }, { getAuth, signInAnonymously, onAuthStateChanged }, { getFirestore, doc, getDoc, setDoc }] = await Promise.all([
      import(`${BASE}/firebase-app.js`),
      import(`${BASE}/firebase-auth.js`),
      import(`${BASE}/firebase-firestore.js`),
    ]);
    const app = initializeApp(firebaseConfig);
    const auth = getAuth(app);
    const db = getFirestore(app);
    const user = await new Promise((resolve, reject) => {
      const off = onAuthStateChanged(auth, (u) => {
        if (u) { off(); resolve(u); }
      });
      if (!auth.currentUser) signInAnonymously(auth).catch(reject);
    });
    const ref = doc(db, "users", user.uid);
    window.cloud = {
      uid: user.uid,
      load: async () => {
        const snap = await getDoc(ref);
        return snap.exists() ? snap.data() : null;
      },
      save: (data) => setDoc(ref, { ...data, updatedAt: Date.now() }),
    };
    return window.cloud;
  } catch (err) {
    console.warn("파이어베이스에 연결하지 못했어요:", err);
    return null;
  }
})();
