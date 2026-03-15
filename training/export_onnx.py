"""
Export trained FLAN-T5-small to ONNX and quantize to INT8.

Usage:
    python export_onnx.py --model output/chirp-cleanup/best --output output/onnx

Produces:
    output/onnx/encoder_model.onnx      (~15MB quantized)
    output/onnx/decoder_model.onnx      (~15MB quantized)
    output/onnx/spiece.model            (sentencepiece vocab)
    output/onnx/config.json             (model config for inference)
"""

import argparse
import json
import shutil
from pathlib import Path

from optimum.onnxruntime import ORTModelForSeq2SeqLM
from transformers import T5Tokenizer, T5ForConditionalGeneration
from onnxruntime.quantization import quantize_dynamic, QuantType


def export_to_onnx(model_dir, output_dir):
    print(f"Loading model from {model_dir}...")
    tokenizer = T5Tokenizer.from_pretrained(model_dir)

    # Export using optimum
    print("Exporting to ONNX...")
    ort_model = ORTModelForSeq2SeqLM.from_pretrained(model_dir, export=True)
    ort_model.save_pretrained(output_dir)
    tokenizer.save_pretrained(output_dir)

    print(f"ONNX models saved to {output_dir}")
    return output_dir


def quantize_models(output_dir):
    """Quantize encoder and decoder to INT8 for smaller size and faster inference."""
    onnx_dir = Path(output_dir)

    for model_name in ["encoder_model.onnx", "decoder_model.onnx", "decoder_with_past_model.onnx"]:
        model_path = onnx_dir / model_name
        if not model_path.exists():
            continue

        quantized_path = onnx_dir / f"{model_name}.tmp"
        print(f"Quantizing {model_name}...")

        quantize_dynamic(
            model_input=str(model_path),
            model_output=str(quantized_path),
            weight_type=QuantType.QInt8,
        )

        # Replace original with quantized
        model_path.unlink()
        quantized_path.rename(model_path)

        size_mb = model_path.stat().st_size / (1024 * 1024)
        print(f"  {model_name}: {size_mb:.1f} MB (quantized)")


def verify_model(output_dir, model_dir):
    """Quick sanity check that the exported model produces reasonable output."""
    print("\nVerifying exported model...")

    tokenizer = T5Tokenizer.from_pretrained(output_dir)
    ort_model = ORTModelForSeq2SeqLM.from_pretrained(output_dir)

    test_inputs = [
        "clean transcript: um so i was thinking we should uh update the database",
        "clean transcript: the meeting is at two thirty pm on friday and uh we need to prepare the slides",
        "clean transcript: hey can you send that to john at example dot com question mark",
    ]

    print("\nTest results:")
    for inp in test_inputs:
        inputs = tokenizer(inp, return_tensors="pt", max_length=256, truncation=True)
        outputs = ort_model.generate(**inputs, max_length=256, num_beams=1)
        result = tokenizer.decode(outputs[0], skip_special_tokens=True)

        display_input = inp.replace("clean transcript: ", "")
        print(f"  Input:  {display_input}")
        print(f"  Output: {result}")
        print()


def prepare_for_rust(output_dir, deploy_dir=None):
    """Copy only the files needed for Rust inference."""
    if deploy_dir is None:
        deploy_dir = Path(output_dir) / "deploy"

    deploy_dir = Path(deploy_dir)
    deploy_dir.mkdir(parents=True, exist_ok=True)

    onnx_dir = Path(output_dir)

    # Copy ONNX models
    for f in ["encoder_model.onnx", "decoder_model.onnx"]:
        src = onnx_dir / f
        if src.exists():
            shutil.copy2(src, deploy_dir / f)

    # Copy tokenizer files
    for f in ["spiece.model", "tokenizer_config.json", "special_tokens_map.json"]:
        src = onnx_dir / f
        if src.exists():
            shutil.copy2(src, deploy_dir / f)

    # Write a minimal config
    config = {
        "task_prefix": "clean transcript: ",
        "max_input_length": 256,
        "max_output_length": 256,
        "model_type": "t5-small",
    }
    with open(deploy_dir / "cleanup_config.json", "w") as f:
        json.dump(config, f, indent=2)

    total_size = sum(f.stat().st_size for f in deploy_dir.iterdir()) / (1024 * 1024)
    print(f"\nDeploy package ready at {deploy_dir}")
    print(f"Total size: {total_size:.1f} MB")
    print(f"\nTo use in Chirp, copy contents to:")
    print(f"  %APPDATA%/com.chirp.app/models/cleanup/")


def main():
    parser = argparse.ArgumentParser(description="Export T5 to ONNX")
    parser.add_argument("--model", type=str, default="output/chirp-cleanup/best",
                        help="Path to trained model")
    parser.add_argument("--output", type=str, default="output/onnx",
                        help="Output directory for ONNX files")
    parser.add_argument("--no-quantize", action="store_true",
                        help="Skip INT8 quantization")
    parser.add_argument("--deploy-dir", type=str, default=None,
                        help="Directory to prepare deployment package")
    args = parser.parse_args()

    # Export
    export_to_onnx(args.model, args.output)

    # Quantize
    if not args.no_quantize:
        quantize_models(args.output)

    # Verify
    verify_model(args.output, args.model)

    # Prepare deployment package
    prepare_for_rust(args.output, args.deploy_dir)


if __name__ == "__main__":
    main()
