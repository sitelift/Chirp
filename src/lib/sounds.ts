let audioCtx: AudioContext | null = null

function getAudioContext(): AudioContext {
  if (!audioCtx) {
    audioCtx = new AudioContext()
  }
  return audioCtx
}

export async function playCompletionSound() {
  const ctx = getAudioContext()
  // Tauri webview may have suspended AudioContext — resume it
  if (ctx.state === 'suspended') {
    await ctx.resume()
  }
  const osc = ctx.createOscillator()
  const gain = ctx.createGain()
  osc.connect(gain)
  gain.connect(ctx.destination)
  osc.type = 'sine'
  osc.frequency.setValueAtTime(880, ctx.currentTime) // A5
  osc.frequency.setValueAtTime(1174.66, ctx.currentTime + 0.08) // D6
  gain.gain.setValueAtTime(0.3, ctx.currentTime)
  gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.25)
  osc.start(ctx.currentTime)
  osc.stop(ctx.currentTime + 0.25)
}
