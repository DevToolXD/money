/* 파이어베이스 연결: 처음에는 익명으로 로그인하고, 이메일·구글로 로그인(익명 계정을 이어붙임)할 수 있어요.
   내 상태(프로필·배경화면·대화)는 Firestore의 users/{uid} 문서 하나에 저장해요.
   설정값이 비어 있거나 연결에 실패하면 window.cloud는 null이고, 앱은 이 기기(localStorage)에만 저장해요. */
import { firebaseConfig } from "./firebase-config.js";

const BASE = "https://www.gstatic.com/firebasejs/11.0.2";

const MESSAGES = {
  "auth/invalid-email": "이메일 주소가 올바르지 않아요",
  "auth/missing-password": "비밀번호를 입력해 주세요",
  "auth/weak-password": "비밀번호는 6자 이상이어야 해요",
  "auth/email-already-in-use": "이미 가입된 이메일이에요. 로그인해 주세요",
  "auth/invalid-credential": "이메일 또는 비밀번호가 맞지 않아요",
  "auth/wrong-password": "이메일 또는 비밀번호가 맞지 않아요",
  "auth/user-not-found": "이메일 또는 비밀번호가 맞지 않아요",
  "auth/too-many-requests": "시도가 너무 많아요. 잠시 뒤 다시 해 주세요",
  "auth/popup-closed-by-user": "로그인 창이 닫혔어요",
  "auth/cancelled-popup-request": "로그인 창이 닫혔어요",
  "auth/popup-blocked": "팝업이 막혔어요. 허용한 뒤 다시 눌러 주세요",
  "auth/operation-not-allowed": "이 로그인 방법이 아직 켜지지 않았어요",
  "auth/unauthorized-domain": "이 주소는 아직 허용되지 않았어요",
  "auth/network-request-failed": "인터넷 연결을 확인해 주세요",
};
const explain = (err) => MESSAGES[err?.code] || "로그인하지 못했어요. 잠시 뒤 다시 해 주세요";

window.cloudReady = (async () => {
  if (!firebaseConfig.apiKey || !firebaseConfig.projectId) return null;
  try {
    const [{ initializeApp }, A, { getFirestore, doc, getDoc, setDoc }] = await Promise.all([
      import(`${BASE}/firebase-app.js`),
      import(`${BASE}/firebase-auth.js`),
      import(`${BASE}/firebase-firestore.js`),
    ]);
    const app = initializeApp(firebaseConfig);
    const auth = A.getAuth(app);
    const db = getFirestore(app);
    auth.languageCode = "ko";
    const user = await new Promise((resolve, reject) => {
      const off = A.onAuthStateChanged(auth, (u) => {
        if (u) { off(); resolve(u); }
      });
      if (!auth.currentUser) A.signInAnonymously(auth).catch(reject);
    });
    const wrap = (fn) => fn().catch((err) => { throw new Error(explain(err)); });
    window.cloud = {
      get uid() { return auth.currentUser.uid; },
      get user() {
        const u = auth.currentUser;
        return { anonymous: u.isAnonymous, email: u.email || "", name: u.displayName || "" };
      },
      load: async () => {
        const snap = await getDoc(doc(db, "users", auth.currentUser.uid));
        return snap.exists() ? snap.data() : null;
      },
      save: (data) => setDoc(doc(db, "users", auth.currentUser.uid), { ...data, updatedAt: Date.now() }),
      // 가입: 지금의 익명 계정에 이메일을 이어붙여서 그동안 쓴 것이 그대로 남아요
      signUp: (email, pw) => wrap(() => A.linkWithCredential(auth.currentUser, A.EmailAuthProvider.credential(email, pw))),
      signIn: (email, pw) => wrap(() => A.signInWithEmailAndPassword(auth, email, pw)),
      google: () =>
        wrap(async () => {
          const provider = new A.GoogleAuthProvider();
          try {
            await A.linkWithPopup(auth.currentUser, provider);
          } catch (err) {
            if (err.code === "auth/credential-already-in-use") await A.signInWithCredential(auth, A.GoogleAuthProvider.credentialFromError(err));
            else throw err;
          }
        }),
      signOut: () => A.signOut(auth),
    };
    return window.cloud;
  } catch (err) {
    console.warn("파이어베이스에 연결하지 못했어요:", err);
    return null;
  }
})();
