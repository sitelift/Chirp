//! Native runtime selection must happen before VAD or any sherpa call. Sherpa's
//! import is delay-loaded; absolute paths prevent the installed CPU core from
//! winning Windows DLL search order. A child probe contains native init crashes.
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};
use windows_sys::Win32::{Foundation::FreeLibrary, System::LibraryLoader::*};
static ACTIVE: Mutex<Option<String>> = Mutex::new(None);
static GPU_RUNTIME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn failure_path() -> PathBuf {
    crate::settings::models_dir().join("cuda-failure.txt")
}
pub fn remember_failure(reason: &str) -> Result<(), String> {
    let path = failure_path();
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(path, reason).map_err(|e| e.to_string())
}
pub fn gpu_runtime() -> bool {
    GPU_RUNTIME.load(std::sync::atomic::Ordering::SeqCst)
}
pub fn set_active(provider: &str) {
    *ACTIVE.lock().unwrap() = Some(provider.into());
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    active: Option<String>,
    model: Option<String>,
    gpu_available: bool,
    gpu_downloaded: bool,
    restart_required: bool,
    failure: Option<String>,
}
#[tauri::command]
pub fn get_engine_status() -> EngineStatus {
    let active = ACTIVE.lock().unwrap().clone();
    let ready = pack_ready();
    let gpu_available = nvidia_available();
    let failure = std::fs::read_to_string(failure_path()).ok();
    EngineStatus {
        model: active.as_ref().map(|a| {
            if a == "gpu" {
                "parakeet-v3-fp32"
            } else {
                "parakeet-v3-int8"
            }
            .into()
        }),
        active,
        gpu_available,
        gpu_downloaded: ready,
        restart_required: ready
            && !gpu_runtime()
            && gpu_available
            && failure.is_none()
            && std::env::var("CHIRP_ASR_PROVIDER").as_deref() != Ok("cpu"),
        failure,
    }
}
#[tauri::command]
pub fn retry_gpu(app: tauri::AppHandle) -> Result<EngineStatus, String> {
    match std::fs::remove_file(failure_path()) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.to_string()),
    }
    crate::downloads::background(app);
    Ok(get_engine_status())
}
fn wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
pub fn nvidia_available() -> bool {
    unsafe {
        let dll = LoadLibraryExW(
            wide(Path::new("nvcuda.dll")).as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        );
        if dll.is_null() {
            return false;
        }
        let result = (|| {
            let init: GetInit = std::mem::transmute(GetProcAddress(dll, b"cuInit\0".as_ptr())?);
            let count: GetCount =
                std::mem::transmute(GetProcAddress(dll, b"cuDeviceGetCount\0".as_ptr())?);
            let mut n = 0;
            Some(init(0) == 0 && count(&mut n) == 0 && n > 0)
        })()
        .unwrap_or(false);
        FreeLibrary(dll);
        result
    }
}
type GetInit = unsafe extern "system" fn(u32) -> i32;
type GetCount = unsafe extern "system" fn(*mut i32) -> i32;
fn pack_ready() -> bool {
    crate::downloads::cuda_dir().join(".verified").is_file()
        && crate::downloads::fp32_dir().join(".verified").is_file()
}
fn load_runtime(dir: &Path) -> Result<(), String> {
    unsafe {
        // cuDNN itself uses LoadLibrary for its component DLLs. Adding a user
        // directory alone does not affect those unflagged calls.
        if SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        // Retain the directory cookie and handles for the process lifetime: ORT
        // loads provider dependencies lazily, including during a later decode.
        if AddDllDirectory(wide(dir).as_ptr()).is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        for name in ["onnxruntime.dll", "sherpa-onnx-c-api.dll"] {
            let module = LoadLibraryExW(
                wide(&dir.join(name)).as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            );
            if module.is_null() {
                return Err(format!("{name}: {}", std::io::Error::last_os_error()));
            }
        }
    }
    Ok(())
}
fn probe() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let mut child = std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
        .arg("--chirp-probe-cuda")
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| e.to_string())?;
    let start = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return if status.success() {
                Ok(())
            } else {
                Err(format!("CUDA probe failed: {status}"))
            };
        }
        if start.elapsed().as_secs() > 90 {
            let _ = child.kill();
            let _ = child.wait();
            return Err("CUDA probe timed out".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
/// Called by the executable before Tauri/single-instance/audio initialization.
/// Returns Some(exit code) for the headless validation modes.
pub fn bootstrap() -> Option<i32> {
    let args: Vec<_> = std::env::args().collect();
    if args
        .iter()
        .any(|a| a == "--chirp-download-cpu" || a == "--chirp-download-gpu")
    {
        let runtime = tokio::runtime::Runtime::new().expect("download runtime");
        let result = runtime.block_on(async {
            if args.iter().any(|a| a == "--chirp-download-gpu") {
                crate::downloads::gpu(None).await
            } else {
                crate::downloads::cpu(None).await
            }
        });
        if let Err(e) = &result {
            eprintln!("{e}");
        }
        return Some(if result.is_ok() { 0 } else { 1 });
    }
    if args.iter().any(|a| a == "--chirp-probe-cuda") {
        let result = load_runtime(&crate::downloads::cuda_dir())
            .and_then(|_| {
                GPU_RUNTIME.store(true, std::sync::atomic::Ordering::SeqCst);
                crate::transcribe::load_model_config(
                    &crate::downloads::fp32_dir(),
                    false,
                    &[],
                    "cuda",
                    4,
                    false,
                )
            })
            .and_then(|rec| crate::transcribe::transcribe(&rec, &vec![0.0; 16000]));
        return Some(if result.is_ok() { 0 } else { 1 });
    }
    let forced_cpu = std::env::var("CHIRP_ASR_PROVIDER").as_deref() == Ok("cpu");
    if !forced_cpu && pack_ready() && !failure_path().exists() && nvidia_available() {
        // Write BEFORE calling native code. Crash/forced termination also leaves
        // CPU usable on the next launch, until the user explicitly retries.
        if remember_failure(
            "CUDA initialization did not finish; retry GPU after checking your driver",
        )
        .is_ok()
        {
            match probe() {
                Ok(()) => {
                    if load_runtime(&crate::downloads::cuda_dir()).is_ok() {
                        GPU_RUNTIME.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                }
                Err(e) => {
                    let _ = remember_failure(&e);
                }
            }
        }
    }
    if args.iter().any(|a| a == "--chirp-check-engine") {
        let result = crate::transcribe::load_model("parakeet-tdt-0.6b", false, &[]);
        if let (Ok(rec), Some(wav)) = (
            &result,
            args.iter()
                .position(|a| a == "--wav")
                .and_then(|i| args.get(i + 1)),
        ) {
            let mut reader = hound::WavReader::open(wav).expect("WAV file");
            assert_eq!(reader.spec().sample_rate, 16000);
            assert_eq!(reader.spec().channels, 1);
            let samples: Vec<f32> = reader
                .samples::<i16>()
                .map(|s| s.unwrap() as f32 / 32768.0)
                .collect();
            let text = crate::transcribe::transcribe(rec, &samples).expect("decode WAV");
            println!("transcript: {text}");
            if text.is_empty() {
                return Some(2);
            }
        }
        println!("{}", serde_json::to_string(&get_engine_status()).unwrap());
        if let Err(e) = &result {
            eprintln!("{e}")
        };
        return Some(if result.is_ok() { 0 } else { 1 });
    }
    None
}
pub fn warmed_gpu() {
    let _ = std::fs::remove_file(failure_path());
    set_active("gpu");
}

pub fn disable_gpu(reason: &str) {
    let _ = remember_failure(reason);
    GPU_RUNTIME.store(false, std::sync::atomic::Ordering::SeqCst);
}
