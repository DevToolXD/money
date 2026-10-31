// Firestore 보안 규칙 시험: 공격하려는 쓰기가 실제로 막히는지, 정상 쓰기는 통과하는지 에뮬레이터에서 확인해요.
// 실행: cd tests && npm install && npm run rules
import { readFileSync } from "node:fs";
import { initializeTestEnvironment, assertSucceeds, assertFails } from "@firebase/rules-unit-testing";
import { doc, setDoc, getDoc, getDocs, updateDoc, collection, query, where, limit, orderBy, writeBatch, deleteDoc, serverTimestamp, Timestamp, increment } from "firebase/firestore";

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
await t("지갑은 정확히 1,250,000으로 한 번만, 프로필이 있을 때만 만들어요", async () => {
  const db = pw("uA", "alice");
  const w = { bal: 1250000, rate: 0, t: serverTimestamp(), wt: serverTimestamp(), op: { kind: "init" } };
  await assertFails(setDoc(doc(db, "wallets/uA"), w)); // 프로필이 아직 없어요
  await seed((s) => setDoc(doc(s, "profiles/uA"), { id: "alice", name: "김철수", photo: "" }));
  await assertFails(setDoc(doc(db, "wallets/uA"), { ...w, bal: 99999999 }));
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
  await assertSucceeds(updateDoc(doc(db, "wallets/uA"), { rate: 30, bal: 1250000 - 503000, t: serverTimestamp(), wt: serverTimestamp(), op: { kind: "buy" } }));
  await sleep(2100);
  await assertFails(updateDoc(doc(db, "wallets/uA"), { rate: 10, bal: 747000 - 70000, t: serverTimestamp(), wt: serverTimestamp(), op: { kind: "buy" } })); // 낮은 이율로 갈아타기
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

console.log(`\n${pass}개 통과, ${fails.length}개 실패`);
if (fails.length) console.log("실패:", fails.join(" / "));
await env.cleanup();
process.exit(fails.length ? 1 : 0);
