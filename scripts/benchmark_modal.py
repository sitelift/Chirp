#!/usr/bin/env python3
"""
Chirp Cloud Pipeline Benchmark (Modal — multi-GPU)

Benchmarks the full Chirp voice-to-text pipeline across Modal GPU types:
  - Parakeet TDT v3 0.6B int8 on CPU (sherpa-onnx)
  - Gemma 4 E2B-it Q4_K_M on GPU (llama-cpp-python)

Sends raw f32 PCM bytes (16kHz mono) — matches production where the
desktop app already has resampled audio in its buffer.

Usage:
    modal run scripts/benchmark_modal.py --audio path/to/recording.wav
"""

import math
import time

import modal

app = modal.App("chirp-benchmark")

volume = modal.Volume.from_name("chirp-models", create_if_missing=True)
MODELS_DIR = "/models"

image = (
    modal.Image.from_registry(
        "nvidia/cuda:12.4.1-runtime-ubuntu22.04", add_python="3.11"
    )
    .entrypoint([])
    .apt_install("wget", "bzip2", "libgomp1")
    .pip_install("sherpa-onnx", "numpy")
    .pip_install(
        "llama-cpp-python",
        extra_options="--extra-index-url https://abetlen.github.io/llama-cpp-python/whl/cu124",
    )
)

# Exact BASE_SYSTEM_PROMPT from src-tauri/src/llm.rs:120-153
SYSTEM_PROMPT = """\
You clean up speech-to-text output. Make it read like the person typed it themselves. Preserve their voice and tone.

Remove:
- Filler sounds: um, uh, uh huh, hmm, mmhmm
- Filler "like" (but keep meaningful "like" as in "I like pizza" or "something like that")
- Stutters and repeated words

Keep exactly as spoken:
- Words like alright, yeah, so, also, I mean, basically, honestly
- Profanity
- The beginning of sentences -- never drop opening words
- The speaker's exact phrasing -- do not rephrase or restructure

Self-corrections: when someone says "X actually no Y" or "X wait Y" or "X no I mean Y", they are correcting X to Y. Remove X entirely and keep Y.

Format:
- Ordinal lists (first/second/third, number one/two/three) -> numbered list
- "New paragraph" or "new line" -> actual line break

Examples:
Input: So I think we should launch on Tuesday actually no Wednesday because um the design team needs one more day to finish the icons
Output: So I think we should launch on Wednesday because the design team needs one more day to finish the icons.

Input: Send it to John no I mean send it to Mike
Output: Send it to Mike.

Input: For the release we need to number one finish the migration number two update the docs and number three notify the customers
Output: For the release we need to:
1. Finish the migration
2. Update the docs
3. Notify the customers

Output only the cleaned text."""

PARAKEET_DIR = f"{MODELS_DIR}/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8"
GEMMA_PATH = f"{MODELS_DIR}/gemma-4-E2B-it-Q4_K_M.gguf"

# Modal GPU pricing ($/sec)
GPU_RATES = {
    "T4":    0.000164,
    "L4":    0.000222,
    "A10G":  0.000306,
    "L40S":  0.000542,
    "A100":  0.000583,
    "H100":  0.001097,
}


def download_models():
    """Download models to volume if not already present."""
    import os
    import subprocess

    if not os.path.exists(f"{PARAKEET_DIR}/tokens.txt"):
        print("Downloading Parakeet TDT v3 0.6B int8...")
        subprocess.run([
            "wget", "-q",
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2",
            "-O", f"{MODELS_DIR}/parakeet.tar.bz2",
        ], check=True)
        subprocess.run(["tar", "xjf", f"{MODELS_DIR}/parakeet.tar.bz2", "-C", MODELS_DIR], check=True)
        os.remove(f"{MODELS_DIR}/parakeet.tar.bz2")
        print("Parakeet downloaded.")
    else:
        print("Parakeet already on volume.")

    if not os.path.exists(GEMMA_PATH):
        print("Downloading Gemma 4 E2B-it Q4_K_M...")
        subprocess.run([
            "wget", "-q",
            "https://huggingface.co/unsloth/gemma-4-E2B-it-GGUF/resolve/main/gemma-4-E2B-it-Q4_K_M.gguf",
            "-O", GEMMA_PATH,
        ], check=True)
        print("Gemma downloaded.")
    else:
        print("Gemma already on volume.")

    volume.commit()


def _load_models(self):
    import glob
    import sherpa_onnx

    download_models()

    t0 = time.perf_counter()
    encoder = glob.glob(f"{PARAKEET_DIR}/*encoder*.onnx")[0]
    decoder = glob.glob(f"{PARAKEET_DIR}/*decoder*.onnx")[0]
    joiner = glob.glob(f"{PARAKEET_DIR}/*joiner*.onnx")[0]
    tokens = f"{PARAKEET_DIR}/tokens.txt"

    self.recognizer = sherpa_onnx.OfflineRecognizer.from_transducer(
        encoder=encoder, decoder=decoder, joiner=joiner, tokens=tokens,
        num_threads=4, sample_rate=16000, feature_dim=80,
        decoding_method="greedy_search", max_active_paths=4,
        provider="cpu", model_type="nemo_transducer",
    )
    self.parakeet_load_time = time.perf_counter() - t0

    t0 = time.perf_counter()
    from llama_cpp import Llama
    self.llm = Llama(
        model_path=GEMMA_PATH, n_ctx=2048, n_gpu_layers=99,
        flash_attn=True, n_batch=512, verbose=False,
    )
    self.gemma_load_time = time.perf_counter() - t0


def _transcribe(self, samples):
    sample_rate = 16000
    chunk_samples = int(30.0 * sample_rate)
    step = chunk_samples - int(1.0 * sample_rate)

    if len(samples) <= chunk_samples:
        chunks = [samples]
    else:
        chunks = []
        pos = 0
        while pos < len(samples):
            end = min(pos + chunk_samples, len(samples))
            chunks.append(samples[pos:end])
            if end >= len(samples):
                break
            pos += step

    t0 = time.perf_counter()
    segments = []
    for chunk in chunks:
        stream = self.recognizer.create_stream()
        stream.accept_waveform(sample_rate, chunk.tolist())
        self.recognizer.decode_stream(stream)
        text = stream.result.text.strip()
        if text:
            segments.append(text)
    elapsed = time.perf_counter() - t0

    if not segments:
        return "", elapsed
    merged = segments[0]
    for next_seg in segments[1:]:
        prev_words = merged.split()
        next_words = next_seg.split()
        max_check = min(len(prev_words), len(next_words), 8)
        best_overlap = 0
        for length in range(1, max_check + 1):
            suffix = prev_words[-length:]
            prefix = next_words[:length]
            if all(
                a.lower().strip(".,!?;:") == b.lower().strip(".,!?;:")
                for a, b in zip(suffix, prefix)
            ):
                best_overlap = length
        if best_overlap > 0:
            remainder = " ".join(next_words[best_overlap:])
            if remainder:
                merged += " " + remainder
        else:
            merged += " " + next_seg
    return merged, elapsed


def _cleanup(self, text):
    input_tokens = len(self.llm.tokenize(text.encode()))
    max_tokens = max(math.ceil(input_tokens * 1.2), 64)

    t0 = time.perf_counter()
    result = self.llm.create_chat_completion(
        messages=[
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": text},
        ],
        temperature=0.3, top_p=0.95, top_k=64, max_tokens=max_tokens,
    )
    elapsed = time.perf_counter() - t0

    cleaned = result["choices"][0]["message"]["content"].strip()
    if not cleaned:
        return text, elapsed
    input_words = len(text.split())
    output_words = len(cleaned.split())
    if output_words > input_words * 2 + 15:
        return text, elapsed
    return cleaned, elapsed


def _run_pipeline(self, audio_bytes: bytes) -> dict:
    import numpy as np
    samples = np.frombuffer(audio_bytes, dtype=np.float32)
    audio_duration = len(samples) / 16000.0
    transcript, parakeet_time = _transcribe(self, samples)
    cleaned, gemma_time = _cleanup(self, transcript)
    return {
        "audio_duration": audio_duration,
        "transcript": transcript,
        "cleaned": cleaned,
        "parakeet_time": parakeet_time,
        "gemma_time": gemma_time,
        "pipeline_time": parakeet_time + gemma_time,
        "parakeet_load_time": self.parakeet_load_time,
        "gemma_load_time": self.gemma_load_time,
    }


# --- One class per GPU type ---

@app.cls(gpu="T4", image=image, volumes={MODELS_DIR: volume}, timeout=600)
class PipelineT4:
    @modal.enter()
    def load_models(self): _load_models(self)
    @modal.method()
    def run(self, audio_bytes: bytes) -> dict: return _run_pipeline(self, audio_bytes)

@app.cls(gpu="L4", image=image, volumes={MODELS_DIR: volume}, timeout=600)
class PipelineL4:
    @modal.enter()
    def load_models(self): _load_models(self)
    @modal.method()
    def run(self, audio_bytes: bytes) -> dict: return _run_pipeline(self, audio_bytes)

@app.cls(gpu="A10G", image=image, volumes={MODELS_DIR: volume}, timeout=600)
class PipelineA10G:
    @modal.enter()
    def load_models(self): _load_models(self)
    @modal.method()
    def run(self, audio_bytes: bytes) -> dict: return _run_pipeline(self, audio_bytes)

@app.cls(gpu="L40S", image=image, volumes={MODELS_DIR: volume}, timeout=600)
class PipelineL40S:
    @modal.enter()
    def load_models(self): _load_models(self)
    @modal.method()
    def run(self, audio_bytes: bytes) -> dict: return _run_pipeline(self, audio_bytes)

@app.cls(gpu="A100", image=image, volumes={MODELS_DIR: volume}, timeout=600)
class PipelineA100:
    @modal.enter()
    def load_models(self): _load_models(self)
    @modal.method()
    def run(self, audio_bytes: bytes) -> dict: return _run_pipeline(self, audio_bytes)

@app.cls(gpu="H100", image=image, volumes={MODELS_DIR: volume}, timeout=600)
class PipelineH100:
    @modal.enter()
    def load_models(self): _load_models(self)
    @modal.method()
    def run(self, audio_bytes: bytes) -> dict: return _run_pipeline(self, audio_bytes)


GPU_CLASSES = {
    "T4":   PipelineT4,
    "L4":   PipelineL4,
    "A10G": PipelineA10G,
    "L40S": PipelineL40S,
    "A100": PipelineA100,
    "H100": PipelineH100,
}


@app.local_entrypoint()
def main(audio: str = r"C:\Users\dutch\OneDrive\Documents\Sound Recordings\Recording.wav"):
    import numpy as np
    import soundfile as sf

    data, sr = sf.read(audio, dtype="float32")
    if len(data.shape) > 1:
        data = data.mean(axis=1)
    if sr != 16000:
        duration = len(data) / sr
        target_len = int(duration * 16000)
        indices = np.linspace(0, len(data) - 1, target_len)
        data = np.interp(indices, np.arange(len(data)), data).astype(np.float32)

    pcm_bytes = data.tobytes()
    audio_duration = len(data) / 16000.0

    results = {}

    for gpu_name, cls in GPU_CLASSES.items():
        print(f"\n--- Benchmarking {gpu_name} ---")
        pipeline = cls()

        try:
            # Cold run
            cold_start = time.perf_counter()
            cold = pipeline.run.remote(pcm_bytes)
            cold_wall = time.perf_counter() - cold_start

            # Warm run
            warm_start = time.perf_counter()
            warm = pipeline.run.remote(pcm_bytes)
            warm_wall = time.perf_counter() - warm_start

            results[gpu_name] = {
                "cold_wall": cold_wall,
                "cold_overhead": cold_wall - cold["pipeline_time"],
                "cold_parakeet": cold["parakeet_time"],
                "cold_gemma": cold["gemma_time"],
                "cold_pipeline": cold["pipeline_time"],
                "warm_wall": warm_wall,
                "warm_parakeet": warm["parakeet_time"],
                "warm_gemma": warm["gemma_time"],
                "warm_pipeline": warm["pipeline_time"],
                "parakeet_load": cold["parakeet_load_time"],
                "gemma_load": cold["gemma_load_time"],
                "transcript": cold["transcript"],
            }
            print(f"  Cold: {cold_wall:.2f}s  Warm: {warm_wall:.2f}s  Pipeline: {warm['pipeline_time']:.2f}s")
        except Exception as e:
            print(f"  FAILED: {e}")
            results[gpu_name] = None

    # --- Print comparison table ---
    total_requests = 500 * 750

    print(f"\n{'=' * 100}")
    print(f"  Chirp Cloud Benchmark — Multi-GPU Comparison")
    print(f"  Audio: {audio_duration:.1f}s | 500 users x 750 req/mo | 15s scaledown idle | $8/mo price")
    print(f"{'=' * 100}")

    header = f"{'GPU':<7} {'$/hr':>6} {'Cold':>7} {'Warm':>7} {'Parakeet':>9} {'Gemma':>7} {'Load P':>7} {'Load G':>7} {'$/req':>10} {'$/user':>8} {'Margin':>8}"
    print(f"\n{header}")
    print("-" * len(header))

    for gpu_name in GPU_CLASSES:
        r = results.get(gpu_name)
        if r is None:
            print(f"{gpu_name:<7} {'FAILED':>6}")
            continue

        rate = GPU_RATES[gpu_name]
        hr_rate = rate * 3600
        cost_per_req = r["warm_wall"] * rate
        gpu_sec_with_idle = r["warm_wall"] + 15.0
        monthly_cost = total_requests * gpu_sec_with_idle * rate
        cost_per_user = monthly_cost / 500
        margin = (8.0 - cost_per_user) / 8.0 * 100

        print(
            f"{gpu_name:<7} "
            f"${hr_rate:>5.2f} "
            f"{r['cold_wall']:>6.2f}s "
            f"{r['warm_wall']:>6.2f}s "
            f"{r['warm_parakeet']:>8.2f}s "
            f"{r['warm_gemma']:>6.2f}s "
            f"{r['parakeet_load']:>6.2f}s "
            f"{r['gemma_load']:>6.2f}s "
            f"${cost_per_req:>9.6f} "
            f"${cost_per_user:>6.2f} "
            f"{margin:>6.1f}%"
        )

    print(f"\nTranscript: \"{list(r for r in results.values() if r)[-1]['transcript'][:300]}\"")
    print()
