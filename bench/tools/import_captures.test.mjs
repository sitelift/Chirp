import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { importCaptures } from './import_captures.mjs';

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'chirp-import-test-'));
  t.after(() => {
    const resolved=fs.realpathSync(root), temp=fs.realpathSync(os.tmpdir());
    assert.equal(path.dirname(resolved),temp);
    assert.ok(path.basename(resolved).startsWith('chirp-import-test-'));
    fs.rmSync(resolved, { recursive: true, force: true });
  });
  const baseManifest = path.join(root, 'manifest.json');
  const original = '[\r\n  { "id": "existing", "spoken": "literal \\u00e9", "ideal": "keep spacing" }\r\n]\r\n';
  fs.writeFileSync(baseManifest, original);
  const dir = path.join(root, '20261024-test'); fs.mkdirSync(dir);
  const audio = Buffer.alloc(48);
  audio.write('RIFF'); audio.writeUInt32LE(40, 4); audio.write('WAVEfmt ', 8); audio.writeUInt32LE(16, 16);
  audio.writeUInt16LE(3, 20); audio.writeUInt16LE(1, 22); audio.writeUInt32LE(16000, 24);
  audio.writeUInt32LE(64000, 28); audio.writeUInt16LE(4, 32); audio.writeUInt16LE(32, 34);
  audio.write('data', 36); audio.writeUInt32LE(4, 40); audio.writeFloatLE(0.12345679, 44);
  fs.writeFileSync(path.join(dir, 'audio.wav'), audio);
  fs.writeFileSync(path.join(dir, 'meta.json'), JSON.stringify({ status:'pasted', id:'test', created_at:'now', raw_asr_text:'Peter de Brown', pasted_text:'Pieter de Bruijn', dictionary_terms:['Pieter de Bruijn'], context:{app:'Slack',title:'DM'} }));
  return { root, dir, original, audio, baseManifest, manifest:path.join(root,'manifest.local.json'), wavDir:path.join(root,'wav'), captures:['20261024-test'] };
}
test('append preserves every original byte and copies float audio without requantizing', t => {
  const f = fixture(t); const result = importCaptures(f);
  assert.equal(fs.readFileSync(f.baseManifest,'utf8'), f.original);
  const next = fs.readFileSync(f.manifest,'utf8');
  let at=f.original.lastIndexOf(']'); while(/\s/.test(f.original[at-1])) at--;
  assert.equal(next.slice(0,at)+next.slice(at+next.length-f.original.length),f.original);
  const entry=JSON.parse(next).at(-1);
  assert.equal(entry.spoken,'Peter de Brown'); assert.equal(entry.ideal,'Pieter de Bruijn');
  assert.equal(entry.needs_review,true); assert.deepEqual(entry.dictionary_terms,['Pieter de Bruijn']);
  assert.deepEqual(fs.readFileSync(path.join(f.wavDir,`${result.imported[0]}.wav`)),f.audio);
  assert.deepEqual(importCaptures(f).skipped,result.imported);
  assert.equal(fs.readFileSync(f.manifest,'utf8'),next);
});
test('invalid or unpasted selection cannot partially import a batch', t => {
  const f=fixture(t), bad=path.join(f.root,'bad'); fs.mkdirSync(bad);
  fs.writeFileSync(path.join(bad,'meta.json'),JSON.stringify({status:'failed'}));
  fs.writeFileSync(path.join(bad,'audio.wav'),f.audio);
  assert.throws(()=>importCaptures({...f,captures:[...f.captures,'bad']}),/no completed paste/);
  assert.equal(fs.existsSync(f.manifest),false); assert.equal(fs.existsSync(f.wavDir),false);
});
test('partial folders are skipped even with metadata, without creating a manifest', t => {
  const f = fixture(t);
  const temp = path.join(f.root, '.capture.tmp');
  fs.renameSync(f.dir, temp);
  assert.deepEqual(importCaptures({...f, captures:['.capture.tmp']}).skipped, ['.capture.tmp']);
  assert.equal(fs.existsSync(f.manifest), false);
  fs.renameSync(temp, f.dir);
  fs.unlinkSync(path.join(f.dir, 'audio.wav'));
  assert.deepEqual(importCaptures(f).skipped, f.captures);
  fs.writeFileSync(path.join(f.dir, 'audio.wav'), f.audio);
  fs.unlinkSync(path.join(f.dir, 'meta.json'));
  assert.deepEqual(importCaptures(f).skipped, f.captures);
  assert.equal(fs.existsSync(f.manifest), false);
});
test('missing ASR inputs and interrupted metadata are incomplete', t => {
  const f = fixture(t);
  fs.writeFileSync(path.join(f.dir, 'meta.json'), JSON.stringify({status:'pasted', asr_inputs:[{wav:'asr-input-000.wav'}]}));
  assert.deepEqual(importCaptures(f).skipped, f.captures);
  fs.writeFileSync(path.join(f.dir, 'meta.json'), '{"status":');
  assert.deepEqual(importCaptures(f).skipped, f.captures);
  assert.equal(fs.existsSync(f.manifest), false);
});
