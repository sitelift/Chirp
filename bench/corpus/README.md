# Bench corpus

A fixed set of 45 dictations used to measure Chirp's latency and transcript quality (see "Measurement first" in `docs/v2/DIRECTION.md`).

- `manifest.json`: one entry per dictation: `id`, `category` (`chat`, `email`, `prompt`, `code`, `list`, `ramble`, `short`), `wav`, `spoken` (exactly what is said, fillers and self-corrections included), `ideal` (the best written output for that context), and `context` (`app` + window `title`, as the router and LLM would see them). Optional `tags`: `long` (30-45 s), `injection` (dictated text that looks like an instruction; the ideal is the cleaned text, not obeyed), `router-skip` (short enough that the LLM should be skipped).
- `wav/`: 16 kHz, 16-bit, mono PCM, rendered from `spoken` with offline Windows SAPI voices, with 300 ms of silence at each end.

**These files are fixed.** Don't edit an entry's `spoken`, `ideal` or audio casually: results are only comparable run to run if the corpus stays the same. If an entry is wrong, fix it in a dedicated commit and note that scores before and after aren't directly comparable.

## Regenerating the audio

```powershell
powershell -ExecutionPolicy Bypass -File bench/tools/render_corpus.ps1
powershell -ExecutionPolicy Bypass -File bench/tools/render_corpus.ps1 -Only chat-01,email-05
```

Voice and rate are picked from the entry's position in the manifest, so reruns on the same machine are byte-identical. Other machines with other installed voices produce different audio, so commit the WAVs rather than regenerating them elsewhere. Inserting or reordering entries changes the voice and rate of later entries: append new entries at the end.

## Human recordings

TTS audio is clean and evenly paced, so it flatters the speech model. Add real recordings with the same schema and an id prefixed `human-` (e.g. `human-email-01`, `wav/human-email-01.wav`, 16 kHz, 16-bit, mono). The render script never touches `human-` entries.
