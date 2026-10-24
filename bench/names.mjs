#!/usr/bin/env node
// Name accuracy: how often dictionary terms come out spelled exactly as listed.
//
//   node bench/names.mjs bench/results/a.json bench/results/b.json ...
//
// Each file is either a haiku.mjs results JSON (scores `output`) or a plain
// [{id, text}] list (scores `text`, e.g. the deterministic pass). Expected
// counts come from each item's `ideal` in the manifest; terms are matched
// case-sensitively, longest first, so "Pieter" inside "Pieter de Bruijn" is
// not counted twice.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifest = JSON.parse(fs.readFileSync(path.join(root, "bench/corpus/manifest.json"), "utf8"));
const ideal = Object.fromEntries(manifest.map((m) => [m.id, m.ideal]));
const terms = JSON.parse(fs.readFileSync(path.join(root, "bench/corpus/dictionary.json"), "utf8"))
  .map((e) => e.term)
  .sort((a, b) => b.length - a.length);

function counts(text) {
  const out = {};
  let rest = text;
  for (const t of terms) {
    const re = new RegExp(`(?<![\\p{L}\\p{N}])${t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}(?![\\p{L}\\p{N}])`, "gu");
    out[t] = (rest.match(re) ?? []).length;
    rest = rest.replace(re, " ");
  }
  return out;
}

console.log("file".padEnd(34) + "names   exact   inserted   items fully right");
for (const f of process.argv.slice(2)) {
  const data = JSON.parse(fs.readFileSync(path.resolve(f), "utf8"));
  const rows = (Array.isArray(data) ? data : data.results).filter((r) => r.id in ideal && !r.error);
  let want = 0, got = 0, extra = 0, full = 0;
  const misses = {};
  for (const r of rows) {
    const e = counts(ideal[r.id]);
    const o = counts(r.output ?? r.text);
    let ok = true;
    for (const t of terms) {
      want += e[t];
      got += Math.min(e[t], o[t]);
      extra += Math.max(0, o[t] - e[t]);
      if (o[t] !== e[t]) {
        ok = false;
        if (o[t] < e[t]) misses[t] = (misses[t] ?? 0) + e[t] - o[t];
      }
    }
    if (ok) full++;
  }
  const pct = want ? ((100 * got) / want).toFixed(0) : "-";
  console.log(
    `${path.basename(f).padEnd(34)}${String(want).padStart(5)}   ${String(pct + "%").padStart(5)}   ${String(extra).padStart(8)}   ${full}/${rows.length}` +
      (Object.keys(misses).length ? `   missed: ${Object.entries(misses).map(([t, n]) => `${t} x${n}`).join(", ")}` : ""),
  );
}
