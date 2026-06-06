# Chirp Cleanup Model Integration Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate the fine-tuned FLAN-T5-small "chirp-cleanup" model as an alternative AI cleanup backend, with a settings toggle to switch between Qwen (llama-server) and chirp-cleanup (ct2_server.py).

**Architecture:** The new model runs via `ct2_server.py` — a minimal Python HTTP server wrapping CTranslate2. It follows the exact same subprocess + HTTP pattern as the existing llama-server approach. The Rust backend dispatches to either server based on a `cleanup_model` setting. The frontend gets a model selector dropdown in the Smart Cleanup section.

**Tech Stack:** CTranslate2 (Python), FLAN-T5-small fine-tuned model (78MB CT2 INT8), HTTP API on localhost

---

## Context

### What exists today
- `src-tauri/src/llm.rs` — Downloads llama-server binary + Qwen 3B GGUF model, starts subprocess, sends HTTP requests to `/v1/chat/completions` with chat format + datamarking + JSON schema, parses JSON response
- `src-tauri/src/commands.rs:483-500` — AI cleanup dispatch: checks `ai_cleanup` bool + `llm_port`, calls `llm::cleanup_text(port, &formatted, &tone_mode)`
- `src-tauri/src/state.rs` — `Settings.ai_cleanup: bool`, `AppState.llm_process`, `AppState.llm_port`
- `src/lib/constants.ts` — `LLM_MODEL` config, `TONE_MODES`
- `src/components/settings/SettingsPage.tsx:505-555` — Smart Cleanup toggle + Tone selector

### What the new model needs
- **Server:** `ct2_server.py` (already written in `training/ct2_server.py`)
  - `GET /health` → `{"status": "ok"}`
  - `POST /v1/completions` with `{"text": "..."}` → `{"text": "cleaned text"}`
- **Model:** CT2 INT8 format, 78MB, downloaded from `sitelift/chirp-cleanup` on HuggingFace
  - 3 files: `model.bin` (75MB), `config.json`, `shared_vocabulary.json`
- **Tokenizer:** Uses `google/flan-t5-small` tokenizer (downloaded by transformers at first run)
- **Prefix:** Input text is prefixed with `"Rewrite as typed text: "` inside ct2_server.py — Rust side just sends raw text
- **No datamarking, no JSON schema, no system prompt, no chat format** — the T5 model takes plain text and returns plain text

### Key differences from Qwen path

| Aspect | Qwen (llama-server) | chirp-cleanup (ct2_server) |
|---|---|---|
| Binary | `llama-server.exe` (~15MB + DLLs) | Python + ctranslate2 package |
| Model | `qwen2.5-3b-instruct-q4_k_m.gguf` (2.1GB) | 3 files totaling 78MB |
| Download source | GitHub releases + HuggingFace | HuggingFace only |
| Request format | Chat completion + datamarking + JSON schema | `{"text": "plain text"}` |
| Response format | JSON `{"cleaned_text": "..."}` | JSON `{"text": "..."}` |
| Speed (CPU) | ~1500ms p50 | ~103ms p50 |
| Speed (GPU) | ~118ms p50 | ~58ms p50 |

### Python dependency
The ct2_server.py requires Python + `ctranslate2` + `transformers` packages. For dev/testing this is fine (Python is already on both machines). For production release, this would be packaged as a standalone executable via PyInstaller or replaced with native Rust integration via `ct2rs` crate. This plan covers the dev/testing integration only.

---

## File Structure

**Create:**
- `src-tauri/src/t5.rs` — New module for chirp-cleanup model management (download, start/stop ct2_server, cleanup_text)
- `src-tauri/ct2_server.py` — Copy from `training/ct2_server.py` (the Python HTTP server)

**Modify:**
- `src-tauri/src/state.rs` — Add `cleanup_model: String` to Settings
- `src-tauri/src/commands.rs` — Dispatch AI cleanup to either `llm::cleanup_text` or `t5::cleanup_text` based on setting
- `src-tauri/src/main.rs` — Add `mod t5;`
- `src/lib/constants.ts` — Add `CLEANUP_MODELS` config, update `LLM_MODEL`
- `src/stores/appStore.ts` — Add `cleanupModel` to store
- `src/components/settings/SettingsPage.tsx` — Add model selector dropdown under Smart Cleanup toggle
- `src/hooks/useSettingsSync.ts` — Add `cleanupModel` to synced keys
- `src-tauri/Cargo.toml` — Version bump to 1.3.0

---

### Task 1: Copy ct2_server.py and version bump

**Files:**
- Copy: `training/ct2_server.py` → `src-tauri/ct2_server.py`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Copy the server script**

```bash
cp training/ct2_server.py src-tauri/ct2_server.py
```

- [ ] **Step 2: Bump version to 1.3.0**

In `src-tauri/Cargo.toml`, change:
```toml
version = "1.3.0"
```

- [ ] **Step 3: Create v1.3 branch and commit**

```bash
git checkout -b v1.3.0
git add src-tauri/ct2_server.py src-tauri/Cargo.toml
git commit -m "feat: add ct2_server.py for chirp-cleanup model"
```

---

### Task 2: Add `cleanup_model` to Settings (Rust)

**Files:**
- Modify: `src-tauri/src/state.rs`

- [ ] **Step 1: Add field to Settings struct**

In `src-tauri/src/state.rs`, add to the `Settings` struct after the `ai_cleanup` field:

```rust
    #[serde(default = "default_cleanup_model")]
    pub cleanup_model: String,
```

Add the default function:

```rust
fn default_cleanup_model() -> String {
    "qwen".into()
}
```

Update `Default for Settings`:

```rust
            cleanup_model: "qwen".into(),
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/state.rs
git commit -m "feat: add cleanup_model setting (qwen vs chirp-cleanup)"
```

---

### Task 3: Create `t5.rs` module

**Files:**
- Create: `src-tauri/src/t5.rs`
- Modify: `src-tauri/src/main.rs` (add `mod t5;`)

This module mirrors `llm.rs` structure: model download, server start/stop, cleanup_text. But simpler — no binary download (uses system Python), no datamarking, no JSON schema.

- [ ] **Step 1: Create `src-tauri/src/t5.rs`**

```rust
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

use crate::settings;

/// CT2 model files from HuggingFace
const MODEL_REPO: &str = "sitelift/chirp-cleanup";
const MODEL_FILES: &[(&str, &str)] = &[
    ("ct2-int8/model.bin", "model.bin"),
    ("ct2-int8/config.json", "config.json"),
    ("ct2-int8/shared_vocabulary.json", "shared_vocabulary.json"),
];
const MODEL_TOTAL_SIZE: u64 = 78_000_000;

/// Directory for chirp-cleanup files: %APPDATA%/com.chirp.app/chirp-cleanup/
pub fn model_dir() -> PathBuf {
    settings::config_dir().join("chirp-cleanup")
}

fn server_script_path() -> PathBuf {
    // In dev, ct2_server.py is next to the Cargo.toml
    // In production, it would be bundled as a resource
    let dev_path = std::env::current_dir()
        .unwrap_or_default()
        .join("ct2_server.py");
    if dev_path.exists() {
        return dev_path;
    }
    // Fallback: look next to the executable
    std::env::current_exe()
        .unwrap_or_default()
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("ct2_server.py")
}

/// Check if all model files exist
pub fn model_exists() -> bool {
    let dir = model_dir();
    MODEL_FILES.iter().all(|(_, local)| dir.join(local).exists())
}

/// Download model files from HuggingFace
pub async fn download_model(app_handle: &AppHandle) -> Result<(), String> {
    let dir = model_dir();
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("Failed to create model dir: {e}"))?;

    if model_exists() {
        return Ok(());
    }

    let client = reqwest::Client::new();
    let mut downloaded_total: u64 = 0;

    for (remote_path, local_name) in MODEL_FILES {
        let dest = dir.join(local_name);
        if dest.exists() {
            continue;
        }

        let url = format!(
            "https://huggingface.co/{}/resolve/main/{}",
            MODEL_REPO, remote_path
        );

        let tmp_path = dir.join(format!("{local_name}.tmp"));

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Download failed for {local_name}: {e}"))?;

        if !response.status().is_success() {
            return Err(format!("Download failed for {local_name}: {}", response.status()));
        }

        let file_size = response.content_length().unwrap_or(0);
        let mut file = tokio::fs::File::create(&tmp_path)
            .await
            .map_err(|e| format!("Failed to create {local_name}: {e}"))?;

        use futures_util::StreamExt;
        let mut stream = response.bytes_stream();
        let mut file_downloaded: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Download error: {e}"))?;
            tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
                .await
                .map_err(|e| format!("Write error: {e}"))?;

            file_downloaded += chunk.len() as u64;
            downloaded_total += chunk.len() as u64;
            let progress = ((downloaded_total as f64 / MODEL_TOTAL_SIZE as f64) * 100.0).min(100.0) as u32;
            let _ = app_handle.emit("llm-download-progress", progress);
        }
        drop(file);

        tokio::fs::rename(&tmp_path, &dest)
            .await
            .map_err(|e| format!("Failed to finalize {local_name}: {e}"))?;

        log::info!("Downloaded {local_name} ({file_downloaded} bytes)");
    }

    Ok(())
}

/// Find Python executable
fn find_python() -> Option<String> {
    // Try common Python paths
    for candidate in &["python3", "python", "python.exe"] {
        if let Ok(output) = std::process::Command::new(candidate)
            .arg("--version")
            .output()
        {
            if output.status.success() {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

/// Start ct2_server.py on a given port
pub async fn start_server(port: u16) -> Result<tokio::process::Child, String> {
    let script = server_script_path();
    if !script.exists() {
        return Err(format!("ct2_server.py not found at {}", script.display()));
    }

    let model = model_dir();
    if !model_exists() {
        return Err("chirp-cleanup model not downloaded".to_string());
    }

    let python = find_python().ok_or("Python not found. Install Python 3.10+ to use chirp-cleanup.")?;

    let mut cmd = tokio::process::Command::new(&python);
    cmd.arg(script.to_string_lossy().to_string())
        .arg("--model")
        .arg(model.to_string_lossy().to_string())
        .arg("--tokenizer")
        .arg("google/flan-t5-small")
        .arg("--port")
        .arg(port.to_string())
        .arg("--device")
        .arg("auto")
        .env("PYTHONIOENCODING", "utf-8")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    #[cfg(windows)]
    {
        #[allow(unused_imports)]
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let child = cmd.spawn()
        .map_err(|e| format!("Failed to start ct2_server: {e}"))?;

    // Wait for server to be ready
    let health_url = format!("http://127.0.0.1:{port}/health");
    let client = reqwest::Client::new();

    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if let Ok(resp) = client.get(&health_url).send().await {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                if body.get("status").and_then(|s| s.as_str()) == Some("ok") {
                    log::info!("ct2_server ready on port {port}");
                    return Ok(child);
                }
            }
        }
    }

    Err("ct2_server failed to start within 30s".to_string())
}

/// Stop the ct2_server process
pub async fn stop_server(child: &mut tokio::process::Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
    log::info!("ct2_server stopped");
}

/// Send text through chirp-cleanup for AI cleanup
pub async fn cleanup_text(port: u16, text: &str) -> Result<String, String> {
    let payload = serde_json::json!({
        "text": text,
        "beam_size": 4,
        "max_length": 256,
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/v1/completions"))
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("chirp-cleanup request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("chirp-cleanup returned status: {}", resp.status()));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {e}"))?;

    let result = body["text"]
        .as_str()
        .unwrap_or(text)
        .trim()
        .to_string();

    // Sanity check: if output is much longer than input, something went wrong
    let input_words = text.split_whitespace().count();
    let output_words = result.split_whitespace().count();
    if output_words > input_words * 3 / 2 + 10 {
        log::warn!(
            "chirp-cleanup output ({output_words} words) much longer than input ({input_words} words), using original"
        );
        return Ok(text.to_string());
    }

    Ok(result)
}
```

- [ ] **Step 2: Add `mod t5;` to main.rs**

In `src-tauri/src/main.rs` (or `lib.rs`), add alongside the other `mod` declarations:

```rust
mod t5;
```

Find the existing `mod llm;` line and add `mod t5;` next to it.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/t5.rs src-tauri/src/main.rs
git commit -m "feat: add t5 module for chirp-cleanup model backend"
```

---

### Task 4: Modify commands.rs to dispatch based on cleanup_model

**Files:**
- Modify: `src-tauri/src/commands.rs`

The key change is in the `stop_recording` command where AI cleanup happens. We also need to modify the LLM commands to handle both backends.

- [ ] **Step 1: Update stop_recording to read cleanup_model**

In `src-tauri/src/commands.rs`, find the block at ~line 408 that reads settings:

```rust
    let (recognizer, smart_fmt, dict, snips, ai_cleanup, llm_port, tone_mode) = {
        let s = state.lock().await;
        let rec = s.recognizer.clone().ok_or("model_not_loaded".to_string())?;
        (
            rec,
            s.settings.smart_formatting,
            s.dictionary.clone(),
            s.snippets.clone(),
            s.settings.ai_cleanup,
            s.llm_port,
            s.settings.tone_mode.clone(),
        )
    };
```

Replace with:

```rust
    let (recognizer, smart_fmt, dict, snips, ai_cleanup, llm_port, tone_mode, cleanup_model) = {
        let s = state.lock().await;
        let rec = s.recognizer.clone().ok_or("model_not_loaded".to_string())?;
        (
            rec,
            s.settings.smart_formatting,
            s.dictionary.clone(),
            s.snippets.clone(),
            s.settings.ai_cleanup,
            s.llm_port,
            s.settings.tone_mode.clone(),
            s.settings.cleanup_model.clone(),
        )
    };
```

- [ ] **Step 2: Update the AI cleanup dispatch block**

Find the block at ~line 483:

```rust
    let after_llm = if ai_cleanup && llm_port.is_some() {
        let port = llm_port.unwrap();
        let _ = app_handle.emit("recording-state", "polishing");
        log::info!("Running AI cleanup on text...");
        match llm::cleanup_text(port, &formatted, &tone_mode).await {
```

Replace with:

```rust
    let after_llm = if ai_cleanup && llm_port.is_some() {
        let port = llm_port.unwrap();
        let _ = app_handle.emit("recording-state", "polishing");
        log::info!("Running AI cleanup ({cleanup_model}) on text...");
        let cleanup_result = if cleanup_model == "chirp-cleanup" {
            t5::cleanup_text(port, &formatted).await
        } else {
            llm::cleanup_text(port, &formatted, &tone_mode).await
        };
        match cleanup_result {
```

- [ ] **Step 3: Update `get_llm_status` to handle both models**

Find the `get_llm_status` command (~line 729). Replace:

```rust
#[tauri::command]
pub async fn get_llm_status(
    state: State<'_, SharedState>,
) -> Result<llm::LlmStatus, String> {
    let s = state.lock().await;
    Ok(llm::LlmStatus {
        binary_downloaded: llm::binary_exists(),
        model_downloaded: llm::model_exists(),
        server_running: s.llm_port.is_some(),
    })
}
```

With:

```rust
#[tauri::command]
pub async fn get_llm_status(
    state: State<'_, SharedState>,
) -> Result<llm::LlmStatus, String> {
    let s = state.lock().await;
    let is_t5 = s.settings.cleanup_model == "chirp-cleanup";
    Ok(llm::LlmStatus {
        binary_downloaded: if is_t5 { true } else { llm::binary_exists() },
        model_downloaded: if is_t5 { t5::model_exists() } else { llm::model_exists() },
        server_running: s.llm_port.is_some(),
    })
}
```

- [ ] **Step 4: Update `download_llm` to handle both models**

Find the `download_llm` command (~line 741). Replace:

```rust
#[tauri::command]
pub async fn download_llm(
    app_handle: AppHandle,
) -> Result<(), String> {
    llm::download_binary(&app_handle).await?;
    llm::download_model(&app_handle).await?;
    Ok(())
}
```

With:

```rust
#[tauri::command]
pub async fn download_llm(
    app_handle: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let is_t5 = {
        let s = state.lock().await;
        s.settings.cleanup_model == "chirp-cleanup"
    };
    if is_t5 {
        t5::download_model(&app_handle).await?;
    } else {
        llm::download_binary(&app_handle).await?;
        llm::download_model(&app_handle).await?;
    }
    Ok(())
}
```

Note: if `download_llm` is registered in the invoke_handler without state, you may need to add the `State` parameter there too. Check `main.rs` / `lib.rs` for the `.invoke_handler(tauri::generate_handler![...])` call.

- [ ] **Step 5: Update `start_llm` to handle both models**

Find the `start_llm` command (~line 750). Replace:

```rust
    let child = llm::start_server(port).await?;
```

With:

```rust
    let is_t5 = {
        let s = state.lock().await;
        s.settings.cleanup_model == "chirp-cleanup"
    };
    let child = if is_t5 {
        t5::start_server(port).await?
    } else {
        llm::start_server(port).await?
    };
```

- [ ] **Step 6: Update `test_llm_cleanup` to handle both models**

Find the `test_llm_cleanup` command (~line 796). Replace:

```rust
    llm::cleanup_text(port, &text, &mode.unwrap_or_else(|| "message".to_string())).await
```

With:

```rust
    let cleanup_model = {
        let s = state.lock().await;
        s.settings.cleanup_model.clone()
    };
    if cleanup_model == "chirp-cleanup" {
        t5::cleanup_text(port, &text).await
    } else {
        llm::cleanup_text(port, &text, &mode.unwrap_or_else(|| "message".to_string())).await
    }
```

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat: dispatch AI cleanup to either Qwen or chirp-cleanup based on setting"
```

---

### Task 5: Frontend — Add cleanup model selector

**Files:**
- Modify: `src/lib/constants.ts`
- Modify: `src/stores/appStore.ts`
- Modify: `src/hooks/useSettingsSync.ts`
- Modify: `src/components/settings/SettingsPage.tsx`

- [ ] **Step 1: Update constants.ts**

In `src/lib/constants.ts`, add after the `LLM_MODEL` constant:

```typescript
export const CLEANUP_MODELS = [
  {
    id: 'chirp-cleanup',
    name: 'Chirp Cleanup',
    size: '78 MB',
    description: 'Fast and lightweight. Optimized for Chirp.',
    recommended: true,
  },
  {
    id: 'qwen',
    name: 'Qwen 2.5 (Legacy)',
    size: '2.1 GB',
    description: 'Larger general-purpose model. Slower on laptops.',
    recommended: false,
  },
] as const
```

Update `DEFAULT_SETTINGS` — add:

```typescript
  cleanupModel: 'chirp-cleanup' as string,
```

- [ ] **Step 2: Update appStore.ts**

In `src/stores/appStore.ts`, add `cleanupModel: string` to the store state type (find where `aiCleanup: boolean` is defined, add after it):

```typescript
  cleanupModel: string
```

And in the initial state, add:

```typescript
  cleanupModel: DEFAULT_SETTINGS.cleanupModel,
```

- [ ] **Step 3: Update useSettingsSync.ts**

In `src/hooks/useSettingsSync.ts`, find the array of synced setting keys (the array that contains `'aiCleanup'`). Add `'cleanupModel'` to it:

```typescript
  'cleanupModel',
```

- [ ] **Step 4: Update SettingsPage.tsx**

In `src/components/settings/SettingsPage.tsx`, find the Smart Cleanup section (the `{store.aiCleanup && (` block with the Tone selector, around line 539). Replace the Tone `<Row>` with a Model selector:

```tsx
          {store.aiCleanup && (
            <Row>
              <div>
                <div className="text-[13px] font-medium text-[#1a1a1a]">Cleanup Model</div>
                <div className="text-[11px] text-[#aaa] mt-0.5">
                  {CLEANUP_MODELS.find(m => m.id === store.cleanupModel)?.description}
                </div>
              </div>
              <div className="w-[180px]">
                <Select
                  options={CLEANUP_MODELS.map(m => ({ value: m.id, label: `${m.name} (${m.size})` }))}
                  value={store.cleanupModel}
                  onChange={(v) => store.updateSettings({ cleanupModel: String(v) })}
                />
              </div>
            </Row>
          )}
```

Import `CLEANUP_MODELS` from constants at the top of the file.

The existing Tone selector can stay if Qwen is selected, but should be hidden for chirp-cleanup (which doesn't support tone modes). Wrap the Tone `<Row>` with:

```tsx
          {store.aiCleanup && store.cleanupModel === 'qwen' && (
            <Row>
              {/* ... existing Tone selector ... */}
            </Row>
          )}
```

- [ ] **Step 5: Commit**

```bash
git add src/lib/constants.ts src/stores/appStore.ts src/hooks/useSettingsSync.ts src/components/settings/SettingsPage.tsx
git commit -m "feat: add cleanup model selector UI (chirp-cleanup vs qwen)"
```

---

### Task 6: Build and test

- [ ] **Step 1: Verify Rust compiles**

```bash
cd src-tauri && cargo check
```

Fix any compilation errors. Common issues:
- `download_llm` command signature changed (added `State` param) — update the `generate_handler!` macro call
- Import `t5` module in files that reference it

- [ ] **Step 2: Install Python dependencies on this machine**

```bash
pip install ctranslate2 transformers sentencepiece
```

- [ ] **Step 3: Run dev server**

```bash
npx tauri dev
```

- [ ] **Step 4: Test the toggle**

1. Open Settings → Smart Cleanup section
2. Switch model to "Chirp Cleanup (78 MB)"
3. It should download the 78MB model (progress bar)
4. Server should start (green "Active" indicator)
5. Dictate something with a self-correction: "I'll see you at two PM wait I mean three PM"
6. Verify output: "I'll see you at 3:00 PM." (or similar)

- [ ] **Step 5: Test switching back to Qwen**

1. Switch model back to "Qwen 2.5 (Legacy)"
2. Verify it restarts with llama-server
3. Dictate the same thing, compare results

- [ ] **Step 6: Commit any fixes and tag**

```bash
git add -A
git commit -m "fix: integration fixes from testing"
```

---

### Task 7: Laptop testing prep

- [ ] **Step 1: Push v1.3.0 branch**

```bash
git push -u origin v1.3.0
```

- [ ] **Step 2: On laptop, clone and set up**

```bash
git checkout v1.3.0
npm install
pip install ctranslate2 transformers sentencepiece
npx tauri dev
```

- [ ] **Step 3: Test on laptop (CPU-only)**

The chirp-cleanup model should run at ~100ms on CPU. Compare against Qwen which will be ~1500ms+ on CPU. This is the key test — does it feel instant on the laptop?
