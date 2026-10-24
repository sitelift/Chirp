# Cleanup Model v2 — Session Progress (2026-04-03)

## What we did tonight

### 1. Integrated chirp-cleanup model into Chirp (v1.3.0 branch)
- Created `src-tauri/src/t5.rs` — Rust module for CT2 model download, Python server management, cleanup API
- Created `src-tauri/ct2_server.py` — Python HTTP server wrapping CTranslate2
- Added `cleanup_model` setting (qwen vs chirp-cleanup) to Rust state + frontend
- Model selector dropdown in Settings under Smart Cleanup
- Auto-start correct backend on app boot
- Server restart when switching models
- Fixed Python discovery on Windows (scans LOCALAPPDATA)
- All on `v1.3.0` branch, 8 commits

### 2. Tested the current fine-tuned model — it's bad
The FLAN-T5-small model trained on v1 data aggressively summarizes instead of lightly polishing:
- "I don't know. I think our rejects pipeline..." → "Our rejection pipeline..." (dropped meaning)
- Long paragraphs compressed to single sentences
- Added words never spoken ("Thanks", "Thank you")

**Root cause:** The training data taught summarization. Examples like:
- "Okay so the partnership is moving forward..." → "Partnership moving forward." 
- "What I'm thinking is that we should probably test..." → "We should test..."

### 3. Brainstormed and designed v2 training approach
**Target: Level B — Light Polish**
- Fix spoken grammar (gonna → going to, me and X → X and I)
- Resolve self-corrections (keep only the corrected version)
- Remove stutters/repeated words
- Fix punctuation and question marks
- Normalize spoken numbers
- DO NOT summarize, restructure, or change the speaker's voice

**Data strategy decided:** Use the 11,656 real inputs from `training_pairs_clean.jsonl` (LARD + Disfl-QA datasets), relabel with Qwen 72B teacher on Modal using a strict minimal-edit prompt.

### 4. Built and ran the relabeling pipeline
**Script:** `training/relabel_t5.py`
- Takes real inputs, runs regex cleanup, sends to Qwen 72B-AWQ on Modal via vLLM
- Batches of 200 prompts per vLLM generate() call
- Validation: similarity >0.55-0.60, length ratio 0.50-1.50, content word preservation, no hallucination
- Saves raw outputs to .raw.jsonl for re-validation without GPU cost

**Results from test runs:**
- Quality is excellent — 19/20 pairs rated GOOD in deep review
- Edits are purely punctuation fixes, number formatting, light grammar — exactly Level B
- One duplication bug found (need dedup check in validation)

**Problems encountered:**
- 32B teacher is bad for this (adds 2x content, doesn't follow minimal-edit instructions)
- 72B works well but takes ~20 min for 3,000 prompts on L40S
- Multiple runs clobbered each other's output files (fixed with separate filenames + raw saves)
- Validation threshold was too strict at 1.3 length ratio (bumped to 1.5)

### 5. Identified remaining gaps

**Length distribution is the main problem:**
- Source data maxes out at 65 words, avg 25 words
- Only 46 pairs in the entire 11K dataset have 2+ issues
- Zero examples of 100-250 word dictation with multi-issue corrections
- Real user dictation is often 150-200 words

**Proposed solution: Semantic concatenation**
Instead of random stitching, combine pairs that share similar topics:
- Group by domain (meetings, budgets, projects, personal, technical)
- Combine 3-5 same-topic pairs into one coherent long example
- Both input and target get combined, preserving the edit quality
- This creates realistic long-form dictation without GPU cost

## What needs to happen next

### Immediate (next session)
1. **Finish the 72B relabel run** — rerun `relabel_t5.py` with raw output saving, get 1,500 validated pairs
2. **Build semantic concat script** — group pairs by topic/domain, combine for 75-250 word examples, target ~300 long pairs
3. **Add dedup validation** — catch duplicated output lines (bug found in pair #18)
4. **Merge dataset** — 1,500 relabeled + 300 semantic-concat long = ~1,800 pairs
5. **Audit 30 random pairs** — human review before training (non-negotiable)

### Training
6. **Train all three sizes** on Modal: FLAN-T5 small (77M), base (248M), large (783M)
7. **Benchmark all three** — speed + quality on 50 test transcripts
8. **Pick the best tradeoff** for production

### Integration  
9. **Update ct2_server.py** for the winning model
10. **Upload to HuggingFace** (sitelift/chirp-cleanup)
11. **Test in Chirp** with real dictation

## Files created/modified

### New files
- `training/generate_t5_v2.py` — Single-pass teacher generation (Approach A, not recommended)
- `training/relabel_t5.py` — Relabel real inputs with 72B teacher (Approach B, recommended)
- `training/concat_long_pairs.py` — Random concatenation for long examples (to be replaced with semantic version)
- `src-tauri/src/t5.rs` — Rust module for chirp-cleanup backend
- `src-tauri/ct2_server.py` — Python CTranslate2 HTTP server

### Modified files (on v1.3.0 branch)
- `src-tauri/Cargo.toml` — Version 1.3.0
- `src-tauri/src/state.rs` — Added cleanup_model setting
- `src-tauri/src/commands.rs` — Dispatch to t5 or llm based on setting
- `src-tauri/src/lib.rs` — Added mod t5, auto-start for chirp-cleanup
- `src/lib/constants.ts` — CLEANUP_MODELS array, cleanupModel default
- `src/stores/appStore.ts` — cleanupModel state
- `src/hooks/useSettingsSync.ts` — cleanupModel sync
- `src/components/settings/SettingsPage.tsx` — Model selector dropdown, server restart on switch

### Data files
- `training/data/training_pairs_clean.jsonl` — 11,656 real input pairs (source, inputs are good, outputs are bad)
- `training/data/training_t5_v2.jsonl` — 170 pairs from 32B run (not useful)
- `training/data/training_t5_v2_72b.jsonl` — 51 pairs from clobbered 72B run

## Key learnings
- **32B is not good enough** for minimal-edit cleanup instructions — it adds too much content
- **72B with enforce_eager=True** on L40S works reliably (0.95 gpu_memory_utilization)
- **Always save raw GPU outputs** separately so validation can be rerun without GPU cost
- **Always use separate output files** for concurrent runs
- **The source data (LARD/Disfl-QA) inputs are high quality** — only the targets need replacing
- **Training data quality > quantity** — 1,500 excellent pairs beats 10,000 mediocre ones
