#!/usr/bin/env node
// Local files only. No network, subprocesses, automatic discovery/import or git operations.
import fs from 'node:fs';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { fileURLToPath } from 'node:url';

function validateWav(bytes, source) {
  if (bytes.toString('ascii', 0, 4) !== 'RIFF' || bytes.toString('ascii', 8, 12) !== 'WAVE') throw new Error(`Not a WAV: ${source}`);
  for (let offset = 12; offset + 8 <= bytes.length;) {
    const size = bytes.readUInt32LE(offset + 4);
    if (offset + 8 + size > bytes.length) throw new Error(`Truncated WAV: ${source}`);
    if (bytes.toString('ascii', offset, offset + 4) === 'fmt ') {
      if (size < 16) throw new Error(`Invalid WAV format: ${source}`);
      const format = bytes.readUInt16LE(offset + 8), channels = bytes.readUInt16LE(offset + 10);
      const rate = bytes.readUInt32LE(offset + 12), bits = bytes.readUInt16LE(offset + 22);
      if (channels !== 1 || rate !== 16000 || !((format === 1 && bits === 16) || (format === 3 && bits === 32))) throw new Error(`Expected 16kHz mono PCM16/float32: ${source}`);
      return;
    }
    offset += 8 + size + (size % 2);
  }
  throw new Error(`Missing WAV format: ${source}`);
}

export function isCompleteCapture(dir) {
  if (path.basename(dir).startsWith('.') && path.basename(dir).endsWith('.tmp')) return false;
  try {
    const meta = JSON.parse(fs.readFileSync(path.join(dir, 'meta.json'), 'utf8'));
    // Legacy captures used meta.json as their commit marker. Require all audio too.
    const files = ['audio.wav', ...(meta.asr_inputs ?? []).map(input => input.wav)];
    return files.every(file => typeof file === 'string' && path.basename(file) === file && fs.statSync(path.join(dir, file)).isFile());
  } catch (error) {
    if (error instanceof SyntaxError || error.code === 'ENOENT') return false;
    throw error;
  }
}

export function importCaptures({ captures, root, manifest, baseManifest, wavDir }) {
  if (!captures.length) throw new Error('Select at least one capture directory. Nothing was imported.');
  manifest = path.resolve(manifest);
  wavDir = path.resolve(wavDir);
  const existed = fs.existsSync(manifest);
  const original = fs.readFileSync(existed ? manifest : baseManifest);
  const source = original.toString('utf8');
  const entries = JSON.parse(source.replace(/^\uFEFF/, ''));
  if (!Array.isArray(entries)) throw new Error('Manifest must be a JSON array');
  const ids = new Set(entries.map(e => e.id)), pending = [], skipped = [];
  for (const selected of captures) {
    const dir = path.resolve(root, selected);
    if (!isCompleteCapture(dir)) { skipped.push(selected); continue; }
    const meta = JSON.parse(fs.readFileSync(path.join(dir, 'meta.json'), 'utf8'));
    if (meta.status !== 'pasted' || typeof meta.pasted_text !== 'string' || typeof meta.raw_asr_text !== 'string') throw new Error(`Capture has no completed paste/reference draft: ${dir}`);
    const id = `human-${path.basename(dir).toLowerCase().replace(/[^a-z0-9_-]/g, '-')}`;
    const audio = fs.readFileSync(path.join(dir, 'audio.wav'));
    validateWav(audio, dir);
    const dest = path.join(wavDir, `${id}.wav`);
    if (ids.has(id)) {
      const prior = entries.find(e => e.id === id);
      if (!prior || prior.source_capture?.id !== meta.id || !fs.existsSync(dest) || !fs.readFileSync(dest).equals(audio)) throw new Error(`Capture ID collision: ${id}`);
      skipped.push(id); continue;
    }
    if (fs.existsSync(dest)) throw new Error(`Refusing to overwrite WAV: ${dest}`);
    ids.add(id);
    pending.push({ dest, audio, entry: {
      id, category: 'human', wav: path.relative(path.dirname(manifest), dest).split(path.sep).join('/'),
      spoken: meta.raw_asr_text, ideal: meta.pasted_text, context: meta.context ?? {},
      dictionary_terms: meta.dictionary_terms ?? [],
      tags: ['human', 'draft-reference'], needs_review: true,
      source_capture: { id: meta.id, created_at: meta.created_at },
    }});
  }
  if (!pending.length) return { imported: [], skipped, manifest };
  // Insert before the original array's closing whitespace. Every original byte
  // remains in order; existing entries are never parsed and reserialized.
  let at = source.lastIndexOf(']');
  while (at > 0 && /\s/.test(source[at - 1])) at--;
  const added = pending.map(p => JSON.stringify(p.entry, null, 2).split('\n').map(l => `  ${l}`).join('\n')).join(',\n');
  const next = source.slice(0, at) + (entries.length ? ',' : '') + '\n' + added + source.slice(at);
  JSON.parse(next.replace(/^\uFEFF/, ''));
  fs.mkdirSync(wavDir, { recursive: true });
  fs.mkdirSync(path.dirname(manifest), { recursive: true });
  const created = [], temp = `${manifest}.${randomUUID()}.tmp`;
  try {
    for (const item of pending) {
      fs.writeFileSync(item.dest, item.audio, { flag: 'wx' });
      created.push(item.dest);
    }
    if (existed ? !fs.readFileSync(manifest).equals(original) : fs.existsSync(manifest)) throw new Error('Manifest changed during import; retry without concurrent edits');
    fs.writeFileSync(temp, next, { flag: 'wx' });
    fs.renameSync(temp, manifest);
  } catch (error) {
    // Only files exclusively created by this invocation; never recursive deletion.
    for (const file of created) fs.unlinkSync(file);
    if (fs.existsSync(temp)) fs.unlinkSync(temp);
    throw error;
  }
  return { imported: pending.map(p => p.entry.id), skipped, manifest };
}

function main() {
  const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
  const args = process.argv.slice(2), selected = [], options = {};
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--list') options.list = true;
    else if (['--root', '--manifest', '--base-manifest', '--wav-dir'].includes(args[i])) {
      const key = args[i].slice(2); if (!args[i + 1]) throw new Error(`Missing value: ${args[i]}`); options[key] = args[++i];
    } else if (args[i].startsWith('--')) throw new Error(`Unknown option: ${args[i]}`);
    else selected.push(args[i]);
  }
  const root = options.root || path.join(process.env.APPDATA || '', 'com.chirp.app/captures');
  if (options.list) {
    if (!fs.existsSync(root)) return console.log('No local captures.');
    for (const entry of fs.readdirSync(root, { withFileTypes: true })) if (entry.isDirectory() && isCompleteCapture(path.join(root, entry.name))) console.log(entry.name);
    return;
  }
  if (!selected.length) throw new Error('Usage: node bench/tools/import_captures.mjs --list\n       node bench/tools/import_captures.mjs <selected-folder> [...]');
  const manifest = options.manifest || path.join(repo, 'bench/corpus/manifest.local.json');
  const result = importCaptures({ captures: selected, root, manifest,
    baseManifest: options['base-manifest'] || path.join(repo, 'bench/corpus/manifest.json'),
    wavDir: options['wav-dir'] || path.join(path.dirname(path.resolve(manifest)), 'wav'),
  });
  console.log(JSON.stringify(result, null, 2));
  console.log('Local drafts only: correct spoken and ideal before using them as quality references.');
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { main(); } catch (error) { console.error(error.message); process.exitCode = 1; }
}
