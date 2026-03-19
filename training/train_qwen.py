#!/usr/bin/env python3
"""
Fine-tune Qwen 2.5 1.5B Instruct for transcript cleanup using MLX on Apple Silicon.

Uses LoRA (Low-Rank Adaptation) for efficient fine-tuning.
Exports to GGUF for deployment with llama-server.

Usage:
    python training/train_qwen.py                    # Full training
    python training/train_qwen.py --epochs 1 --test  # Quick test run
    python training/train_qwen.py --resume            # Resume from checkpoint
"""

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent
DATA_FILE = SCRIPT_DIR / "data" / "training_qwen.jsonl"
DEFAULT_OUTPUT_DIR = SCRIPT_DIR / "output" / "qwen-lora"
DEFAULT_MODEL = "Qwen/Qwen2.5-1.5B-Instruct"

# These get set in main() based on args
OUTPUT_DIR = DEFAULT_OUTPUT_DIR
MODEL_NAME = DEFAULT_MODEL


def check_deps():
    """Verify all dependencies are installed."""
    try:
        import mlx
        import mlx_lm
        print(f"MLX ready")
    except ImportError:
        print("ERROR: mlx-lm not installed. Run: pip3 install mlx-lm")
        sys.exit(1)

    if not DATA_FILE.exists():
        print(f"ERROR: Training data not found at {DATA_FILE}")
        print("Run: python training/clean_data.py")
        sys.exit(1)


def split_data():
    """Split data into train/valid/test sets."""
    import random
    random.seed(42)

    with open(DATA_FILE) as f:
        data = [json.loads(line) for line in f]

    random.shuffle(data)

    # 90% train, 5% valid, 5% test
    n = len(data)
    train_end = int(n * 0.90)
    valid_end = int(n * 0.95)

    splits = {
        "train": data[:train_end],
        "valid": data[train_end:valid_end],
        "test": data[valid_end:],
    }

    data_dir = OUTPUT_DIR / "data"
    data_dir.mkdir(parents=True, exist_ok=True)

    for name, items in splits.items():
        out_path = data_dir / f"{name}.jsonl"
        with open(out_path, 'w') as f:
            for item in items:
                f.write(json.dumps(item) + '\n')
        print(f"  {name}: {len(items)} pairs -> {out_path}")

    return splits


def write_lora_config():
    """Write LoRA configuration."""
    config = {
        "lora_layers": 16,
        "lora_parameters": {
            "rank": 16,
            "alpha": 32,
            "dropout": 0.05,
            "scale": 2.0,
        },
    }
    config_path = OUTPUT_DIR / "lora_config.yaml"

    # mlx-lm uses yaml-like args, but we'll pass via CLI
    # Save config for reference
    with open(config_path, 'w') as f:
        json.dump(config, f, indent=2)
    print(f"  LoRA config saved to {config_path}")
    return config


def train(args):
    """Run LoRA fine-tuning with mlx-lm."""
    from mlx_lm import lora

    adapter_path = OUTPUT_DIR / "adapters"
    data_dir = OUTPUT_DIR / "data"

    # Write LoRA YAML config
    config_path = OUTPUT_DIR / "lora_training_config.yaml"
    config_content = f"""# LoRA fine-tuning config for Chirp transcript cleanup
model: {MODEL_NAME}
data: {data_dir}
adapter_path: {adapter_path}
train: true
fine_tune_type: lora
num_layers: 16
batch_size: {args.batch_size}
iters: {args.iters}
learning_rate: {args.lr}
val_batches: 20
steps_per_eval: 100
steps_per_report: 10
save_every: 200
max_seq_length: 512
mask_prompt: true
grad_checkpoint: true
seed: 42
"""
    if args.resume and (adapter_path / "adapters.safetensors").exists():
        config_content += f"resume_adapter_file: {adapter_path / 'adapters.safetensors'}\n"
        print("Resuming from checkpoint...")

    with open(config_path, 'w') as f:
        f.write(config_content)

    # Build training args
    train_args = [
        sys.executable, "-m", "mlx_lm.lora",
        "-c", str(config_path),
    ]

    print(f"\nStarting training...")
    print(f"  Model: {MODEL_NAME}")
    print(f"  Iterations: {args.iters}")
    print(f"  Batch size: {args.batch_size}")
    print(f"  Learning rate: {args.lr}")
    print(f"  Output: {adapter_path}")
    print(f"\nCommand: {' '.join(train_args)}")
    print("=" * 60)

    result = subprocess.run(train_args, cwd=str(SCRIPT_DIR))
    if result.returncode != 0:
        print("Training failed!")
        sys.exit(1)

    print("\nTraining complete!")
    return adapter_path


def fuse_and_export(adapter_path):
    """Fuse LoRA adapters into base model and export to GGUF."""
    fused_dir = OUTPUT_DIR / "fused"

    # Step 1: Fuse adapters
    print("\nFusing LoRA adapters into base model...")
    fuse_args = [
        sys.executable, "-m", "mlx_lm.fuse",
        "--model", MODEL_NAME,
        "--adapter-path", str(adapter_path),
        "--save-path", str(fused_dir),
    ]
    result = subprocess.run(fuse_args)
    if result.returncode != 0:
        print("Fuse failed!")
        sys.exit(1)
    print(f"Fused model saved to {fused_dir}")

    # Step 2: Export to GGUF
    print("\nExporting to GGUF (Q4_K_M)...")
    gguf_path = OUTPUT_DIR / "chirp-cleanup-q4_k_m.gguf"

    # Use llama.cpp's convert script if available, otherwise use mlx_lm
    convert_args = [
        sys.executable, "-m", "mlx_lm.convert",
        "--hf-path", str(fused_dir),
        "--mlx-path", str(OUTPUT_DIR / "mlx_export"),
        "-q",
    ]

    # Try the mlx_lm GGUF export first
    try:
        from mlx_lm.gguf_utils import convert_to_gguf
        print("  Using mlx_lm GGUF export...")
        # This may not exist in all versions
    except ImportError:
        pass

    # More reliable: use llama.cpp convert_hf_to_gguf.py if available
    llama_convert = Path.home() / "chirp-benchmark-models" / ".." / "llama.cpp" / "convert_hf_to_gguf.py"

    # Simplest approach: just tell the user how to convert
    print(f"\n  Fused model is at: {fused_dir}")
    print(f"  To convert to GGUF, run:")
    print(f"    python convert_hf_to_gguf.py {fused_dir} --outfile {gguf_path} --outtype q4_k_m")
    print(f"")
    print(f"  Or use llama-quantize:")
    print(f"    llama-quantize {fused_dir}/model.gguf {gguf_path} Q4_K_M")

    return fused_dir


def test_model(adapter_path):
    """Quick test of the fine-tuned model using mlx_lm Python API."""
    print("\nTesting fine-tuned model...")

    test_cases = [
        "I need to go to the store and pick up some milk",
        "um so I was thinking we could um maybe go to the park",
        "so like um you know I was uh basically going to like tell him that we need to uh figure this out",
        "send it to the team on Tuesday sorry I mean Wednesday morning",
        "we need to update the app not the app I mean the website or rather the landing page before Friday",
        "I want to I want to make sure that the the project is is done by next week",
        "The quarterly earnings report shows a 15% increase in revenue compared to last year.",
    ]

    from mlx_lm import load, generate
    import mlx.core as mx

    model, tokenizer = load(MODEL_NAME, adapter_path=str(adapter_path))

    # Greedy sampler (temperature=0)
    def greedy_sampler(logits):
        return mx.argmax(logits, axis=-1)

    system_msg = "Clean up dictated speech. Remove fillers, fix stutters, resolve self-corrections (keep only the final version). Output only the cleaned text."

    for test in test_cases:
        messages = [
            {"role": "system", "content": system_msg},
            {"role": "user", "content": test},
        ]
        prompt = tokenizer.apply_chat_template(messages, tokenize=False, add_generation_prompt=True)

        response = generate(model, tokenizer, prompt=prompt, max_tokens=256, sampler=greedy_sampler, verbose=False)
        # Strip any trailing special tokens
        response = response.split("<|im_end|>")[0].strip()

        print(f"  IN:  {test[:80]}")
        print(f"  OUT: {response[:80]}")
        print()


def main():
    parser = argparse.ArgumentParser(description="Fine-tune Qwen for Chirp transcript cleanup")
    parser.add_argument("--model", default=DEFAULT_MODEL, help=f"HuggingFace model name (default: {DEFAULT_MODEL})")
    parser.add_argument("--output-dir", type=Path, default=None, help="Output directory (auto-generated from model name if not set)")
    parser.add_argument("--iters", type=int, default=1000, help="Training iterations (default: 1000)")
    parser.add_argument("--batch-size", type=int, default=4, help="Batch size (default: 4)")
    parser.add_argument("--lr", type=float, default=1e-5, help="Learning rate (default: 1e-5)")
    parser.add_argument("--test", action="store_true", help="Quick test run (100 iters)")
    parser.add_argument("--resume", action="store_true", help="Resume from checkpoint")
    parser.add_argument("--test-only", action="store_true", help="Only run test inference")
    parser.add_argument("--export-only", action="store_true", help="Only run export")
    args = parser.parse_args()

    if args.test:
        args.iters = 100

    # Set globals based on args
    global MODEL_NAME, OUTPUT_DIR
    MODEL_NAME = args.model
    if args.output_dir:
        OUTPUT_DIR = args.output_dir
    else:
        # Auto-generate output dir from model name
        safe_name = MODEL_NAME.split("/")[-1].lower().replace(".", "-")
        OUTPUT_DIR = SCRIPT_DIR / "output" / f"{safe_name}-lora"

    print("=" * 60)
    print("  Chirp Transcript Cleanup - Qwen LoRA Fine-Tuning")
    print("=" * 60)

    check_deps()

    adapter_path = OUTPUT_DIR / "adapters"

    if args.test_only:
        if not (adapter_path / "adapters.safetensors").exists():
            print("ERROR: No trained adapters found. Run training first.")
            sys.exit(1)
        test_model(adapter_path)
        return

    if args.export_only:
        if not (adapter_path / "adapters.safetensors").exists():
            print("ERROR: No trained adapters found. Run training first.")
            sys.exit(1)
        fuse_and_export(adapter_path)
        return

    # Step 1: Prepare data
    print("\nStep 1: Preparing data splits...")
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    splits = split_data()

    # Step 2: Write config
    print("\nStep 2: Writing LoRA config...")
    write_lora_config()

    # Step 3: Train
    print("\nStep 3: Training...")
    start = time.time()
    adapter_path = train(args)
    elapsed = time.time() - start
    hours = int(elapsed // 3600)
    mins = int((elapsed % 3600) // 60)
    print(f"\nTraining took: {hours}h {mins}m")

    # Step 4: Test
    print("\nStep 4: Testing...")
    test_model(adapter_path)

    # Step 5: Export
    print("\nStep 5: Exporting...")
    fuse_and_export(adapter_path)

    print("\n" + "=" * 60)
    print("  DONE!")
    print("=" * 60)


if __name__ == "__main__":
    main()
