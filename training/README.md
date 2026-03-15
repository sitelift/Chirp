# Chirp Cleanup Model Training

Fine-tune grammarly/coedit-small (T5-small pre-trained on text correction) to transform dictated speech into polished written text.

The model runs AFTER regex cleanup (filler removal, spoken punctuation, basic capitalization) and handles what regex can't: restructuring, formatting, course correction, and making text read like it was typed.

## Setup (Mac Mini M4)

```bash
cd training
python -m venv venv
source venv/bin/activate
pip install -r requirements.txt
```

## Step 1: Generate training data

```bash
export ANTHROPIC_API_KEY=sk-ant-...
python generate_data.py --pairs 25000 --output data/training_pairs.jsonl
```

Takes ~1-2 hours, costs ~$15-20. Supports `--resume` if interrupted.

## Step 2: Train

```bash
python train.py --data data/training_pairs.jsonl --epochs 5 --batch-size 16
```

Takes ~4-6 hours on M4 MPS. Best model saved to `output/chirp-cleanup/best/`.

## Step 3: Export to ONNX

```bash
python export_onnx.py --model output/chirp-cleanup/best --output output/onnx
```

Produces quantized INT8 ONNX models (~30MB total) in `output/onnx/deploy/`.

## Step 4: Deploy

Copy the contents of `output/onnx/deploy/` to `%APPDATA%/com.chirp.app/models/cleanup/` on the Windows machine. Chirp will auto-detect and load the model on next launch.
