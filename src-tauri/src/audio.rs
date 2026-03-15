use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use rubato::{FftFixedIn, Resampler};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use crate::state::{AmplitudeData, AudioBuffer, AudioDevice};

const TARGET_SAMPLE_RATE: u32 = 16_000;
const BAR_COUNT: usize = 48;

/// List available input devices
pub fn list_devices() -> Vec<AudioDevice> {
    let host = cpal::default_host();
    let mut devices = Vec::new();

    // Add "default" device entry
    if host.default_input_device().is_some() {
        devices.push(AudioDevice {
            name: "Default".into(),
            id: "default".into(),
        });
    }

    if let Ok(input_devices) = host.input_devices() {
        for dev in input_devices {
            if let Ok(name) = dev.name() {
                let id = name.clone();
                devices.push(AudioDevice { name, id });
            }
        }
    }

    devices
}

/// Resolve a device ID to a cpal Device
fn resolve_device(device_id: &str) -> Result<cpal::Device, String> {
    let host = cpal::default_host();

    if device_id == "default" {
        return host
            .default_input_device()
            .ok_or_else(|| "No default input device".into());
    }

    host.input_devices()
        .map_err(|e| format!("Failed to enumerate devices: {e}"))?
        .find(|d| d.name().map(|n| n == device_id).unwrap_or(false))
        .ok_or_else(|| format!("Device not found: {device_id}"))
}

/// Start audio capture. Returns the stream handle (must be kept alive) and populates the buffer.
pub fn start_capture(
    device_id: &str,
    buffer: AudioBuffer,
    app_handle: AppHandle,
) -> Result<Stream, String> {
    let device = resolve_device(device_id)?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("No input config: {e}"))?;

    let source_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let sample_format = config.sample_format();

    let needs_resample = source_rate != TARGET_SAMPLE_RATE;

    // Rubato resampler (if needed)
    let resampler = if needs_resample {
        let chunk_size = (source_rate as usize) / 100; // 10ms chunks
        Some(Arc::new(std::sync::Mutex::new(
            FftFixedIn::<f32>::new(source_rate as usize, TARGET_SAMPLE_RATE as usize, chunk_size, 1, 1)
                .map_err(|e| format!("Resampler init failed: {e}"))?,
        )))
    } else {
        None
    };

    // Accumulator for resampler input chunks
    let resample_buf: Arc<std::sync::Mutex<Vec<f32>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));

    let chunk_size = if needs_resample {
        (source_rate as usize) / 100
    } else {
        0
    };

    // Amplitude emission counter
    let amp_counter = Arc::new(std::sync::Mutex::new(0u32));
    let amp_interval = source_rate / 60; // ~60fps amplitude updates

    let buffer_clone = buffer.clone();
    let resampler_clone = resampler.clone();
    let resample_buf_clone = resample_buf.clone();

    let err_fn = |err: cpal::StreamError| {
        log::error!("Audio stream error: {err}");
    };

    let stream_config: cpal::StreamConfig = config.clone().into();

    let stream = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                process_audio(
                    data,
                    channels,
                    &buffer_clone,
                    &resampler_clone,
                    &resample_buf_clone,
                    chunk_size,
                    &amp_counter,
                    amp_interval,
                    &app_handle,
                );
            },
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &stream_config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                let float_data: Vec<f32> = data
                    .iter()
                    .map(|&s| s as f32 / i16::MAX as f32)
                    .collect();
                process_audio(
                    &float_data,
                    channels,
                    &buffer_clone,
                    &resampler_clone,
                    &resample_buf_clone,
                    chunk_size,
                    &amp_counter,
                    amp_interval,
                    &app_handle,
                );
            },
            err_fn,
            None,
        ),
        SampleFormat::U16 => device.build_input_stream(
            &stream_config,
            move |data: &[u16], _: &cpal::InputCallbackInfo| {
                let float_data: Vec<f32> = data
                    .iter()
                    .map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0)
                    .collect();
                process_audio(
                    &float_data,
                    channels,
                    &buffer_clone,
                    &resampler_clone,
                    &resample_buf_clone,
                    chunk_size,
                    &amp_counter,
                    amp_interval,
                    &app_handle,
                );
            },
            err_fn,
            None,
        ),
        _ => return Err(format!("Unsupported sample format: {:?}", sample_format)),
    }
    .map_err(|e| format!("Failed to build stream: {e}"))?;

    stream.play().map_err(|e| format!("Failed to start stream: {e}"))?;
    Ok(stream)
}

/// Process incoming audio: downmix to mono, resample if needed, buffer, emit amplitude
fn process_audio(
    data: &[f32],
    channels: usize,
    buffer: &AudioBuffer,
    resampler: &Option<Arc<std::sync::Mutex<FftFixedIn<f32>>>>,
    resample_buf: &Arc<std::sync::Mutex<Vec<f32>>>,
    chunk_size: usize,
    amp_counter: &Arc<std::sync::Mutex<u32>>,
    amp_interval: u32,
    app_handle: &AppHandle,
) {
    // Downmix to mono
    let mono: Vec<f32> = if channels > 1 {
        data.chunks(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        data.to_vec()
    };

    if let Some(resampler) = resampler {
        // Accumulate for resampler
        let mut rbuf = resample_buf.lock().unwrap();
        rbuf.extend_from_slice(&mono);

        // Process complete chunks
        while rbuf.len() >= chunk_size {
            let chunk: Vec<f32> = rbuf.drain(..chunk_size).collect();
            if let Ok(mut rs) = resampler.lock() {
                if let Ok(resampled) = rs.process(&[chunk], None) {
                    if let Some(output) = resampled.first() {
                        buffer.lock().unwrap().extend_from_slice(output);
                    }
                }
            }
        }
    } else {
        buffer.lock().unwrap().extend_from_slice(&mono);
    }

    // Emit amplitude data periodically
    let mut counter = amp_counter.lock().unwrap();
    *counter += mono.len() as u32;
    if *counter >= amp_interval {
        *counter = 0;
        let buf = buffer.lock().unwrap();
        let bars = compute_amplitude_bars(&buf, BAR_COUNT);
        let _ = app_handle.emit("amplitude-data", AmplitudeData { bars });
    }
}

/// Compute RMS amplitude for N bars from the tail of the buffer
fn compute_amplitude_bars(samples: &[f32], bar_count: usize) -> Vec<f32> {
    if samples.is_empty() {
        return vec![0.0; bar_count];
    }

    // Use the last ~0.5s of audio (8000 samples at 16kHz)
    let window = 8000.min(samples.len());
    let tail = &samples[samples.len() - window..];
    let chunk_size = (tail.len() / bar_count).max(1);

    (0..bar_count)
        .map(|i| {
            let start = i * chunk_size;
            let end = (start + chunk_size).min(tail.len());
            if start >= tail.len() {
                return 0.0;
            }
            let chunk = &tail[start..end];
            let rms = (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt();
            // Scale up for visibility (RMS of speech is typically 0.01-0.1)
            (rms * 12.0).min(1.0)
        })
        .collect()
}

/// Check if trailing N seconds of the buffer are below threshold (silence detection)
pub fn detect_silence(buffer: &[f32], seconds: f32, threshold: f32) -> bool {
    let sample_count = (TARGET_SAMPLE_RATE as f32 * seconds) as usize;
    if buffer.len() < sample_count {
        return false;
    }
    let tail = &buffer[buffer.len() - sample_count..];
    let rms = (tail.iter().map(|s| s * s).sum::<f32>() / tail.len() as f32).sqrt();
    rms < threshold
}
