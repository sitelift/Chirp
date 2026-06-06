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
    // Try PATH candidates first
    for candidate in &["python3", "python"] {
        if let Ok(output) = std::process::Command::new(candidate)
            .arg("--version")
            .output()
        {
            if output.status.success() {
                let ver = String::from_utf8_lossy(&output.stdout);
                let ver2 = String::from_utf8_lossy(&output.stderr);
                // Windows Store stub returns success but prints nothing useful
                if ver.contains("Python") || ver2.contains("Python") {
                    return Some(candidate.to_string());
                }
            }
        }
    }
    // Try common Windows Python install locations
    #[cfg(windows)]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let base = std::path::PathBuf::from(&local).join("Python");
            if let Ok(entries) = std::fs::read_dir(&base) {
                for entry in entries.flatten() {
                    let python = entry.path().join("python.exe");
                    if python.exists() {
                        if let Ok(output) = std::process::Command::new(&python)
                            .arg("--version")
                            .output()
                        {
                            if output.status.success() {
                                return Some(python.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
            // Also check Programs/Python
            let programs = std::path::PathBuf::from(&local).join("Programs").join("Python");
            if let Ok(entries) = std::fs::read_dir(&programs) {
                for entry in entries.flatten() {
                    let python = entry.path().join("python.exe");
                    if python.exists() {
                        return Some(python.to_string_lossy().to_string());
                    }
                }
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
pub async fn cleanup_text(port: u16, text: &str, client: &reqwest::Client) -> Result<String, String> {
    let payload = serde_json::json!({
        "text": text,
        "beam_size": 4,
        "max_length": 256,
    });

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
