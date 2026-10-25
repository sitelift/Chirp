use crate::settings::models_dir;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::{AppHandle, Emitter};

#[derive(Clone, Deserialize)]
pub struct Artifact {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
    pub group: String,
    pub kind: String,
}
#[derive(Deserialize)]
struct Manifest {
    files: Vec<Artifact>,
}
pub fn artifacts() -> Vec<Artifact> {
    serde_json::from_str::<Manifest>(include_str!("../downloads.json"))
        .expect("embedded download manifest")
        .files
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub group: String,
    pub file: String,
    pub phase: String,
    pub downloaded: u64,
    pub total: u64,
    pub error: Option<String>,
}
static PROGRESS: Mutex<Option<Progress>> = Mutex::new(None);
static BACKGROUND: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static DOWNLOAD: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
#[tauri::command]
pub fn get_download_status() -> Option<Progress> {
    PROGRESS.lock().unwrap().clone()
}
fn progress(app: Option<&AppHandle>, a: &Artifact, phase: &str, bytes: u64) {
    let p = Progress {
        group: a.group.clone(),
        file: a.name.clone(),
        phase: phase.into(),
        downloaded: bytes,
        total: a.size,
        error: None,
    };
    *PROGRESS.lock().unwrap() = Some(p.clone());
    if let Some(app) = app {
        let _ = app.emit("engine-download-progress", p);
        if a.group == "cpu" {
            let _ = app.emit(
                "model-download-progress",
                if phase == "ready" {
                    100
                } else {
                    ((bytes as f64 / a.size as f64) * 95.0).min(95.0) as u32
                },
            );
        }
    }
}
pub fn cuda_dir() -> PathBuf {
    models_dir().join("runtime/cuda-1.12.36-v1")
}
pub fn fp32_dir() -> PathBuf {
    models_dir().join("sherpa/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3")
}
fn hash(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut digest = Sha256::new();
    let mut buf = [0u8; 1024 * 256];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        digest.update(&buf[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn check_space(path: &Path, needed: u64) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut free = 0;
    if unsafe {
        windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().to_string());
    }
    if free < needed {
        return Err(format!(
            "Not enough disk space: need {} MiB free",
            needed / 1024 / 1024
        ));
    }
    Ok(())
}
fn resume_start(value: &str, offset: u64, total: u64) -> bool {
    let Some(value) = value.strip_prefix("bytes ") else {
        return false;
    };
    let Some((range, size)) = value.split_once('/') else {
        return false;
    };
    let Some((start, end)) = range.split_once('-') else {
        return false;
    };
    start.parse::<u64>() == Ok(offset)
        && size.parse::<u64>() == Ok(total)
        && end.parse::<u64>().is_ok_and(|n| n >= offset && n < total)
}
/// Keep interrupted bytes, validate Range responses, and promote only verified files.
async fn fetch(a: &Artifact, app: Option<&AppHandle>) -> Result<PathBuf, String> {
    fetch_into(a, app, &models_dir().join("downloads")).await
}
async fn fetch_into(
    a: &Artifact,
    app: Option<&AppHandle>,
    cache: &Path,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(cache).map_err(|e| e.to_string())?;
    let dest = cache.join(&a.sha256);
    let part = cache.join(format!("{}.part", a.sha256));
    if dest.exists() && hash(&dest)? == a.sha256 {
        return Ok(dest);
    }
    let mut offset = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    if offset > a.size {
        std::fs::remove_file(&part).map_err(|e| e.to_string())?;
        offset = 0;
    }
    check_space(&cache, a.size - offset + 64 * 1024 * 1024)?;
    if offset < a.size {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(30))
            .read_timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| e.to_string())?;
        let mut request = client.get(&a.url).header("Accept-Encoding", "identity");
        if offset > 0 {
            request = request.header("Range", format!("bytes={offset}-"));
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
            if !response
                .headers()
                .get("content-range")
                .and_then(|h| h.to_str().ok())
                .is_some_and(|h| resume_start(h, offset, a.size))
            {
                return Err("Invalid download Content-Range".into());
            }
        } else if response.status() == reqwest::StatusCode::OK {
            offset = 0;
        } else {
            return Err(format!("Download HTTP {}", response.status()));
        }
        let mut out = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(offset == 0)
            .append(offset > 0)
            .open(&part)
            .await
            .map_err(|e| e.to_string())?;
        let mut stream = response.bytes_stream();
        let mut last = std::time::Instant::now();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| e.to_string())?;
            offset += chunk.len() as u64;
            if offset > a.size {
                return Err("Download exceeds pinned size".into());
            }
            tokio::io::AsyncWriteExt::write_all(&mut out, &chunk)
                .await
                .map_err(|e| e.to_string())?;
            if last.elapsed().as_millis() > 150 {
                progress(app, a, "downloading", offset);
                last = std::time::Instant::now();
            }
        }
        out.sync_all().await.map_err(|e| e.to_string())?;
    }
    progress(app, a, "verifying", offset);
    let p = part.clone();
    let actual = tokio::task::spawn_blocking(move || hash(&p))
        .await
        .map_err(|e| e.to_string())??;
    if offset != a.size || actual != a.sha256 {
        let _ = std::fs::remove_file(&part);
        return Err(format!("Checksum/size mismatch: {}", a.name));
    }
    std::fs::rename(&part, &dest).map_err(|e| e.to_string())?;
    Ok(dest)
}
fn safe_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}
fn extract(a: &Artifact, archive: &Path, stage: &Path) -> Result<(), String> {
    std::fs::create_dir_all(stage).map_err(|e| e.to_string())?;
    let write_entry = |path: &Path, input: &mut dyn Read| -> Result<(), String> {
        if !safe_path(path) {
            return Err("Unsafe archive path".into());
        }
        let dest = if a.kind == "model-tar" {
            stage.join(path)
        } else {
            let name = path.file_name().ok_or("Missing archive filename")?;
            if path.extension().is_some_and(|e| e == "dll") {
                stage.join(name)
            } else if name.to_string_lossy().to_lowercase().contains("license") {
                stage.join("licenses").join(&a.sha256).join(path)
            } else {
                return Ok(());
            }
        };
        std::fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
        let mut out = std::fs::File::create(dest).map_err(|e| e.to_string())?;
        std::io::copy(input, &mut out).map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())
    };
    if a.kind.ends_with("tar") {
        let mut tar = tar::Archive::new(bzip2::read::BzDecoder::new(
            std::fs::File::open(archive).map_err(|e| e.to_string())?,
        ));
        for entry in tar.entries().map_err(|e| e.to_string())? {
            let mut entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path().map_err(|e| e.to_string())?.into_owned();
            if !safe_path(&path) {
                return Err("Unsafe archive path".into());
            }
            if entry.header().entry_type().is_file() {
                write_entry(&path, &mut entry)?;
            } else if !entry.header().entry_type().is_dir() {
                return Err("Archive links are not permitted".into());
            }
        }
    } else {
        let mut zip =
            zip::ZipArchive::new(std::fs::File::open(archive).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            if entry.is_dir() {
                continue;
            }
            let path = PathBuf::from(entry.name());
            write_entry(&path, &mut entry)?;
        }
    }
    Ok(())
}
async fn install_group(group: &str, app: Option<&AppHandle>) -> Result<(), String> {
    let root = models_dir();
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let dest = match group {
        "cpu" => crate::transcribe::model_dir("parakeet-tdt-0.6b"),
        "fp32" => fp32_dir(),
        "cuda" => cuda_dir(),
        "vad" => crate::settings::vad_model_path(),
        _ => return Err("Unknown group".into()),
    };
    if (group == "cpu" && crate::transcribe::model_exists("parakeet-tdt-0.6b"))
        || (group != "vad" && dest.join(".verified").exists())
    {
        return Ok(());
    }
    let stage = root.join(format!(".staging-{group}"));
    std::fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    check_space(
        &root,
        match group {
            "cuda" => 5_000_000_000,
            "fp32" => 5_500_000_000,
            "cpu" => 2_000_000_000,
            _ => 10_000_000,
        },
    )?;
    let files: Vec<_> = artifacts()
        .into_iter()
        .filter(|a| a.group == group)
        .collect();
    for a in &files {
        let archive = fetch(a, app).await?;
        progress(app, a, "extracting", a.size);
        if a.kind == "file" {
            std::fs::copy(&archive, stage.join(&a.name)).map_err(|e| e.to_string())?;
        } else {
            let (a, p, s) = (a.clone(), archive.clone(), stage.clone());
            tokio::task::spawn_blocking(move || extract(&a, &p, &s))
                .await
                .map_err(|e| e.to_string())??;
        }
    }
    let source = if group == "cpu" {
        stage.join(dest.file_name().unwrap())
    } else if group == "vad" {
        stage.join("silero_vad.onnx")
    } else {
        stage.clone()
    };
    if group == "cpu" && !crate::transcribe::model_files_exist(&source) {
        return Err("Incomplete CPU model".into());
    }
    std::fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
    // Keep existing good files until every replacement has been verified. A completed
    // install is immutable; incomplete directories are moved aside, never loaded.
    if group != "vad" {
        std::fs::write(source.join(".verified"), "downloads-v1").map_err(|e| e.to_string())?;
    }
    if dest.exists() {
        let old = dest.with_extension("incomplete");
        if old.exists() {
            if old.is_dir() {
                std::fs::remove_dir_all(&old)
            } else {
                std::fs::remove_file(&old)
            }
            .map_err(|e| e.to_string())?;
        }
        std::fs::rename(&dest, &old).map_err(|e| e.to_string())?;
    }
    std::fs::rename(source, &dest).map_err(|e| e.to_string())?;
    for a in &files {
        progress(app, a, "ready", a.size);
        let _ = std::fs::remove_file(root.join("downloads").join(&a.sha256));
    }
    Ok(())
}
pub async fn cpu(app: Option<&AppHandle>) -> Result<(), String> {
    let _guard = DOWNLOAD.lock().await;
    if let Err(e) = install_group("cpu", app).await {
        report_error(app, &e);
        return Err(e);
    }
    // Full-waveform ASR still works without segmentation. Retry VAD on startup.
    if let Err(e) = install_group("vad", app).await {
        report_error(app, &e);
    }
    Ok(())
}
pub async fn gpu(app: Option<&AppHandle>) -> Result<(), String> {
    let _guard = DOWNLOAD.lock().await;
    install_group("cuda", app).await?;
    install_group("fp32", app).await
}
pub fn background(app: AppHandle) {
    if !crate::transcribe::model_exists("parakeet-tdt-0.6b") {
        return;
    }
    let want_gpu = std::env::var("CHIRP_ASR_PROVIDER").as_deref() != Ok("cpu")
        && crate::engine::nvidia_available()
        && !crate::engine::failure_path().exists();
    if BACKGROUND.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        if !crate::settings::vad_model_path().exists() {
            let _guard = DOWNLOAD.lock().await;
            if let Err(e) = install_group("vad", Some(&app)).await {
                report_error(Some(&app), &e);
            }
        }
        if want_gpu {
            if let Err(e) = gpu(Some(&app)).await {
                report_error(Some(&app), &e);
            } else {
                let _ = app.emit("engine-status", crate::engine::get_engine_status());
            }
        }
        BACKGROUND.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}
fn report_error(app: Option<&AppHandle>, error: &str) {
    let mut p = PROGRESS.lock().unwrap().clone().unwrap_or_default();
    p.phase = "error".into();
    p.error = Some(error.into());
    *PROGRESS.lock().unwrap() = Some(p.clone());
    if let Some(app) = app {
        let _ = app.emit("engine-download-progress", p);
    }
    log::warn!("Engine download: {error}");
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranges_must_match() {
        assert!(resume_start("bytes 10-19/20", 10, 20));
        for v in [
            "bytes 0-19/20",
            "bytes 10-20/20",
            "bytes */20",
            "bytes 10-19/21",
        ] {
            assert!(!resume_start(v, 10, 20));
        }
    }
    #[test]
    fn archive_paths_cannot_escape() {
        for p in ["../x", "C:/x", "/x", "a/../../x", r"a\..\x"] {
            assert!(!safe_path(Path::new(p)));
        }
        assert!(safe_path(Path::new("model/encoder.onnx")));
    }
    #[test]
    fn manifest_is_pinned() {
        for a in artifacts() {
            assert_eq!(a.sha256.len(), 64);
            assert!(a.size > 0);
            assert!(a.url.starts_with("https://"));
        }
    }
}

#[cfg(test)]
mod transfer_tests {
    use super::*;
    fn serve(response: String) -> (String, std::thread::JoinHandle<String>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file", listener.local_addr().unwrap());
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0; 4096];
            let n = stream.read(&mut buf).unwrap();
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8_lossy(&buf[..n]).to_lowercase()
        });
        (url, thread)
    }
    async fn run_case(response: &str, seed: &[u8], expected: bool, suffix: &str) {
        let (url, thread) = serve(response.into());
        let cache =
            std::env::temp_dir().join(format!("chirp-transfer-{}-{suffix}", std::process::id()));
        std::fs::create_dir_all(&cache).unwrap();
        let a = Artifact {
            name: "test".into(),
            url,
            size: 6,
            sha256: format!("{:x}", Sha256::digest(b"abcdef")),
            group: "test".into(),
            kind: "file".into(),
        };
        std::fs::write(cache.join(format!("{}.part", a.sha256)), seed).unwrap();
        let result = fetch_into(&a, None, &cache).await;
        assert_eq!(result.is_ok(), expected, "{result:?}");
        let request = thread.join().unwrap();
        if !seed.is_empty() {
            assert!(request.contains("range: bytes=3-"));
        }
        if let Ok(path) = result {
            assert_eq!(std::fs::read(path).unwrap(), b"abcdef");
        }
        std::fs::remove_dir_all(cache).unwrap();
    }
    #[test]
    fn resumes_verified_bytes() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
        run_case("HTTP/1.1 206 Partial Content\r\nContent-Length: 3\r\nContent-Range: bytes 3-5/6\r\nConnection: close\r\n\r\ndef",b"abc",true,"resume").await;
        });
    }
    #[test]
    fn ignored_range_restarts() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            run_case(
                "HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabcdef",
                b"abc",
                true,
                "restart",
            )
            .await;
        });
    }
    #[test]
    fn wrong_hash_is_not_promoted() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            run_case(
                "HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabcdeg",
                b"",
                false,
                "hash",
            )
            .await;
        });
    }
    #[test]
    fn wrong_range_is_not_appended() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
        run_case("HTTP/1.1 206 Partial Content\r\nContent-Length: 3\r\nContent-Range: bytes 2-4/6\r\nConnection: close\r\n\r\ndef",b"abc",false,"range").await;
        });
    }
}
