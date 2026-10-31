/* 파이어베이스 연결의 원본 파일. `npm run build`(tests 폴더)가 이 파일을 app/firebase.js 한 개로 묶어요.
   - 로그인은 아이디·비밀번호(안 보이는 이메일로 바꿔서) 하나뿐이고 익명 로그인은 쓰지 않아요.
   - 서버 규칙(firestore.rules)이 진짜 지킴이고, 여기서는 규칙이 받아 주는 모양으로만 써요.
   - 읽기·쓰기를 아끼려고: 메시지는 최근 40개만 듣고, 사람 정보는 30분 캐시하고, 지갑 이자는 10분에 한 번만 정산해요.
   설정값이 비어 있거나 연결에 실패하면 window.cloud는 null이에요. */
import { firebaseConfig } from "./firebase-config.js";
import { initializeApp } from "firebase/app";
import {
  initializeAuth,
  connectAuthEmulator,
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
  connectFirestoreEmulator,
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
  getCountFromServer,
  documentId,
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
// 프로젝트에서 Firestore(데이터베이스)가 아직 꺼져 있거나 만들어지지 않았을 때 서버가 보내는 오류예요
const dbDown = (err) => /has not been used|is disabled|SERVICE_DISABLED|database \(default\) does not exist/i.test(String(err?.message || ""));
const report = (err) => {
  if (dbDown(err) && !window.cloudDown) {
    window.cloudDown = true;
    window.dispatchEvent(new CustomEvent("cloud-down"));
  }
};
const rethrow = (err) => {
  report(err);
  throw err;
};
const explain = (err) =>
  dbDown(err) ? "서버 데이터베이스(Firestore)가 꺼져 있어요. 관리자가 Firebase에서 켜야 해요" : MESSAGES[err?.code] || "처리하지 못했어요. 잠시 뒤 다시 해 주세요";
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

    // 시험용 묶음에서만 켜져요 (실제 묶음에서는 __EMU__가 false라 이 줄이 통째로 사라져요)
    if (__EMU__) {
      connectAuthEmulator(auth, "http://127.0.0.1:9099", { disableWarnings: true });
      connectFirestoreEmulator(db, "127.0.0.1", 8081);
    }

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
        report(err);
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

    let listCursor = null;
    const rank = { sent: null, at: 0, me: null, top: null, topAt: 0 };
    let ensured = "";
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
        if (ensured === uid) return; // 한 번 열 때마다 한 번만 확인해요
        const ref = doc(db, "profiles", uid);
        let exists = true;
        try {
          exists = (await getDoc(ref)).exists();
        } catch (err) {
          report(err);
          exists = false;
        }
        if (!exists) {
          // 옛 방식으로 만든 계정은 프로필이 없어요. 실명이 있으면 쓰고, 없으면 아이디를 이름으로 써요
          const okName = (n) => typeof n === "string" && (/^[가-힣]{2,6}$/.test(n) || /^[A-Za-z][A-Za-z ]{1,38}[A-Za-z]$/.test(n));
          let name = auth.currentUser.displayName || "";
          if (!okName(name)) {
            try {
              const s = await getDoc(doc(db, "users", uid));
              name = String(s.data()?.profile?.name || "");
            } catch (_) {
              name = "";
            }
          }
          await setDoc(ref, { id: myId(), name: okName(name) ? name : myId(), photo: "" });
        }
        ensured = uid;
      },

      /* ----- 내 설정 (프로필 글, 대화 배경) ----- */
      loadSettings: async () => {
        const s = await getDoc(doc(db, "users", uidOf())).catch((err) => {
          report(err);
          throw err;
        });
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
      // 모든 사람을 이름순으로 20명씩 보여줘요 (reset이면 처음부터, 아니면 이어서)
      listUsers: async (reset) => {
        if (reset) listCursor = null;
        const base = [collection(db, "profiles"), orderBy("name")];
        const snap = await getDocs(query(...base, ...(listCursor ? [startAfter(listCursor)] : []), limit(20))).catch(rethrow);
        if (snap.docs.length) listCursor = snap.docs[snap.docs.length - 1];
        const list = snap.docs
          .filter((d) => d.id !== uidOf())
          .map((d) => {
            writeCache(d.id, d.data());
            return { uid: d.id, ...d.data() };
          });
        return { list, more: snap.docs.length >= 20 };
      },
      /* ----- 랭킹: 내 지갑 잔액을 공개 순위표에 올려요 (서버 규칙이 지갑 잔액과 같은 값만 받아요) ----- */
      syncRank: async (bal) => {
        const uid = uidOf();
        if (rank.sent === bal) return "same";
        if (Date.now() - rank.at < 21000) return "soon"; // 방금 썼으면 쉬어요
        const ref = doc(db, "ranks", uid);
        if (rank.sent === null) {
          const cur = await getDoc(ref);
          rank.sent = cur.exists() ? cur.data().bal : -1;
          if (rank.sent === bal) return "same";
        }
        if (!rank.me) rank.me = (await getDoc(doc(db, "profiles", uid))).data();
        await setDoc(ref, { id: rank.me.id, name: rank.me.name, bal, at: serverTimestamp() });
        rank.sent = bal;
        rank.at = Date.now();
        return "sent";
      },
      // 가장 토큰이 많은 50명 (45초 동안은 다시 읽지 않아요)
      loadRanks: async (force) => {
        if (!force && rank.top && Date.now() - rank.topAt < 45000) return rank.top;
        const snap = await getDocs(query(collection(db, "ranks"), orderBy("bal", "desc"), limit(50))).catch(rethrow);
        rank.top = snap.docs.map((d) => ({ uid: d.id, id: d.data().id, name: d.data().name, bal: d.data().bal }));
        rank.topAt = Date.now();
        return rank.top;
      },
      // 나보다 많이 가진 사람 수 (50명까지만 세요)
      countAbove: async (bal) => (await getCountFromServer(query(collection(db, "ranks"), where("bal", ">", bal), limit(50)))).data().count,
      loadRanksOf: async (uids) => {
        const out = [];
        for (let i = 0; i < uids.length && i < 30; i += 10) {
          const snap = await getDocs(query(collection(db, "ranks"), where(documentId(), "in", uids.slice(i, i + 10)), limit(10)));
          for (const d of snap.docs) out.push({ uid: d.id, id: d.data().id, name: d.data().name, bal: d.data().bal });
        }
        return out;
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
          (err) => {
            report(err);
            cb(null);
          }
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
          cb(snap.docs.map((d) => chatPart({ id: d.id, ...d.data({ serverTimestamps: "estimate" }) }))), report
        ),
      watchMessages: (cid, cb, n = 40) =>
        onSnapshot(query(collection(db, "chats", cid, "messages"), orderBy("at", "desc"), limit(n)), (snap) => cb(snap.docs.map(msgPart).reverse(), snap.docs.length >= n), report),
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
