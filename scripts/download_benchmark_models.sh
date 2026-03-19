#!/bin/bash
# Download GGUF models for benchmarking
# Requires: huggingface-cli (pip install huggingface_hub)
#
# Usage: ./scripts/download_benchmark_models.sh [output_dir]
#        ./scripts/download_benchmark_models.sh --tier small|mid|best|all

set -euo pipefail

OUTPUT_DIR="${1:-$HOME/chirp-benchmark-models}"
TIER="${2:-all}"

mkdir -p "$OUTPUT_DIR"

echo "Downloading benchmark models to: $OUTPUT_DIR"
echo "Tier: $TIER"
echo ""

download_model() {
    local repo="$1"
    local file="$2"
    local label="$3"
    local dest="$OUTPUT_DIR/$file"

    if [ -f "$dest" ]; then
        echo "  [SKIP] $label — already exists"
        return
    fi

    echo "  [DOWN] $label..."
    huggingface-cli download "$repo" "$file" --local-dir "$OUTPUT_DIR" --quiet
    echo "  [DONE] $label"
}

# Current baseline
echo "=== BASELINE ==="
download_model \
    "Qwen/Qwen2.5-1.5B-Instruct-GGUF" \
    "qwen2.5-1.5b-instruct-q4_k_m.gguf" \
    "Qwen 2.5 1.5B (current baseline)"

if [ "$TIER" = "small" ] || [ "$TIER" = "all" ]; then
    echo ""
    echo "=== SMALL / FAST TIER ==="
    download_model \
        "bartowski/Qwen_Qwen3.5-0.6B-Instruct-GGUF" \
        "Qwen3.5-0.6B-Instruct-Q4_K_M.gguf" \
        "Qwen 3.5 0.6B"
    download_model \
        "bartowski/Qwen_Qwen3.5-1.5B-Instruct-GGUF" \
        "Qwen3.5-1.5B-Instruct-Q4_K_M.gguf" \
        "Qwen 3.5 1.5B"
    download_model \
        "bartowski/google_gemma-3-1b-it-GGUF" \
        "google_gemma-3-1b-it-Q4_K_M.gguf" \
        "Gemma 3 1B"
fi

if [ "$TIER" = "mid" ] || [ "$TIER" = "all" ]; then
    echo ""
    echo "=== MID / BALANCED TIER ==="
    download_model \
        "bartowski/microsoft_Phi-4-mini-instruct-GGUF" \
        "Phi-4-mini-instruct-Q4_K_M.gguf" \
        "Phi-4-mini 3.8B"
    download_model \
        "bartowski/Qwen_Qwen3.5-4B-Instruct-GGUF" \
        "Qwen3.5-4B-Instruct-Q4_K_M.gguf" \
        "Qwen 3.5 4B"
    download_model \
        "bartowski/google_gemma-3-4b-it-GGUF" \
        "google_gemma-3-4b-it-Q4_K_M.gguf" \
        "Gemma 3 4B"
fi

if [ "$TIER" = "best" ] || [ "$TIER" = "all" ]; then
    echo ""
    echo "=== BEST / QUALITY TIER ==="
    download_model \
        "bartowski/Qwen_Qwen3.5-8B-Instruct-GGUF" \
        "Qwen3.5-8B-Instruct-Q4_K_M.gguf" \
        "Qwen 3.5 8B"
    download_model \
        "bartowski/Meta-Llama-3.3-8B-Instruct-GGUF" \
        "Meta-Llama-3.3-8B-Instruct-Q4_K_M.gguf" \
        "Llama 3.3 8B"
    download_model \
        "bartowski/google_gemma-3-12b-it-GGUF" \
        "google_gemma-3-12b-it-Q4_K_M.gguf" \
        "Gemma 3 12B"
fi

echo ""
echo "=== Downloaded models ==="
ls -lhS "$OUTPUT_DIR"/*.gguf 2>/dev/null || echo "No models found"
echo ""
echo "Run benchmarks with:"
echo "  python scripts/benchmark.py $OUTPUT_DIR/<model>.gguf"
