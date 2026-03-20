use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decode an audio file to mono f32 samples at 16kHz
pub fn decode_audio_file(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("Failed to open file: {e}"))?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("Failed to probe audio format: {e}"))?;

    let mut format = probed.format;

    let track = format
        .default_track()
        .ok_or("No audio track found")?;

    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(1);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Failed to create decoder: {e}"))?;

    let mut all_samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => {
                log::warn!("Error reading packet: {e}");
                break;
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(e) => {
                log::warn!("Decode error: {e}");
                continue;
            }
        };

        let spec = *decoded.spec();
        let num_frames = decoded.capacity();

        if num_frames == 0 {
            continue;
        }

        let mut sample_buf = SampleBuffer::<f32>::new(num_frames as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);
        let samples = sample_buf.samples();

        // Downmix to mono
        if channels > 1 {
            for chunk in samples.chunks(channels) {
                let mono = chunk.iter().sum::<f32>() / channels as f32;
                all_samples.push(mono);
            }
        } else {
            all_samples.extend_from_slice(samples);
        }
    }

    if all_samples.is_empty() {
        return Err("No audio data decoded".to_string());
    }

    // Resample to 16kHz if needed
    let target_rate = 16000u32;
    if sample_rate != target_rate {
        let resampled = resample_to_16k(&all_samples, sample_rate, target_rate)?;
        Ok((resampled, target_rate))
    } else {
        Ok((all_samples, sample_rate))
    }
}

/// Resample audio to 16kHz using rubato
fn resample_to_16k(samples: &[f32], from_rate: u32, to_rate: u32) -> Result<Vec<f32>, String> {
    use rubato::{FftFixedIn, Resampler};

    let chunk_size = from_rate as usize / 100; // 10ms chunks
    let mut resampler = FftFixedIn::<f32>::new(
        from_rate as usize,
        to_rate as usize,
        chunk_size,
        1,
        1,
    )
    .map_err(|e| format!("Resampler init failed: {e}"))?;

    let mut output = Vec::new();
    let mut pos = 0;

    while pos + chunk_size <= samples.len() {
        let chunk = samples[pos..pos + chunk_size].to_vec();
        match resampler.process(&[chunk], None) {
            Ok(resampled) => {
                if let Some(ch) = resampled.first() {
                    output.extend_from_slice(ch);
                }
            }
            Err(e) => {
                log::warn!("Resample error at pos {pos}: {e}");
            }
        }
        pos += chunk_size;
    }

    Ok(output)
}

/// Split audio into overlapping chunks for transcription
pub fn chunk_audio(samples: &[f32], sample_rate: u32, chunk_secs: f32, overlap_secs: f32) -> Vec<&[f32]> {
    let chunk_samples = (chunk_secs * sample_rate as f32) as usize;
    let overlap_samples = (overlap_secs * sample_rate as f32) as usize;
    let step = chunk_samples - overlap_samples;

    if samples.len() <= chunk_samples {
        return vec![samples];
    }

    let mut chunks = Vec::new();
    let mut pos = 0;

    while pos < samples.len() {
        let end = (pos + chunk_samples).min(samples.len());
        chunks.push(&samples[pos..end]);
        if end >= samples.len() {
            break;
        }
        pos += step;
    }

    chunks
}

/// Merge transcriptions from overlapping chunks, deduplicating words at boundaries.
/// Finds the longest suffix of chunk N that matches a prefix of chunk N+1 and removes it.
pub fn merge_transcriptions(segments: Vec<String>) -> String {
    let segments: Vec<&str> = segments.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return String::new();
    }

    let mut merged = segments[0].to_string();

    for next in &segments[1..] {
        let prev_words: Vec<&str> = merged.split_whitespace().collect();
        let next_words: Vec<&str> = next.split_whitespace().collect();

        // Look for the longest suffix of prev that matches a prefix of next.
        // Only check up to 8 words (overlap region is small).
        let max_check = prev_words.len().min(next_words.len()).min(8);
        let mut best_overlap = 0;

        for len in 1..=max_check {
            let suffix = &prev_words[prev_words.len() - len..];
            let prefix = &next_words[..len];
            if suffix.iter().zip(prefix.iter()).all(|(a, b)| {
                a.to_lowercase().trim_matches(|c: char| c.is_ascii_punctuation())
                    == b.to_lowercase().trim_matches(|c: char| c.is_ascii_punctuation())
            }) {
                best_overlap = len;
            }
        }

        if best_overlap > 0 {
            // Skip the overlapping prefix from the next segment
            let remainder = next_words[best_overlap..].join(" ");
            if !remainder.is_empty() {
                merged.push(' ');
                merged.push_str(&remainder);
            }
        } else {
            merged.push(' ');
            merged.push_str(next);
        }
    }

    merged
}
