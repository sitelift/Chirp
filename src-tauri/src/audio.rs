use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use rubato::{FftFixedIn, Resampler};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use crate::state::{AmplitudeData, AudioBuffer, AudioDevice};

/// Holds resampler state so callers can flush remaining samples on stop.
pub struct ResamplerState {
    pub resampler: Option<Arc<std::sync::Mutex<FftFixedIn<f32>>>>,
    pub buffer: Arc<std::sync::Mutex<Vec<f32>>>,
    pub chunk_size: usize,
}

impl ResamplerState {
    /// Flush any leftover samples in the resampler buffer by zero-padding to a full chunk.
    pub fn flush(&self, output: &AudioBuffer) {
        let Some(resampler) = &self.resampler else { return };
        let mut rbuf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
        let remaining = rbuf.len();
        if remaining == 0 {
            return;
        }
        log::info!("Flushing {remaining} resampler samples (chunk_size={})", self.chunk_size);
        // Zero-pad to fill the chunk
        rbuf.resize(self.chunk_size, 0.0);
        let chunk: Vec<f32> = rbuf.drain(..).collect();
        if let Ok(mut rs) = resampler.lock() {
            if let Ok(resampled) = rs.process(&[chunk], None) {
                if let Some(out) = resampled.first() {
                    // Only take the proportional amount of output (not the zero-padded portion)
                    let useful_samples = (remaining as f64 / self.chunk_size as f64 * out.len() as f64).round() as usize;
                    let useful = &out[..useful_samples.min(out.len())];
                    output.lock().unwrap_or_else(|e| e.into_inner()).extend_from_slice(useful);
                    log::info!("Flushed resampler: {remaining} input → {useful_len} output samples", useful_len = useful.len());
                }
            }
        }
    }
}

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

/// Flag set by the audio error callback when a stream error occurs (e.g. device disconnected).
pub type StreamErrorFlag = Arc<AtomicBool>;

/// Flag that callbacks check before writing to the buffer. Set to false to
/// silence zombie callbacks from streams that macOS hasn't fully stopped yet.
pub type StreamActiveFlag = Arc<AtomicBool>;

/// Start audio capture. Returns the stream handle, error flag, active flag, resampler state, and populates the buffer.
pub fn start_capture(
    device_id: &str,
    buffer: AudioBuffer,
    app_handle: AppHandle,
) -> Result<(Stream, StreamErrorFlag, StreamActiveFlag, ResamplerState), String> {
    let device = resolve_device(device_id)?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("No input config: {e}"))?;

    let source_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let sample_format = config.sample_format();

    let device_name = device.name().unwrap_or_else(|_| "unknown".into());
    let needs_resample = source_rate != TARGET_SAMPLE_RATE;
    log::info!("Audio capture: device='{}', rate={}Hz, channels={}, format={:?}, resample={}",
        device_name, source_rate, channels, sample_format, needs_resample);

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

    let stream_active: StreamActiveFlag = Arc::new(AtomicBool::new(true));
    let stream_error = Arc::new(AtomicBool::new(false));
    let stream_error_clone = stream_error.clone();
    let err_fn = move |err: cpal::StreamError| {
        log::error!("Audio stream error: {err}");
        stream_error_clone.store(true, Ordering::SeqCst);
    };

    let stream_config: cpal::StreamConfig = config.clone().into();

    // Each callback closure captures its own active flag clone.
    // When stop_recording sets it to false, zombie callbacks become no-ops.
    let active_f32 = stream_active.clone();
    let active_i16 = stream_active.clone();
    let active_u16 = stream_active.clone();

    let stream = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if !active_f32.load(Ordering::Relaxed) { return; }
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
                if !active_i16.load(Ordering::Relaxed) { return; }
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
                if !active_u16.load(Ordering::Relaxed) { return; }
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

    let resampler_state = ResamplerState {
        resampler,
        buffer: resample_buf,
        chunk_size,
    };

    Ok((stream, stream_error, stream_active, resampler_state))
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
        let mut rbuf = resample_buf.lock().unwrap_or_else(|e| e.into_inner());
        rbuf.extend_from_slice(&mono);

        // Process complete chunks
        while rbuf.len() >= chunk_size {
            let chunk: Vec<f32> = rbuf.drain(..chunk_size).collect();
            if let Ok(mut rs) = resampler.lock() {
                if let Ok(resampled) = rs.process(&[chunk], None) {
                    if let Some(output) = resampled.first() {
                        buffer.lock().unwrap_or_else(|e| e.into_inner()).extend_from_slice(output);
                    }
                }
            }
        }
    } else {
        buffer.lock().unwrap_or_else(|e| e.into_inner()).extend_from_slice(&mono);
    }

    // Emit amplitude data periodically
    let mut counter = amp_counter.lock().unwrap_or_else(|e| e.into_inner());
    *counter += mono.len() as u32;
    if *counter >= amp_interval {
        *counter = 0;
        let buf = buffer.lock().unwrap_or_else(|e| e.into_inner());
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

/// Encode f32 samples as a WAV byte vector
pub fn encode_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>, String> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::new(&mut cursor, spec)
        .map_err(|e| format!("WAV writer init failed: {e}"))?;
    for &s in samples {
        let sample = (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        writer.write_sample(sample).map_err(|e| format!("WAV write failed: {e}"))?;
    }
    writer.finalize().map_err(|e| format!("WAV finalize failed: {e}"))?;
    Ok(cursor.into_inner())
}
