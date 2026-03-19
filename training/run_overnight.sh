#!/bin/bash
# Overnight training run for Chirp transcript cleanup model
#
# What this does:
#   1. Trains Qwen 2.5 1.5B with LoRA on 11,656 cleaned transcript pairs
#   2. Tests the fine-tuned model
#   3. Fuses LoRA adapters into base model
#   4. Exports to GGUF for llama-server deployment
#
# Expected time: ~4-6 hours on M4 Mac Mini
# Peak memory: ~4.5 GB
#
# Usage: ./training/run_overnight.sh

set -euo pipefail

cd "$(dirname "$0")/.."

echo "============================================"
echo "  Chirp Overnight Training Run"
echo "  Started: $(date)"
echo "============================================"
echo ""

# Step 1: Clean data (if not already done)
if [ ! -f training/data/training_qwen.jsonl ]; then
    echo "Step 0: Cleaning training data..."
    python3 training/clean_data.py
    echo ""
fi

PAIRS=$(wc -l < training/data/training_qwen.jsonl | tr -d ' ')
echo "Training pairs: $PAIRS"
echo ""

# Step 2: Train
# 3 epochs over ~10,500 train samples at batch size 4 = ~7,875 iters
# At ~1.7 it/sec = ~4,600 seconds = ~1.3 hours per epoch
echo "Step 1: Training (3 epochs, ~7,875 iterations)..."
echo "  Estimated time: 4-6 hours"
echo ""

python3 training/train_qwen.py \
    --iters 7875 \
    --batch-size 4 \
    --lr 1e-5 \
    2>&1 | tee training/output/training_log.txt

echo ""
echo "Step 2: Exporting to GGUF..."
python3 training/export_gguf.py 2>&1 | tee -a training/output/training_log.txt

echo ""
echo "============================================"
echo "  1.5B Training Complete!"
echo "  $(date)"
echo "============================================"

# ==========================================
# ROUND 2: Qwen 3 0.6B (smaller, faster)
# ==========================================
echo ""
echo "============================================"
echo "  Starting Qwen 3 0.6B Fine-Tuning"
echo "  $(date)"
echo "============================================"
echo ""

# 0.6B is smaller so more iters/sec, but same 3 epochs
# ~10,500 train samples / batch 4 = ~2,625 iters per epoch
# 3 epochs = ~7,875 iters (same count, just runs faster)
python3 training/train_qwen.py \
    --model "Qwen/Qwen3-0.6B" \
    --iters 7875 \
    --batch-size 4 \
    --lr 2e-5 \
    2>&1 | tee training/output/training_log_0.6b.txt

echo ""
echo "Exporting 0.6B to GGUF..."
python3 training/export_gguf.py \
    --fused-path training/output/qwen3-0-6b-lora/fused \
    --output training/output/qwen3-0-6b-lora/chirp-cleanup-0.6b-q4_k_m.gguf \
    2>&1 | tee -a training/output/training_log_0.6b.txt

echo ""
echo "============================================"
echo "  ALL TRAINING COMPLETE!"
echo "  Finished: $(date)"
echo "============================================"
echo ""
echo "Models ready for testing:"
echo "  1.5B: training/output/qwen-lora/chirp-cleanup-q4_k_m.gguf"
echo "  0.6B: training/output/qwen3-0-6b-lora/chirp-cleanup-0.6b-q4_k_m.gguf"
echo ""
echo "Run benchmarks:"
echo "  python3 scripts/benchmark.py training/output/qwen-lora/chirp-cleanup-q4_k_m.gguf"
echo "  python3 scripts/benchmark.py training/output/qwen3-0-6b-lora/chirp-cleanup-0.6b-q4_k_m.gguf"
