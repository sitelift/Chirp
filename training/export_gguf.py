#!/usr/bin/env python3
"""
Export fine-tuned Qwen model to GGUF format for llama-server deployment.

Usage:
    python training/export_gguf.py                          # Export fused model
    python training/export_gguf.py --fused-path path/to/fused  # Custom path
"""

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent
DEFAULT_FUSED = SCRIPT_DIR / "output" / "qwen-lora" / "fused"
OUTPUT_DIR = SCRIPT_DIR / "output" / "qwen-lora"
GGUF_NAME = "chirp-cleanup-q4_k_m.gguf"


def get_convert_script():
    """Download convert_hf_to_gguf.py from llama.cpp if not available."""
    convert_dir = SCRIPT_DIR / "output" / "llama_cpp_tools"
    convert_script = convert_dir / "convert_hf_to_gguf.py"

    if convert_script.exists():
        return str(convert_script)

    print("Downloading llama.cpp conversion tools...")
    convert_dir.mkdir(parents=True, exist_ok=True)

    import urllib.request
    # Get the main conversion script
    base_url = "https://raw.githubusercontent.com/ggerganov/llama.cpp/master"
    files_to_download = [
        "convert_hf_to_gguf.py",
    ]

    for fname in files_to_download:
        url = f"{base_url}/{fname}"
        dest = convert_dir / fname
        print(f"  Downloading {fname}...")
        urllib.request.urlretrieve(url, dest)

    # Also need the gguf-py lib but we installed it via pip
    return str(convert_script)


def main():
    parser = argparse.ArgumentParser(description="Export fine-tuned model to GGUF")
    parser.add_argument("--fused-path", type=Path, default=DEFAULT_FUSED,
                        help="Path to fused model directory")
    parser.add_argument("--output", type=Path, default=OUTPUT_DIR / GGUF_NAME,
                        help="Output GGUF file path")
    parser.add_argument("--quantize", default="q4_k_m",
                        help="Quantization type (default: q4_k_m)")
    args = parser.parse_args()

    if not args.fused_path.exists():
        print(f"ERROR: Fused model not found at {args.fused_path}")
        print("Run training first: python training/train_qwen.py")
        sys.exit(1)

    print(f"Fused model: {args.fused_path}")
    print(f"Output: {args.output}")
    print(f"Quantization: {args.quantize}")

    # Get conversion script
    convert_script = get_convert_script()

    # First convert to f16 GGUF
    f16_gguf = args.output.parent / "chirp-cleanup-f16.gguf"
    print(f"\nStep 1: Converting to F16 GGUF...")
    result = subprocess.run([
        sys.executable, convert_script,
        str(args.fused_path),
        "--outfile", str(f16_gguf),
        "--outtype", "f16",
    ], capture_output=True, text=True)

    if result.returncode != 0:
        print(f"Conversion failed:\n{result.stderr}")
        # Try alternate approach
        print("\nTrying alternate approach with --outtype auto...")
        result = subprocess.run([
            sys.executable, convert_script,
            str(args.fused_path),
            "--outfile", str(f16_gguf),
        ], capture_output=True, text=True)
        if result.returncode != 0:
            print(f"Still failed:\n{result.stderr}")
            sys.exit(1)

    print(f"  F16 GGUF: {f16_gguf} ({f16_gguf.stat().st_size / 1024 / 1024:.0f} MB)")

    # Quantize using llama-quantize if available
    llama_quantize = Path.home() / "Library/Application Support/com.chirp.app/llm/llama-quantize"
    if not llama_quantize.exists():
        # Try to find it
        for candidate in [
            Path("/usr/local/bin/llama-quantize"),
            Path("/opt/homebrew/bin/llama-quantize"),
        ]:
            if candidate.exists():
                llama_quantize = candidate
                break

    if llama_quantize.exists():
        print(f"\nStep 2: Quantizing to {args.quantize}...")
        result = subprocess.run([
            str(llama_quantize),
            str(f16_gguf),
            str(args.output),
            args.quantize.upper().replace("-", "_"),
        ], capture_output=True, text=True)

        if result.returncode != 0:
            print(f"Quantization failed:\n{result.stderr}")
            print(f"\nFalling back to F16 GGUF (larger but works)")
            os.rename(f16_gguf, args.output)
        else:
            print(f"  Quantized: {args.output} ({args.output.stat().st_size / 1024 / 1024:.0f} MB)")
            # Clean up f16
            os.remove(f16_gguf)
    else:
        print(f"\nllama-quantize not found. Using F16 GGUF (larger but works).")
        print(f"To quantize later: llama-quantize {f16_gguf} {args.output} Q4_K_M")
        os.rename(f16_gguf, args.output)

    print(f"\nDone! GGUF model at: {args.output}")
    print(f"\nTo test with llama-server:")
    print(f"  llama-server --model {args.output} --port 8099 --ctx-size 2048 --gpu-layers 99")
    print(f"\nTo deploy in Chirp, copy to:")
    print(f"  ~/Library/Application Support/com.chirp.app/llm/")


if __name__ == "__main__":
    main()
