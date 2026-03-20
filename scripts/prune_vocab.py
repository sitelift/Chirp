#!/usr/bin/env python3
"""
Prune Qwen 2.5 vocabulary to only tokens seen in training data.
Reduces embedding + LM head layers (~30% of model params) dramatically.

Usage:
    python scripts/prune_vocab.py training/output/qwen-lora/fused/ training/output/qwen-lora-pruned/ training/data/training_pairs_clean.jsonl
"""

import argparse
import json
import os
import sys
from pathlib import Path

import torch
from transformers import AutoTokenizer, AutoModelForCausalLM


def collect_used_tokens(data_path: str, tokenizer) -> set:
    """Tokenize all training data and collect used token IDs."""
    used = set()

    # Always keep special tokens
    for tid in tokenizer.all_special_ids:
        used.add(tid)

    # Keep chat template tokens
    special_tokens = [
        "<|im_start|>", "<|im_end|>", "<|endoftext|>",
        "<|fim_prefix|>", "<|fim_suffix|>", "<|fim_middle|>",
    ]
    for tok in special_tokens:
        ids = tokenizer.encode(tok, add_special_tokens=False)
        used.update(ids)

    # Tokenize the system prompt used in production
    system_prompt = "Clean up dictated speech. Remove fillers, fix stutters, resolve self-corrections (keep only the final version). Output only the cleaned text."
    ids = tokenizer.encode(system_prompt, add_special_tokens=False)
    used.update(ids)

    # Also keep tokens for all mode prompts
    mode_prompts = [
        "Clean up dictated speech. Remove fillers, fix stutters, resolve self-corrections (keep only the final version). Format as an email with greeting, body paragraphs, and sign-off. Output only the cleaned text.",
        "Clean up dictated speech. Remove fillers, fix stutters, resolve self-corrections (keep only the final version). Use professional, formal language. Output only the cleaned text.",
        "Clean up dictated speech. Remove fillers, fix stutters, resolve self-corrections (keep only the final version). Keep it casual and conversational. Output only the cleaned text.",
    ]
    for prompt in mode_prompts:
        ids = tokenizer.encode(prompt, add_special_tokens=False)
        used.update(ids)

    # Tokenize training data
    with open(data_path) as f:
        for line in f:
            d = json.loads(line)
            for key in ["input", "output"]:
                if key in d:
                    ids = tokenizer.encode(d[key], add_special_tokens=False)
                    used.update(ids)

    return used


def prune_model(old_model_path: str, new_model_path: str, data_path: str):
    """Prune vocab of Qwen 2.5 model to only used tokens."""
    print(f"Loading tokenizer from {old_model_path}...")
    tokenizer = AutoTokenizer.from_pretrained(old_model_path, trust_remote_code=True)

    print(f"Collecting used tokens from {data_path}...")
    used_tokens = collect_used_tokens(data_path, tokenizer)
    old_vocab_size = tokenizer.vocab_size
    print(f"  Old vocab size: {old_vocab_size}")
    print(f"  Used tokens: {len(used_tokens)}")

    # Create mapping: old_id -> new_id
    # Keep tokens sorted by original ID to preserve ordering
    sorted_used = sorted(used_tokens)
    old_to_new = {old_id: new_id for new_id, old_id in enumerate(sorted_used)}
    new_vocab_size = len(sorted_used)
    print(f"  New vocab size: {new_vocab_size} ({100*new_vocab_size/old_vocab_size:.1f}% of original)")

    # Load model
    print(f"Loading model from {old_model_path}...")
    model = AutoModelForCausalLM.from_pretrained(
        old_model_path,
        trust_remote_code=True,
        torch_dtype=torch.float16,
    )

    # Get embedding and LM head
    embed = model.model.embed_tokens
    lm_head = model.lm_head

    print(f"  Old embed shape: {embed.weight.shape}")
    print(f"  Old lm_head shape: {lm_head.weight.shape}")

    # Create new smaller layers
    new_embed = torch.nn.Embedding(new_vocab_size, embed.embedding_dim, dtype=embed.weight.dtype)
    new_lm_head = torch.nn.Linear(lm_head.in_features, new_vocab_size, bias=False, dtype=lm_head.weight.dtype)

    # Copy weights for kept tokens
    idx_tensor = torch.LongTensor(sorted_used)
    new_embed.weight.data = embed.weight.data[idx_tensor]
    new_lm_head.weight.data = lm_head.weight.data[idx_tensor]

    # Replace in model
    model.model.embed_tokens = new_embed
    model.lm_head = new_lm_head

    # Update config
    model.config.vocab_size = new_vocab_size

    print(f"  New embed shape: {new_embed.weight.shape}")
    print(f"  New lm_head shape: {new_lm_head.weight.shape}")

    # Save model
    os.makedirs(new_model_path, exist_ok=True)
    print(f"Saving pruned model to {new_model_path}...")
    model.save_pretrained(new_model_path)

    # Save token mapping for GGUF conversion
    mapping_path = os.path.join(new_model_path, "token_mapping.json")
    with open(mapping_path, "w") as f:
        json.dump({"old_to_new": {str(k): v for k, v in old_to_new.items()}, "new_to_old": sorted_used}, f)
    print(f"  Token mapping saved to {mapping_path}")

    # Now we need to create a new tokenizer with the pruned vocabulary
    # Copy tokenizer files and modify them
    _create_pruned_tokenizer(old_model_path, new_model_path, sorted_used, tokenizer)

    print(f"\nDone! Pruned model saved to {new_model_path}")
    print(f"  Vocab: {old_vocab_size} -> {new_vocab_size}")
    embed_params_old = old_vocab_size * embed.embedding_dim * 2  # embed + lm_head
    embed_params_new = new_vocab_size * embed.embedding_dim * 2
    print(f"  Embedding params: {embed_params_old/1e6:.0f}M -> {embed_params_new/1e6:.0f}M ({100*embed_params_new/embed_params_old:.1f}%)")


def _create_pruned_tokenizer(old_model_path, new_model_path, sorted_used, old_tokenizer):
    """Create a new tokenizer with pruned vocabulary."""
    import shutil

    # Copy tokenizer config files
    for fname in ["tokenizer_config.json", "chat_template.jinja", "special_tokens_map.json"]:
        src = os.path.join(old_model_path, fname)
        if os.path.exists(src):
            shutil.copy2(src, os.path.join(new_model_path, fname))

    # Load and prune the tokenizer.json (HuggingFace fast tokenizer format)
    tokenizer_path = os.path.join(old_model_path, "tokenizer.json")
    if os.path.exists(tokenizer_path):
        with open(tokenizer_path) as f:
            tok_data = json.load(f)

        # Get the vocab mapping from the old tokenizer
        old_vocab = tok_data["model"]["vocab"]

        # Create reverse mapping: token_string -> old_id
        # We need to keep tokens that map to IDs in sorted_used
        kept_ids = set(sorted_used)

        # Build new vocab with new IDs
        new_vocab = {}
        old_id_to_token = {v: k for k, v in old_vocab.items()}
        for new_id, old_id in enumerate(sorted_used):
            if old_id in old_id_to_token:
                new_vocab[old_id_to_token[old_id]] = new_id

        # Filter merges - keep only merges where both result tokens are in new vocab
        old_merges = tok_data["model"]["merges"]
        new_merges = []
        for merge in old_merges:
            # Merges can be strings "a b" or lists ["a", "b"]
            if isinstance(merge, str):
                parts = merge.split(" ")
            elif isinstance(merge, list):
                parts = merge
            else:
                continue
            if len(parts) == 2:
                p1, p2 = parts
                if p1 in new_vocab and p2 in new_vocab:
                    new_merges.append(merge)

        tok_data["model"]["vocab"] = new_vocab
        tok_data["model"]["merges"] = new_merges

        # Update added_tokens
        if "added_tokens" in tok_data:
            new_added = []
            for token_entry in tok_data["added_tokens"]:
                old_id = token_entry["id"]
                if old_id in kept_ids:
                    token_entry["id"] = sorted_used.index(old_id)
                    new_added.append(token_entry)
            tok_data["added_tokens"] = new_added

        with open(os.path.join(new_model_path, "tokenizer.json"), "w") as f:
            json.dump(tok_data, f, ensure_ascii=False)

        print(f"  Pruned tokenizer.json: vocab {len(old_vocab)} -> {len(new_vocab)}, merges {len(old_merges)} -> {len(new_merges)}")

    # Update tokenizer_config.json with new special token IDs
    config_path = os.path.join(new_model_path, "tokenizer_config.json")
    if os.path.exists(config_path):
        with open(config_path) as f:
            config = json.load(f)

        # Update eos_token_id, bos_token_id etc in generation_config
        gen_config_path = os.path.join(new_model_path, "generation_config.json")
        if os.path.exists(os.path.join(old_model_path, "generation_config.json")):
            with open(os.path.join(old_model_path, "generation_config.json")) as f:
                gen_config = json.load(f)
            if "eos_token_id" in gen_config:
                eid = gen_config["eos_token_id"]
                if isinstance(eid, list):
                    gen_config["eos_token_id"] = [sorted_used.index(e) for e in eid if e in kept_ids]
                elif eid in kept_ids:
                    gen_config["eos_token_id"] = sorted_used.index(eid)
            if "bos_token_id" in gen_config and gen_config["bos_token_id"] in kept_ids:
                gen_config["bos_token_id"] = sorted_used.index(gen_config["bos_token_id"])
            with open(gen_config_path, "w") as f:
                json.dump(gen_config, f, indent=2)

    # Copy config.json and update vocab_size
    config_path = os.path.join(old_model_path, "config.json")
    if os.path.exists(config_path):
        with open(config_path) as f:
            config = json.load(f)
        config["vocab_size"] = len(sorted_used)
        with open(os.path.join(new_model_path, "config.json"), "w") as f:
            json.dump(config, f, indent=2)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Prune Qwen 2.5 vocabulary")
    parser.add_argument("model_path", help="Path to HuggingFace model")
    parser.add_argument("output_path", help="Path to save pruned model")
    parser.add_argument("data_path", help="Path to training JSONL data")
    args = parser.parse_args()

    prune_model(args.model_path, args.output_path, args.data_path)
