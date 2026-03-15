"""
Fine-tune FLAN-T5-small on transcript cleanup pairs.

Usage:
    python train.py --data data/training_pairs.jsonl --epochs 5

Trains on Apple Silicon MPS (M4) or falls back to CPU.
Expects JSONL with {"input": "...", "output": "..."} per line.
"""

import json
import argparse
from pathlib import Path

import torch
from torch.utils.data import Dataset, DataLoader, random_split
from transformers import (
    T5ForConditionalGeneration,
    T5Tokenizer,
    get_linear_schedule_with_warmup,
)

TASK_PREFIX = "clean transcript: "
MAX_INPUT_LEN = 256
MAX_OUTPUT_LEN = 256


class TranscriptDataset(Dataset):
    def __init__(self, pairs, tokenizer):
        self.pairs = pairs
        self.tokenizer = tokenizer

    def __len__(self):
        return len(self.pairs)

    def __getitem__(self, idx):
        pair = self.pairs[idx]
        input_text = TASK_PREFIX + pair["input"]
        target_text = pair["output"]

        input_enc = self.tokenizer(
            input_text,
            max_length=MAX_INPUT_LEN,
            padding="max_length",
            truncation=True,
            return_tensors="pt",
        )
        target_enc = self.tokenizer(
            target_text,
            max_length=MAX_OUTPUT_LEN,
            padding="max_length",
            truncation=True,
            return_tensors="pt",
        )

        labels = target_enc.input_ids.squeeze()
        # Replace padding token id with -100 so it's ignored in loss
        labels[labels == self.tokenizer.pad_token_id] = -100

        return {
            "input_ids": input_enc.input_ids.squeeze(),
            "attention_mask": input_enc.attention_mask.squeeze(),
            "labels": labels,
        }


def load_pairs(path):
    pairs = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if line:
                pair = json.loads(line)
                if "input" in pair and "output" in pair:
                    pairs.append(pair)
    return pairs


def get_device():
    if torch.backends.mps.is_available():
        print("Using Apple Silicon MPS")
        return torch.device("mps")
    elif torch.cuda.is_available():
        print("Using CUDA")
        return torch.device("cuda")
    else:
        print("Using CPU (this will be slow)")
        return torch.device("cpu")


def train_epoch(model, dataloader, optimizer, scheduler, device, epoch, total_epochs):
    model.train()
    total_loss = 0
    num_batches = len(dataloader)

    for i, batch in enumerate(dataloader):
        input_ids = batch["input_ids"].to(device)
        attention_mask = batch["attention_mask"].to(device)
        labels = batch["labels"].to(device)

        outputs = model(
            input_ids=input_ids,
            attention_mask=attention_mask,
            labels=labels,
        )

        loss = outputs.loss
        total_loss += loss.item()

        loss.backward()
        torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
        optimizer.step()
        scheduler.step()
        optimizer.zero_grad()

        if (i + 1) % 50 == 0 or (i + 1) == num_batches:
            avg_loss = total_loss / (i + 1)
            lr = scheduler.get_last_lr()[0]
            print(f"  Epoch {epoch+1}/{total_epochs} [{i+1}/{num_batches}] loss={avg_loss:.4f} lr={lr:.2e}")

    return total_loss / num_batches


def evaluate(model, dataloader, tokenizer, device, num_samples=5):
    model.eval()
    total_loss = 0

    with torch.no_grad():
        for batch in dataloader:
            input_ids = batch["input_ids"].to(device)
            attention_mask = batch["attention_mask"].to(device)
            labels = batch["labels"].to(device)

            outputs = model(
                input_ids=input_ids,
                attention_mask=attention_mask,
                labels=labels,
            )
            total_loss += outputs.loss.item()

    avg_loss = total_loss / len(dataloader)

    # Show a few example predictions
    print(f"\n  Val loss: {avg_loss:.4f}")
    print("  Sample predictions:")

    model.eval()
    sample_batch = next(iter(dataloader))
    input_ids = sample_batch["input_ids"][:num_samples].to(device)
    attention_mask = sample_batch["attention_mask"][:num_samples].to(device)
    labels = sample_batch["labels"][:num_samples]

    with torch.no_grad():
        generated = model.generate(
            input_ids=input_ids,
            attention_mask=attention_mask,
            max_length=MAX_OUTPUT_LEN,
            num_beams=1,
            do_sample=False,
        )

    for i in range(min(num_samples, len(generated))):
        inp = tokenizer.decode(input_ids[i], skip_special_tokens=True)
        pred = tokenizer.decode(generated[i], skip_special_tokens=True)
        target_ids = labels[i].clone()
        target_ids[target_ids == -100] = tokenizer.pad_token_id
        target = tokenizer.decode(target_ids, skip_special_tokens=True)

        # Trim the task prefix for display
        inp = inp.replace(TASK_PREFIX, "", 1)
        print(f"\n    Input:  {inp[:100]}")
        print(f"    Target: {target[:100]}")
        print(f"    Pred:   {pred[:100]}")

    return avg_loss


def main():
    parser = argparse.ArgumentParser(description="Train FLAN-T5-small for transcript cleanup")
    parser.add_argument("--data", type=str, default="data/training_pairs.jsonl")
    parser.add_argument("--output", type=str, default="output/chirp-cleanup")
    parser.add_argument("--epochs", type=int, default=3)
    parser.add_argument("--batch-size", type=int, default=16)
    parser.add_argument("--lr", type=float, default=3e-4)
    parser.add_argument("--val-split", type=float, default=0.05)
    parser.add_argument("--save-every", type=int, default=1, help="Save checkpoint every N epochs")
    args = parser.parse_args()

    device = get_device()
    output_dir = Path(args.output)
    output_dir.mkdir(parents=True, exist_ok=True)

    # Load data
    print(f"Loading data from {args.data}...")
    pairs = load_pairs(args.data)
    print(f"Loaded {len(pairs)} pairs")

    # Load model and tokenizer
    print("Loading FLAN-T5-small...")
    model_name = "grammarly/coedit-small"
    tokenizer = T5Tokenizer.from_pretrained(model_name)
    model = T5ForConditionalGeneration.from_pretrained(model_name)
    model.to(device)

    param_count = sum(p.numel() for p in model.parameters() if p.requires_grad)
    print(f"Model parameters: {param_count:,}")

    # Create datasets
    dataset = TranscriptDataset(pairs, tokenizer)
    val_size = int(len(dataset) * args.val_split)
    train_size = len(dataset) - val_size
    train_dataset, val_dataset = random_split(
        dataset, [train_size, val_size],
        generator=torch.Generator().manual_seed(42),
    )

    print(f"Train: {train_size}, Val: {val_size}")

    train_loader = DataLoader(train_dataset, batch_size=args.batch_size, shuffle=True, num_workers=0)
    val_loader = DataLoader(val_dataset, batch_size=args.batch_size, shuffle=False, num_workers=0)

    # Optimizer and scheduler
    total_steps = len(train_loader) * args.epochs
    warmup_steps = total_steps // 10

    optimizer = torch.optim.AdamW(model.parameters(), lr=args.lr, weight_decay=0.01)
    scheduler = get_linear_schedule_with_warmup(optimizer, warmup_steps, total_steps)

    print(f"\nTraining for {args.epochs} epochs ({total_steps} steps, {warmup_steps} warmup)")
    print(f"Batch size: {args.batch_size}")
    print()

    best_val_loss = float("inf")

    for epoch in range(args.epochs):
        train_loss = train_epoch(model, train_loader, optimizer, scheduler, device, epoch, args.epochs)
        print(f"\n  Epoch {epoch+1} train loss: {train_loss:.4f}")

        val_loss = evaluate(model, val_loader, tokenizer, device)

        # Save checkpoint
        if (epoch + 1) % args.save_every == 0:
            ckpt_dir = output_dir / f"checkpoint-{epoch+1}"
            model.save_pretrained(ckpt_dir)
            tokenizer.save_pretrained(ckpt_dir)
            print(f"\n  Saved checkpoint to {ckpt_dir}")

        # Save best model
        if val_loss < best_val_loss:
            best_val_loss = val_loss
            best_dir = output_dir / "best"
            model.save_pretrained(best_dir)
            tokenizer.save_pretrained(best_dir)
            print(f"  New best model (val_loss={val_loss:.4f})")

    # Save final model
    final_dir = output_dir / "final"
    model.save_pretrained(final_dir)
    tokenizer.save_pretrained(final_dir)
    print(f"\nTraining complete! Final model saved to {final_dir}")
    print(f"Best model (val_loss={best_val_loss:.4f}) saved to {output_dir / 'best'}")


if __name__ == "__main__":
    main()
