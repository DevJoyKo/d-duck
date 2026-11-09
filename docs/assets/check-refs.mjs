#!/usr/bin/env node
// Checks doc image references against docs/assets/shots.json (oss-publish-prep, spec "에셋과 현재 버전 일치").
//
// Usage:
//   node docs/assets/check-refs.mjs            # all md/html docs in the repo
//   node docs/assets/check-refs.mjs --public   # skip docs that .gitignore keeps out of the public repo
//   node docs/assets/check-refs.mjs --todo     # also print pendingDocEdits from shots.json
//   node docs/assets/check-refs.mjs --json     # machine-readable result
//
// Reports (exit code 1 when any of the first five is non-empty):
//   missing       a doc references an image file that does not exist
//   unlisted      a doc references a screen capture that shots.json does not list (nonCapture patterns excepted)
//   wrongLang     an English doc references a Korean capture, or a Korean doc an English one
//   stale         a PNG in a capture folder that no doc references (delete it, or reference it)
//   notTaken      shots.json lists a file that does not exist yet
//   unreferenced  shots.json lists a file no scanned doc references (status "proposed" is shown apart, as info)
//
// Doc language: README.md and *.en.* are English; README.ko.md, *.ko.* and docs/screen-spec/index.html are Korean;
// any other doc is Korean when more than 5% of its letters are Hangul.

import { readFileSync, readdirSync, existsSync, statSync } from "node:fs";
import { dirname, resolve, relative, join, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, "../..");
const SKIP_DIRS = new Set(["node_modules", "target", "dist", "dist-ssr", ".git", ".claude", "publish", "archive"]);
const CAPTURE_DIRS = ["docs/assets/screenshots", "docs/screen-spec/img", "docs/service-intro/video"];

const args = new Set(process.argv.slice(2));
const posix = (p) => p.split(sep).join("/");

function globToRegExp(g) {
  let s = "";
  for (let i = 0; i < g.length; i++) {
    const c = g[i];
    if (c === "*" && g[i + 1] === "*") (s += ".*"), i++;
    else if (c === "*") s += "[^/]*";
    else s += c.replace(/[.+?^${}()|[\]\\]/g, "\\$&");
  }
  return new RegExp(`^${s}$`);
}

function walk(dir, pick, out = []) {
  for (const name of readdirSync(dir)) {
    if (SKIP_DIRS.has(name)) continue;
    const p = join(dir, name);
    const st = statSync(p);
    if (st.isDirectory()) walk(p, pick, out);
    else if (pick(name)) out.push(posix(relative(ROOT, p)));
  }
  return out;
}

/** Paths .gitignore keeps out (git reads the nested .gitignore even when the enclosing repo is elsewhere). */
function ignoredSet(paths) {
  if (paths.length === 0) return new Set();
  try {
    const out = execFileSync("git", ["-C", ROOT, "check-ignore", "--no-index", "--stdin"], { input: paths.join("\n"), encoding: "utf8" });
    return new Set(out.split("\n").filter(Boolean).map((p) => posix(p)));
  } catch (e) {
    if (e.status === 1) return new Set(); // nothing ignored
    throw new Error(`git check-ignore failed: ${e.message}`);
  }
}

function docLang(doc, text) {
  const base = doc.split("/").pop();
  if (base === "README.md" && doc === "README.md") return "en";
  if (/\.en\.[a-z]+$/.test(base)) return "en";
  if (base === "README.ko.md" || /\.ko\.[a-z]+$/.test(base) || doc === "docs/screen-spec/index.html") return "ko";
  const letters = text.replace(/<[^>]*>|`[^`]*`|\([^)]*\)/g, "").match(/[A-Za-z가-힣]/g) ?? [];
  const hangul = letters.filter((c) => c >= "가").length;
  return letters.length && hangul / letters.length > 0.05 ? "ko" : "en";
}

function imageRefs(doc, text) {
  const refs = [];
  const add = (raw) => {
    const src = raw.trim().split(/\s+/)[0].replace(/^<|>$/g, "");
    if (!/\.(png|jpe?g|gif|webp|svg)$/i.test(src.split(/[?#]/)[0]) || /^(https?:|data:|\/\/)/i.test(src)) return;
    const file = posix(relative(ROOT, resolve(ROOT, dirname(doc), decodeURI(src.split(/[?#]/)[0]))));
    refs.push(file);
  };
  for (const m of text.matchAll(/<img\b[^>]*?\bsrc\s*=\s*["']([^"']+)["']/gi)) add(m[1]);
  for (const m of text.matchAll(/!\[[^\]]*\]\(([^)]+)\)/g)) add(m[1]);
  return refs;
}

const list = JSON.parse(readFileSync(join(HERE, "shots.json"), "utf8"));
const nonCapture = (list.nonCapture?.patterns ?? []).map(globToRegExp);
const isNonCapture = (f) => nonCapture.some((r) => r.test(f));
const shotByFile = new Map(list.shots.map((s) => [s.file, s]));

let docs = walk(ROOT, (n) => /\.(md|html)$/i.test(n));
const ignored = ignoredSet(docs);
if (args.has("--public")) docs = docs.filter((d) => !ignored.has(d));

const refsByFile = new Map(); // image → [{doc, lang}]
const r = { scanned: docs.length, missing: [], unlisted: [], wrongLang: [], stale: [], notTaken: [], unreferenced: [], proposed: [] };
for (const doc of docs) {
  const text = readFileSync(join(ROOT, doc), "utf8");
  const lang = docLang(doc, text);
  for (const file of new Set(imageRefs(doc, text))) {
    if (!refsByFile.has(file)) refsByFile.set(file, []);
    refsByFile.get(file).push({ doc, lang });
    if (!existsSync(join(ROOT, file))) r.missing.push({ doc, file });
    if (isNonCapture(file)) continue;
    const shot = shotByFile.get(file);
    if (!shot) r.unlisted.push({ doc, file });
    else if ((shot.lang === "ko" || shot.lang === "en") && shot.lang !== lang) r.wrongLang.push({ doc, docLang: lang, file, shotLang: shot.lang });
  }
}

// When --public, docs that stay private do not count as references.
for (const s of list.shots) {
  if (!existsSync(join(ROOT, s.file))) r.notTaken.push(s.file);
  if (refsByFile.has(s.file)) continue;
  const hidden = args.has("--public") && (s.docs ?? []).length > 0 && s.docs.every((d) => ignored.has(d));
  if (hidden) continue;
  (s.status === "proposed" ? r.proposed : r.unreferenced).push(s.file);
}
for (const dir of CAPTURE_DIRS) {
  if (!existsSync(join(ROOT, dir))) continue;
  for (const f of walk(join(ROOT, dir), (n) => /\.png$/i.test(n))) {
    if (refsByFile.has(f)) continue;
    const s = shotByFile.get(f);
    if (s?.status === "proposed" || s?.kind === "video") continue;
    r.stale.push(f);
  }
}
if (args.has("--public")) {
  // A capture under a private path (e.g. docs/service-intro/) is not published, so it cannot be stale there.
  const extra = ignoredSet(CAPTURE_DIRS.flatMap((d) => (existsSync(join(ROOT, d)) ? walk(join(ROOT, d), (n) => /\.png$/i.test(n)) : [])));
  r.stale = r.stale.filter((f) => !extra.has(f));
}

if (args.has("--json")) {
  console.log(JSON.stringify(r, null, 1));
} else {
  const show = (title, rows, fmt) => {
    console.log(`\n${title}: ${rows.length}`);
    for (const x of rows.slice(0, 80)) console.log(`  ${fmt(x)}`);
    if (rows.length > 80) console.log(`  … ${rows.length - 80} more`);
  };
  console.log(`docs scanned: ${r.scanned}${args.has("--public") ? " (public only)" : ""}, shots listed: ${list.shots.length}`);
  show("missing (referenced, file absent)", r.missing, (x) => `${x.doc} → ${x.file}`);
  show("unlisted (referenced, not in shots.json)", r.unlisted, (x) => `${x.doc} → ${x.file}`);
  show("wrongLang (doc language ≠ capture language)", r.wrongLang, (x) => `${x.doc} [${x.docLang}] → ${x.file} [${x.shotLang}]`);
  show("stale (capture folder PNG nobody references)", r.stale, (x) => x);
  show("notTaken (listed, file absent)", r.notTaken, (x) => x);
  show("unreferenced (listed, no doc uses it)", r.unreferenced, (x) => x);
  show("proposed (listed for a future doc, info)", r.proposed, (x) => x);
  if (args.has("--todo")) show("pendingDocEdits", list.pendingDocEdits ?? [], (x) => `[${x.task}] ${x.doc}: ${x.edit}`);
}
const bad = r.missing.length + r.unlisted.length + r.wrongLang.length + r.stale.length + r.notTaken.length;
process.exitCode = bad ? 1 : 0;
