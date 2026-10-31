/* 파이어베이스 연결의 원본 파일. `npm run build`(tests 폴더)가 이 파일을 app/firebase.js 한 개로 묶어요.
   - 로그인은 아이디·비밀번호(안 보이는 이메일로 바꿔서) 하나뿐이고 익명 로그인은 쓰지 않아요.
   - 서버 규칙(firestore.rules)이 진짜 지킴이고, 여기서는 규칙이 받아 주는 모양으로만 써요.
   - 읽기·쓰기를 아끼려고: 메시지는 최근 40개만 듣고, 사람 정보는 30분 캐시하고, 지갑 이자는 10분에 한 번만 정산해요.
   설정값이 비어 있거나 연결에 실패하면 window.cloud는 null이에요. */
import { firebaseConfig } from "./firebase-config.js";
import { initializeApp } from "firebase/app";
import {
  initializeAuth,
  indexedDBLocalPersistence,
  browserLocalPersistence,
  onAuthStateChanged,
  createUserWithEmailAndPassword,
  signInWithEmailAndPassword,
  signOut as fbSignOut,
  updateProfile,
  deleteUser,
} from "firebase/auth";
import {
  initializeFirestore,
  memoryLocalCache,
  doc,
  getDoc,
  setDoc,
  updateDoc,
  collection,
  query,
  where,
  orderBy,
  limit,
  startAfter,
  onSnapshot,
  getDocs,
  writeBatch,
  serverTimestamp,
  increment,
  Timestamp,
} from "firebase/firestore";

const MESSAGES = {
  "auth/invalid-email": "아이디가 올바르지 않아요",
  "auth/missing-password": "비밀번호를 입력해 주세요",
  "auth/weak-password": "비밀번호가 너무 약해요",
  "auth/email-already-in-use": "이미 있는 아이디예요. 다른 아이디를 써 주세요",
  "auth/invalid-credential": "아이디 또는 비밀번호가 맞지 않아요",
  "auth/wrong-password": "아이디 또는 비밀번호가 맞지 않아요",
  "auth/user-not-found": "아이디 또는 비밀번호가 맞지 않아요",
  "auth/too-many-requests": "시도가 너무 많아요. 잠시 뒤 다시 해 주세요",
  "auth/operation-not-allowed": "이 로그인 방법이 아직 켜지지 않았어요",
  "auth/network-request-failed": "인터넷 연결을 확인해 주세요",
};
const explain = (err) => MESSAGES[err?.code] || "처리하지 못했어요. 잠시 뒤 다시 해 주세요";
const toEmail = (id) => `${id.trim().toLowerCase()}@money-sim.app`;
const ms = (t) => (t && typeof t.toMillis === "function" ? t.toMillis() : Number(t) || 0);
const sorted = (a, b) => (a < b ? [a, b] : [b, a]);

const PROFILE_TTL = 30 * 60 * 1000;

window.cloudReady = (async () => {
  if (!firebaseConfig.apiKey || !firebaseConfig.projectId) {
    window.cloudError = "설정값이 비어 있어요";
    return null;
  }
  try {
    const app = initializeApp(firebaseConfig);
    const auth = initializeAuth(app, { persistence: [indexedDBLocalPersistence, browserLocalPersistence] });
    auth.languageCode = "ko";
    // 느린 망이나 프록시에서도 끊기지 않게 자동으로 긴 연결 방식을 골라요. 오프라인 저장은 쓰지 않아요(메모리만).
    const db = initializeFirestore(app, { localCache: memoryLocalCache(), experimentalAutoDetectLongPolling: true });

    // 로그인 상태가 정해질 때까지 한 번만 기다려요
    await new Promise((resolve) => {
      const off = onAuthStateChanged(auth, () => {
        off();
        resolve();
      });
    });

    const me = () => {
      if (!auth.currentUser) throw new Error("로그인이 필요해요");
      return auth.currentUser;
    };
    const uidOf = () => me().uid;
    const walletRef = () => doc(db, "wallets", uidOf());
    const wrap = async (fn) => {
      try {
        return await fn();
      } catch (err) {
        throw Object.assign(new Error(explain(err)), { code: err?.code });
      }
    };
    const myId = () => (auth.currentUser?.email || "").replace(/@money-sim\.app$/, "");

    /* ----- 사람 정보 캐시 (메모리 + 이 기기) ----- */
    const mem = new Map();
    const readCache = (uid) => {
      const hit = mem.get(uid);
      if (hit) return hit;
      try {
        const raw = localStorage.getItem("pf:" + uid);
        if (raw) {
          const v = JSON.parse(raw);
          mem.set(uid, v);
          return v;
        }
      } catch (_) {
        /* 캐시가 없어도 괜찮아요 */
      }
      return null;
    };
    const writeCache = (uid, d) => {
      const v = { d, t: Date.now() };
      mem.set(uid, v);
      try {
        localStorage.setItem("pf:" + uid, JSON.stringify(v));
      } catch (_) {
        /* 가득 찼어요 */
      }
    };

    const makeWallet = () => setDoc(walletRef(), { bal: 1250000, rate: 0, t: serverTimestamp(), wt: serverTimestamp(), op: { kind: "init" } });
    const chatPart = (d) => ({
      id: d.id,
      members: d.members,
      last: d.last || "",
      lastFrom: d.lastFrom || "",
      lastAt: ms(d.lastAt),
      reads: Object.fromEntries(Object.entries(d.reads || {}).map(([k, v]) => [k, ms(v)])),
    });
    const msgPart = (s) => {
      const d = s.data({ serverTimestamps: "estimate" });
      return { id: s.id, ...d, at: ms(d.at) };
    };

    window.cloud = {
      get uid() {
        return auth.currentUser?.uid || "";
      },
      // 로그인한 사람 (아니면 null)
      get user() {
        const u = auth.currentUser;
        return u ? { id: myId(), name: u.displayName || "" } : null;
      },

      /* ----- 계정 ----- */
      signUp: (id, pw, name, photo = "") =>
        wrap(async () => {
          const cred = await createUserWithEmailAndPassword(auth, toEmail(id), pw);
          try {
            await updateProfile(cred.user, { displayName: name });
            await setDoc(doc(db, "profiles", cred.user.uid), { id: id.trim().toLowerCase(), name, photo: photo.length <= 40000 ? photo : "" });
            await makeWallet();
          } catch (err) {
            // 반쪽짜리 계정이 남지 않게 지워요
            await deleteUser(cred.user).catch(() => {});
            throw err;
          }
        }),
      signIn: (id, pw) => wrap(() => signInWithEmailAndPassword(auth, toEmail(id), pw)),
      signOut: () => fbSignOut(auth),
      // 예전 버전으로 만든 계정에는 사람 정보가 없어서 한 번 채워요
      ensureAccount: async () => {
        const uid = uidOf();
        if (localStorage.getItem("acct-ok") === uid) return;
        const ref = doc(db, "profiles", uid);
        let exists = true;
        try {
          exists = (await getDoc(ref)).exists();
        } catch (_) {
          exists = false;
        }
        if (!exists) await setDoc(ref, { id: myId(), name: auth.currentUser.displayName || "", photo: "" });
        localStorage.setItem("acct-ok", uid);
      },

      /* ----- 내 설정 (프로필 글, 대화 배경) ----- */
      loadSettings: async () => {
        const s = await getDoc(doc(db, "users", uidOf()));
        return s.exists() ? s.data() : null;
      },
      saveSettings: (data) => setDoc(doc(db, "users", uidOf()), { ...data, updatedAt: Date.now() }, { merge: true }),

      /* ----- 사람 찾기와 사진 ----- */
      peek: (uid) => readCache(uid)?.d || null,
      fetchProfiles: async (uids) => {
        const need = [...new Set(uids)].filter((u) => {
          const c = readCache(u);
          return !c || Date.now() - c.t > PROFILE_TTL;
        });
        await Promise.all(
          need.slice(0, 30).map(async (u) => {
            try {
              const s = await getDoc(doc(db, "profiles", u));
              if (s.exists()) writeCache(u, s.data());
            } catch (_) {
              /* 못 읽어도 다음에 다시 시도해요 */
            }
          })
        );
        return need.length;
      },
      searchUsers: async (q) => {
        const t = q.trim();
        if (!t) return [];
        const snap = await getDocs(query(collection(db, "profiles"), where("name", ">=", t), where("name", "<=", t + ""), limit(10)));
        return snap.docs
          .filter((d) => d.id !== uidOf())
          .map((d) => {
            writeCache(d.id, d.data());
            return { uid: d.id, ...d.data() };
          });
      },
      setPhoto: async (photo) => {
        await updateDoc(doc(db, "profiles", uidOf()), { photo });
        const c = readCache(uidOf());
        writeCache(uidOf(), { ...(c?.d || { id: myId(), name: auth.currentUser.displayName || "" }), photo });
      },

      /* ----- 토큰 지갑: 값은 서버 규칙이 지켜요 ----- */
      watchWallet: (cb) =>
        onSnapshot(
          walletRef(),
          (s) => {
            if (!s.exists()) {
              makeWallet().catch(() => {});
              return;
            }
            const d = s.data({ serverTimestamps: "estimate" });
            cb({ bal: d.bal, rate: d.rate, t: ms(d.t), wt: ms(d.wt) });
          },
          () => cb(null)
        ),
      // 이자 정산: gain만큼 늘리고 이자 시계(t)를 그만큼만 앞으로 보내요
      claimInterest: (gain, tMs) => updateDoc(walletRef(), { bal: increment(gain), t: Timestamp.fromMillis(tMs), wt: serverTimestamp(), op: { kind: "interest" } }),
      buyProduct: (rate, price) => updateDoc(walletRef(), { rate, bal: increment(-price), t: serverTimestamp(), wt: serverTimestamp(), op: { kind: "buy" } }),

      /* ----- 대화 ----- */
      openChat: async (other) => {
        const uid = uidOf();
        const members = sorted(uid, other);
        const cid = members.join("_");
        try {
          await setDoc(doc(db, "chats", cid), { members, last: "", lastFrom: "", lastAt: Timestamp.fromMillis(0), reads: {} });
        } catch (err) {
          // 이미 있는 대화면 만들기가 막혀요. 읽을 수 있으면 있는 거예요
          await getDoc(doc(db, "chats", cid));
        }
        return cid;
      },
      watchChats: (cb) =>
        onSnapshot(query(collection(db, "chats"), where("members", "array-contains", uidOf()), limit(50)), (snap) =>
          cb(snap.docs.map((d) => chatPart({ id: d.id, ...d.data({ serverTimestamps: "estimate" }) })))
        ),
      watchMessages: (cid, cb, n = 40) =>
        onSnapshot(query(collection(db, "chats", cid, "messages"), orderBy("at", "desc"), limit(n)), (snap) => cb(snap.docs.map(msgPart).reverse(), snap.docs.length >= n)),
      loadOlder: async (cid, beforeMs, n = 30) => {
        const snap = await getDocs(query(collection(db, "chats", cid, "messages"), orderBy("at", "desc"), startAfter(Timestamp.fromMillis(beforeMs)), limit(n)));
        return { list: snap.docs.map(msgPart).reverse(), more: snap.docs.length >= n };
      },
      // 글·사진·그림·요청·투표: 메시지와 대화 목록 갱신을 한 묶음으로 써요
      send: async (cid, to, payload, preview, extra) => {
        const uid = uidOf();
        const mref = doc(collection(db, "chats", cid, "messages"));
        const b = writeBatch(db);
        b.set(mref, { ...payload, from: uid, to, at: serverTimestamp() });
        b.update(doc(db, "chats", cid), { last: String(preview).slice(0, 200), lastAt: serverTimestamp(), lastFrom: uid });
        if (extra) extra(b, mref);
        await b.commit();
        return mref.id;
      },
      // 선물: 같은 묶음에서 내 지갑이 보낸 만큼 줄어요
      sendGift: (cid, to, amount) =>
        window.cloud.send(cid, to, { type: "gift", amount, done: false }, `토큰 ${amount.toLocaleString("ko-KR")} 선물`, (b, mref) =>
          b.update(walletRef(), { bal: increment(-amount), wt: serverTimestamp(), op: { kind: "gift", cid, mid: mref.id } })
        ),
      // 받기: 선물 카드를 '받음'으로 바꾸는 묶음에서만 내 지갑이 늘어요
      claimGift: async (cid, mid, amount) => {
        const b = writeBatch(db);
        b.update(doc(db, "chats", cid, "messages", mid), { done: true });
        b.update(walletRef(), { bal: increment(amount), wt: serverTimestamp(), op: { kind: "claim", cid, mid } });
        await b.commit();
      },
      // 요청에 응하기: 요청 카드를 닫고 같은 묶음에서 선물을 보내요
      payRequest: async (cid, to, reqMid, amount) => {
        const uid = uidOf();
        const gref = doc(collection(db, "chats", cid, "messages"));
        const b = writeBatch(db);
        b.update(doc(db, "chats", cid, "messages", reqMid), { done: true });
        b.set(gref, { type: "gift", amount, done: false, from: uid, to, at: serverTimestamp() });
        b.update(doc(db, "chats", cid), { last: `토큰 ${amount.toLocaleString("ko-KR")} 선물`, lastAt: serverTimestamp(), lastFrom: uid });
        b.update(walletRef(), { bal: increment(-amount), wt: serverTimestamp(), op: { kind: "gift", cid, mid: gref.id } });
        await b.commit();
      },
      vote: (cid, mid, idx) => updateDoc(doc(db, "chats", cid, "messages", mid), { [`votes.${uidOf()}`]: idx }),
      markRead: (cid) => updateDoc(doc(db, "chats", cid), { [`reads.${uidOf()}`]: serverTimestamp() }).catch(() => {}),
    };
    return window.cloud;
  } catch (err) {
    console.warn("파이어베이스에 연결하지 못했어요:", err);
    window.cloudError = err?.code ? `${explain(err)} (${err.code})` : `연결하지 못했어요 (${err?.message || err})`;
    return null;
  }
})();
