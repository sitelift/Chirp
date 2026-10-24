//! Live input levels for every microphone while the picker is open.
//!
//! One thread owns a lightweight cpal stream per input device (streams are
//! `!Send`), keeps a peak-hold level per device and emits `device-levels`
//! about 15 times a second. It stops when the picker closes, and on its own
//! after a minute as a safety net.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

static STOP: Mutex<Option<crossbeam_channel::Sender<()>>> = Mutex::new(None);

#[derive(Serialize, Clone)]
pub struct DeviceLevel {
    pub id: String,
    pub level: f32,
}

fn stream_for(device: &cpal::Device, level: Arc<AtomicU32>) -> Option<cpal::Stream> {
    let config = device.default_input_config().ok()?;
    let format = config.sample_format();
    let config: cpal::StreamConfig = config.into();
    let update = move |rms: f32| {
        let v = (rms * 6.0).min(1.0);
        let prev = f32::from_bits(level.load(Ordering::Relaxed));
        if v > prev {
            level.store(v.to_bits(), Ordering::Relaxed);
        }
    };
    fn rms(it: impl Iterator<Item = f32>) -> f32 {
        let (mut sum, mut n) = (0.0f32, 0usize);
        for s in it {
            sum += s * s;
            n += 1;
        }
        if n == 0 { 0.0 } else { (sum / n as f32).sqrt() }
    }
    let err = |e| log::debug!("level monitor stream error: {e}");
    let stream = match format {
        cpal::SampleFormat::F32 => device.build_input_stream(&config, move |d: &[f32], _: &_| update(rms(d.iter().copied())), err, None),
        cpal::SampleFormat::I16 => device.build_input_stream(&config, move |d: &[i16], _: &_| update(rms(d.iter().map(|&s| s as f32 / i16::MAX as f32))), err, None),
        cpal::SampleFormat::U16 => device.build_input_stream(&config, move |d: &[u16], _: &_| update(rms(d.iter().map(|&s| s as f32 / u16::MAX as f32 * 2.0 - 1.0))), err, None),
        _ => return None,
    }
    .ok()?;
    stream.play().ok()?;
    Some(stream)
}

#[tauri::command]
pub fn start_level_monitor(app: AppHandle) {
    stop_level_monitor();
    let (tx, rx) = crossbeam_channel::bounded::<()>(1);
    *STOP.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
    let _ = std::thread::Builder::new().name("chirp-levels".into()).spawn(move || {
        let host = cpal::default_host();
        let default_name = host.default_input_device().and_then(|d| d.name().ok());
        let mut meters: Vec<(String, Arc<AtomicU32>, cpal::Stream)> = Vec::new();
        if let Ok(devices) = host.input_devices() {
            for device in devices {
                let Ok(name) = device.name() else { continue };
                let level = Arc::new(AtomicU32::new(0));
                if let Some(stream) = stream_for(&device, level.clone()) {
                    meters.push((name, level, stream));
                }
            }
        }
        let started = Instant::now();
        loop {
            match rx.recv_timeout(Duration::from_millis(66)) {
                Err(crossbeam_channel::RecvTimeoutError::Timeout) if started.elapsed() < Duration::from_secs(60) => {}
                _ => break,
            }
            let mut out: Vec<DeviceLevel> = meters
                .iter()
                .map(|(id, level, _)| {
                    let v = f32::from_bits(level.load(Ordering::Relaxed));
                    level.store((v * 0.55).to_bits(), Ordering::Relaxed);
                    DeviceLevel { id: id.clone(), level: v }
                })
                .collect();
            if let Some(name) = &default_name {
                if let Some(l) = out.iter().find(|d| &d.id == name).map(|d| d.level) {
                    out.push(DeviceLevel { id: "default".into(), level: l });
                }
            }
            let _ = app.emit("device-levels", out);
        }
        drop(meters);
    });
}

#[tauri::command]
pub fn stop_level_monitor() {
    if let Some(tx) = STOP.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = tx.send(());
    }
}
