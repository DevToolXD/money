// Firestore 보안 규칙 시험: 공격하려는 쓰기가 실제로 막히는지, 정상 쓰기는 통과하는지 에뮬레이터에서 확인해요.
// 실행: cd tests && npm install && npm run rules
import { readFileSync } from "node:fs";
import { initializeTestEnvironment, assertSucceeds, assertFails } from "@firebase/rules-unit-testing";
import { doc, setDoc, getDoc, getDocs, updateDoc, collection, query, where, limit, orderBy, writeBatch, deleteDoc, serverTimestamp, Timestamp, increment, collectionGroup } from "firebase/firestore";

const env = await initializeTestEnvironment({ projectId: "demo-money", firestore: { rules: readFileSync(process.env.RULES_FILE || new URL("../firestore.rules", import.meta.url), "utf8") } });
const pw = (uid, id) => env.authenticatedContext(uid, { email: `${id}@money-sim.app`, firebase: { sign_in_provider: "password" } }).firestore();
const anon = () => env.authenticatedContext("anon1", { firebase: { sign_in_provider: "anonymous" } }).firestore();
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const JPEG = "data:image/jpeg;base64,/9j/4AAQSkZJRgABAQ==";
const EPOCH = Timestamp.fromMillis(0);

let pass = 0;
const fails = [];
async function t(name, fn) {
  await env.clearFirestore();
  try {
    await fn();
    pass++;
    console.log("  ok   ", name);
  } catch (e) {
    fails.push(name);
    console.log("  FAIL ", name, "\n        ", String(e.message || e).split("\n")[0]);
  }
}
const seed = async (fn) => env.withSecurityRulesDisabled(async (c) => fn(c.firestore()));
const seedBase = (db) =>
  Promise.all([
    setDoc(doc(db, "profiles/uA"), { id: "alice", name: "김철수", photo: "" }),
    setDoc(doc(db, "profiles/uB"), { id: "bob", name: "이영희", photo: "" }),
    setDoc(doc(db, "chats/uA_uB"), { members: ["uA", "uB"], last: "", lastAt: EPOCH, lastFrom: "", reads: {} }),
    setDoc(doc(db, "wallets/uA"), { bal: 1250000, rate: 0, t: Timestamp.fromMillis(Date.now() - 7200e3), wt: EPOCH, op: { kind: "init" } }),
    setDoc(doc(db, "wallets/uB"), { bal: 1250000, rate: 0, t: Timestamp.fromMillis(Date.now() - 7200e3), wt: EPOCH, op: { kind: "init" } }),
  ]);
const text = (from, to, s = "안녕") => ({ type: "text", text: s, from, to, at: serverTimestamp() });
const send = (db, from, to, payload, last = "m") => {
  const mref = doc(collection(db, "chats/uA_uB/messages"));
  const b = writeBatch(db);
  b.set(mref, payload ?? text(from, to));
  b.update(doc(db, "chats/uA_uB"), { last, lastAt: serverTimestamp(), lastFrom: from });
  return { b, mref };
};

console.log("\n[로그인 방식]");
await t("익명 계정은 프로필도 지갑도 대화도 못 읽어요", async () => {
  await seed(seedBase);
  const db = anon();
  await assertFails(getDoc(doc(db, "profiles/uA")));
  await assertFails(getDoc(doc(db, "wallets/uA")));
  await assertFails(getDoc(doc(db, "chats/uA_uB")));
  await assertFails(setDoc(doc(db, "profiles/anon1"), { id: "x", name: "홍길동", photo: "" }));
});
await t("로그인하지 않으면 아무것도 못 해요", async () => {
  await seed(seedBase);
  const db = env.unauthenticatedContext().firestore();
  await assertFails(getDoc(doc(db, "profiles/uA")));
  await assertFails(getDocs(collection(db, "profiles")));
});

console.log("\n[프로필]");
await t("내 실명 프로필을 만들 수 있어요", async () => {
  const db = pw("uA", "alice");
  await assertSucceeds(setDoc(doc(db, "profiles/uA"), { id: "alice", name: "김철수", photo: "" }));
});
await t("아이디가 내 이메일과 다르면 못 만들어요 (남의 아이디 가로채기)", async () => {
  const db = pw("uA", "alice");
  await assertFails(setDoc(doc(db, "profiles/uA"), { id: "bob", name: "김철수", photo: "" }));
});
await t("남의 uid 자리에는 못 만들어요", async () => {
  const db = pw("uA", "alice");
  await assertFails(setDoc(doc(db, "profiles/uB"), { id: "alice", name: "김철수", photo: "" }));
});
await t("별명·숫자·너무 긴 이름은 실명 규칙에서 막혀요", async () => {
  const db = pw("uA", "alice");
  for (const name of ["철수123", "x", "가나다라마바사", "<script>", ""]) await assertFails(setDoc(doc(db, "profiles/uA"), { id: "alice", name, photo: "" }));
});
await t("영문 이름은 허용돼요", async () => {
  const db = pw("uA", "alice");
  await assertSucceeds(setDoc(doc(db, "profiles/uA"), { id: "alice", name: "John Smith", photo: "" }));
});
await t("이름이 없던 옛 계정은 아이디를 이름으로 쓸 수 있어요 (아이디와 다른 별명은 여전히 막혀요)", async () => {
  const db = pw("uA", "alice");
  await assertSucceeds(setDoc(doc(db, "profiles/uA"), { id: "alice", name: "alice", photo: "" }));
  const db2 = pw("uB", "bob");
  await assertFails(setDoc(doc(db2, "profiles/uB"), { id: "bob", name: "alice9", photo: "" }));
});
await t("이름·아이디는 바꿀 수 없고 사진만 바꿀 수 있어요", async () => {
  await seed(seedBase);
  const db = pw("uA", "alice");
  await assertFails(updateDoc(doc(db, "profiles/uA"), { name: "박지민" }));
  await assertFails(updateDoc(doc(db, "profiles/uA"), { id: "evil" }));
  await assertSucceeds(updateDoc(doc(db, "profiles/uA"), { photo: JPEG }));
});
await t("사진 자리에 자바스크립트·큰 값·다른 모양은 못 넣어요", async () => {
  await seed(seedBase);
  const db = pw("uA", "alice");
  await assertFails(updateDoc(doc(db, "profiles/uA"), { photo: "javascript:alert(1)" }));
  await assertFails(updateDoc(doc(db, "profiles/uA"), { photo: 'data:image/jpeg;base64,AAA");background:url(x' }));
  await assertFails(updateDoc(doc(db, "profiles/uA"), { photo: "data:image/svg+xml;base64,AAAA" }));
  await assertFails(updateDoc(doc(db, "profiles/uA"), { photo: "data:image/jpeg;base64," + "A".repeat(45000) }));
});
await t("남의 프로필은 고칠 수 없어요", async () => {
  await seed(seedBase);
  await assertFails(updateDoc(doc(pw("uB", "bob"), "profiles/uA"), { photo: JPEG }));
});
await t("이름으로 찾기는 한 번에 20명까지만, 한도 없이 전부 읽기는 막혀요", async () => {
  await seed(seedBase);
  const db = pw("uB", "bob");
  await assertSucceeds(getDocs(query(collection(db, "profiles"), where("name", ">=", "김"), where("name", "<=", "김"), limit(20))));
  await assertFails(getDocs(query(collection(db, "profiles"), limit(21))));
  await assertSucceeds(getDocs(query(collection(db, "profiles"), orderBy("name"), limit(20))));
  await assertFails(getDocs(collection(db, "profiles")));
});

console.log("\n[내 설정]");
await t("내 설정만 읽고 쓰고, 정해진 모양만 받아요", async () => {
  const db = pw("uA", "alice");
  await assertSucceeds(setDoc(doc(db, "users/uA"), { profile: { name: "철수", bio: "안녕" }, walls: { c: "white" }, updatedAt: 1 }));
  await assertFails(setDoc(doc(db, "users/uA"), { profile: { name: "철수", bio: "안녕" }, bank: { bal: 99999999 } }));
  await assertFails(setDoc(doc(db, "users/uA"), { profile: { name: "x".repeat(31), bio: "" } }));
  await assertFails(getDoc(doc(pw("uB", "bob"), "users/uA")));
  await assertFails(setDoc(doc(pw("uB", "bob"), "users/uA"), { profile: { name: "해킹", bio: "" } }));
});

console.log("\n[지갑: 토큰은 서버가 지켜요]");
await t("지갑은 정확히 0 토큰으로 한 번만, 프로필이 있을 때만 만들어요", async () => {
  const db = pw("uA", "alice");
  const w = { bal: 0, rate: 0, t: serverTimestamp(), wt: serverTimestamp(), op: { kind: "init" } };
  await assertFails(setDoc(doc(db, "wallets/uA"), w)); // 프로필이 아직 없어요
  await seed((s) => setDoc(doc(s, "profiles/uA"), { id: "alice", name: "김철수", photo: "" }));
  await assertFails(setDoc(doc(db, "wallets/uA"), { ...w, bal: 1250000 }));
  await assertFails(setDoc(doc(db, "wallets/uA"), { ...w, rate: 50 }));
  await assertSucceeds(setDoc(doc(db, "wallets/uA"), w));
  await sleep(2100);
  await assertFails(setDoc(doc(db, "wallets/uA"), w)); // 다시 만들어 돈을 되돌릴 수 없어요
});
await t("남의 지갑은 읽지도 만들지도 못해요", async () => {
  await seed(seedBase);
  await assertFails(getDoc(doc(pw("uB", "bob"), "wallets/uA")));
});
await t("토큰 값을 마음대로 고칠 수 없어요", async () => {
  await seed(seedBase);
  const db = pw("uA", "alice");
  await assertFails(updateDoc(doc(db, "wallets/uA"), { bal: 999999999, wt: serverTimestamp(), op: { kind: "interest" } }));
  await assertFails(updateDoc(doc(db, "wallets/uA"), { bal: 999999999, wt: serverTimestamp(), op: { kind: "cheat" } }));
  await assertFails(updateDoc(doc(db, "wallets/uA"), { bal: increment(5000000), wt: serverTimestamp(), op: { kind: "buy" }, rate: 50 }));
  await assertFails(updateDoc(doc(db, "wallets/uA"), { extra: 1, wt: serverTimestamp(), op: { kind: "init" } }));
});
await t("이자 상품: 정해진 값만 내고 살 수 있어요", async () => {
  await seed(seedBase);
  const db = pw("uA", "alice");
  await assertFails(updateDoc(doc(db, "wallets/uA"), { rate: 50, bal: 1250000, t: serverTimestamp(), wt: serverTimestamp(), op: { kind: "buy" } })); // 공짜
  await assertFails(updateDoc(doc(db, "wallets/uA"), { rate: 50, bal: 1250000 - 20000, t: serverTimestamp(), wt: serverTimestamp(), op: { kind: "buy" } })); // 5% 값으로 50%
  await assertSucceeds(updateDoc(doc(db, "wallets/uA"), { rate: 30, bal: 1250000 - 503, t: serverTimestamp(), wt: serverTimestamp(), op: { kind: "buy" } }));
  await sleep(2100);
  await assertFails(updateDoc(doc(db, "wallets/uA"), { rate: 10, bal: 1250000 - 503 - 70, t: serverTimestamp(), wt: serverTimestamp(), op: { kind: "buy" } })); // 낮은 이율로 갈아타기
});
await t("이자: 흐른 시간만큼의 복리만 받을 수 있어요", async () => {
  await seed(async (s) => {
    await seedBase(s);
    await setDoc(doc(s, "wallets/uA"), { bal: 1250000, rate: 50, t: Timestamp.fromMillis(Date.now() - 7200e3), wt: EPOCH, op: { kind: "buy" } });
  });
  const db = pw("uA", "alice");
  const t0 = Date.now() - 7200e3;
  const to = (secs) => Timestamp.fromMillis(t0 + secs * 1000);
  // 1시간치 이자의 위쪽 한계는 약 57.9 토큰
  await assertFails(updateDoc(doc(db, "wallets/uA"), { bal: 1250000 + 5000, t: to(3600), wt: serverTimestamp(), op: { kind: "interest" } }));
  await assertFails(updateDoc(doc(db, "wallets/uA"), { bal: 1250000 + 59, t: to(3600), wt: serverTimestamp(), op: { kind: "interest" } }));
  await assertSucceeds(updateDoc(doc(db, "wallets/uA"), { bal: 1250000 + 57, t: to(3600), wt: serverTimestamp(), op: { kind: "interest" } }));
});
await t("이자: 시간을 거꾸로 돌리거나 미래로 보낼 수 없어요", async () => {
  await seed(async (s) => {
    await seedBase(s);
    await setDoc(doc(s, "wallets/uA"), { bal: 1250000, rate: 50, t: Timestamp.fromMillis(Date.now() - 7200e3), wt: EPOCH, op: { kind: "buy" } });
  });
  const db = pw("uA", "alice");
  await assertFails(updateDoc(doc(db, "wallets/uA"), { bal: 1250001, t: Timestamp.fromMillis(Date.now() + 3600e3), wt: serverTimestamp(), op: { kind: "interest" } })); // 미래
  await assertFails(updateDoc(doc(db, "wallets/uA"), { bal: 1250001, t: Timestamp.fromMillis(Date.now() - 9000e3), wt: serverTimestamp(), op: { kind: "interest" } })); // 과거로
});
await t("이자 상품이 없으면 이자를 못 받아요", async () => {
  await seed(seedBase);
  const db = pw("uA", "alice");
  await assertFails(updateDoc(doc(db, "wallets/uA"), { bal: 1250001, t: Timestamp.fromMillis(Date.now() - 3600e3), wt: serverTimestamp(), op: { kind: "interest" } }));
});
await t("지갑은 2초에 한 번만 쓸 수 있어요 (이자 쪼개기 반복 방지)", async () => {
  await seed(async (s) => {
    await seedBase(s);
    await setDoc(doc(s, "wallets/uA"), { bal: 1250000, rate: 50, t: Timestamp.fromMillis(Date.now() - 7200e3), wt: EPOCH, op: { kind: "buy" } });
  });
  const db = pw("uA", "alice");
  const t0 = Date.now() - 7200e3;
  await assertSucceeds(updateDoc(doc(db, "wallets/uA"), { bal: 1250010, t: Timestamp.fromMillis(t0 + 700e3), wt: serverTimestamp(), op: { kind: "interest" } }));
  await assertFails(updateDoc(doc(db, "wallets/uA"), { bal: 1250020, t: Timestamp.fromMillis(t0 + 1400e3), wt: serverTimestamp(), op: { kind: "interest" } }));
});

console.log("\n[선물과 토큰 받기]");
const gift = (from, to, amount) => ({ type: "gift", amount, from, to, at: serverTimestamp(), done: false });
await t("선물: 보낸 만큼 내 지갑이 줄 때만 보낼 수 있어요", async () => {
  await seed(seedBase);
  const db = pw("uA", "alice");
  const { b, mref } = send(db, "uA", "uB", gift("uA", "uB", 300), "토큰 300 선물");
  b.update(doc(db, "wallets/uA"), { bal: 1250000 - 300, wt: serverTimestamp(), op: { kind: "gift", cid: "uA_uB", mid: mref.id } });
  await assertSucceeds(b.commit());
});
await t("선물: 지갑을 덜 줄이거나(공짜 선물) 지갑 없이 보내면 막혀요", async () => {
  await seed(seedBase);
  const db = pw("uA", "alice");
  const a = send(db, "uA", "uB", gift("uA", "uB", 300), "x");
  a.b.update(doc(db, "wallets/uA"), { bal: 1250000 - 100, wt: serverTimestamp(), op: { kind: "gift", cid: "uA_uB", mid: a.mref.id } });
  await assertFails(a.b.commit());
  const c = send(db, "uA", "uB", gift("uA", "uB", 300), "x");
  await assertFails(c.b.commit());
});
await t("선물: 남의 지갑을 늘리거나 건드리는 묶음은 막혀요", async () => {
  await seed(seedBase);
  const db = pw("uA", "alice");
  const a = send(db, "uA", "uB", gift("uA", "uB", 300), "x");
  a.b.update(doc(db, "wallets/uB"), { bal: 1250000 + 300, wt: serverTimestamp(), op: { kind: "gift", cid: "uA_uB", mid: a.mref.id } });
  await assertFails(a.b.commit());
});
await t("선물: 잔액보다 많이 보낼 수 없어요", async () => {
  await seed(seedBase);
  const db = pw("uA", "alice");
  const a = send(db, "uA", "uB", gift("uA", "uB", 2000000), "x");
  a.b.update(doc(db, "wallets/uA"), { bal: 1250000 - 2000000, wt: serverTimestamp(), op: { kind: "gift", cid: "uA_uB", mid: a.mref.id } });
  await assertFails(a.b.commit());
});
await t("선물 받기: 받는 사람이 한 번만, 선물 금액만큼만 받아요", async () => {
  await seed(async (s) => {
    await seedBase(s);
    await setDoc(doc(s, "chats/uA_uB/messages/g1"), { type: "gift", amount: 300, from: "uA", to: "uB", at: Timestamp.now(), done: false });
  });
  const bob = pw("uB", "bob");
  const claim = (db, bal) => {
    const b = writeBatch(db);
    b.update(doc(db, "chats/uA_uB/messages/g1"), { done: true });
    b.update(doc(db, "wallets/uB"), { bal, wt: serverTimestamp(), op: { kind: "claim", cid: "uA_uB", mid: "g1" } });
    return b;
  };
  await assertFails(claim(bob, 1250000 + 5000).commit()); // 더 받기
  await assertFails(claim(pw("uA", "alice"), 1250000 + 300).commit()); // 보낸 사람이 자기 선물 받기
  await assertSucceeds(claim(bob, 1250000 + 300).commit());
  await sleep(2100);
  await assertFails(claim(bob, 1250000 + 600).commit()); // 두 번 받기
});
await t("선물 받기: 선물 메시지를 거치지 않고 지갑만 늘릴 수 없어요", async () => {
  await seed(async (s) => {
    await seedBase(s);
    await setDoc(doc(s, "chats/uA_uB/messages/g1"), { type: "gift", amount: 300, from: "uA", to: "uB", at: Timestamp.now(), done: false });
  });
  const bob = pw("uB", "bob");
  await assertFails(updateDoc(doc(bob, "wallets/uB"), { bal: 1250000 + 300, wt: serverTimestamp(), op: { kind: "claim", cid: "uA_uB", mid: "g1" } }));
});
await t("토큰 요청 카드는 받는 사람만 '보냈어요'로 바꿔요", async () => {
  await seed(async (s) => {
    await seedBase(s);
    await setDoc(doc(s, "chats/uA_uB/messages/r1"), { type: "request", amount: 200, from: "uA", to: "uB", at: Timestamp.now(), done: false });
  });
  await assertFails(updateDoc(doc(pw("uA", "alice"), "chats/uA_uB/messages/r1"), { done: true }));
  await assertSucceeds(updateDoc(doc(pw("uB", "bob"), "chats/uA_uB/messages/r1"), { done: true }));
});

console.log("\n[대화]");
await t("대화 만들기: 사전순 짝, 내가 들어 있고 둘 다 프로필이 있어야 해요", async () => {
  await seed((s) => Promise.all([setDoc(doc(s, "profiles/uA"), { id: "alice", name: "김철수", photo: "" }), setDoc(doc(s, "profiles/uB"), { id: "bob", name: "이영희", photo: "" })]));
  const db = pw("uA", "alice");
  const base = { last: "", lastFrom: "", lastAt: EPOCH, reads: {} };
  await assertFails(setDoc(doc(db, "chats/uB_uA"), { ...base, members: ["uB", "uA"] })); // 정렬 아님
  await assertFails(setDoc(doc(db, "chats/uA_uZ"), { ...base, members: ["uA", "uZ"] })); // 없는 사람
  await assertFails(setDoc(doc(db, "chats/uX_uY"), { ...base, members: ["uX", "uY"] })); // 나와 무관
  await assertFails(setDoc(doc(db, "chats/아무거나"), { ...base, members: ["uA", "uB"] })); // 아이디 불일치
  await assertFails(setDoc(doc(db, "chats/uA_uB"), { ...base, members: ["uA", "uB"], last: "광고 문구" }));
  await assertSucceeds(setDoc(doc(db, "chats/uA_uB"), { ...base, members: ["uA", "uB"] }));
});
await t("글을 보내면 대화 문서도 함께 갱신돼야 하고, 참가자만 보낼 수 있어요", async () => {
  await seed(seedBase);
  const a = send(pw("uA", "alice"), "uA", "uB", null, "안녕");
  await assertSucceeds(a.b.commit());
  // 대화 문서 갱신 없이 메시지만 (속도 제한 우회)
  const db = pw("uB", "bob");
  await assertFails(setDoc(doc(collection(db, "chats/uA_uB/messages")), text("uB", "uA")));
  // 제3자
  await seed((s) => setDoc(doc(s, "profiles/uC"), { id: "carol", name: "박지민", photo: "" }));
  const c = send(pw("uC", "carol"), "uC", "uA", null, "침입");
  await assertFails(c.b.commit());
});
await t("글 속도 제한: 0.6초 안에 연달아 보내면 막혀요", async () => {
  await seed(seedBase);
  const db = pw("uA", "alice");
  await assertSucceeds(send(db, "uA", "uB", null, "1").b.commit());
  await assertFails(send(db, "uA", "uB", null, "2").b.commit());
  await sleep(700);
  await assertSucceeds(send(db, "uA", "uB", null, "3").b.commit());
});
await t("남의 이름으로 보내거나(from 위조) 시각을 속일 수 없어요", async () => {
  await seed(seedBase);
  const db = pw("uA", "alice");
  await assertFails(send(db, "uA", "uB", text("uB", "uA")).b.commit());
  const m = send(db, "uA", "uB", { ...text("uA", "uB"), at: Timestamp.fromMillis(1) }).b;
  await assertFails(m.commit());
  const bad = send(db, "uA", "uB", { ...text("uA", "uB"), extra: "x" }).b;
  await assertFails(bad.commit());
});
await t("너무 긴 글, 빈 글, 이상한 종류, 잘못된 사진은 막혀요", async () => {
  await seed(seedBase);
  const db = pw("uA", "alice");
  const tries = [
    text("uA", "uB", "x".repeat(1001)),
    text("uA", "uB", ""),
    { type: "virus", from: "uA", to: "uB", at: serverTimestamp() },
    { type: "photo", img: "javascript:alert(1)", from: "uA", to: "uB", at: serverTimestamp() },
    { type: "photo", img: "data:image/jpeg;base64," + "A".repeat(150000), from: "uA", to: "uB", at: serverTimestamp() },
    { type: "photo", img: 'data:image/jpeg;base64,AAAA"onerror="alert(1)', from: "uA", to: "uB", at: serverTimestamp() },
  ];
  for (const p of tries) {
    await assertFails(send(db, "uA", "uB", p).b.commit());
    await sleep(650);
  }
  await assertSucceeds(send(db, "uA", "uB", { type: "photo", img: JPEG, from: "uA", to: "uB", at: serverTimestamp() }).b.commit());
});
await t("메시지는 지울 수도 고칠 수도 없어요 (받은 선물 금액 수정 포함)", async () => {
  await seed(async (s) => {
    await seedBase(s);
    await setDoc(doc(s, "chats/uA_uB/messages/m1"), { ...text("uA", "uB"), at: Timestamp.now() });
    await setDoc(doc(s, "chats/uA_uB/messages/g1"), { type: "gift", amount: 300, from: "uA", to: "uB", at: Timestamp.now(), done: false });
  });
  const a = pw("uA", "alice");
  await assertFails(updateDoc(doc(a, "chats/uA_uB/messages/m1"), { text: "수정" }));
  await assertFails(updateDoc(doc(a, "chats/uA_uB/messages/g1"), { amount: 99999999 }));
  await assertFails(updateDoc(doc(pw("uB", "bob"), "chats/uA_uB/messages/g1"), { amount: 99999999, done: true }));
});
await t("남의 대화는 읽을 수 없어요 / 메시지를 읽을 때는 60개까지만", async () => {
  await seed(async (s) => {
    await seedBase(s);
    await setDoc(doc(s, "profiles/uC"), { id: "carol", name: "박지민", photo: "" });
  });
  await assertFails(getDoc(doc(pw("uC", "carol"), "chats/uA_uB")));
  await assertFails(getDocs(query(collection(pw("uC", "carol"), "chats/uA_uB/messages"), limit(10))));
  const a = pw("uA", "alice");
  await assertSucceeds(getDocs(query(collection(a, "chats/uA_uB/messages"), orderBy("at", "desc"), limit(40))));
  await assertFails(getDocs(collection(a, "chats/uA_uB/messages")));
  await assertSucceeds(getDocs(query(collection(a, "chats"), where("members", "array-contains", "uA"), limit(50))));
});
await t("읽음 표시는 내 칸만", async () => {
  await seed(seedBase);
  await assertSucceeds(updateDoc(doc(pw("uA", "alice"), "chats/uA_uB"), { "reads.uA": serverTimestamp() }));
  await assertFails(updateDoc(doc(pw("uA", "alice"), "chats/uA_uB"), { "reads.uB": serverTimestamp() }));
  await assertFails(updateDoc(doc(pw("uA", "alice"), "chats/uA_uB"), { members: ["uA", "uZ"] }));
});

console.log("\n[투표]");
await t("투표: 내 표만, 선택지 안에서만 (남의 표 바꾸기·범위 밖·글 고치기 막힘)", async () => {
  await seed(async (s) => {
    await seedBase(s);
    await setDoc(doc(s, "chats/uA_uB/messages/p1"), { type: "poll", from: "uA", to: "uB", at: Timestamp.now(), poll: { q: "점심?", options: [{ t: "김밥", img: "" }, { t: "라면", img: "" }] }, votes: { uA: 0 } });
  });
  const b = pw("uB", "bob");
  await assertSucceeds(updateDoc(doc(b, "chats/uA_uB/messages/p1"), { "votes.uB": 1 }));
  await assertFails(updateDoc(doc(b, "chats/uA_uB/messages/p1"), { "votes.uA": 1 })); // 남의 표
  await assertFails(updateDoc(doc(b, "chats/uA_uB/messages/p1"), { "votes.uB": 5 })); // 범위 밖
  await assertFails(updateDoc(doc(b, "chats/uA_uB/messages/p1"), { "votes.uB": -1 }));
  await assertFails(updateDoc(doc(b, "chats/uA_uB/messages/p1"), { poll: { q: "바꿈", options: [{ t: "a", img: "" }, { t: "b", img: "" }] } }));
});
await t("투표 만들기: 선택지는 2~4개, 글이나 사진이 있어야 해요", async () => {
  await seed(seedBase);
  const db = pw("uA", "alice");
  const poll = (options) => ({ type: "poll", from: "uA", to: "uB", at: serverTimestamp(), poll: { q: "뭐 먹을까?", options }, votes: {} });
  await assertFails(send(db, "uA", "uB", poll([{ t: "하나", img: "" }])).b.commit());
  await sleep(650);
  await assertFails(send(db, "uA", "uB", poll([{ t: "", img: "" }, { t: "둘", img: "" }])).b.commit());
  await sleep(650);
  await assertSucceeds(send(db, "uA", "uB", poll([{ t: "김밥", img: "" }, { t: "", img: JPEG }])).b.commit());
});

console.log("\n[랭킹]");
await t("랭킹: 내 지갑 잔액과 같은 값만, 남의 것·부풀린 값·다른 이름은 막혀요", async () => {
  await seed(seedBase);
  const a = pw("uA", "alice");
  const rk = (n) => doc(a, "ranks/" + n);
  await assertSucceeds(setDoc(rk("uA"), { id: "alice", name: "김철수", bal: 1250000, at: serverTimestamp() }));
  await assertFails(setDoc(rk("uB"), { id: "bob", name: "이영희", bal: 1250000, at: serverTimestamp() })); // 남의 것
  await seed((d) => setDoc(doc(d, "ranks/uA"), { id: "alice", name: "김철수", bal: 1250000, at: EPOCH }));
  await assertFails(setDoc(rk("uA"), { id: "alice", name: "김철수", bal: 99999999, at: serverTimestamp() })); // 부풀림
  await assertFails(setDoc(rk("uA"), { id: "alice", name: "대통령", bal: 1250000, at: serverTimestamp() })); // 다른 이름
  await assertFails(setDoc(rk("uA"), { id: "bob", name: "김철수", bal: 1250000, at: serverTimestamp() })); // 다른 아이디
  await assertFails(setDoc(rk("uA"), { id: "alice", name: "김철수", bal: 1250000, at: Timestamp.fromMillis(1) })); // 시간 조작
  await assertFails(setDoc(rk("uA"), { id: "alice", name: "김철수", bal: 1250000, extra: 1, at: serverTimestamp() }));
  await assertSucceeds(setDoc(rk("uA"), { id: "alice", name: "김철수", bal: 1250000, at: serverTimestamp() })); // 지난 기록이 오래됨 → 갱신 가능
});
await t("랭킹: 너무 자주 갱신 못 하고, 지갑 값이 바뀌면 그 값으로 갱신돼요, 목록은 50명까지", async () => {
  await seed(seedBase);
  await seed((d) => setDoc(doc(d, "ranks/uA"), { id: "alice", name: "김철수", bal: 1250000, at: Timestamp.fromMillis(Date.now() - 5000) }));
  const a = pw("uA", "alice");
  await assertFails(setDoc(doc(a, "ranks/uA"), { id: "alice", name: "김철수", bal: 1250000, at: serverTimestamp() })); // 5초 전에 썼음
  await assertSucceeds(getDocs(query(collection(a, "ranks"), orderBy("bal", "desc"), limit(50))));
  await assertFails(getDocs(query(collection(a, "ranks"), orderBy("bal", "desc"), limit(51))));
  await assertFails(getDocs(collection(a, "ranks")));
  await assertFails(deleteDoc(doc(a, "ranks/uA")));
  await assertFails(getDoc(doc(anon(), "ranks/uA")));
});

console.log("\n[회사]");
const foundBatch = (db, uid, { name = "내 회사", logo = "", cost = 10, kind = "food", open = 0, staff = 0, cid = "co" + Math.random().toString(36).slice(2, 9), extra = {} } = {}) => {
  const b = writeBatch(db);
  b.set(doc(db, "companies/" + cid), { name, logo, kind, products: [], open, staff, owner: uid, at: serverTimestamp(), ...extra });
  b.update(doc(db, "wallets/" + uid), { bal: increment(-cost), wt: serverTimestamp(), op: { kind: "found", cid } });
  return b;
};
await t("창업: 회사 문서와 정확히 10 토큰이 빠지는 지갑 쓰기가 한 묶음일 때만 돼요", async () => {
  await seed(async (d) => { await seedBase(d); await setDoc(doc(d, "wallets/uA"), { bal: 100, rate: 0, t: Timestamp.fromMillis(Date.now() - 7200e3), wt: EPOCH, op: { kind: "init" } }); });
  const a = pw("uA", "alice");
  await assertFails(setDoc(doc(a, "companies/uA"), { name: "공짜", logo: "", products: [], at: serverTimestamp() })); // 돈 안 내고
  await assertFails(foundBatch(a, "uA", { cost: 9 }).commit());
  await assertFails(foundBatch(a, "uA", { cost: 11 }).commit());
  await assertFails(foundBatch(a, "uA", { cost: 0 }).commit());
  await assertFails(updateDoc(doc(a, "wallets/uA"), { bal: increment(-10), wt: serverTimestamp(), op: { kind: "found" } })); // 회사 없이 돈만
  await assertFails(foundBatch(pw("uB", "bob"), "uA").commit()); // 남의 회사
  await assertSucceeds(foundBatch(a, "uA", { cid: "uA" }).commit());
  await sleep(2100);
  await assertFails(foundBatch(a, "uA", { cid: "uA" }).commit()); // 같은 번호의 회사는 또 못 만들어요
  await sleep(2100);
  await assertSucceeds(foundBatch(a, "uA").commit()); // 회사는 여러 개 만들 수 있어요 (10 토큰 더)
});
await t("회사 이름·마크·여분의 값은 검사해요", async () => {
  await seed(async (d) => { await seedBase(d); await setDoc(doc(d, "wallets/uA"), { bal: 100, rate: 0, t: Timestamp.fromMillis(Date.now() - 7200e3), wt: EPOCH, op: { kind: "init" } }); });
  const a = pw("uA", "alice");
  await assertFails(foundBatch(a, "uA", { name: "<b>해킹</b>" }).commit());
  await assertFails(foundBatch(a, "uA", { name: "" }).commit());
  await assertFails(foundBatch(a, "uA", { name: "가".repeat(21) }).commit());
  await assertFails(foundBatch(a, "uA", { logo: "https://evil.example/x.png" }).commit());
  await assertFails(foundBatch(a, "uA", { extra: { owner: "uB" } }).commit());
  await assertFails(foundBatch(a, "uA", { kind: "bank" }).commit()); // 모르는 회사 종류
  await assertFails(foundBatch(a, "uA", { open: 1 }).commit()); // 처음부터 모집 중일 수 없어요
  await assertFails(foundBatch(a, "uA", { staff: 1 }).commit()); // 처음부터 직원이 있을 수 없어요
  await assertFails(foundBatch(a, "uA", { extra: { cap: 999 } }).commit()); // 시가총액은 따로 적지 않아요
  await assertSucceeds(foundBatch(a, "uA", { name: "Good 회사 1", logo: JPEG, kind: "ai" }).commit());
});
console.log("\n[사업 상품]");
const coSeed = (over = {}) => seed(async (d) => { await seedBase(d); await setDoc(doc(d, "companies/uA"), { name: "내 회사", logo: "", kind: "food", products: [], open: 0, staff: 0, at: EPOCH, ...over }); });
const pid = () => "p" + Math.random().toString(36).slice(2, 10);
const pdoc = (over = {}) => ({ owner: "uA", cat: "food", group: "water", item: "water", name: "생수 500ml", shape: 1, cap: 3, capColor: 5, color: -1, costCents: 500, priceCents: 1000, paused: false, active: true, at: serverTimestamp(), ...over });
await t("상품은 회사가 있고 내 회사일 때만 만들 수 있어요", async () => {
  await seed(seedBase);
  await assertFails(setDoc(doc(pw("uC", "carol"), "companies/uC/products/" + pid()), pdoc({ owner: "uC" }))); // 회사가 없어요
  await coSeed();
  await assertSucceeds(setDoc(doc(pw("uA", "alice"), "companies/uA/products/" + pid()), pdoc()));
  await assertFails(setDoc(doc(pw("uA", "alice"), "companies/uA/products/" + pid()), pdoc({ owner: "uB" }))); // 남의 이름으로
  await assertFails(setDoc(doc(pw("uB", "bob"), "companies/uA/products/" + pid()), pdoc({ owner: "uB" }))); // 남의 회사에
});
await t("상품 종류·모양·색·뚜껑은 정해진 범위와 짝만 받아요", async () => {
  await coSeed();
  const a = pw("uA", "alice");
  const ok = (over) => assertSucceeds(setDoc(doc(a, "companies/uA/products/" + pid()), pdoc(over)));
  const bad = (over) => assertFails(setDoc(doc(a, "companies/uA/products/" + pid()), pdoc(over)));
  await bad({ cat: "pc" });
  await bad({ item: "bomb", group: "water" });
  await bad({ item: "jelly" }); // 물 종류(group)와 안 맞아요
  await bad({ group: "snack", item: "water" });
  await bad({ shape: 50 });
  await bad({ shape: -1 });
  await bad({ color: 50 });
  await bad({ color: -2 });
  await bad({ cap: 50 });
  await bad({ item: "jelly", group: "snack", cap: 3, capColor: 5 }); // 뚜껑은 물에만
  await bad({ item: "coffee", group: "energy", content: 9 });
  await ok({ color: -1 });
  await ok({ shape: 49, cap: 0, capColor: -1 });
  await assertSucceeds(setDoc(doc(a, "companies/uA/products/" + pid()), { owner: "uA", cat: "food", group: "energy", item: "coffee", name: "커피 · 모카", shape: 3, color: 7, content: 5, costCents: 40, priceCents: 80, paused: false, active: true, at: serverTimestamp() }));
});
await t("이름·가격·원가·시간은 정해진 값만 받아요", async () => {
  await coSeed();
  const a = pw("uA", "alice");
  const bad = (over) => assertFails(setDoc(doc(a, "companies/uA/products/" + pid()), pdoc(over)));
  await bad({ name: "<b>x</b>" });
  await bad({ name: "" });
  await bad({ name: "가".repeat(31) });
  await bad({ priceCents: 0 });
  await bad({ costCents: 0 });
  await bad({ costCents: 0.5 }); // 센트는 정수로만 (0.5 토큰은 50으로 보내요)
  await bad({ priceCents: "10" });
  await bad({ priceCents: 1.5 });
  await bad({ priceCents: 100000000 });
  await bad({ costCents: 1000000 });
  await bad({ cost: 5, price: 10 }); // 예전 토큰 단위 필드는 더 못 써요
  await bad({ paused: true }); // 만들 때는 멈춘 상태가 아니에요
  await bad({ paused: "no" });
  await bad({ active: false });
  await bad({ at: Timestamp.fromMillis(1) });
  await bad({ extra: 1 });
  await assertSucceeds(setDoc(doc(a, "companies/uA/products/" + pid()), pdoc({ name: "가".repeat(30), priceCents: 99999999, costCents: 999999 })));
  await assertSucceeds(setDoc(doc(a, "companies/uA/products/" + pid()), pdoc({ costCents: 50, priceCents: 1 }))); // 원가 0.5 토큰, 가격 0.01 토큰
});
await t("이름 바꾸기·판매 멈춤·판매 종료만 하고, 값을 바꾸거나 지우지는 못해요", async () => {
  await coSeed();
  const id = pid();
  await seed((d) => setDoc(doc(d, "companies/uA/products/" + id), pdoc({ at: EPOCH })));
  const ref = doc(pw("uA", "alice"), "companies/uA/products/" + id);
  const other = doc(pw("uB", "bob"), "companies/uA/products/" + id);
  // 이름 바꾸기: 만들 때와 같은 글자 규칙
  await assertFails(updateDoc(ref, { name: "<b>x</b>" }));
  await assertFails(updateDoc(ref, { name: "" }));
  await assertFails(updateDoc(ref, { name: "가".repeat(31) }));
  await assertFails(updateDoc(ref, { name: "생수 500ml" })); // 그대로면 바꾼 게 아니에요
  await assertSucceeds(updateDoc(ref, { name: "새 이름" }));
  // 멈춤·다시 시작: paused만 바꿔요
  await assertFails(updateDoc(ref, { paused: "yes" }));
  await assertFails(updateDoc(ref, { paused: false })); // 이미 멈춘 게 아니에요
  await assertFails(updateDoc(ref, { paused: true, name: "같이" })); // 두 가지를 한 번에는 안 돼요
  await assertFails(updateDoc(other, { paused: true })); // 남의 상품은 못 멈춰요
  await assertSucceeds(updateDoc(ref, { paused: true }));
  await assertFails(updateDoc(ref, { paused: true })); // 이미 멈춘 상태예요
  await assertSucceeds(updateDoc(ref, { paused: false }));
  // 가격·원가·모양은 바꿀 수 없어요
  await assertFails(updateDoc(ref, { priceCents: 1 }));
  await assertFails(updateDoc(ref, { costCents: 1 }));
  await assertFails(updateDoc(ref, { shape: 2 }));
  await assertFails(updateDoc(other, { active: false }));
  await assertFails(deleteDoc(ref));
  // 판매 종료: active를 false로만 (되돌릴 수 없어요)
  await assertSucceeds(updateDoc(ref, { active: false }));
  await assertFails(updateDoc(ref, { name: "다시" }));
  await assertFails(updateDoc(ref, { paused: true }));
  await assertFails(updateDoc(ref, { active: true }));
});
await t("예전 토큰 단위 상품도 멈추거나 끌 수 있어요", async () => {
  await coSeed();
  const id = pid();
  await seed((d) => setDoc(doc(d, "companies/uA/products/" + id), { owner: "uA", cat: "food", group: "water", item: "water", name: "옛 생수", shape: 1, cap: 3, capColor: 5, color: -1, cost: 5, price: 10, active: true, at: EPOCH }));
  const ref = doc(pw("uA", "alice"), "companies/uA/products/" + id);
  await assertSucceeds(updateDoc(ref, { paused: true }));
  await assertSucceeds(updateDoc(ref, { active: false }));
});
await t("상품은 로그인한 사람이 보고, 목록은 50개까지 읽어요", async () => {
  await coSeed();
  await assertSucceeds(getDoc(doc(pw("uB", "bob"), "companies/uA/products/" + pid())));
  await assertFails(getDoc(doc(anon(), "companies/uA/products/x")));
  await assertSucceeds(getDocs(query(collection(pw("uB", "bob"), "companies/uA/products"), where("active", "==", true), limit(50))));
  await assertFails(getDocs(query(collection(pw("uB", "bob"), "companies/uA/products"), where("active", "==", true), limit(51))));
  await assertFails(getDocs(collection(pw("uB", "bob"), "companies/uA/products")));
});
await t("회사 정보와 예전 상품 목록은 더 이상 바꿀 수 없어요", async () => {
  await coSeed();
  await assertFails(updateDoc(doc(pw("uA", "alice"), "companies/uA"), { products: ["water"] }));
  await assertFails(updateDoc(doc(pw("uA", "alice"), "companies/uA"), { name: "바뀐 이름" }));
});

console.log("\n[취업]");
const coRef = (db, uid = "uA") => doc(db, "companies/" + uid);
const jobFields = (kind = "food", cid = "uA") => ({ cid, kind, at: serverTimestamp() });
const joinBatch = (db, uid, { cid = "uA", kind = "food", dOpen = -1, dStaff = 1 } = {}) => {
  const b = writeBatch(db);
  b.set(doc(db, "jobs/" + uid), jobFields(kind, cid));
  b.update(coRef(db, cid), { open: increment(dOpen), staff: increment(dStaff) });
  return b;
};
const quitBatch = (db, uid, { cid = "uA", dStaff = -1 } = {}) => {
  const b = writeBatch(db);
  b.delete(doc(db, "jobs/" + uid));
  b.update(coRef(db, cid), { staff: increment(dStaff) });
  return b;
};
const opened = async (db, cid = "uA") => {
  const s = await getDoc(coRef(db, cid));
  return [s.data().open ?? 0, s.data().staff ?? 0];
};
const same = (a, b) => { if (a !== b) throw new Error(`기대 ${b}, 실제 ${a}`); };
await t("사장은 채용 공고 숫자를 정한 수로 맞춰요: 0명 이상이면 몇 명이든, 줄이기도 돼요", async () => {
  await coSeed({ open: 0 });
  const alice = pw("uA", "alice");
  await assertSucceeds(updateDoc(coRef(alice), { open: 100 })); // 100명으로 맞추기
  await assertSucceeds(updateDoc(coRef(alice), { open: 90 })); // 90명으로 줄이기
  await assertFails(updateDoc(coRef(alice), { open: -1 })); // 음수는 안 돼요
  await assertFails(updateDoc(coRef(alice), { open: "5" })); // 숫자가 아니면 안 돼요
  await assertFails(updateDoc(coRef(alice), { open: 90, staff: 5 })); // 직원 수는 사장이 못 바꿔요
  await assertFails(updateDoc(coRef(alice), { open: 91, name: "바뀐 이름" }));
  await assertFails(updateDoc(coRef(pw("uB", "bob")), { open: 1 })); // 남의 회사 모집은 못 맞춰요
});
await t("입사: 일자리 기록과 모집·직원 수가 같은 묶음에서 함께 바뀔 때만 돼요", async () => {
  await coSeed({ open: 2 });
  const bob = pw("uB", "bob");
  await assertFails(setDoc(doc(bob, "jobs/uB"), jobFields())); // 기록만 만들어요
  await assertFails(updateDoc(coRef(bob), { open: increment(-1), staff: increment(1) })); // 기록 없이 인원만 바꿔요
  await assertFails(joinBatch(bob, "uB", { kind: "ai" }).commit()); // 회사 종류가 달라요
  await assertFails(joinBatch(bob, "uB", { dStaff: 2 }).commit()); // 직원을 두 명 늘려요
  await assertFails(joinBatch(bob, "uB", { dOpen: -2 }).commit()); // 모집을 두 명 줄여요
  await assertFails(joinBatch(pw("uA", "alice"), "uA").commit()); // 사장이 자기 회사에
  await assertSucceeds(joinBatch(bob, "uB").commit());
  const [o, s] = await opened(bob);
  same(o, 1);
  same(s, 1);
  await assertFails(joinBatch(bob, "uB").commit()); // 두 번 들어갈 수는 없어요
});
await t("입사 지원: 모집 0명, 이미 다니는 곳이 있음. 사장도 다른 회사에는 다닐 수 있어요", async () => {
  await coSeed({ open: 0 }); // 앨리스 회사 (모집 0명)
  await seed((d) => setDoc(doc(d, "companies/uC"), { name: "캐럴 회사", logo: "", kind: "food", products: [], open: 1, staff: 0, at: EPOCH })); // 캐럴 회사 (모집 1명)
  await assertFails(joinBatch(pw("uB", "bob"), "uB", { cid: "uA" }).commit()); // 모집 0명
  await seed((d) => updateDoc(doc(d, "companies/uA"), { open: 1 }));
  await assertSucceeds(joinBatch(pw("uA", "alice"), "uA", { cid: "uC" }).commit()); // 사장도 다른 회사에 다닐 수 있어요
  await assertSucceeds(joinBatch(pw("uB", "bob"), "uB", { cid: "uA" }).commit());
  await seed((d) => updateDoc(doc(d, "companies/uC"), { open: 1 }));
  await assertFails(joinBatch(pw("uB", "bob"), "uB", { cid: "uC" }).commit()); // 이미 다니는 곳이 있어요
});
await t("퇴사: 일자리 기록을 지우면서 직원 수가 1명 줄 때만 돼요", async () => {
  await coSeed({ open: 2 });
  const bob = pw("uB", "bob");
  await assertSucceeds(joinBatch(bob, "uB").commit());
  await assertFails(deleteDoc(doc(bob, "jobs/uB"))); // 기록만 지워요
  await assertFails(updateDoc(coRef(bob), { staff: increment(-1) })); // 직원 수만 줄여요
  await assertFails(quitBatch(bob, "uB", { dStaff: -2 }).commit()); // 두 명 줄여요
  await assertFails(quitBatch(pw("uC", "carol"), "uB").commit()); // 남의 기록을 지워요
  await assertSucceeds(quitBatch(bob, "uB").commit());
  const [o, s] = await opened(bob);
  same(o, 1);
  same(s, 0);
});
await t("퇴사: 직원 수가 0명인데 기록만 남아 있으면 줄일 수 없어요", async () => {
  await coSeed({ open: 2 });
  await seed((d) => setDoc(doc(d, "jobs/uB"), jobFields()));
  await assertFails(quitBatch(pw("uB", "bob"), "uB").commit());
});
await t("일자리 기록은 나만 보고, 고칠 수는 없어요", async () => {
  await coSeed({ open: 2 });
  await assertSucceeds(joinBatch(pw("uB", "bob"), "uB").commit());
  await assertSucceeds(getDoc(doc(pw("uB", "bob"), "jobs/uB")));
  await assertFails(getDoc(doc(pw("uA", "alice"), "jobs/uB")));
  await assertFails(getDoc(doc(anon(), "jobs/uB")));
  await assertFails(updateDoc(doc(pw("uB", "bob"), "jobs/uB"), { kind: "ai" }));
});
await t("회사 목록은 최근 50곳, 이름 찾기는 20곳까지만 읽어요", async () => {
  await coSeed();
  const bob = pw("uB", "bob");
  await assertSucceeds(getDocs(query(collection(bob, "companies"), orderBy("at", "desc"), limit(50))));
  await assertFails(getDocs(query(collection(bob, "companies"), orderBy("at", "desc"), limit(51))));
  await assertSucceeds(getDocs(query(collection(bob, "companies"), orderBy("name"), where("name", ">=", "내"), where("name", "<=", "내"), limit(20))));
  await assertFails(getDocs(collection(bob, "companies")));
  await assertFails(getDocs(query(collection(anon(), "companies"), orderBy("at", "desc"), limit(10))));
});
await t("예전에 만든 회사(종류·모집 값이 없어요)도 모집하고 입사를 받아요", async () => {
  await seed((d) => setDoc(doc(d, "companies/uA"), { name: "옛 회사", logo: "", products: [], at: EPOCH }));
  await assertSucceeds(updateDoc(coRef(pw("uA", "alice")), { open: increment(2) }));
  await assertSucceeds(joinBatch(pw("uB", "bob"), "uB", { kind: "food" }).commit());
  const [o, s] = await opened(pw("uB", "bob"));
  same(o, 1);
  same(s, 1);
});

console.log("\n[여러 회사]");
await t("회사를 여러 개 만들고, 회사마다 사장·상품·모집·업종이 따로 있어요", async () => {
  await seed(async (d) => { await seedBase(d); await setDoc(doc(d, "wallets/uA"), { bal: 100, rate: 0, t: Timestamp.fromMillis(Date.now() - 7200e3), wt: EPOCH, op: { kind: "init" } }); });
  const a = pw("uA", "alice");
  const bob = pw("uB", "bob");
  await assertSucceeds(foundBatch(a, "uA", { cid: "c1" }).commit());
  await sleep(2100);
  await assertSucceeds(foundBatch(a, "uA", { cid: "c2", kind: "pc" }).commit());
  await sleep(2100);
  await assertFails(foundBatch(a, "uA", { cid: "c2" }).commit()); // 같은 번호의 회사는 또 못 만들어요
  await assertSucceeds(updateDoc(doc(a, "companies/c2"), { open: increment(3) })); // 사장 모집
  await assertSucceeds(updateDoc(doc(a, "companies/c2"), { kind: "ai" })); // 업종 바꾸기
  await assertSucceeds(updateDoc(doc(a, "companies/c2"), { kind: "nuclear" })); // 원자력 발전도 업종이에요
  await assertSucceeds(updateDoc(doc(a, "companies/c2"), { kind: "ai" }));
  await assertFails(updateDoc(doc(a, "companies/c2"), { kind: "bank" })); // 모르는 업종은 안 돼요
  await assertFails(updateDoc(doc(bob, "companies/c2"), { kind: "food" })); // 남의 회사 업종은 못 바꿔요
  await assertFails(updateDoc(doc(a, "companies/c2"), { name: "바뀐 이름" })); // 이름은 못 바꿔요
  await assertSucceeds(setDoc(doc(a, "companies/c2/products/" + pid()), pdoc({ owner: "uA" }))); // 내 회사에 상품
  await assertFails(setDoc(doc(bob, "companies/c2/products/" + pid()), pdoc({ owner: "uB" }))); // 남의 회사에 상품은 못 만들어요
  await assertFails(joinBatch(a, "uA", { cid: "c2", kind: "ai" }).commit()); // 내 회사에는 지원할 수 없어요
  await assertSucceeds(joinBatch(bob, "uB", { cid: "c2", kind: "ai" }).commit()); // 남의 회사에는 지원할 수 있어요
  const [o, s] = await opened(bob, "c2");
  same(o, 2);
  same(s, 1);
});

console.log("\n[연구실]");
const labRef = (db, cid = "uA") => doc(db, "companies/" + cid + "/lab/main");
const labFields = (over = {}) => ({ nperf: 100, elec: 0, gen: null, cap: 0, spd: 0, pow: 0, rel: null, aiData: 0, aiScore: -1, cash: 0, price: 0, ...over });
const jobSeed = (db) => setDoc(doc(db, "jobs/uB"), { cid: "uA", kind: "nuclear", at: EPOCH });
const walletSeed = (uid, bal) => seed((d) => setDoc(doc(d, "wallets/" + uid), { bal, rate: 0, t: EPOCH, wt: EPOCH, op: { kind: "init" } }));
const labSeed = (over = {}) => seed((d) => setDoc(doc(d, "companies/uA/lab/main"), labFields(over)));
// 지갑 쪽 묶음: 넣기(deposit)는 지갑 토큰이 빠지고, 빼기(withdraw)는 지갑에 토큰이 들어와요
const tokenBatch = (db, uid, kind, cid, amt) => {
  const b = writeBatch(db);
  b.update(doc(db, "wallets/" + uid), { bal: increment(kind === "deposit" ? -amt : amt), wt: serverTimestamp(), op: { kind, cid, amt } });
  return b;
};
await t("연구실: 사장과 그 회사 직원만 숫자를 올려요 (성능·용량·데이터는 줄일 수 없어요)", async () => {
  await coSeed();
  const alice = pw("uA", "alice"), bob = pw("uB", "bob"), carol = pw("uC", "carol");
  await seed(jobSeed);
  await assertSucceeds(setDoc(labRef(alice), labFields()));
  await assertSucceeds(updateDoc(labRef(bob), { nperf: 120 })); // 직원: 핵연료 배합
  await assertSucceeds(updateDoc(labRef(bob), { cap: 6, spd: 3, pow: 2 })); // 직원: 컴퓨터 검사
  await assertSucceeds(updateDoc(labRef(bob), { aiData: 50 })); // 직원: 데이터 받기
  await assertFails(updateDoc(labRef(carol), { cap: 9 })); // 회사 밖 사람은 못 해요
  await assertFails(updateDoc(labRef(bob), { nperf: 110 })); // 성능은 줄일 수 없어요
  await assertFails(updateDoc(labRef(bob), { cap: 5 })); // 처리 용량도 줄일 수 없어요
  await assertFails(updateDoc(labRef(bob), { aiData: 10 })); // 데이터도 줄일 수 없어요
});
await t("연구실: 출시 값은 사장만, 출시 숫자는 지금 숫자와 같아야 해요", async () => {
  await coSeed();
  const alice = pw("uA", "alice"), bob = pw("uB", "bob");
  await seed(jobSeed);
  await labSeed({ cap: 6, spd: 4, pow: 2, aiData: 50, aiDev: 5 }); // 개발 점수 5 (학습 결과)
  await assertFails(updateDoc(labRef(bob), { rel: { cap: 6, spd: 4, pow: 2 } })); // 직원은 출시 못 해요
  await assertFails(updateDoc(labRef(alice), { rel: { cap: 7, spd: 4, pow: 2 } })); // 지금 숫자와 다르면 출시 못 해요
  await assertSucceeds(updateDoc(labRef(alice), { rel: { cap: 6, spd: 4, pow: 2 } })); // 출시
  await assertSucceeds(updateDoc(labRef(bob), { cap: 7 })); // 출시한 뒤에도 직원은 검사를 계속해요
  await assertFails(updateDoc(labRef(bob), { rel: null })); // 직원은 출시 값을 지울 수 없어요
  await assertFails(updateDoc(labRef(alice), { aiScore: 6 })); // 출시 점수는 지금 개발 점수(5)와 같아야 해요
  await assertSucceeds(updateDoc(labRef(alice), { aiScore: 5 })); // AI 출시
});
await t("연구실: 발전은 사장만 시작하고, 걸린 초만큼 토큰(1초 = 100센트)이 금고에서 나가요", async () => {
  await coSeed();
  const alice = pw("uA", "alice"), bob = pw("uB", "bob");
  await seed(jobSeed);
  await labSeed({ cash: 50000 });
  await assertFails(updateDoc(labRef(bob), { gen: { amt: 10, secs: 10, started: serverTimestamp() }, cash: 49000 })); // 직원은 시작 못 해요
  await assertFails(updateDoc(labRef(alice), { gen: { amt: 10, secs: 10, started: serverTimestamp() } })); // 돈이 나가야 해요
  await assertFails(updateDoc(labRef(alice), { gen: { amt: 10, secs: 10, started: serverTimestamp() }, cash: 49500 })); // 10초는 1000센트
  await assertSucceeds(updateDoc(labRef(alice), { gen: { amt: 10, secs: 10, started: serverTimestamp() }, cash: 49000 })); // 발전 시작
  await assertFails(updateDoc(labRef(alice), { gen: { amt: 1, secs: 1, started: serverTimestamp() }, cash: 48900 })); // 이미 발전 중이면 또 시작 못 해요
});
await t("연구실: 성능이 높을수록 적은 돈으로 빨리, 한 번에 최대 성능×10까지 만들어요", async () => {
  await coSeed();
  const alice = pw("uA", "alice");
  await labSeed({ cash: 1000000 });
  await assertFails(updateDoc(labRef(alice), { gen: { amt: 2000, secs: 2000, started: serverTimestamp() }, cash: 800000 })); // 성능 100이면 한 번에 1000까지
  await assertSucceeds(updateDoc(labRef(alice), { nperf: 200 })); // 핵연료 성능을 올려요
  await assertFails(updateDoc(labRef(alice), { gen: { amt: 2000, secs: 999, started: serverTimestamp() }, cash: 900100 })); // 시간이 맞아야 해요 (1000초)
  await assertFails(updateDoc(labRef(alice), { gen: { amt: 2000, secs: 1000, started: EPOCH }, cash: 900000 })); // 과거 시각으로 시작 못 해요
  await assertFails(updateDoc(labRef(alice), { elec: 5, gen: { amt: 2000, secs: 1000, started: serverTimestamp() }, cash: 900000 })); // 창고를 채우면서 시작 못 해요
  await assertSucceeds(updateDoc(labRef(alice), { gen: { amt: 2000, secs: 1000, started: serverTimestamp() }, cash: 900000 })); // 2000 전기, 1000초, 1000 토큰
});
await t("연구실: 창고 받기는 발전 시간이 지난 뒤에만, 발전량만큼 늘어요 (회사 사람 누구나)", async () => {
  await coSeed();
  const alice = pw("uA", "alice"), bob = pw("uB", "bob"), carol = pw("uC", "carol");
  await seed(jobSeed);
  await labSeed({ cash: 100000 });
  await assertSucceeds(updateDoc(labRef(alice), { gen: { amt: 3, secs: 3, started: serverTimestamp() }, cash: 99700 })); // 3 전기는 3초, 300센트
  await assertFails(updateDoc(labRef(bob), { elec: 3, gen: null })); // 시간이 덜 지났어요
  await sleep(3500);
  await assertFails(updateDoc(labRef(carol), { elec: 3, gen: null })); // 회사 밖 사람은 못 받아요
  await assertFails(updateDoc(labRef(bob), { elec: 5, gen: null })); // 발전량과 다르면 안 돼요
  await assertFails(updateDoc(labRef(bob), { elec: 3 })); // 발전 기록을 지워야 해요
  await assertSucceeds(updateDoc(labRef(bob), { elec: 3, gen: null })); // 직원이 창고에 받아요 (돈은 그대로)
  await assertFails(updateDoc(labRef(alice), { elec: 9 })); // 발전 없이는 창고가 늘지 않아요
});
await t("연구실: 처음 만들 때 창고는 비어 있고, 금고는 사장이 지갑에서 넣은 만큼만 있어요", async () => {
  await coSeed();
  const alice = pw("uA", "alice"), carol = pw("uC", "carol");
  await walletSeed("uA", 100);
  await assertFails(setDoc(labRef(alice), labFields({ elec: 10 }))); // 창고는 비어 있어야 해요
  await assertFails(setDoc(labRef(alice), labFields({ cash: 500 }))); // 지갑에서 넣은 기록 없이 금고가 생길 수 없어요
  await assertFails(setDoc(labRef(alice), labFields({ gen: { amt: 1, secs: 1, started: serverTimestamp() } }))); // 처음부터 발전할 수 없어요
  await assertFails(setDoc(labRef(carol), labFields())); // 회사 밖 사람은 만들 수 없어요
  await assertSucceeds(setDoc(labRef(alice), labFields()));
});
await t("금고: 사장이 지갑 토큰을 넣고 빼요 (지갑과 금고가 같은 묶음에서 정확히 맞아야 해요)", async () => {
  await coSeed();
  const alice = pw("uA", "alice"), bob = pw("uB", "bob");
  await seed(jobSeed);
  await walletSeed("uA", 100);
  await walletSeed("uB", 100);
  const b1 = tokenBatch(alice, "uA", "deposit", "uA", 30); // 30 토큰 → 금고 3000센트 (처음 만들면서)
  b1.set(labRef(alice), labFields({ cash: 3000 }));
  await assertSucceeds(b1.commit());
  await sleep(2100);
  const b2 = tokenBatch(alice, "uA", "deposit", "uA", 20); // 20 토큰 더 → 5000센트
  b2.update(labRef(alice), { cash: 5000 });
  await assertSucceeds(b2.commit());
  await sleep(2100);
  const bad = tokenBatch(alice, "uA", "deposit", "uA", 10); // 지갑은 10 빠졌는데 금고는 1500 늘었어요 → 안 돼요
  bad.update(labRef(alice), { cash: 6500 });
  await assertFails(bad.commit());
  const bobDep = tokenBatch(bob, "uB", "deposit", "uA", 10); // 직원은 넣을 수 없어요
  bobDep.update(labRef(bob), { cash: 6000 });
  await assertFails(bobDep.commit());
  const lone = tokenBatch(alice, "uA", "deposit", "uA", 10); // 지갑만 빠지고 금고는 그대로인 것도 안 돼요
  await assertFails(lone.commit());
  const w1 = tokenBatch(alice, "uA", "withdraw", "uA", 10); // 10 토큰 빼기 → 금고 4000센트, 지갑 +10
  w1.update(labRef(alice), { cash: 4000 });
  await assertSucceeds(w1.commit());
  await sleep(2100);
  const w2 = tokenBatch(alice, "uA", "withdraw", "uA", 100); // 금고보다 많이 빼면 안 돼요
  w2.update(labRef(alice), { cash: -96000 });
  await assertFails(w2.commit());
});
await t("단가: 사장만 정해요 (센트, 0 이상)", async () => {
  await coSeed();
  const alice = pw("uA", "alice"), bob = pw("uB", "bob");
  await seed(jobSeed);
  await assertSucceeds(setDoc(labRef(alice), labFields()));
  await assertSucceeds(updateDoc(labRef(alice), { price: 300 })); // 3 토큰
  await assertFails(updateDoc(labRef(bob), { price: 500 })); // 직원은 단가를 못 정해요
  await assertFails(updateDoc(labRef(alice), { price: -1 }));
});

console.log("\n[사고팔기]");
const check = (m, c) => { if (!c) throw new Error("확인 실패: " + m); };
const compAt = (cid, kind, over = {}) => seed((d) => setDoc(doc(d, "companies/" + cid), { name: cid, logo: "", kind, owner: cid, products: [], open: 0, staff: 0, at: EPOCH, ...over }));
const labAt = (cid, over) => seed((d) => setDoc(doc(d, "companies/" + cid + "/lab/main"), labFields(over)));
const labRefOf = (db, cid) => doc(db, "companies/" + cid + "/lab/main");
const readLab = async (cid) => { let data; await env.withSecurityRulesDisabled(async (c) => { data = (await getDoc(doc(c.firestore(), "companies/" + cid + "/lab/main"))).data(); }); return data; };
// 한 묶음: 파는 쪽(seller)과 사는 쪽(buyer)이 함께 바뀌어요
const buyBatch = (db, o) => {
  const b = writeBatch(db);
  const soldRec = { to: o.buyer, kind: o.kind, n: o.n, total: o.total, at: o.at ?? serverTimestamp() };
  const boughtRec = { from: o.seller, kind: o.kind, n: o.n, total: o.total, at: o.at ?? serverTimestamp() };
  if (o.model) { Object.assign(soldRec, o.model); Object.assign(boughtRec, o.model); }
  const sUpd = { cash: o.sellerCash, sold: soldRec };
  if (o.sellerElec !== undefined) sUpd.elec = o.sellerElec;
  const bUpd = { cash: o.buyerCash, bought: boughtRec };
  if (o.buyerElec !== undefined) bUpd.elec = o.buyerElec;
  if (o.buyerPcN !== undefined) bUpd.pcN = o.buyerPcN;
  if (o.buyerPcM !== undefined) bUpd.pcM = o.buyerPcM;
  if (o.buyerAiH !== undefined) bUpd.aiH = o.buyerAiH;
  b.update(labRefOf(db, o.seller), sUpd);
  b.update(labRefOf(db, o.buyer), bUpd);
  return b;
};
const seedMarket = async () => {
  await compAt("uA", "nuclear");
  await labAt("uA", { elec: 10, price: 300, cash: 0 });
  await compAt("uB", "ai");
  await labAt("uB", { cash: 10000 });
};
await t("사고팔기: 원자력 회사가 전기를 팔고, 인공지능 회사 사장이 같은 묶음에서 사요", async () => {
  await seedMarket();
  const bob = pw("uB", "bob");
  await assertSucceeds(buyBatch(bob, { seller: "uA", buyer: "uB", kind: "e", n: 3, total: 900,
    sellerCash: 900, sellerElec: 7, buyerCash: 9100, buyerElec: 3 }).commit());
  const s = await readLab("uA"), bu = await readLab("uB");
  check("판 쪽: 금고 900 (센트), 창고 7", s.cash === 900 && s.elec === 7);
  check("산 쪽: 금고 9100, 창고 3, 기록 남음", bu.cash === 9100 && bu.elec === 3 && bu.bought.from === "uA");
});
await t("사고팔기 막힘: 한쪽만 쓰거나, 금액·재고·사는 사람이 틀리면 안 돼요", async () => {
  await seedMarket();
  const alice = pw("uA", "alice"), bob = pw("uB", "bob"), carol = pw("uX", "carol");
  // 파는 쪽만 기록을 쓰면 안 돼요 (산 쪽 기록이 같은 묶음에 없어요)
  const sellOnly = writeBatch(alice);
  sellOnly.update(labRefOf(alice, "uA"), { cash: 900, elec: 7, sold: { to: "uB", kind: "e", n: 3, total: 900, at: serverTimestamp() } });
  await assertFails(sellOnly.commit());
  // 산 쪽만 쓰면 안 돼요 (파는 쪽 기록이 없어요)
  const buyOnly = writeBatch(bob);
  buyOnly.update(labRefOf(bob, "uB"), { cash: 9100, elec: 3, bought: { from: "uA", kind: "e", n: 3, total: 900, at: serverTimestamp() } });
  await assertFails(buyOnly.commit());
  await assertFails(buyBatch(bob, { seller: "uA", buyer: "uB", kind: "e", n: 3, total: 800, sellerCash: 800, sellerElec: 7, buyerCash: 9200, buyerElec: 3 }).commit()); // 단가 300 × 3 = 900
  await assertFails(buyBatch(bob, { seller: "uA", buyer: "uB", kind: "e", n: 3, total: 900, sellerCash: 900, sellerElec: 7, buyerCash: 9000, buyerElec: 3 }).commit()); // 산 쪽 돈이 맞지 않아요
  await assertFails(buyBatch(bob, { seller: "uA", buyer: "uB", kind: "e", n: 11, total: 3300, sellerCash: 3300, sellerElec: -1, buyerCash: 6700, buyerElec: 11 }).commit()); // 재고(10)보다 많이 못 팔아요
  await assertFails(buyBatch(carol, { seller: "uA", buyer: "uB", kind: "e", n: 3, total: 900, sellerCash: 900, sellerElec: 7, buyerCash: 9100, buyerElec: 3 }).commit()); // 남의 회사로 살 수 없어요
  const s = await readLab("uA");
  check("막힌 뒤에도 파는 쪽은 그대로 (창고 10, 금고 0)", s.elec === 10 && s.cash === 0 && s.sold === undefined);
});
await t("사고팔기: 지난 거래 기록을 다시 쓸 수 없어요 (거래 시각은 이번 쓰기의 시각)", async () => {
  await seedMarket();
  const bob = pw("uB", "bob");
  await assertSucceeds(buyBatch(bob, { seller: "uA", buyer: "uB", kind: "e", n: 1, total: 300, sellerCash: 300, sellerElec: 9, buyerCash: 9700, buyerElec: 1 }).commit());
  await assertFails(buyBatch(bob, { seller: "uA", buyer: "uB", kind: "e", n: 1, total: 300, at: EPOCH, sellerCash: 600, sellerElec: 8, buyerCash: 9400, buyerElec: 2 }).commit()); // 옛 시각으로 다시 쓰기
});
await t("사고팔기: 사는 회사는 인공지능 회사만(전기·컴퓨터), 파는 회사는 맞는 종류만", async () => {
  await seedMarket();
  await compAt("uF", "food");
  await labAt("uF", { cash: 10000 });
  await labAt("uC", { elec: 10, price: 300, cash: 0 }); // 컴퓨터 회사이지만 전기를 팔 수는 없어요
  await compAt("uC", "pc");
  const frank = pw("uF", "frank"), bob = pw("uB", "bob");
  await assertFails(buyBatch(frank, { seller: "uA", buyer: "uF", kind: "e", n: 1, total: 300, sellerCash: 300, sellerElec: 9, buyerCash: 9700, buyerElec: 1 }).commit()); // 식품 회사는 전기를 못 사요
  await assertFails(buyBatch(bob, { seller: "uC", buyer: "uB", kind: "e", n: 1, total: 300, sellerCash: 300, sellerElec: 9, buyerCash: 9700, buyerElec: 1 }).commit()); // 컴퓨터 회사는 전기를 못 팔아요
});
await t("사고팔기: 컴퓨터는 파는 회사가 출시한 모델만, 같은 모델이면 더해지고 다른 모델이면 바꿔요", async () => {
  await compAt("uB", "ai");
  await labAt("uB", { cash: 10000 });
  await compAt("uC", "pc");
  await labAt("uC", { rel: { cap: 6, spd: 3, pow: 2 }, cap: 6, spd: 3, pow: 2, price: 500, cash: 0 });
  await compAt("uG", "pc");
  await labAt("uG", { rel: { cap: 4, spd: 4, pow: 4 }, cap: 4, spd: 4, pow: 4, price: 400, cash: 0 });
  const bob = pw("uB", "bob");
  const m1 = { cap: 6, spd: 3, pow: 2 };
  await assertSucceeds(buyBatch(bob, { seller: "uC", buyer: "uB", kind: "p", n: 2, total: 1000, model: m1, sellerCash: 1000, buyerCash: 9000, buyerPcN: 2, buyerPcM: m1 }).commit());
  await assertSucceeds(buyBatch(bob, { seller: "uC", buyer: "uB", kind: "p", n: 1, total: 500, model: m1, sellerCash: 1500, buyerCash: 8500, buyerPcN: 3, buyerPcM: m1 }).commit());
  const bu = await readLab("uB");
  check("같은 모델은 더해져요 (3대)", bu.pcN === 3 && bu.pcM.cap === 6);
  await assertFails(buyBatch(bob, { seller: "uC", buyer: "uB", kind: "p", n: 1, total: 500, model: m1, sellerCash: 2000, buyerCash: 8000, buyerPcN: 5, buyerPcM: m1 }).commit()); // 대수가 틀려요
  const m2 = { cap: 4, spd: 4, pow: 4 };
  await assertFails(buyBatch(bob, { seller: "uG", buyer: "uB", kind: "p", n: 1, total: 400, model: m1, sellerCash: 400, buyerCash: 8100, buyerPcN: 1, buyerPcM: m2 }).commit()); // 기록 모델이 파는 모델과 달라요
  await assertSucceeds(buyBatch(bob, { seller: "uG", buyer: "uB", kind: "p", n: 1, total: 400, model: m2, sellerCash: 400, buyerCash: 8100, buyerPcN: 1, buyerPcM: m2 }).commit()); // 다른 모델은 바꿔요 (1대)
  const bu2 = await readLab("uB");
  check("다른 모델이면 바뀌고 1대", bu2.pcN === 1 && bu2.pcM.cap === 4);
});
await t("사고팔기: 인공지능 사용 시간은 누구나 살 수 있고, 출시한 인공지능만 팔려요", async () => {
  await compAt("uD", "ai");
  await labAt("uD", { aiData: 50, aiScore: 5, price: 200, cash: 0 });
  await compAt("uE", "food");
  await labAt("uE", { cash: 1000 });
  await compAt("uZ", "ai");
  await labAt("uZ", { aiData: 50, aiScore: -1, price: 200, cash: 0 });
  const carol = pw("uE", "carol");
  await assertFails(buyBatch(carol, { seller: "uZ", buyer: "uE", kind: "h", n: 2, total: 400, sellerCash: 400, buyerCash: 600, buyerAiH: 2 }).commit()); // 출시 안 한 인공지능은 못 팔아요
  await assertSucceeds(buyBatch(carol, { seller: "uD", buyer: "uE", kind: "h", n: 2, total: 400, sellerCash: 400, buyerCash: 600, buyerAiH: 2 }).commit());
  const eu = await readLab("uE");
  check("사용 시간 2 (금고 600)", eu.aiH === 2 && eu.cash === 600);
});
await t("사고팔기: 회사 사람이 판매·구매 기록이나 물건 숫자를 혼자 바꿀 수 없어요", async () => {
  await seedMarket();
  const alice = pw("uA", "alice"), bob = pw("uB", "bob");
  await assertFails(updateDoc(labRefOf(alice, "uA"), { sold: { to: "uB", kind: "e", n: 1, total: 300, at: serverTimestamp() } }));
  await assertFails(updateDoc(labRefOf(bob, "uB"), { pcN: 5 }));
  await assertFails(updateDoc(labRefOf(bob, "uB"), { bought: { from: "uA", kind: "e", n: 1, total: 300, at: serverTimestamp() } }));
  await assertFails(updateDoc(labRefOf(bob, "uB"), { aiH: 9 }));
});

console.log("\n[학습]");
await t("학습: 사장이 컴퓨터로 학습을 시작하고, 전기가 나가고, 데이터는 그대로예요", async () => {
  await coSeed();
  const alice = pw("uA", "alice"), bob = pw("uB", "bob");
  await seed(jobSeed);
  await labSeed({ aiData: 100, pcN: 2, pcM: { cap: 5, spd: 4, pow: 3 }, elec: 50 });
  // 40 MB: 배치 8(40÷5), 전기 24(8×3), 시간 2초(8÷4), 점수 3(40×9÷100)
  const trn = (d, secs, score) => ({ d, secs, score, started: serverTimestamp() });
  await assertFails(updateDoc(labRef(bob), { trn: trn(40, 2, 3), elec: 26 })); // 직원은 학습을 못 시켜요
  await assertFails(updateDoc(labRef(pw("uC", "carol")), { trn: trn(40, 2, 3), elec: 26 })); // 회사 밖 사람도 못 시켜요
  await assertFails(updateDoc(labRef(alice), { trn: trn(40, 2, 3), elec: 40 })); // 전기가 나가야 해요
  await assertFails(updateDoc(labRef(alice), { trn: trn(40, 3, 3), elec: 26 })); // 시간이 맞아야 해요
  await assertFails(updateDoc(labRef(alice), { trn: trn(40, 2, 4), elec: 26 })); // 점수가 맞아야 해요
  await assertFails(updateDoc(labRef(alice), { trn: trn(101, 3, 4), elec: 26 })); // 데이터보다 많이 못 써요
  await assertFails(updateDoc(labRef(alice), { trn: { d: 40, secs: 2, score: 3, started: EPOCH }, elec: 26 })); // 과거 시각 불가
  await assertSucceeds(updateDoc(labRef(alice), { trn: trn(40, 2, 3), elec: 26 })); // 학습 시작
  await assertFails(updateDoc(labRef(alice), { trn: trn(10, 1, 0), elec: 23 })); // 학습 중에는 또 못 시작해요
});
await t("학습: 결과는 시간이 지난 뒤에만 회사 사람 누구나 받고, 출시는 사장만 해요", async () => {
  await coSeed();
  const alice = pw("uA", "alice"), bob = pw("uB", "bob"), carol = pw("uC", "carol");
  await seed(jobSeed);
  await labSeed({ aiData: 100, pcN: 1, pcM: { cap: 10, spd: 1, pow: 3 }, elec: 50 });
  // 20 MB: 배치 2(20÷10), 전기 6(2×3), 시간 2초(2÷1), 점수 2(20×11÷100)
  await assertSucceeds(updateDoc(labRef(alice), { trn: { d: 20, secs: 2, score: 2, started: serverTimestamp() }, elec: 44 }));
  await assertFails(updateDoc(labRef(bob), { trn: null, aiDev: 2 })); // 시간이 덜 지났어요
  await sleep(2500);
  await assertFails(updateDoc(labRef(carol), { trn: null, aiDev: 2 })); // 회사 밖 사람은 못 받아요
  await assertFails(updateDoc(labRef(bob), { trn: null, aiDev: 3 })); // 점수는 정해진 값(2)만
  await assertSucceeds(updateDoc(labRef(bob), { trn: null, aiDev: 2 })); // 직원이 결과를 받아요
  await assertFails(updateDoc(labRef(bob), { aiScore: 2 })); // 직원은 출시 못 해요
  await assertFails(updateDoc(labRef(alice), { aiScore: 3 })); // 출시는 개발 점수와 같아야 해요
  await assertSucceeds(updateDoc(labRef(alice), { aiScore: 2 })); // 사장이 출시
});

console.log("\n[음식]");
const prodSeed = (cid, pid, over = {}) => seed((d) => setDoc(doc(d, "companies/" + cid + "/products/" + pid), { owner: cid, cat: "food", priceCents: 300, costCents: 100, active: true, paused: false, at: EPOCH, ...over }));
const prodRef = (db, pid = "p1", cid = "uA") => doc(db, "companies/" + cid + "/products/" + pid);
const readProd = async (pid = "p1", cid = "uA") => { let data; await env.withSecurityRulesDisabled(async (c) => { data = (await getDoc(doc(c.firestore(), "companies/" + cid + "/products/" + pid))).data(); }); return data; };
// 공장 돌리기: 재료값을 금고에서 내고, 재고를 같은 묶음에서 바로 늘려요
const runB = (db, n, cash, stockDelta = n, pid = "p1") => { const b = writeBatch(db); b.update(labRef(db), { cash, prod: { pid, n, started: serverTimestamp() } }); b.update(prodRef(db, pid), { stock: increment(stockDelta) }); return b.commit(); };
await t("음식: 판매를 끝낸 상품은 돌릴 수 없고, 멈춘 상품은 돌릴 수 있어요", async () => {
  await coSeed();
  await labSeed({ cash: 10000 });
  await prodSeed("uA", "p1", { active: false });
  await assertFails(updateDoc(labRef(pw("uA", "alice")), { prod: { pid: "p1", left: 1, at: serverTimestamp() } }));
  await prodSeed("uA", "p1", { paused: true });
  await assertSucceeds(updateDoc(labRef(pw("uA", "alice")), { prod: { pid: "p1", left: 1, at: serverTimestamp() } }));
});

console.log("\n[상점]");
const shopOp = (n, tokens, cid = "uA", pid = "p1") => ({ kind: "shop", cid, pid, n, tokens });
// 사기 묶음: 지갑(paid만큼 빠지고 사기 기록), 파는 회사 금고(cents만큼 늘어요), 상품 재고(stock만큼 줄어요)가 같은 묶음에서 바뀌어요
const buyBatch2 = (db, uid, n, tokens, o = {}) => {
  const b = writeBatch(db);
  if (o.wallet !== false) b.update(doc(db, "wallets/" + uid), { bal: increment(-(o.paid ?? tokens)), wt: serverTimestamp(), op: o.op ?? shopOp(n, tokens) });
  if (o.lab !== false) b.update(labRef(db, "uA"), { cash: increment(o.cents ?? tokens * 100) });
  if (o.prod !== false) b.update(prodRef(db), { stock: increment(-(o.stock ?? n)) });
  return b.commit();
};
await t("상점: 지갑·금고·재고가 같은 묶음에서 맞게 바뀌면 사고, 하나라도 빠지면 못 사요", async () => {
  await coSeed();
  await labSeed({ cash: 0 });
  await prodSeed("uA", "p1", { stock: 5, priceCents: 300 });
  const bob = pw("uB", "bob");
  await assertFails(buyBatch2(bob, "uB", 3, 9, { wallet: false })); // 지갑 없이 금고·재고만은 안 돼요
  await assertFails(buyBatch2(bob, "uB", 3, 9, { lab: false })); // 금고가 안 늘면 안 돼요
  await assertFails(buyBatch2(bob, "uB", 3, 9, { prod: false })); // 재고가 안 줄면 안 돼요
  await assertFails(buyBatch2(bob, "uB", 3, 8, { paid: 8, cents: 800 })); // 값(900 센트)보다 싸게는 안 돼요
  await assertFails(buyBatch2(bob, "uB", 3, 10, { paid: 10, cents: 1000 })); // 값보다 많이도 안 돼요
  await assertFails(buyBatch2(bob, "uB", 6, 18, { stock: 6, cents: 1800 })); // 재고(5)보다 많이는 못 사요
  await assertFails(buyBatch2(bob, "uB", 3, 9, { paid: 8 })); // 지갑에서 빠진 양과 기록이 달라요
  await assertSucceeds(buyBatch2(bob, "uB", 3, 9));
  check("금고 900 센트", (await readLab("uA")).cash === 900);
  check("재고 2", (await readProd()).stock === 2);
});
await t("상점: 지난번 사기 기록을 다시 써서 공짜로 살 수 없고, 0 토큰으로도 못 사요", async () => {
  await coSeed();
  await labSeed({ cash: 0 });
  await prodSeed("uA", "p1", { stock: 5, priceCents: 300 });
  const bob = pw("uB", "bob");
  await assertSucceeds(buyBatch2(bob, "uB", 1, 3)); // 한 번 산다
  // 방금 산 기록은 그대로 두고 금고·재고만 다시 바꾸려는 속임수
  const b = writeBatch(bob);
  b.update(labRef(bob, "uA"), { cash: increment(300) });
  b.update(prodRef(bob), { stock: increment(-1) });
  await assertFails(b.commit());
  await sleep(2100);
  await assertFails(buyBatch2(bob, "uB", 1, 0)); // 0 토큰으로 사기는 안 돼요
  check("금고는 300 센트 그대로", (await readLab("uA")).cash === 300);
});
await t("상점: 소수 가격(0.8 토큰)은 올려서 받아요", async () => {
  await coSeed();
  await labSeed({ cash: 0 });
  await prodSeed("uA", "p1", { stock: 5, priceCents: 80 });
  await seed((d) => setDoc(doc(d, "wallets/uC"), { bal: 100, rate: 0, t: EPOCH, wt: EPOCH, op: { kind: "init" } }));
  const carol = pw("uC", "carol");
  await assertFails(buyBatch2(carol, "uC", 2, 1)); // 1.6 토큰을 1 토큰에 사면 안 돼요
  await assertFails(buyBatch2(carol, "uC", 2, 3)); // 3 토큰은 너무 많아요
  await assertSucceeds(buyBatch2(carol, "uC", 2, 2)); // 2 토큰
  check("금고 200 센트", (await readLab("uA")).cash === 200);
  check("재고 3", (await readProd()).stock === 3);
});
await t("상점: 회사 밖 사람도 살 수 있어요, 멈춘·끝난 상품과 로그인하지 않은 사람은 못 사요", async () => {
  await coSeed();
  await labSeed({ cash: 0 });
  await prodSeed("uA", "p1", { stock: 5, priceCents: 300, paused: true });
  await seed((d) => setDoc(doc(d, "wallets/uC"), { bal: 100, rate: 0, t: EPOCH, wt: EPOCH, op: { kind: "init" } }));
  const carol = pw("uC", "carol");
  await assertFails(buyBatch2(carol, "uC", 1, 3)); // 멈춘 상품
  await prodSeed("uA", "p1", { stock: 5, priceCents: 300, active: false });
  await assertFails(buyBatch2(carol, "uC", 1, 3)); // 끝난 상품
  await prodSeed("uA", "p1", { stock: 5, priceCents: 300 });
  await assertFails(buyBatch2(anon(), "uC", 1, 3)); // 로그인하지 않으면 안 돼요
  await assertSucceeds(buyBatch2(carol, "uC", 1, 3)); // 회사 밖 사람도 살 수 있어요
  check("재고 4", (await readProd()).stock === 4);
});
await t("상점: 재고가 있는 음식 상품을 여러 회사에서 한꺼번에 찾아요 (로그인한 사람만)", async () => {
  await coSeed();
  await seed((d) => setDoc(doc(d, "companies/uC"), { name: "다른 회사", logo: "", kind: "food", owner: "uC", products: [], open: 0, staff: 0, at: EPOCH }));
  await prodSeed("uA", "p1", { stock: 5 });
  await prodSeed("uA", "p2", { stock: 0 });
  await prodSeed("uC", "p3", { stock: 2 });
  const q = query(collectionGroup(pw("uC", "carol"), "products"), where("stock", ">", 0), limit(50));
  await assertSucceeds(getDocs(q));
  const snap = await getDocs(q);
  check("재고 있는 상품 2개", snap.docs.length === 2);
  check("회사 번호를 알 수 있어요 (uA, uC)", snap.docs.map((d) => d.ref.parent.parent.id).sort().join() === "uA,uC");
  await assertFails(getDocs(query(collectionGroup(anon(), "products"), limit(5))));
});

console.log("\n[먹기]");
// 인벤토리: 내가 가진 음식 칸. 먹으면 한 개가 줄고, 같은 묶음에서 먹은 사람의 상태가 올라가요
const dec1h = (v, full) => v - (3600e3 * 100) / full; // 한 시간 전에 기록한 값이 지금 닳은 만큼
const readVit = async (uid) => { let d; await env.withSecurityRulesDisabled(async (c) => { d = (await getDoc(doc(c.firestore(), "vitals/" + uid))).data(); }); return d; };
const vitSeed = (uid, over = {}) => seed((d) => setDoc(doc(d, "vitals/" + uid), { h: 60, t: 30, e: 60, at: Date.now() - 3600e3, deaths: 0, ...over }));
const invFields = (cid, pid, group, n) => ({ cid, pid, group, name: "생수", item: "water", shape: 1, color: -1, cap: 3, capColor: 5, n });
const itemSeed = (uid, cid, pid, group, n) => seed((d) => setDoc(doc(d, `inventory/${uid}/items/${cid}_${pid}`), invFields(cid, pid, group, n)));
const invRef = (db, uid, cid = "uA", pid = "p1") => doc(db, `inventory/${uid}/items/${cid}_${pid}`);
const readItem = async (uid, cid = "uA", pid = "p1") => { let d; await env.withSecurityRulesDisabled(async (c) => { d = (await getDoc(doc(c.firestore(), `inventory/${uid}/items/${cid}_${pid}`))).data(); }); return d; };
// 먹기 묶음: 칸(n)을 한 개 줄이고 상태를 써요. invUid는 칸의 주인이에요 (남의 칸을 건드리는 시험에 써요)
const eatBatch = (db, uid, o, invUid = uid) => {
  const b = writeBatch(db);
  b.update(invRef(db, invUid, o.cid, o.pid), { n: increment(-1) });
  b.update(doc(db, "vitals/" + uid), { h: o.h, t: o.t, e: o.e, at: o.at ?? Date.now(), eat: { cid: o.cid, pid: o.pid } });
  return b.commit();
};
await t("먹기: 내 인벤토리에서 물을 먹으면 목마름이 40 올라가고 한 개가 줄어요 (나머지는 그대로)", async () => {
  await itemSeed("uA", "uA", "p1", "water", 3);
  await vitSeed("uA");
  const alice = pw("uA", "alice");
  const t2 = dec1h(30, 64800000) + 40;
  await assertFails(updateDoc(doc(alice, "vitals/uA"), { h: dec1h(60, 86400000), t: t2, e: dec1h(60, 129600000), at: Date.now(), eat: { cid: "uA", pid: "p1" } })); // 칸을 한 개 줄이지 않으면 안 돼요
  await assertFails(eatBatch(alice, "uA", { cid: "uA", pid: "p1", h: dec1h(60, 86400000), t: dec1h(30, 64800000), e: dec1h(60, 129600000) })); // 목마름이 안 오르면 안 돼요
  await assertFails(eatBatch(alice, "uA", { cid: "uA", pid: "p1", h: dec1h(60, 86400000) - 10, t: t2, e: dec1h(60, 129600000) })); // 다른 상태를 마음대로 바꾸면 안 돼요
  await assertSucceeds(eatBatch(alice, "uA", { cid: "uA", pid: "p1", h: dec1h(60, 86400000), t: t2, e: dec1h(60, 129600000) }));
  check("칸 2개 남음", (await readItem("uA")).n === 2);
  const v = await readVit("uA");
  check("목마름 약 64", Math.abs(v.t - t2) < 0.05);
  check("배고픔은 닳은 값 그대로", Math.abs(v.h - dec1h(60, 86400000)) < 0.05);
});
await t("먹기: 남의 인벤토리는 건드릴 수 없고, 쉬는 중에도 못 먹어요", async () => {
  await itemSeed("uA", "uA", "p1", "water", 3);
  await vitSeed("uB");
  await vitSeed("uA");
  const args = { cid: "uA", pid: "p1", h: dec1h(60, 86400000), t: dec1h(30, 64800000) + 40, e: dec1h(60, 129600000) };
  await assertFails(eatBatch(pw("uB", "bob"), "uB", args, "uA")); // 밥이 앨리스의 칸을 줄일 수 없어요
  await vitSeed("uA", { at: Date.now() + 3600e3 }); // 쉬는 중 (다시 일어날 시각이 아직 안 됐어요)
  await assertFails(eatBatch(pw("uA", "alice"), "uA", { ...args, at: Date.now() }));
  check("칸 그대로 3개", (await readItem("uA")).n === 3);
});
await t("먹기: 음식은 배고픔 20, 식사는 40, 에너지 식품은 에너지 40을 올리고, 100을 넘지 않아요", async () => {
  await seed((d) => setDoc(doc(d, "vitals/uA"), { h: 95, t: 50, e: 50, at: Date.now(), deaths: 0 }));
  await itemSeed("uA", "uA", "p2", "snack", 2);
  await itemSeed("uA", "uA", "p3", "meal", 2);
  await itemSeed("uA", "uA", "p4", "energy", 2);
  await itemSeed("uA", "uA", "p5", "snack", 2);
  const alice = pw("uA", "alice");
  await assertFails(eatBatch(alice, "uA", { cid: "uA", pid: "p2", h: 115, t: 50, e: 50 })); // 100을 넘을 수 없어요
  await assertFails(eatBatch(alice, "uA", { cid: "uA", pid: "p5", h: 95, t: 50, e: 50 })); // 음식을 먹었는데 배고픔이 안 오르면 안 돼요
  await assertSucceeds(eatBatch(alice, "uA", { cid: "uA", pid: "p2", h: 100, t: 50, e: 50 })); // 95 + 20 → 100에서 멈춰요
  await assertSucceeds(eatBatch(alice, "uA", { cid: "uA", pid: "p4", h: 100, t: 50, e: 90 })); // 에너지 50 + 40
  await assertSucceeds(eatBatch(alice, "uA", { cid: "uA", pid: "p3", h: 100, t: 50, e: 90 })); // 식사: 배고픔은 이미 100
  check("p2 1개 남음", (await readItem("uA", "uA", "p2")).n === 1);
  check("p3 1개 남음", (await readItem("uA", "uA", "p3")).n === 1);
  check("p4 1개 남음", (await readItem("uA", "uA", "p4")).n === 1);
  check("p5는 그대로 2개", (await readItem("uA", "uA", "p5")).n === 2);
});

console.log("\n[인벤토리]");
// 꺼내기 (사장): 창고에서 한 개가 줄면 내 인벤토리에 한 개가 들어와요 (같은 묶음)
const takeBatch = (db, uid, pid, addN = 1) => {
  const b = writeBatch(db);
  b.update(prodRef(db, pid), { stock: increment(-1) });
  b.set(doc(db, `inventory/${uid}/items/uA_${pid}`), invFields("uA", pid, "water", addN));
  return b.commit();
};
await t("인벤토리: 사장이 창고에서 한 개 꺼내면 내 인벤토리에 한 개가 들어와요 (직원은 못 꺼내요)", async () => {
  await coSeed();
  await seed(jobSeed);
  await prodSeed("uA", "p1", { stock: 3, group: "water" });
  const alice = pw("uA", "alice");
  await assertFails(setDoc(doc(alice, "inventory/uA/items/uA_p1"), invFields("uA", "p1", "water", 1))); // 창고가 줄지 않으면 안 돼요
  await assertFails(takeBatch(alice, "uA", "p1", 2)); // 한 개씩만 꺼낼 수 있어요
  await assertFails(takeBatch(pw("uB", "bob"), "uB", "p1", 1)); // 직원은 꺼낼 수 없어요
  await assertSucceeds(takeBatch(alice, "uA", "p1", 1));
  check("창고 2개", (await readProd()).stock === 2);
  check("인벤토리 1개", (await readItem("uA")).n === 1);
});
await t("인벤토리: 상점에서 사면 산 만큼 내 인벤토리에 들어와요 (사기 기록과 같은 묶음)", async () => {
  await coSeed();
  await labSeed({ cash: 0 });
  await prodSeed("uA", "p1", { stock: 5, priceCents: 300, group: "water" });
  const bob = pw("uB", "bob");
  const buyWithItem = (n, tokens, inv = n) => {
    const b = writeBatch(bob);
    b.update(doc(bob, "wallets/uB"), { bal: increment(-tokens), wt: serverTimestamp(), op: shopOp(n, tokens) });
    b.update(labRef(bob, "uA"), { cash: increment(tokens * 100) });
    b.update(prodRef(bob), { stock: increment(-n) });
    b.set(doc(bob, "inventory/uB/items/uA_p1"), invFields("uA", "p1", "water", inv));
    return b.commit();
  };
  await assertFails(buyWithItem(2, 6, 3)); // 산 만큼만 들어와요
  await assertSucceeds(buyWithItem(2, 6));
  check("인벤토리 2개", (await readItem("uB")).n === 2);
});
await t("인벤토리: 마음대로 늘릴 수 없고, 남의 칸은 못 건드려요", async () => {
  await itemSeed("uA", "uA", "p1", "water", 1);
  const alice = pw("uA", "alice");
  await assertFails(updateDoc(invRef(alice, "uA"), { n: 5 })); // 늘리기 (사기나 꺼내기 없이)
  await assertFails(updateDoc(invRef(pw("uB", "bob"), "uA"), { n: 0 })); // 남의 칸
  await assertSucceeds(updateDoc(invRef(alice, "uA"), { n: 0 })); // 한 개 줄이기 (버리기)
  check("인벤토리 0개", (await readItem("uA")).n === 0);
});

console.log("\n[관리]");
// 관리자 (오너 계정): 인증토큰을 만들고, 3개가 같을 때만 행동이 돼요. 한 번 쓰면 없어지고, 5분이 지나면 못 써요
const ownerDb = () => pw("uOwner", "owner");
const TK = ["aaaaaaaaaaaa", "bbbbbbbbbbbb", "cccccccccccc"];
const issueTk = (db, tk = TK, expMs = 300000) => setDoc(doc(db, "adminAuth/uOwner"), { tk, exp: Date.now() + expMs });
const readWallet = async (uid) => { let d; await env.withSecurityRulesDisabled(async (c) => { d = (await getDoc(doc(c.firestore(), "wallets/" + uid))).data(); }); return d.bal; };
const grantB = (db, tk, n = 50, { consume = true } = {}) => {
  const b = writeBatch(db);
  b.update(doc(db, "wallets/uA"), { bal: increment(n), wt: serverTimestamp(), op: { kind: "grant", amt: n, tk } });
  if (consume) b.delete(doc(db, "adminAuth/uOwner"));
  return b.commit();
};
await t("관리: 오너만 인증토큰을 만들 수 있고, 3개와 5분 규칙을 지켜야 해요", async () => {
  await seed(seedBase);
  await assertFails(issueTk(pw("uB", "bob"))); // 오너가 아니면 못 만들어요
  await assertFails(issueTk(ownerDb(), ["aaaaaaaaaaaa", "bbbbbbbbbbbb"])); // 3개여야 해요
  await assertFails(issueTk(ownerDb(), TK, 3600e3)); // 5분을 넘는 기한은 안 돼요
  await assertSucceeds(issueTk(ownerDb()));
});
await t("관리: 토큰 3개가 다 맞아야 지급되고, 한 번 쓰면 없어져요", async () => {
  await seed(seedBase);
  await issueTk(ownerDb());
  await assertFails(grantB(ownerDb(), ["aaaaaaaaaaaa", "bbbbbbbbbbbb", "dddddddddddd"], 50)); // 하나라도 다르면 안 돼요
  await assertFails(grantB(ownerDb(), TK, 50, { consume: false })); // 토큰을 없애지 않으면 안 돼요
  await assertSucceeds(grantB(ownerDb(), TK, 50));
  const w = await readWallet("uA");
  check("지급 후 잔액 +50", w === 1250050);
  await assertFails(grantB(ownerDb(), TK, 50)); // 같은 토큰은 두 번 못 써요
});
await t("관리: 인증토큰은 5분이 지나면 쓸 수 없어요", async () => {
  await seed(seedBase);
  await issueTk(ownerDb(), TK, 2000);
  await sleep(2600);
  await assertFails(grantB(ownerDb(), TK, 50));
});
await t("관리: 공지는 오너만 쓰고, 토큰이 맞아야 써져요", async () => {
  await seed(seedBase);
  await issueTk(ownerDb());
  const notice = (db, tk, extra = {}) => { const b = writeBatch(db); b.set(doc(db, "notices/n1"), { title: "점검", body: "점검 안내", by: "uOwner", createdAt: serverTimestamp(), tk, ...extra }); b.delete(doc(db, "adminAuth/uOwner")); return b.commit(); };
  await assertFails(notice(pw("uA", "alice"), TK)); // 오너가 아니에요
  await assertFails(notice(ownerDb(), ["aaaaaaaaaaaa", "bbbbbbbbbbbb", "zzzzzzzzzzzz"])); // 토큰이 달라요
  await assertFails(notice(ownerDb(), TK, { options: ["하나"] })); // 투표 항목은 2개 이상
  await assertSucceeds(notice(ownerDb(), TK, { options: ["예", "아니오"] }));
  await assertSucceeds(getDoc(doc(pw("uA", "alice"), "notices/n1")));
});
await t("공지: 투표는 한 번만, 고르는 번호는 항목 안에서만, 목록은 오너만 봐요", async () => {
  await seed(seedBase);
  await issueTk(ownerDb());
  const od = ownerDb();
  const b = writeBatch(od); b.set(doc(od, "notices/n2"), { title: "투표", body: "고르세요", by: "uOwner", createdAt: serverTimestamp(), tk: TK, options: ["가", "나"] }); b.set(doc(od, "meta/noticeIndex"), { ids: ["n2"] }); b.delete(doc(od, "adminAuth/uOwner")); await assertSucceeds(b.commit());
  await assertSucceeds(getDoc(doc(pw("uA", "alice"), "meta/noticeIndex")));
  await assertSucceeds(setDoc(doc(pw("uA", "alice"), "notices/n2/votes/uA"), { choice: 1, at: serverTimestamp() }));
  await assertFails(setDoc(doc(pw("uA", "alice"), "notices/n2/votes/uA"), { choice: 2, at: serverTimestamp() })); // 항목은 2개뿐
  await assertFails(setDoc(doc(pw("uB", "bob"), "notices/n2/votes/uA"), { choice: 0, at: serverTimestamp() })); // 남의 이름으로는 못 골라요
  await assertFails(getDoc(doc(pw("uA", "alice"), "notices/n2/votes/uB"))); // 남의 선택은 못 봐요
  await assertFails(getDocs(collection(pw("uA", "alice"), "notices")));  // 목록은 오너만
  await assertSucceeds(getDocs(collection(ownerDb(), "notices")));
  await assertSucceeds(setDoc(doc(pw("uA", "alice"), "noticeReads/uA"), { seen: { n2: true } }));
  await assertFails(setDoc(doc(pw("uB", "bob"), "noticeReads/uA"), { seen: { n2: true } })); // 남의 읽음 기록은 못 써요
  const da = pw("uA", "alice");
  await assertFails(writeBatch(da).set(doc(da, "meta/noticeIndex"), { ids: ["x"] }).commit()); // 목록은 오너만
});
await t("관리: 계정 데이터 삭제는 오너가 토큰을 써서만 돼요 (기록이 남아요)", async () => {
  await seed(seedBase);
  await issueTk(ownerDb());
  const del = (db) => { const b = writeBatch(db); b.set(doc(db, "adminLog/l1"), { tk: TK, kind: "delete", target: "uA", at: serverTimestamp() }); b.delete(doc(db, "profiles/uA")); b.delete(doc(db, "wallets/uA")); b.delete(doc(db, "adminAuth/uOwner")); return b.commit(); };
  await assertFails(del(pw("uB", "bob")));
  await assertSucceeds(del(ownerDb()));
  let gone; await env.withSecurityRulesDisabled(async (c) => { gone = !(await getDoc(doc(c.firestore(), "profiles/uA"))).exists(); });
  check("프로필이 지워졌어요", gone);
});
await t("설정: 이름은 가입할 때 정한 이름으로만 저장돼요 (바꿀 수 없어요)", async () => {
  await seed(seedBase);
  const alice = pw("uA", "alice");
  await assertFails(setDoc(doc(alice, "users/uA"), { profile: { name: "다른 이름", bio: "" }, updatedAt: Date.now() }));
  await assertSucceeds(setDoc(doc(alice, "users/uA"), { profile: { name: "김철수", bio: "소개" }, updatedAt: Date.now() }));
});

console.log("\n[상태]");
await t("상태: 처음엔 100%로만 만들고, 안 쓰러졌으면 기록을 못 바꿔요", async () => {
  await seed(seedBase);
  const a = pw("uA", "alice");
  await assertFails(setDoc(doc(a, "vitals/uA"), { h: 100, t: 100, e: 100, at: Date.now(), deaths: 0 }.h === 100 ? { h: 100, t: 100, e: 50, at: Date.now(), deaths: 0 } : {}));
  await assertFails(setDoc(doc(a, "vitals/uA"), { h: 100, t: 100, e: 100, at: Date.now() - 3600e3, deaths: 0 })); // 과거로 시간 조작
  await assertFails(setDoc(doc(pw("uB", "bob"), "vitals/uA"), { h: 100, t: 100, e: 100, at: Date.now(), deaths: 0 }));
  await assertSucceeds(setDoc(doc(a, "vitals/uA"), { h: 100, t: 100, e: 100, at: Date.now(), deaths: 0 }));
  await assertFails(getDoc(doc(pw("uB", "bob"), "vitals/uA")));
  // 방금 만들었으니 닳지 않았어요: 쓰러졌다고 쓸 수 없어요 / 값을 올릴 수도 없어요
  await assertFails(updateDoc(doc(a, "vitals/uA"), { h: 10, t: 10, e: 10, at: Date.now() + 3600e3, deaths: 1 }));
  await assertFails(updateDoc(doc(a, "vitals/uA"), { h: 100, t: 100, e: 100, at: Date.now() }));
});
await t("상태: 정말 0%가 됐을 때만 쓰러짐을 기록하고, 10%·쉬는 시간이 맞아야 해요", async () => {
  await seed(async (d) => { await seedBase(d); await setDoc(doc(d, "vitals/uA"), { h: 100, t: 100, e: 100, at: Date.now() - 30 * 3600e3, deaths: 0 }); });
  const a = pw("uA", "alice");
  const ref = doc(a, "vitals/uA");
  await assertFails(updateDoc(ref, { h: 50, t: 50, e: 50, at: Date.now() + 3600e3, deaths: 1 })); // 10%가 아님
  await assertFails(updateDoc(ref, { h: 10, t: 10, e: 10, at: Date.now(), deaths: 1 })); // 쉬는 시간 없음
  await assertFails(updateDoc(ref, { h: 10, t: 10, e: 10, at: Date.now() + 3600e3, deaths: 2 })); // 횟수 건너뜀
  await assertSucceeds(updateDoc(ref, { h: 10, t: 10, e: 10, at: Date.now() + 3600e3, deaths: 1 }));
  await assertFails(updateDoc(ref, { h: 10, t: 10, e: 10, at: Date.now() + 7200e3, deaths: 2 })); // 쉬는 중에는 또 못 써요
});
await t("상태: 두 번째로 쓰러지면 2시간 쉬어요", async () => {
  await seed(async (d) => { await seedBase(d); await setDoc(doc(d, "vitals/uA"), { h: 10, t: 10, e: 10, at: Date.now() - 30 * 3600e3, deaths: 1 }); });
  const ref = doc(pw("uA", "alice"), "vitals/uA");
  await assertFails(updateDoc(ref, { h: 10, t: 10, e: 10, at: Date.now() + 3600e3, deaths: 2 })); // 1시간만 쉬려는 속임수
  await assertSucceeds(updateDoc(ref, { h: 10, t: 10, e: 10, at: Date.now() + 7200e3, deaths: 2 }));
});

// 공장: 돌리기(사장만, 서버 시각), 정산(만든 만큼만, 서버 시각 기준 최대치 이하), 공장 추가(500토큰, 돌고 있지 않을 때만)
const factorySeed = (labOver = {}) => seed(async (d) => {
  await setDoc(doc(d, "companies/uA/products/p1"), { owner: "uA", cat: "food", group: "water", item: "water", name: "생수", shape: 1, color: -1, costCents: 100, priceCents: 300, paused: false, active: true, stock: 0, at: EPOCH });
  await setDoc(doc(d, "companies/uA/lab/main"), labFields({ cash: 10000, ...labOver }));
});
await t("공장: 돌리기는 사장만, 시작 시각은 서버 시각, 돌고 있으면 또 못 돌려요", async () => {
  await coSeed();
  await factorySeed();
  const alice = pw("uA", "alice"), bob = pw("uB", "bob");
  await seed(jobSeed);
  await assertFails(updateDoc(labRef(bob), { prod: { pid: "p1", left: 3, at: serverTimestamp() } })); // 직원은 못 돌려요
  await assertFails(updateDoc(labRef(alice), { prod: { pid: "p1", left: 3, at: EPOCH } })); // 시작 시각은 서버 시각이어야 해요
  await assertFails(updateDoc(labRef(alice), { prod: { pid: "p1", left: 0, at: serverTimestamp() } })); // 개수는 1개 이상
  await assertSucceeds(updateDoc(labRef(alice), { prod: { pid: "p1", left: 3, at: serverTimestamp() } }));
  await assertFails(updateDoc(labRef(alice), { prod: { pid: "p1", left: 5, at: serverTimestamp() } })); // 돌고 있으면 또 못 돌려요
  await assertFails(updateDoc(doc(alice, "companies/uA/products/p1"), { stock: 5 })); // 재고만 따로 늘릴 수 없어요
});
await t("공장: 정산은 아직 시간이 안 지났으면 안 돼요 (재료값과 재고가 같이 바뀌어야 해요)", async () => {
  await coSeed();
  await factorySeed({ prod: { pid: "p1", left: 3, at: EPOCH } });
  const alice = pw("uA", "alice");
  // 시작한 지 60초가 지나지 않았으니 한 개도 받을 수 없어요
  await assertFails(updateDoc(labRef(alice), { cash: 9900, prod: { pid: "p1", left: 2, at: Timestamp.fromMillis(EPOCH.toMillis() + 60000) } }));
  await assertFails(updateDoc(labRef(alice), { cash: 9700, prod: null })); // 다 받은 척
  await assertFails(updateDoc(doc(alice, "companies/uA/products/p1"), { stock: 1 })); // 재고만 늘리기
  // 금고만 바꾸는 것도 안 돼요 (재고가 같은 묶음에서 늘어야 해요)
  await assertFails(updateDoc(labRef(alice), { cash: 9900 }));
});
await t("공장 추가: 500토큰을 금고에서 내고, 돌고 있으면 못 해요", async () => {
  await coSeed();
  await factorySeed({ cash: 60000 });
  const alice = pw("uA", "alice"), bob = pw("uB", "bob");
  await seed(jobSeed);
  await assertFails(updateDoc(labRef(bob), { plv: 2, cash: 10000 })); // 직원은 못 사요
  await assertFails(updateDoc(labRef(alice), { plv: 2, cash: 60000 })); // 돈을 안 뺐어요
  await assertFails(updateDoc(labRef(alice), { plv: 4, cash: 10000 })); // 한 번에 두 대는 못 사요
  await assertSucceeds(updateDoc(labRef(alice), { plv: 2, cash: 10000 }));
});
await t("공장 추가: 돌고 있을 때는 살 수 없어요", async () => {
  await coSeed();
  await factorySeed({ cash: 60000, prod: { pid: "p1", left: 2, at: EPOCH } });
  await assertFails(updateDoc(labRef(pw("uA", "alice")), { plv: 2, cash: 10000 }));
});

console.log(`\n${pass}개 통과, ${fails.length}개 실패`);
if (fails.length) console.log("실패:", fails.join(" / "));
await env.cleanup();
process.exit(fails.length ? 1 : 0);
