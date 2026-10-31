// app/firebase.src.js → app/firebase.js 한 개로 묶어요 (파이어베이스 SDK를 구글 서버에서 받지 않고 이 사이트에서 직접 보내요)
import { build } from "esbuild";
import { fileURLToPath } from "node:url";
import path from "node:path";
const here = path.dirname(fileURLToPath(import.meta.url));
const result = await build({
  entryPoints: [path.join(here, "../app/firebase.src.js")],
  outfile: process.env.EMU ? path.join(here, "out/firebase.emu.js") : path.join(here, "../app/firebase.js"),
  define: { __EMU__: process.env.EMU ? "true" : "false" },
  bundle: true,
  minify: true,
  format: "esm",
  target: "es2020",
  legalComments: "none",
  nodePaths: [path.join(here, "node_modules")],
  metafile: true,
  logLevel: "info",
});
const out = Object.values(result.metafile.outputs)[0];
console.log("묶음 크기:", (out.bytes / 1024).toFixed(0), "KB");
