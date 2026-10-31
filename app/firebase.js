/* 파이어베이스 연결: 처음에는 익명으로 로그인하고, 아이디·비밀번호로 가입·로그인(익명 계정을 이어붙임)할 수 있어요.
   내 상태(프로필·배경화면·대화)는 Firestore의 users/{uid} 문서 하나에 저장해요.
   설정값이 비어 있거나 연결에 실패하면 window.cloud는 null이고, 앱은 이 기기(localStorage)에만 저장해요. */
import { firebaseConfig } from "./firebase-config.js?v=6";

const BASE = "https://www.gstatic.com/firebasejs/11.0.2";

const MESSAGES = {
  "auth/invalid-email": "아이디가 올바르지 않아요",
  "auth/missing-password": "비밀번호를 입력해 주세요",
  "auth/weak-password": "비밀번호는 6자 이상이어야 해요",
  "auth/email-already-in-use": "이미 있는 아이디예요. 다른 아이디를 써 주세요",
  "auth/invalid-credential": "아이디 또는 비밀번호가 맞지 않아요",
  "auth/wrong-password": "아이디 또는 비밀번호가 맞지 않아요",
  "auth/user-not-found": "아이디 또는 비밀번호가 맞지 않아요",
  "auth/too-many-requests": "시도가 너무 많아요. 잠시 뒤 다시 해 주세요",
  "auth/popup-closed-by-user": "로그인 창이 닫혔어요",
  "auth/cancelled-popup-request": "로그인 창이 닫혔어요",
  "auth/popup-blocked": "팝업이 막혔어요. 허용한 뒤 다시 눌러 주세요",
  "auth/operation-not-allowed": "이 로그인 방법이 아직 켜지지 않았어요",
  "auth/admin-restricted-operation": "익명 로그인이 꺼져 있어요",
  "auth/unauthorized-domain": "이 주소는 아직 허용되지 않았어요",
  "auth/network-request-failed": "인터넷 연결을 확인해 주세요",
};
const toEmail = (id) => `${id.trim().toLowerCase()}@money-sim.app`;
const explain = (err) => MESSAGES[err?.code] || "로그인하지 못했어요. 잠시 뒤 다시 해 주세요";

window.cloudReady = (async () => {
  if (!firebaseConfig.apiKey || !firebaseConfig.projectId) {
    window.cloudError = "설정값이 비어 있어요";
    return null;
  }
  try {
    const [{ initializeApp }, A, F] = await Promise.all([
      import(`${BASE}/firebase-app.js`),
      import(`${BASE}/firebase-auth.js`),
      import(`${BASE}/firebase-firestore.js`),
    ]);
    const app = initializeApp(firebaseConfig);
    const auth = A.getAuth(app);
    const { getFirestore, doc, getDoc, setDoc, collection, query, where, orderBy, onSnapshot, addDoc, updateDoc } = F;
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
        return { anonymous: u.isAnonymous, id: (u.email || "").replace(/@money-sim\.app$/, "") };
      },
      load: async () => {
        const snap = await getDoc(doc(db, "users", auth.currentUser.uid));
        return snap.exists() ? snap.data() : null;
      },
      save: (data) => setDoc(doc(db, "users", auth.currentUser.uid), { ...data, updatedAt: Date.now() }),
      // 가입: 지금의 익명 계정에 이메일을 이어붙여서 그동안 쓴 것이 그대로 남아요
      // 아이디는 안 보이는 가짜 이메일(아이디@money-sim.app)로 바꿔서 파이어베이스 이메일 로그인에 써요
      signUp: (id, pw) =>
        wrap(async () => {
          await A.linkWithCredential(auth.currentUser, A.EmailAuthProvider.credential(toEmail(id), pw));
          // 다른 사람이 아이디로 나를 찾을 수 있게 등록해요
          await setDoc(doc(db, "usernames", id.trim().toLowerCase()), { uid: auth.currentUser.uid, id: id.trim() });
        }),
      signIn: (id, pw) => wrap(() => A.signInWithEmailAndPassword(auth, toEmail(id), pw)),
      signOut: () => A.signOut(auth),

      /* ----- 사람 사이의 메시지 ----- */
      findUser: async (id) => {
        const s = await getDoc(doc(db, "usernames", id.trim().toLowerCase()));
        return s.exists() ? s.data() : null;
      },
      chatId: (other) => [auth.currentUser.uid, other].sort().join("_"),
      openChat: async (other, otherId) => {
        const me = auth.currentUser;
        const cid = [me.uid, other].sort().join("_");
        const myId = (me.email || "").replace(/@money-sim\.app$/, "");
        await setDoc(doc(db, "chats", cid), { members: [me.uid, other], names: { [me.uid]: myId, [other]: otherId } }, { merge: true });
        return cid;
      },
      watchChats: (cb) =>
        onSnapshot(query(collection(db, "chats"), where("members", "array-contains", auth.currentUser.uid)), (snap) => cb(snap.docs.map((d) => ({ id: d.id, ...d.data() })))),
      watchMessages: (cid, cb) =>
        onSnapshot(query(collection(db, "chats", cid, "messages"), orderBy("at")), (snap) => cb(snap.docs.map((d) => d.data()))),
      send: async (cid, text) => {
        const at = Date.now();
        await addDoc(collection(db, "chats", cid, "messages"), { from: auth.currentUser.uid, text, at });
        await updateDoc(doc(db, "chats", cid), { last: text, lastAt: at, lastFrom: auth.currentUser.uid, [`reads.${auth.currentUser.uid}`]: at });
      },
      markRead: (cid) => updateDoc(doc(db, "chats", cid), { [`reads.${auth.currentUser.uid}`]: Date.now() }).catch(() => {}),
    };
    return window.cloud;
  } catch (err) {
    console.warn("파이어베이스에 연결하지 못했어요:", err);
    // 화면에서 원인을 알려 주려고 남겨 둬요
    window.cloudError = err?.code ? explain(err) + ` (${err.code})` : `연결하지 못했어요 (${err?.message || err})`;
    return null;
  }
})();
