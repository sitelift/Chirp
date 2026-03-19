use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

use crate::settings;

fn system_prompt_for_mode(mode: &str) -> &'static str {
    match mode {
        "email" => "Clean up dictated speech. Remove fillers, fix stutters, resolve self-corrections (keep only the final version). Format as an email with greeting, body paragraphs, and sign-off. Output only the cleaned text.",
        "formal" => "Clean up dictated speech. Remove fillers, fix stutters, resolve self-corrections (keep only the final version). Use professional, formal language. Output only the cleaned text.",
        "casual" => "Clean up dictated speech. Remove fillers, fix stutters, resolve self-corrections (keep only the final version). Keep it casual and conversational. Output only the cleaned text.",
        _ => "Clean up dictated speech. Remove fillers, fix stutters, resolve self-corrections (keep only the final version). Output only the cleaned text.",
    }
}

const MODEL_FILENAME: &str = "chirp-cleanup-0.6b-q4_k_m.gguf";
// TODO: Upload fine-tuned model to HuggingFace and update URL
const MODEL_URL: &str = "https://huggingface.co/chirpapp/chirp-cleanup-0.6b-GGUF/resolve/main/chirp-cleanup-0.6b-q4_k_m.gguf";
const MODEL_SIZE: u64 = 400_000_000;

/// llama-server release info (Vulkan build for GPU acceleration)
const LLAMA_CPP_VERSION: &str = "b5604";

fn llama_server_url() -> String {
    let platform_suffix = if cfg!(target_os = "windows") {
        "bin-win-vulkan-x64"
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "bin-macos-arm64"
        } else {
            "bin-macos-x64"
        }
    } else {
        "bin-ubuntu-x64"
    };
    format!(
        "https://github.com/ggerganov/llama.cpp/releases/download/{}/llama-{}-{}.zip",
        LLAMA_CPP_VERSION, LLAMA_CPP_VERSION, platform_suffix
    )
}

/// Directory for LLM files: %APPDATA%/com.chirp.app/llm/
pub fn llm_dir() -> PathBuf {
    settings::config_dir().join("llm")
}

fn binary_path() -> PathBuf {
    if cfg!(windows) {
        llm_dir().join("llama-server.exe")
    } else {
        llm_dir().join("llama-server")
    }
}

fn model_path() -> PathBuf {
    llm_dir().join(MODEL_FILENAME)
}

/// Check if llama-server binary exists
pub fn binary_exists() -> bool {
    binary_path().exists()
}

/// Check if the model GGUF exists
pub fn model_exists() -> bool {
    model_path().exists()
}

/// Download llama-server binary from llama.cpp releases
pub async fn download_binary(app_handle: &AppHandle) -> Result<(), String> {
    let dir = llm_dir();
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("Failed to create LLM dir: {e}"))?;

    let dest = binary_path();
    if dest.exists() {
        return Ok(());
    }

    let url = llama_server_url();
    let zip_path = dir.join("llama-server.zip");
    let tmp_path = dir.join("llama-server.zip.tmp");

    // Download the zip
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Download request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Download failed with status: {}", response.status()));
    }

    let total_size = response.content_length().unwrap_or(15_000_000);
    let mut downloaded: u64 = 0;

    let mut file = tokio::fs::File::create(&tmp_path)
        .await
        .map_err(|e| format!("Failed to create file: {e}"))?;

    use futures_util::StreamExt;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download error: {e}"))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("Write error: {e}"))?;

        downloaded += chunk.len() as u64;
        let progress = ((downloaded as f64 / total_size as f64) * 90.0).min(90.0) as u32;
        let _ = app_handle.emit("llm-download-progress", progress);
    }
    drop(file);

    tokio::fs::rename(&tmp_path, &zip_path)
        .await
        .map_err(|e| format!("Failed to finalize download: {e}"))?;

    let _ = app_handle.emit("llm-download-progress", 95u32);

    // Extract llama-server.exe and all required DLLs from the zip
    let zip_clone = zip_path.clone();
    let dir_clone = dir.clone();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&zip_clone)
            .map_err(|e| format!("Failed to open zip: {e}"))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("Failed to read zip: {e}"))?;

        let mut found_server = false;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)
                .map_err(|e| format!("Failed to read zip entry: {e}"))?;
            let name = entry.name().to_string();
            let basename = name.rsplit('/').next().unwrap_or(&name);

            // Validate filename to prevent path traversal
            if basename.contains("..") || basename.contains('/') || basename.contains('\\')
                || std::path::Path::new(basename).is_absolute()
            {
                log::warn!("Skipping suspicious filename in archive: {basename}");
                continue;
            }

            // Extract llama-server.exe and all .dll files (Windows)
            // or llama-server and .dylib/.so files (macOS/Linux)
            let dominated_by_platform = if cfg!(windows) {
                (basename.ends_with(".exe") && basename.contains("llama-server"))
                    || basename.ends_with(".dll")
            } else {
                basename == "llama-server"
                    || basename.ends_with(".dylib")
                    || basename.ends_with(".so")
            };

            if dominated_by_platform {
                let dest_file = dir_clone.join(basename);
                let mut out = std::fs::File::create(&dest_file)
                    .map_err(|e| format!("Failed to create {basename}: {e}"))?;
                std::io::copy(&mut entry, &mut out)
                    .map_err(|e| format!("Failed to extract {basename}: {e}"))?;
                if basename.contains("llama-server") {
                    found_server = true;
                }
            }
        }
        if !found_server {
            return Err("llama-server binary not found in archive".to_string());
        }

        // Set executable permission on Unix platforms
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let server_path = dir_clone.join("llama-server");
            if server_path.exists() {
                let _ = std::fs::set_permissions(
                    &server_path,
                    std::fs::Permissions::from_mode(0o755),
                );
            }
        }

        Ok(())
    })
    .await
    .map_err(|e| format!("Extract task failed: {e}"))??;

    // Clean up zip
    let _ = tokio::fs::remove_file(&zip_path).await;

    let _ = app_handle.emit("llm-download-progress", 100u32);
    Ok(())
}

/// Download the model GGUF
pub async fn download_model(app_handle: &AppHandle) -> Result<(), String> {
    let dir = llm_dir();
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("Failed to create LLM dir: {e}"))?;

    let dest = model_path();
    if dest.exists() {
        return Ok(());
    }

    let file_name = dest.file_name()
        .ok_or_else(|| "Model path has no filename".to_string())?;
    let tmp_path = dir.join(format!("{}.tmp", file_name.to_string_lossy()));

    let client = reqwest::Client::new();
    let response = client
        .get(MODEL_URL)
        .send()
        .await
        .map_err(|e| format!("Download request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Download failed with status: {}", response.status()));
    }

    let total_size = response.content_length().unwrap_or(MODEL_SIZE);
    let mut downloaded: u64 = 0;

    let mut file = tokio::fs::File::create(&tmp_path)
        .await
        .map_err(|e| format!("Failed to create file: {e}"))?;

    use futures_util::StreamExt;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download error: {e}"))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("Write error: {e}"))?;

        downloaded += chunk.len() as u64;
        let progress = ((downloaded as f64 / total_size as f64) * 100.0).min(100.0) as u32;
        let _ = app_handle.emit("llm-download-progress", progress);
    }
    drop(file);

    tokio::fs::rename(&tmp_path, &dest)
        .await
        .map_err(|e| format!("Failed to finalize model download: {e}"))?;

    Ok(())
}

/// Start llama-server on a given port. Returns the child process.
pub async fn start_server(port: u16) -> Result<tokio::process::Child, String> {
    let binary = binary_path();
    let model = model_path();

    if !binary.exists() {
        return Err("llama-server binary not found".to_string());
    }
    if !model.exists() {
        return Err("Model not found".to_string());
    }

    let n_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let mut cmd = tokio::process::Command::new(binary.to_string_lossy().to_string());
    cmd.arg("--model")
        .arg(model.to_string_lossy().to_string())
        .arg("--port")
        .arg(port.to_string())
        .arg("--ctx-size")
        .arg("2048")
        .arg("--n-predict")
        .arg("1024")
        .arg("--threads")
        .arg(n_threads.to_string())
        .arg("--gpu-layers")
        .arg("99")
        .arg("--log-disable")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    #[cfg(windows)]
    {
        #[allow(unused_imports)]
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let child = cmd.spawn()
        .map_err(|e| format!("Failed to start llama-server: {e}"))?;

    // Wait for server to be ready
    let health_url = format!("http://127.0.0.1:{port}/health");
    let client = reqwest::Client::new();

    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if let Ok(resp) = client.get(&health_url).send().await {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                if body.get("status").and_then(|s| s.as_str()) == Some("ok") {
                    log::info!("llama-server ready on port {port}");
                    return Ok(child);
                }
            }
        }
    }

    Err("llama-server failed to start within 30s".to_string())
}

/// Stop a running llama-server process
pub async fn stop_server(child: &mut tokio::process::Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
    log::info!("llama-server stopped");
}

/// Send text through the LLM for cleanup
pub async fn cleanup_text(port: u16, text: &str, tone_mode: &str) -> Result<String, String> {
    let prompt = system_prompt_for_mode(tone_mode);
    let input_tokens_est = (text.split_whitespace().count() as f64 * 1.3) as usize;
    let max_tokens = (input_tokens_est * 2).clamp(64, 1024);

    let payload = serde_json::json!({
        "model": "qwen",
        "messages": [
            {"role": "system", "content": prompt},
            {"role": "user", "content": text},
        ],
        "temperature": 0.0,
        "max_tokens": max_tokens,
        "stream": false,
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/v1/chat/completions"))
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("LLM request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("LLM returned status: {}", resp.status()));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse LLM response: {e}"))?;

    let result = body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or(text)
        .trim()
        .to_string();

    Ok(result)
}

/// LLM status for frontend
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmStatus {
    pub binary_downloaded: bool,
    pub model_downloaded: bool,
    pub server_running: bool,
}
