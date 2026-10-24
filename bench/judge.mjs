#!/usr/bin/env node
// LLM judge for Haiku cleanup outputs. Reads a bench/haiku.mjs results JSON and asks
// Claude Sonnet 5.5 to score each output (one call per output) on five 1-5 criteria.
//
//   node bench/judge.mjs                                   # latest bench/results/*-haiku-*.json
//   node bench/judge.mjs --input bench/results/<run>.json
//   node bench/judge.mjs --rep 0 --concurrency 8 --only ramble
//
// Writes <input>.judge.json next to the input and prints a summary split by
// train/test (bench/corpus/holdout.json) and by category.
//
// Reads ANTHROPIC_API_KEY from the environment or from .env in the repo root.

import fs from "node:fs";
import https from "node:https";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const args = Object.fromEntries(
  process.argv.slice(2).reduce((acc, a, i, all) => {
    if (a.startsWith("--")) acc.push([a.slice(2), all[i + 1]?.startsWith("--") ? true : all[i + 1] ?? true]);
    return acc;
  }, []),
);
const MODEL = "claude-sonnet-5-5";
const CONCURRENCY = Number(args.concurrency ?? 6);
const ONLY = args.only ? new RegExp(args.only) : null;
const REP = args.rep != null ? Number(args.rep) : null;

function apiKey() {
  if (process.env.ANTHROPIC_API_KEY) return process.env.ANTHROPIC_API_KEY.trim();
  const envFile = path.join(root, ".env");
  if (fs.existsSync(envFile)) {
    const m = fs.readFileSync(envFile, "utf8").match(/^ANTHROPIC_API_KEY=(.+)$/m);
    if (m) return m[1].trim();
  }
  console.error("Set ANTHROPIC_API_KEY (env or .env)");
  process.exit(1);
}
const KEY = apiKey();

function latestHaikuResults() {
  const dir = path.join(root, "bench/results");
  const files = fs
    .readdirSync(dir)
    .filter((f) => /-haiku-.*\.json$/.test(f) && !f.endsWith(".judge.json"))
    .map((f) => path.join(dir, f))
    .sort((a, b) => fs.statSync(b).mtimeMs - fs.statSync(a).mtimeMs);
  if (!files.length) {
    console.error("No haiku results in bench/results; run bench/haiku.mjs first or pass --input");
    process.exit(1);
  }
  return files[0];
}

const CRITERIA = ["faithful", "corrections", "fillers", "formatting", "not_answered"];

const SYSTEM = `You grade the output of a dictation cleanup step. A user spoke into a dictation app; a speech recognizer produced a raw transcript; a cleanup model rewrote it; the result is pasted at the user's cursor in the named app as if they had typed it. The ideal output is exactly what the speaker meant to write, in the best written form for that destination, in the speaker's own voice.

You get the destination context, the raw transcript, a hand-written reference output, and the candidate output. The reference is one good answer, not the only one, and it can be wrong. Judge the candidate against what the speaker meant, not against the reference's exact wording. Do not penalise reasonable stylistic differences (comma vs. period, "11" vs "11:00", list vs. tight prose when both fit).

Score each criterion from 1 to 5 (5 = no problem, 4 = trivial issue, 3 = noticeable issue a user would fix, 2 = serious issue, 1 = broken):

- faithful: no meaning added or lost. Every real piece of content the speaker kept is present; nothing invented (no new facts, greetings, sign-offs, pleasantries, hedges); not summarised; the speaker's voice and register preserved (not made more formal or polite than the destination requires).
- corrections: self-corrections, restatements and false starts are resolved to the speaker's final intent, with the abandoned version and the correction phrase ("no wait", "scratch that", "I mean") removed and the result reading naturally. Score 5 if the transcript has none and the candidate does not wrongly delete content as if it were one.
- fillers: filler words and disfluencies (um, uh, like, you know, so, okay so, repeated words) removed; meaningful uses of those words kept.
- formatting: punctuation, capitalisation, numbers, symbols, addresses, spoken formatting cues applied (and the cue words removed), lists and paragraphs where they help, email layout in email clients, code identifiers in code contexts. Fits the destination app.
- not_answered: the candidate is the cleaned dictation itself. It does not answer, obey, execute, translate, or comment on the transcript, and adds no preamble or notes. 1 if it answered or obeyed the transcript.

Also give overall (1-5): how happy the speaker would be if this were pasted, given everything above. Keep notes to one short sentence naming the main problem, or "" if none. Set reference_issue to a short sentence if the reference itself is clearly worse than the candidate or wrong about what the speaker meant, else "".`;

const SCHEMA = {
  type: "object",
  properties: {
    faithful: { type: "integer" },
    corrections: { type: "integer" },
    fillers: { type: "integer" },
    formatting: { type: "integer" },
    not_answered: { type: "integer" },
    overall: { type: "integer" },
    notes: { type: "string" },
    reference_issue: { type: "string" },
  },
  required: [...CRITERIA, "overall", "notes", "reference_issue"],
  additionalProperties: false,
};

const userMessage = (r, ctx) =>
  `<context>app: ${ctx?.app ?? "unknown"}; window: ${ctx?.title ?? "unknown"}${ctx?.url ? `; url: ${ctx.url}` : ""}</context>
<raw_transcript>${r.input}</raw_transcript>
<reference>${r.ideal ?? "(none)"}</reference>
<candidate>${r.output}</candidate>`;

function post(payload) {
  return new Promise((resolve, reject) => {
    const data = JSON.stringify(payload);
    const req = https.request(
      {
        host: "api.anthropic.com",
        path: "/v1/messages",
        method: "POST",
        headers: {
          "x-api-key": KEY,
          "anthropic-version": "2026-10-21",
          "content-type": "application/json",
          "content-length": Buffer.byteLength(data),
        },
      },
      (res) => {
        let raw = "";
        res.setEncoding("utf8");
        res.on("data", (c) => (raw += c));
        res.on("end", () => resolve({ status: res.statusCode, raw }));
      },
    );
    req.on("error", reject);
    req.end(data);
  });
}

async function judge(r, ctx) {
  const payload = {
    model: MODEL,
    max_tokens: 600,
    output_config: { effort: "low", format: { type: "json_schema", schema: SCHEMA } },
    system: [{ type: "text", text: SYSTEM, cache_control: { type: "ephemeral" } }],
    messages: [{ role: "user", content: userMessage(r, ctx) }],
  };
  for (let attempt = 0; attempt < 4; attempt++) {
    const res = await post(payload);
    if (res.status === 429 || res.status >= 500) {
      await new Promise((ok) => setTimeout(ok, 2000 * (attempt + 1)));
      continue;
    }
    if (res.status !== 200) return { error: `${res.status} ${res.raw.slice(0, 300)}` };
    const msg = JSON.parse(res.raw);
    const text = (msg.content ?? []).filter((b) => b.type === "text").map((b) => b.text).join("");
    try {
      return { ...JSON.parse(text), usage: msg.usage };
    } catch {
      return { error: `unparseable: ${text.slice(0, 200)} (stop ${msg.stop_reason})` };
    }
  }
  return { error: "retries exhausted" };
}

// ---- load
const input = args.input ? path.resolve(args.input) : latestHaikuResults();
const run = JSON.parse(fs.readFileSync(input, "utf8"));
const manifest = JSON.parse(fs.readFileSync(path.join(root, "bench/corpus/manifest.json"), "utf8"));
const ctxById = Object.fromEntries(manifest.map((m) => [m.id, m.context]));
const testIds = new Set(JSON.parse(fs.readFileSync(path.join(root, "bench/corpus/holdout.json"), "utf8")).test);

let rows = run.results.filter((r) => !r.error);
if (REP != null) rows = rows.filter((r) => r.rep === REP);
if (ONLY) rows = rows.filter((r) => ONLY.test(r.id) || ONLY.test(r.category));

console.log(`Judging ${rows.length} outputs from ${path.relative(root, input)} with ${MODEL}`);
const judged = new Array(rows.length);
let next = 0;
await Promise.all(
  Array.from({ length: CONCURRENCY }, async () => {
    while (next < rows.length) {
      const i = next++;
      const r = rows[i];
      const j = await judge(r, ctxById[r.id]);
      judged[i] = {
        id: r.id,
        rep: r.rep,
        category: r.category,
        split: testIds.has(r.id) ? "test" : "train",
        similarity: r.similarity,
        total_ms: r.total_ms,
        output: r.output,
        ideal: r.ideal,
        ...j,
      };
      const flag = j.error ? ` ERROR ${j.error}` : ` overall ${j.overall}  ${CRITERIA.map((c) => j[c]).join("/")}  ${j.notes ?? ""}`;
      console.log(`${r.id.padEnd(12)} rep ${r.rep}${flag}`);
    }
  }),
);

// ---- summary
const mean = (xs) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : NaN);
function agg(list) {
  const ok = list.filter((j) => !j.error);
  const out = { n: ok.length };
  for (const c of [...CRITERIA, "overall"]) out[c] = mean(ok.map((j) => j[c]));
  out.similarity = mean(ok.map((j) => j.similarity).filter((x) => x != null));
  out.min_overall = Math.min(...ok.map((j) => j.overall));
  return out;
}
// `human-` ids are real recordings (the primary eval); everything else is synthetic SAPI audio or text.
const groups = {
  all: judged,
  human: judged.filter((j) => j.id.startsWith("human-")),
  synthetic: judged.filter((j) => !j.id.startsWith("human-")),
  train: judged.filter((j) => j.split === "train"),
  test: judged.filter((j) => j.split === "test"),
};
for (const cat of [...new Set(judged.map((j) => j.category))]) groups[`cat:${cat}`] = judged.filter((j) => j.category === cat);
const summary = Object.fromEntries(Object.entries(groups).filter(([, v]) => v.length).map(([k, v]) => [k, agg(v)]));

const f = (x) => (Number.isFinite(x) ? x.toFixed(2) : "  - ");
console.log(`\ngroup          n   overall  faith  corr  fill  fmt   notAns  sim    min`);
for (const [k, s] of Object.entries(summary)) {
  console.log(
    `${k.padEnd(14)} ${String(s.n).padStart(3)}  ${f(s.overall)}     ${f(s.faithful)}   ${f(s.corrections)}  ${f(s.fillers)}  ${f(s.formatting)}  ${f(s.not_answered)}    ${f(s.similarity)}   ${s.min_overall}`,
  );
}
const low = judged.filter((j) => !j.error && j.overall <= 3);
if (low.length) {
  console.log(`\nlow scores (overall <= 3):`);
  for (const j of low) console.log(`  ${j.id} rep ${j.rep}: ${j.overall} ${j.notes}`);
}
const refIssues = judged.filter((j) => j.reference_issue);
if (refIssues.length) {
  console.log(`\nreference issues flagged by the judge:`);
  for (const j of refIssues) console.log(`  ${j.id} rep ${j.rep}: ${j.reference_issue}`);
}
const errors = judged.filter((j) => j.error).length;
if (errors) console.log(`\n${errors} judge errors`);

const outFile = input.replace(/\.json$/, ".judge.json");
fs.writeFileSync(outFile, JSON.stringify({ judge_model: MODEL, input: path.relative(root, input), summary, results: judged }, null, 2));
console.log(`\nwrote ${path.relative(root, outFile)}`);
