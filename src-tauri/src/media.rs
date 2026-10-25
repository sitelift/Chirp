//! Music while dictating: lower other apps' volume, or pause their media,
//! while recording, then put it back.
//!
//! One worker thread owns COM and every audio interface. The recording path
//! only sends it a message, so it never waits on Windows audio.
//!
//! Lowering scales each playing app's mixer volume (`ISimpleAudioVolume`).
//! Windows remembers that volume across restarts, so the originals are
//! journaled before anything changes and a crash mid-dictation is undone on
//! the next launch. An app whose volume the user moved meanwhile is left
//! alone. Pausing uses the system media controls (the sessions the media keys
//! reach) and resumes only what Chirp paused and is still paused.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use windows::core::Interface;
use windows_future::AsyncStatus;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as MediaSession,
    GlobalSystemMediaTransportControlsSessionManager as MediaManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
};
use windows::Win32::Foundation::S_OK;
use windows::Win32::Media::Audio::{
    eRender, AudioSessionStateActive, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
    ISimpleAudioVolume, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED};

use crate::state::MediaMode;

/// 6 x 20 ms: quick enough to be out of the way before the first word, slow
/// enough not to sound like a glitch.
const FADE_STEPS: u32 = 6;
const FADE_STEP: Duration = Duration::from_millis(20);
/// A volume this far from what Chirp set means the user changed it.
const MOVED: f32 = 0.02;
/// Longest wait for the media controls to answer.
const MEDIA_TIMEOUT: Duration = Duration::from_millis(400);

enum Cmd {
    Begin { mode: MediaMode, lower_by: u8 },
    End,
    Shutdown(Sender<()>),
}

fn worker() -> &'static Mutex<Option<Sender<Cmd>>> {
    static WORKER: OnceLock<Mutex<Option<Sender<Cmd>>>> = OnceLock::new();
    WORKER.get_or_init(|| Mutex::new(None))
}

/// Start the worker and undo anything a crash left lowered.
pub fn init() {
    let (tx, rx) = mpsc::channel();
    match std::thread::Builder::new().name("chirp-media".into()).spawn(move || run(rx)) {
        Ok(_) => *worker().lock().unwrap_or_else(|e| e.into_inner()) = Some(tx),
        Err(e) => log::warn!("Media worker unavailable: {e}"),
    }
}

fn send(cmd: Cmd) {
    if let Some(tx) = worker().lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        let _ = tx.send(cmd);
    }
}

/// Recording started. Returns immediately.
pub fn begin(mode: MediaMode, lower_by: u8) {
    if mode != MediaMode::Off {
        send(Cmd::Begin { mode, lower_by });
    }
}

/// Recording stopped or was cancelled. Returns immediately.
pub fn end() {
    send(Cmd::End);
}

/// App exit: restore now, waiting briefly so the process doesn't exit first.
pub fn shutdown() {
    let (tx, rx) = mpsc::channel();
    send(Cmd::Shutdown(tx));
    let _ = rx.recv_timeout(Duration::from_secs(1));
}

enum Quieted {
    Lowered { apps: Vec<Lowered>, leftover: Vec<Saved> },
    Paused(Vec<MediaSession>),
}

struct Lowered {
    volume: ISimpleAudioVolume,
    original: f32,
    target: f32,
}

/// One journaled app, keyed by its session identifier (app + device, stable
/// across restarts of either).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Saved {
    id: String,
    original: f32,
    target: f32,
}

fn run(rx: Receiver<Cmd>) {
    // SAFETY: called once on this thread before any COM use.
    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        log::warn!("Media worker COM init failed: {e}");
        return;
    }
    let leftover = recover(read_journal());
    write_journal(&leftover);

    let mut quieted: Option<Quieted> = None;
    let mut manager: Option<MediaManager> = None;
    for cmd in rx {
        match cmd {
            Cmd::Begin { mode, lower_by } if quieted.is_none() => {
                let t = Instant::now();
                quieted = match mode {
                    MediaMode::Lower => Some(lower(lower_by)),
                    MediaMode::Pause => Some(Quieted::Paused(pause(&mut manager))),
                    MediaMode::Off => None,
                };
                log::info!("Media {mode:?} in {} ms", t.elapsed().as_millis());
            }
            Cmd::Begin { .. } => {}
            Cmd::End => {
                if let Some(q) = quieted.take() {
                    restore(q);
                }
            }
            Cmd::Shutdown(done) => {
                if let Some(q) = quieted.take() {
                    restore(q);
                }
                let _ = done.send(());
                break;
            }
        }
    }
}

fn restore(q: Quieted) {
    let t = Instant::now();
    match q {
        Quieted::Lowered { apps, leftover } => {
            let apps: Vec<_> = apps
                .into_iter()
                // SAFETY: COM calls on the thread that created the interface.
                .filter(|a| unsafe { a.volume.GetMasterVolume() }.is_ok_and(|v| (v - a.target).abs() < MOVED))
                .collect();
            fade(&apps, |a| (a.target, a.original));
            write_journal(&leftover);
        }
        Quieted::Paused(sessions) => {
            for s in sessions {
                if status(&s) == Some(PlaybackStatus::Paused) {
                    if let Ok(op) = s.TryPlayAsync() {
                        wait(|| op.Status());
                    }
                }
            }
        }
    }
    log::info!("Media restored in {} ms", t.elapsed().as_millis());
}

// ---------------------------------------------------------------- lowering

fn lower(lower_by: u8) -> Quieted {
    let scale = 1.0 - f32::from(lower_by.clamp(10, 100)) / 100.0;
    // An earlier crash may have left apps that weren't running at launch.
    let leftover = recover(read_journal());
    let own = std::process::id();
    let mut apps = Vec::new();
    let mut saved = leftover.clone();
    for session in sessions() {
        // SAFETY: COM calls on the thread that created the interfaces.
        unsafe {
            if session.GetState().ok() != Some(AudioSessionStateActive)
                || session.IsSystemSoundsSession() == S_OK
                || session.GetProcessId().ok() == Some(own)
            {
                continue;
            }
            let Ok(volume) = session.cast::<ISimpleAudioVolume>() else { continue };
            let Ok(original) = volume.GetMasterVolume() else { continue };
            if original <= 0.001 {
                continue;
            }
            let target = original * scale;
            if let Some(id) = session_id(&session) {
                saved.push(Saved { id, original, target });
            }
            apps.push(Lowered { volume, original, target });
        }
    }
    // Journal first: Windows persists these volumes.
    write_journal(&saved);
    fade(&apps, |a| (a.original, a.target));
    Quieted::Lowered { apps, leftover }
}

fn fade(apps: &[Lowered], range: impl Fn(&Lowered) -> (f32, f32)) {
    if apps.is_empty() {
        return;
    }
    for step in 1..=FADE_STEPS {
        let k = step as f32 / FADE_STEPS as f32;
        for a in apps {
            let (from, to) = range(a);
            // SAFETY: COM call on the thread that created the interface.
            let _ = unsafe { a.volume.SetMasterVolume(from + (to - from) * k, std::ptr::null()) };
        }
        if step < FADE_STEPS {
            std::thread::sleep(FADE_STEP);
        }
    }
}

/// Put journaled apps back where the user had them. Returns the entries whose
/// app isn't running, to try again later.
fn recover(saved: Vec<Saved>) -> Vec<Saved> {
    if saved.is_empty() {
        return saved;
    }
    let live: Vec<(String, IAudioSessionControl2)> = sessions().into_iter().filter_map(|s| Some((session_id(&s)?, s))).collect();
    let mut left = Vec::new();
    for entry in saved {
        let matches: Vec<_> = live.iter().filter(|(id, _)| *id == entry.id).collect();
        if matches.is_empty() {
            left.push(entry);
            continue;
        }
        for (_, session) in matches {
            // SAFETY: COM calls on the thread that created the interfaces.
            unsafe {
                let Ok(volume) = session.cast::<ISimpleAudioVolume>() else { continue };
                if volume.GetMasterVolume().is_ok_and(|v| (v - entry.target).abs() < MOVED) {
                    let _ = volume.SetMasterVolume(entry.original, std::ptr::null());
                    log::info!("Restored volume left lowered by a previous run");
                }
            }
        }
    }
    left
}

/// Every audio session on every active output device.
fn sessions() -> Vec<IAudioSessionControl2> {
    let mut out = Vec::new();
    // SAFETY: COM is initialized on this thread; each call is checked.
    unsafe {
        let Ok(enumerator) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) else { return out };
        let Ok(devices) = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) else { return out };
        for i in 0..devices.GetCount().unwrap_or(0) {
            let Ok(device) = devices.Item(i) else { continue };
            let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { continue };
            let Ok(list) = manager.GetSessionEnumerator() else { continue };
            for j in 0..list.GetCount().unwrap_or(0) {
                if let Ok(session) = list.GetSession(j).and_then(|s| s.cast::<IAudioSessionControl2>()) {
                    out.push(session);
                }
            }
        }
    }
    out
}

fn session_id(session: &IAudioSessionControl2) -> Option<String> {
    // SAFETY: the returned string is owned by us and freed after copying.
    unsafe {
        let p = session.GetSessionIdentifier().ok()?;
        let id = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        id
    }
}

fn journal_path() -> PathBuf {
    crate::settings::config_dir().join("lowered-audio.json")
}

fn read_journal() -> Vec<Saved> {
    std::fs::read_to_string(journal_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn write_journal(saved: &[Saved]) {
    let path = journal_path();
    let result = if saved.is_empty() {
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    } else {
        serde_json::to_string(saved).map_err(|e| e.to_string()).and_then(|data| std::fs::write(&path, data).map_err(|e| e.to_string()))
    };
    if let Err(e) = result {
        log::warn!("Lowered-volume journal: {e}");
    }
}

// ----------------------------------------------------------------- pausing

fn pause(manager: &mut Option<MediaManager>) -> Vec<MediaSession> {
    if manager.is_none() {
        *manager = MediaManager::RequestAsync().ok().and_then(|op| {
            wait(|| op.Status());
            op.GetResults().ok()
        });
    }
    let Some(sessions) = manager.as_ref().and_then(|m| m.GetSessions().ok()) else { return Vec::new() };
    let mut paused = Vec::new();
    for i in 0..sessions.Size().unwrap_or(0) {
        let Ok(s) = sessions.GetAt(i) else { continue };
        if status(&s) != Some(PlaybackStatus::Playing) {
            continue;
        }
        if let Ok(op) = s.TryPauseAsync() {
            wait(|| op.Status());
            if op.GetResults().unwrap_or(false) {
                paused.push(s);
            }
        }
    }
    paused
}

fn status(s: &MediaSession) -> Option<PlaybackStatus> {
    s.GetPlaybackInfo().and_then(|i| i.PlaybackStatus()).ok()
}

/// Poll an async operation until it completes or `MEDIA_TIMEOUT` passes; an
/// app that never answers must not wedge the worker.
fn wait(status: impl Fn() -> windows::core::Result<AsyncStatus>) {
    let deadline = Instant::now() + MEDIA_TIMEOUT;
    while status().is_ok_and(|s| s == AsyncStatus::Started) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn volumes() -> Vec<(String, f32)> {
        // SAFETY: test thread; COM initialized before use.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            sessions()
                .into_iter()
                .filter(|s| s.GetState().ok() == Some(AudioSessionStateActive))
                .filter_map(|s| {
                    let id = session_id(&s)?;
                    let name = id.rsplit('\\').next().unwrap_or(&id).split('%').next().unwrap_or("").to_string();
                    Some((name, s.cast::<ISimpleAudioVolume>().ok()?.GetMasterVolume().ok()?))
                })
                .collect()
        }
    }

    /// Touches the real mixer: run by hand with something playing.
    /// `cargo test --lib media -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn lowers_then_restores_playing_apps() {
        init();
        let before = volumes();
        begin(MediaMode::Lower, 80);
        std::thread::sleep(Duration::from_millis(1500));
        let during = volumes();
        end();
        shutdown();
        let after = volumes();
        println!("before {before:?}\nduring {during:?}\nafter  {after:?}");
        assert!(!before.is_empty(), "nothing is playing");
        for ((name, b), (_, d)) in before.iter().zip(&during) {
            assert!((d - b * 0.2).abs() < 0.01, "{name} not lowered: {b} -> {d}");
        }
        assert_eq!(before, after);
        assert!(!journal_path().exists());
    }
}
