#!/usr/bin/env python3
"""
Compare benchmark results across all tested models.

Usage:
    python scripts/benchmark_compare.py
    python scripts/benchmark_compare.py --results-dir scripts/benchmark_results
"""

import argparse
import json
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent
RESULTS_DIR = SCRIPT_DIR / "benchmark_results"
TRANSCRIPTS_FILE = SCRIPT_DIR / "benchmark_transcripts.json"


def load_results(results_dir: Path) -> list[dict]:
    """Load all result JSON files."""
    results = []
    for f in sorted(results_dir.glob("*.json")):
        with open(f) as fh:
            data = json.load(fh)
            results.append(data)
    return results


def score_output(output: str, expected: str, raw_input: str) -> dict:
    """Score a single output against expected."""
    output_clean = output.strip()
    expected_clean = expected.strip()

    # Exact match
    exact = output_clean == expected_clean

    # Meaning preservation: check key words from input are in output
    input_words = set(w.lower() for w in raw_input.split() if len(w) > 3)
    filler_words = {"like", "basically", "just", "really", "actually", "know", "mean", "that", "this", "what", "trying"}
    content_words = input_words - filler_words
    output_lower = output_clean.lower()
    preserved = sum(1 for w in content_words if w in output_lower)
    preservation_rate = preserved / len(content_words) if content_words else 1.0

    # Passthrough check (for already-clean text)
    passthrough = output_clean == raw_input.strip() or output_clean == raw_input.strip() + "."

    # Length ratio (output shouldn't be way longer or shorter than expected)
    len_ratio = len(output_clean) / len(expected_clean) if expected_clean else 1.0

    return {
        "exact_match": exact,
        "preservation_rate": round(preservation_rate, 2),
        "passthrough": passthrough,
        "len_ratio": round(len_ratio, 2),
    }


def main():
    parser = argparse.ArgumentParser(description="Compare benchmark results")
    parser.add_argument("--results-dir", type=Path, default=RESULTS_DIR)
    args = parser.parse_args()

    if not args.results_dir.exists():
        print(f"No results directory found at: {args.results_dir}", file=sys.stderr)
        print("Run benchmarks first with: python scripts/benchmark.py <model.gguf>", file=sys.stderr)
        sys.exit(1)

    all_results = load_results(args.results_dir)
    if not all_results:
        print("No result files found.", file=sys.stderr)
        sys.exit(1)

    with open(TRANSCRIPTS_FILE) as f:
        transcripts = json.load(f)
    transcripts_by_id = {t["id"]: t for t in transcripts}

    print(f"Comparing {len(all_results)} models\n")

    # Build summary table
    model_summaries = []

    for model_data in all_results:
        model_name = model_data["model"]
        results = model_data["results"]

        # Group by mode
        by_mode = {}
        for r in results:
            by_mode.setdefault(r["mode"], []).append(r)

        for mode, mode_results in by_mode.items():
            times = [r["time_ms"] for r in mode_results]
            avg_time = sum(times) / len(times)

            scores = []
            for r in mode_results:
                s = score_output(r["output"], r["expected"], r["raw_input"])
                scores.append(s)

            exact_matches = sum(1 for s in scores if s["exact_match"])
            avg_preservation = sum(s["preservation_rate"] for s in scores) / len(scores)

            # Check passthrough on transcript #10 (already clean)
            t10 = [r for r in mode_results if r["id"] == 10]
            passthrough_ok = False
            if t10:
                s10 = score_output(t10[0]["output"], t10[0]["expected"], t10[0]["raw_input"])
                passthrough_ok = s10["passthrough"] or s10["exact_match"]

            model_summaries.append({
                "model": model_name,
                "mode": mode,
                "avg_time_ms": round(avg_time),
                "exact_matches": exact_matches,
                "total": len(mode_results),
                "avg_preservation": round(avg_preservation, 2),
                "passthrough": passthrough_ok,
            })

    # Print comparison table
    print(f"{'Model':<45} {'Mode':<12} {'Avg ms':>8} {'Exact':>7} {'Preserve':>10} {'Pass?':>6}")
    print("-" * 92)

    for s in sorted(model_summaries, key=lambda x: (x["avg_time_ms"])):
        pass_str = "YES" if s["passthrough"] else "NO"
        print(
            f"{s['model']:<45} {s['mode']:<12} {s['avg_time_ms']:>7}ms "
            f"{s['exact_matches']:>3}/{s['total']:<3} "
            f"{s['avg_preservation']:>9.0%} "
            f"{pass_str:>5}"
        )

    # Detailed per-transcript comparison
    print(f"\n{'='*80}")
    print("DETAILED PER-TRANSCRIPT COMPARISON")
    print(f"{'='*80}")

    for tid in range(1, 11):
        t = transcripts_by_id[tid]
        print(f"\n--- [{tid}] {t['name']} ---")
        print(f"  Input:    {t['input'][:90]}")
        print(f"  Expected: {t['expected'][:90]}")

        for model_data in all_results:
            model_name = model_data["model"]
            for r in model_data["results"]:
                if r["id"] == tid:
                    score = score_output(r["output"], r["expected"], r["raw_input"])
                    match_str = "EXACT" if score["exact_match"] else f"pres={score['preservation_rate']:.0%}"
                    print(f"  {model_name[:30]:<30} {r['mode']:<12}: {r['output'][:60]:<60} [{r['time_ms']:>5.0f}ms] {match_str}")

    # Recommendations
    print(f"\n{'='*80}")
    print("RECOMMENDATIONS")
    print(f"{'='*80}")

    best_quality = sorted(model_summaries, key=lambda x: (-x["avg_preservation"], -x["exact_matches"], x["avg_time_ms"]))
    best_speed = sorted(model_summaries, key=lambda x: (x["avg_time_ms"], -x["avg_preservation"]))

    if best_quality:
        bq = best_quality[0]
        print(f"\n  Best quality: {bq['model']} ({bq['mode']})")
        print(f"    {bq['exact_matches']}/{bq['total']} exact, {bq['avg_preservation']:.0%} preservation, {bq['avg_time_ms']}ms avg")

    if best_speed:
        bs = best_speed[0]
        print(f"\n  Fastest:      {bs['model']} ({bs['mode']})")
        print(f"    {bs['exact_matches']}/{bs['total']} exact, {bs['avg_preservation']:.0%} preservation, {bs['avg_time_ms']}ms avg")

    # Check if regex helps
    print("\n  Regex preprocessing effect:")
    for model_data in all_results:
        model_name = model_data["model"]
        by_mode = {}
        for r in model_data["results"]:
            by_mode.setdefault(r["mode"], []).append(r)

        if "regex+AI" in by_mode and "AI-only" in by_mode:
            regex_pres = sum(score_output(r["output"], r["expected"], r["raw_input"])["preservation_rate"] for r in by_mode["regex+AI"]) / len(by_mode["regex+AI"])
            ai_pres = sum(score_output(r["output"], r["expected"], r["raw_input"])["preservation_rate"] for r in by_mode["AI-only"]) / len(by_mode["AI-only"])
            diff = regex_pres - ai_pres
            better = "regex+AI" if diff > 0 else "AI-only"
            print(f"    {model_name[:40]:<40}: {better} wins by {abs(diff):.0%}")


if __name__ == "__main__":
    main()
