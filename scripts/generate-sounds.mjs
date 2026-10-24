// Generates Chirp's three earcons as small mono WAV files in src-tauri/sounds.
// Deterministic: the same script always writes the same bytes.
//
//   node scripts/generate-sounds.mjs
//
// start   rising sweep, 1.5 -> 2.9 kHz, 70 ms   (recording started)
// done    falling sweep, 2.9 -> 1.9 kHz, 60 ms  (text pasted)
// no-ai   two soft falling chirps, 86 ms        (pasted without AI cleanup)
//
// Each sweep is a sine with a faint second harmonic, gliding exponentially
// (even in pitch), with a quick rounded attack and a smooth decay to silence
// so there are no clicks. Peaks sit around -14 dBFS to stay gentle.

import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const RATE = 48_000
const OUT = join(dirname(fileURLToPath(import.meta.url)), '..', 'src-tauri', 'sounds')

/** One sweep, as float samples in [-1, 1]. */
function sweep({ from, to, ms, peak }) {
  const n = Math.round((RATE * ms) / 1000)
  const attack = Math.round(RATE * 0.005)
  const out = new Float64Array(n)
  let phase = 0
  for (let i = 0; i < n; i++) {
    const x = i / n
    const freq = from * Math.pow(to / from, x)
    phase += (2 * Math.PI * freq) / RATE
    // Rounded 5 ms attack, then a decay that lands exactly on zero.
    const rise = i < attack ? Math.sin((Math.PI / 2) * (i / attack)) ** 2 : 1
    const fall = Math.cos((Math.PI / 2) * x) ** 1.5
    const tone = Math.sin(phase) + 0.08 * Math.sin(2 * phase)
    out[i] = (peak / 1.08) * rise * fall * tone
  }
  return out
}

const silence = (ms) => new Float64Array(Math.round((RATE * ms) / 1000))

function concat(...parts) {
  const out = new Float64Array(parts.reduce((n, p) => n + p.length, 0))
  let at = 0
  for (const p of parts) {
    out.set(p, at)
    at += p.length
  }
  return out
}

/** 16-bit PCM mono WAV. */
function wav(samples) {
  const data = samples.length * 2
  const buf = Buffer.alloc(44 + data)
  buf.write('RIFF', 0, 'ascii')
  buf.writeUInt32LE(36 + data, 4)
  buf.write('WAVE', 8, 'ascii')
  buf.write('fmt ', 12, 'ascii')
  buf.writeUInt32LE(16, 16)
  buf.writeUInt16LE(1, 20) // PCM
  buf.writeUInt16LE(1, 22) // mono
  buf.writeUInt32LE(RATE, 24)
  buf.writeUInt32LE(RATE * 2, 28)
  buf.writeUInt16LE(2, 32)
  buf.writeUInt16LE(16, 34)
  buf.write('data', 36, 'ascii')
  buf.writeUInt32LE(data, 40)
  samples.forEach((s, i) => buf.writeInt16LE(Math.round(Math.max(-1, Math.min(1, s)) * 32767), 44 + i * 2))
  return buf
}

const PEAK = 0.2 // about -14 dBFS

const SOUNDS = {
  start: concat(sweep({ from: 1500, to: 2900, ms: 70, peak: PEAK }), silence(4)),
  done: concat(sweep({ from: 2900, to: 1900, ms: 60, peak: PEAK }), silence(4)),
  'no-ai': concat(
    sweep({ from: 2600, to: 1900, ms: 32, peak: PEAK * 0.85 }),
    silence(22),
    sweep({ from: 2600, to: 1900, ms: 32, peak: PEAK * 0.7 }),
    silence(4),
  ),
}

mkdirSync(OUT, { recursive: true })
for (const [name, samples] of Object.entries(SOUNDS)) {
  const file = join(OUT, `${name}.wav`)
  writeFileSync(file, wav(samples))
  console.log(`${file}  ${Math.round((samples.length / RATE) * 1000)} ms`)
}
