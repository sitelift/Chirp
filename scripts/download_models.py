#!/usr/bin/env python3
"""Download GGUF models for benchmarking using huggingface_hub Python API."""

import argparse
import os
import sys
from pathlib import Path

from huggingface_hub import hf_hub_download

MODELS = {
    "baseline": [
        ("Qwen/Qwen2.5-1.5B-Instruct-GGUF", "qwen2.5-1.5b-instruct-q4_k_m.gguf", "Qwen 2.5 1.5B (baseline)"),
    ],
    "small": [
        ("bartowski/google_gemma-3-1b-it-GGUF", "google_gemma-3-1b-it-Q4_K_M.gguf", "Gemma 3 1B"),
        ("bartowski/Qwen_Qwen3-0.6B-GGUF", "Qwen_Qwen3-0.6B-Q4_K_M.gguf", "Qwen 3 0.6B"),
        ("bartowski/Qwen_Qwen3-1.7B-GGUF", "Qwen_Qwen3-1.7B-Q4_K_M.gguf", "Qwen 3 1.7B"),
    ],
    "mid": [
        ("bartowski/microsoft_Phi-4-mini-instruct-GGUF", "Phi-4-mini-instruct-Q4_K_M.gguf", "Phi-4-mini 3.8B"),
        ("bartowski/Qwen_Qwen3-4B-GGUF", "Qwen_Qwen3-4B-Q4_K_M.gguf", "Qwen 3 4B"),
        ("bartowski/google_gemma-3-4b-it-GGUF", "google_gemma-3-4b-it-Q4_K_M.gguf", "Gemma 3 4B"),
    ],
    "best": [
        ("bartowski/Qwen_Qwen3-8B-GGUF", "Qwen_Qwen3-8B-Q4_K_M.gguf", "Qwen 3 8B"),
        ("bartowski/Meta-Llama-3.3-8B-Instruct-GGUF", "Meta-Llama-3.3-8B-Instruct-Q4_K_M.gguf", "Llama 3.3 8B"),
        ("bartowski/google_gemma-3-12b-it-GGUF", "google_gemma-3-12b-it-Q4_K_M.gguf", "Gemma 3 12B"),
    ],
}


def download(output_dir: str, tiers: list[str]):
    os.makedirs(output_dir, exist_ok=True)

    for tier in tiers:
        if tier not in MODELS:
            print(f"Unknown tier: {tier}")
            continue
        print(f"\n=== {tier.upper()} ===")
        for repo, filename, label in MODELS[tier]:
            dest = os.path.join(output_dir, filename)
            if os.path.exists(dest):
                print(f"  [SKIP] {label} — already exists")
                continue
            print(f"  [DOWN] {label} from {repo}...")
            try:
                path = hf_hub_download(
                    repo_id=repo,
                    filename=filename,
                    local_dir=output_dir,
                )
                print(f"  [DONE] {label} -> {path}")
            except Exception as e:
                print(f"  [FAIL] {label}: {e}")

    print(f"\nModels in {output_dir}:")
    for f in sorted(Path(output_dir).glob("*.gguf")):
        size_mb = f.stat().st_size / (1024 * 1024)
        print(f"  {f.name} ({size_mb:.0f} MB)")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", default=os.path.expanduser("~/chirp-benchmark-models"))
    parser.add_argument("--tier", nargs="+", default=["baseline", "small", "mid", "best"],
                        choices=["baseline", "small", "mid", "best"])
    args = parser.parse_args()
    download(args.output_dir, args.tier)


if __name__ == "__main__":
    main()
