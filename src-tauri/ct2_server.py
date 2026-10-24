"""
Minimal HTTP server wrapping CTranslate2 for T5 inference.
Drop-in replacement for llama-server in Chirp's pipeline.

Endpoints:
  GET  /health              → {"status": "ok"}
  POST /v1/completions      → {"text": "cleaned text"}

Usage:
  python ct2_server.py --model path/to/ct2-int8 --port 9999
  python ct2_server.py --model path/to/ct2-int8 --port 9999 --device cuda
"""

import argparse
import json
import os
import re
import sys
from http.server import HTTPServer, BaseHTTPRequestHandler

import ctranslate2
from transformers import AutoTokenizer

# Globals set at startup
translator = None
tokenizer = None
PREFIX = "Rewrite as typed text: "

# Split on sentence boundaries: period/question/exclamation followed by space+capital
SENTENCE_SPLIT = re.compile(r'(?<=[.!?])\s+(?=[A-Z])')


def cleanup_sentence(text, beam_size=4, max_length=256):
    """Run a single sentence through the model."""
    text = text.strip()
    if not text:
        return ""

    prompt = f"{PREFIX}{text}"
    tokens = tokenizer(prompt, return_tensors="np")
    token_list = tokenizer.convert_ids_to_tokens(tokens["input_ids"][0])

    # min_decoding_length prevents the model from producing ultra-short summaries
    input_words = len(text.split())
    min_length = max(1, int(input_words * 0.5))

    result = translator.translate_batch(
        [token_list],
        beam_size=beam_size,
        max_decoding_length=max_length,
        min_decoding_length=min_length,
        length_penalty=1.3,
        repetition_penalty=1.2,
    )

    output_tokens = result[0].hypotheses[0]
    output_text = tokenizer.decode(
        tokenizer.convert_tokens_to_ids(output_tokens),
        skip_special_tokens=True,
    )

    # Safety: if output lost more than half the words, use original
    output_words = len(output_text.split())
    if output_words < input_words * 0.4:
        return text

    return output_text


class Handler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass  # Silence request logging

    def do_GET(self):
        if self.path == "/health":
            self._json_response({"status": "ok"})
        else:
            self.send_error(404)

    def do_POST(self):
        if self.path != "/v1/completions":
            self.send_error(404)
            return

        length = int(self.headers.get("Content-Length", 0))
        body = json.loads(self.rfile.read(length)) if length else {}
        text = body.get("text", "")
        beam_size = body.get("beam_size", 4)
        max_length = body.get("max_length", 256)

        # Split into sentences and clean each individually
        # This prevents the model from summarizing long paragraphs
        sentences = SENTENCE_SPLIT.split(text)
        cleaned = []
        for sentence in sentences:
            result = cleanup_sentence(sentence, beam_size, max_length)
            if result:
                cleaned.append(result)

        output_text = " ".join(cleaned)
        self._json_response({"text": output_text})

    def _json_response(self, data):
        body = json.dumps(data).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def main():
    global translator, tokenizer

    parser = argparse.ArgumentParser()
    parser.add_argument("--model", required=True, help="Path to CT2 model directory")
    parser.add_argument("--tokenizer", default="sitelift/chirp-cleanup", help="Tokenizer name or path")
    parser.add_argument("--port", type=int, default=9999)
    parser.add_argument("--device", default="auto", choices=["auto", "cpu", "cuda"])
    args = parser.parse_args()

    device = args.device
    if device == "auto":
        try:
            import torch
            device = "cuda" if torch.cuda.is_available() else "cpu"
        except ImportError:
            device = "cpu"

    print(f"Loading model from {args.model} on {device}...", flush=True)
    tokenizer = AutoTokenizer.from_pretrained(args.tokenizer)
    translator = ctranslate2.Translator(
        args.model,
        device=device,
        inter_threads=1,
        intra_threads=os.cpu_count() if device == "cpu" else 1,
    )
    print(f"Ready on port {args.port}", flush=True)

    server = HTTPServer(("127.0.0.1", args.port), Handler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    server.server_close()


if __name__ == "__main__":
    main()
