// 회원가입 전화번호 인증 서버 (Zavu Functions, https://fn.zavu.dev/<프로젝트>/phone-verify).
// Zavu 함수는 TypeScript만 돌아가서 이 파일만 TypeScript예요. 앱(러스트)은 이 주소에 글자(JSON)를 보내요.
//
//  op: "send"  { idToken, phone }        → 인증번호 6자리를 문자로 보내요 (한국 휴대폰 010만)
//  op: "check" { idToken, phone, code }  → 맞으면 Firestore에 phones/{번호 해시} = 이 계정, verified/{uid} 를 적어요
//
// - 누가 보냈는지는 Firebase 로그인 토큰(idToken)으로 확인해요.
// - Firestore에는 인증 전용 계정(VERIFIER_EMAIL)으로만 써요. firestore.rules가 그 계정만 phones·verified·otp를 쓰게 막아요.
// - 번호는 비밀 소금값(PEPPER)을 섞은 해시로만 저장해요. 인증번호도 해시로만 저장해요.
// - 문자 비용을 지키려고: 같은 계정은 60초에 한 번, 하루 5번, 서비스 전체는 하루 25통까지만 보내요 (Zavu 무료 한도).
// 비밀값(VERIFIER_EMAIL, VERIFIER_PW, PEPPER)은 `npx zavudev fn secrets set`으로 Zavu에만 있어요. 이 파일에는 없어요.
import { defineFunction } from "@zavudev/functions"
import Zavudev from "@zavudev/sdk"
import { createHash, randomInt } from "node:crypto"

const zavu = new Zavudev({ apiKey: process.env.ZAVU_API_KEY, baseURL: process.env.ZAVU_API_BASE_URL })

// 웹 설정값(비밀 아님, app/firebase-config.js와 같아요)
const FB_KEY = "AIzaSyDAJsjlCok8i88xlFViz_mnWdGJqX7oiKo"
const PROJECT = "money-sim-36ed8"
const IDT = "https://identitytoolkit.googleapis.com/v1"
const DB = `projects/${PROJECT}/databases/(default)/documents`
const FS = `https://firestore.googleapis.com/v1/${DB}`

const CODE_MS = 5 * 60_000
const RESEND_MS = 60_000
const PER_DAY_USER = 5
const PER_DAY_ALL = 25
const MAX_TRIES = 5

const CORS = { "access-control-allow-origin": "*", "access-control-allow-methods": "POST, OPTIONS", "access-control-allow-headers": "content-type", "content-type": "application/json" }
const reply = (status: number, body: Record<string, unknown>) => ({ statusCode: status, headers: CORS, body: JSON.stringify(body) })
const fail = (code: string, status = 400) => reply(status, { ok: false, code })

const sha = (s: string) => createHash("sha256").update(s).digest("hex")
const pepper = () => process.env.PEPPER ?? ""
const day = () => new Date(Date.now() + 9 * 3600_000).toISOString().slice(0, 10) // 한국 날짜

/** 010-1234-5678 / 01012345678 / +82 10 1234 5678 → +821012345678. 한국 휴대폰이 아니면 "" */
export function normalize(raw: string): string {
  const d = String(raw ?? "").replace(/[^\d+]/g, "")
  let n = ""
  if (/^\+8210\d{8}$/.test(d)) n = d
  else if (/^8210\d{8}$/.test(d)) n = "+" + d
  else if (/^010\d{8}$/.test(d)) n = "+82" + d.slice(1)
  return n
}

/* ---------- Firebase ---------- */

let vtoken: { t: string; exp: number } | null = null
async function verifierToken(): Promise<string> {
  if (vtoken && vtoken.exp > Date.now() + 60_000) return vtoken.t
  const body = JSON.stringify({ email: process.env.VERIFIER_EMAIL, password: process.env.VERIFIER_PW, returnSecureToken: true })
  const post = (u: string) => fetch(`${IDT}/${u}?key=${FB_KEY}`, { method: "POST", headers: { "content-type": "application/json" }, body })
  let r = await post("accounts:signInWithPassword")
  let j: any = await r.json()
  if (!r.ok && /EMAIL_NOT_FOUND|INVALID_LOGIN_CREDENTIALS/.test(j?.error?.message ?? "")) {
    // 처음 한 번: 인증 전용 계정을 만들어요
    r = await post("accounts:signUp")
    j = await r.json()
  }
  if (!r.ok) throw new Error("verifier sign-in: " + (j?.error?.message ?? r.status))
  vtoken = { t: j.idToken, exp: Date.now() + Number(j.expiresIn ?? 3600) * 1000 }
  return vtoken.t
}

/** 앱 사용자 토큰 → uid (비밀번호로 만든 계정만) */
async function uidOf(idToken: string): Promise<string | null> {
  if (!idToken || typeof idToken !== "string" || idToken.length > 4000) return null
  const r = await fetch(`${IDT}/accounts:lookup?key=${FB_KEY}`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ idToken }) })
  if (!r.ok) return null
  const j: any = await r.json()
  const u = j?.users?.[0]
  if (!u?.localId || !(u.providerUserInfo ?? []).some((p: any) => p.providerId === "password")) return null
  return u.localId
}

type Fields = Record<string, any>
const enc = (v: unknown): any =>
  typeof v === "number" ? { integerValue: String(Math.trunc(v)) } : typeof v === "boolean" ? { booleanValue: v } : { stringValue: String(v) }
const fields = (o: Record<string, unknown>) => Object.fromEntries(Object.entries(o).map(([k, v]) => [k, enc(v)]))
const val = (f: any) => (f == null ? undefined : f.integerValue != null ? Number(f.integerValue) : f.stringValue ?? f.booleanValue)

async function getDoc(path: string): Promise<Fields | null> {
  const r = await fetch(`${FS}/${path}`, { headers: { authorization: `Bearer ${await verifierToken()}` } })
  if (r.status === 404) return null
  if (!r.ok) throw new Error(`get ${path}: ${r.status} ${await r.text()}`)
  const j: any = await r.json()
  return Object.fromEntries(Object.entries(j.fields ?? {}).map(([k, f]) => [k, val(f)]))
}
/** 여러 문서를 한 번에 (하나라도 조건이 안 맞으면 모두 안 써요) */
async function commit(writes: any[]): Promise<{ ok: boolean; status: number; text: string }> {
  const r = await fetch(`https://firestore.googleapis.com/v1/${DB}:commit`, {
    method: "POST",
    headers: { authorization: `Bearer ${await verifierToken()}`, "content-type": "application/json" },
    body: JSON.stringify({ writes }),
  })
  return { ok: r.ok, status: r.status, text: r.ok ? "" : await r.text() }
}
const put = (path: string, o: Record<string, unknown>, exists?: boolean) => ({
  update: { name: `${DB}/${path}`, fields: fields(o) },
  ...(exists === undefined ? {} : { currentDocument: { exists } }),
})
const del = (path: string) => ({ delete: `${DB}/${path}` })

/* ---------- 할 일 ---------- */

async function send(uid: string, phone: string) {
  const p = normalize(phone)
  if (!p) return fail("bad_phone")
  const h = sha(pepper() + p)
  if (await getDoc(`verified/${uid}`)) return reply(200, { ok: true, already: true })
  const owner = await getDoc(`phones/${h}`)
  if (owner && owner.uid !== uid) return fail("phone_used", 409)
  const now = Date.now()
  const today = day()
  const prev = await getDoc(`otp/${uid}`)
  if (prev && now - Number(prev.sent ?? 0) < RESEND_MS) return fail("too_soon", 429)
  const n = prev && prev.day === today ? Number(prev.n ?? 0) : 0
  if (n >= PER_DAY_USER) return fail("user_limit", 429)
  const all = await getDoc(`otpday/${today}`)
  const total = Number(all?.n ?? 0)
  if (total >= PER_DAY_ALL) return fail("daily_limit", 429)
  const code = String(randomInt(0, 1_000_000)).padStart(6, "0")
  const w = await commit([
    put(`otp/${uid}`, { h, c: sha(pepper() + uid + code), exp: now + CODE_MS, sent: now, day: today, n: n + 1, tries: 0 }),
    put(`otpday/${today}`, { n: total + 1 }),
  ])
  if (!w.ok) throw new Error("save otp: " + w.status + " " + w.text)
  try {
    await zavu.messages.send({ to: p, channel: "sms_oneway", text: `[자본주의] 인증번호 ${code}\n5분 안에 입력해 주세요.` })
  } catch (e: any) {
    console.log("sms failed", e?.status, e?.message)
    await commit([del(`otp/${uid}`)])
    return fail("sms_failed", 502)
  }
  return reply(200, { ok: true })
}

async function check(uid: string, phone: string, code: string) {
  const p = normalize(phone)
  if (!p) return fail("bad_phone")
  const h = sha(pepper() + p)
  const o = await getDoc(`otp/${uid}`)
  if (!o || o.h !== h) return fail("no_code")
  if (Date.now() > Number(o.exp)) return fail("expired")
  const tries = Number(o.tries ?? 0)
  if (tries >= MAX_TRIES) return fail("too_many", 429)
  if (!/^\d{6}$/.test(String(code ?? "")) || sha(pepper() + uid + code) !== o.c) {
    await commit([put(`otp/${uid}`, { ...o, tries: tries + 1 })])
    return fail("wrong_code")
  }
  const now = Date.now()
  const owner = await getDoc(`phones/${h}`)
  if (owner && owner.uid !== uid) return fail("phone_used", 409)
  const w = await commit([
    ...(owner ? [] : [put(`phones/${h}`, { uid, at: now }, false)]),
    put(`verified/${uid}`, { at: now, tail: p.slice(-4) }),
    del(`otp/${uid}`),
  ])
  if (!w.ok) return w.status === 400 || w.status === 409 || /ALREADY_EXISTS|FAILED_PRECONDITION/.test(w.text) ? fail("phone_used", 409) : fail("server", 500)
  return reply(200, { ok: true })
}

export default defineFunction({
  handler: async (event: any) => {
    const method = event?.requestContext?.http?.method
    if (method === "OPTIONS") return { statusCode: 204, headers: CORS, body: "" }
    if (method !== "POST") return fail("method", 405)
    let b: any = {}
    try {
      const raw = event.isBase64Encoded ? Buffer.from(event.body ?? "", "base64").toString("utf8") : event.body ?? ""
      b = JSON.parse(raw || "{}")
    } catch {
      return fail("bad_json")
    }
    try {
      if (b.op === "warmup") {
        await verifierToken()
        return reply(200, { ok: true })
      }
      const uid = await uidOf(b.idToken)
      if (!uid) return fail("not_signed_in", 401)
      if (b.op === "send") return await send(uid, b.phone)
      if (b.op === "check") return await check(uid, b.phone, b.code)
      return fail("bad_op")
    } catch (e: any) {
      console.log("error", e?.message)
      return fail("server", 500)
    }
  },
})
