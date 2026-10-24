# Bench corpus

A fixed set of dictations (45 general plus 8 name clips) used to measure Chirp's latency and transcript quality (see "Measurement first" in `docs/v2/DIRECTION.md`).

- `manifest.json`: one entry per dictation: `id`, `category` (`chat`, `email`, `prompt`, `code`, `list`, `ramble`, `short`, `names`), `wav`, `spoken` (exactly what is said, with real spellings, fillers and self-corrections included; used for WER and scoring), optional `tts` (the text actually sent to SAPI when it differs from `spoken`: a phonetic respelling so US voices say Dutch names the way the user does, e.g. "Peeter de Brewn" for Pieter de Bruijn, "Tice" for Thijs, "Dahn" for Daan), `ideal` (the best written output for that context), and `context` (`app`, plus `title` and browser `url` domain when the app would provide them, as the LLM sees them). Optional `tags`: `long` (30-45 s), `injection` (dictated text that looks like an instruction; the ideal is the cleaned text, not obeyed), `router-skip` (short enough that the LLM should be skipped).
- `wav/`: 16 kHz, 16-bit, mono PCM, rendered from `tts` (else `spoken`) with offline Windows SAPI voices, with 300 ms of silence at each end.
- `holdout.json`: ids held out as a test set; never tune the cleanup prompt against them.
- `dictionary.json`: a sample user dictionary (`term` + mishearings in `replaces`, the app's vocabulary shape) for the name clips. `node bench/haiku.mjs --dictionary bench/corpus/dictionary.json` sends the terms as the `<dictionary>` system block; `node bench/names.mjs <results>...` scores exact name spelling.

**These files are fixed.** Don't edit an entry's `spoken`, `ideal` or audio casually: results are only comparable run to run if the corpus stays the same. If an entry is wrong, fix it in a dedicated commit and note that scores before and after aren't directly comparable.

## Regenerating the audio

```powershell
powershell -ExecutionPolicy Bypass -File bench/tools/render_corpus.ps1
powershell -ExecutionPolicy Bypass -File bench/tools/render_corpus.ps1 -Only chat-01,email-05
```

Voice and rate are picked from the entry's position in the manifest, so reruns on the same machine are byte-identical. Other machines with other installed voices produce different audio, so commit the WAVs rather than regenerating them elsewhere. Inserting or reordering entries changes the voice and rate of later entries: append new entries at the end.

## Human recordings

TTS audio is clean and evenly paced, so it flatters the speech model, and respelled names only approximate how the user says them. Real recordings remain the gold standard: once `human-` entries exist they are the primary eval and the SAPI corpus is secondary (`bench/judge.mjs` reports `human` and `synthetic` groups separately). Add real recordings with the same schema and an id prefixed `human-` (e.g. `human-email-01`, `wav/human-email-01.wav`, 16 kHz, 16-bit, mono). The render script never touches `human-` entries.
