#!/bin/bash
# Monitors data generation, quality checks, then starts training
set -e

cd /Users/dutch/Chirp/training
source .venv/bin/activate

LOG="data/pipeline.log"
DATA="data/training_pairs.jsonl"
GEN_PID=26600

echo "$(date): Waiting for data generation (PID $GEN_PID) to finish..." | tee -a "$LOG"

# Wait for generation to complete
while kill -0 $GEN_PID 2>/dev/null; do
    COUNT=$(wc -l < "$DATA" 2>/dev/null || echo 0)
    echo "$(date): $COUNT pairs so far..." | tee -a "$LOG"
    sleep 300
done

FINAL_COUNT=$(wc -l < "$DATA")
echo "$(date): Generation complete! $FINAL_COUNT pairs total." | tee -a "$LOG"

# Quality check
echo "$(date): Running quality check..." | tee -a "$LOG"

python3 -c "
import json, random, re
from collections import Counter
from difflib import SequenceMatcher

random.seed(42)
with open('$DATA') as f:
    pairs = [json.loads(line) for line in f]

total = len(pairs)
print(f'Total pairs: {total}')

# Check for parsing issues
issues = []
empty = 0
too_short = 0
too_long = 0
has_placeholders = 0
identical = 0
markdown_heavy = 0

for p in pairs:
    inp = p.get('input', '').strip()
    out = p.get('output', '').strip()
    if not inp or not out:
        empty += 1
        continue
    if len(inp.split()) < 5:
        too_short += 1
    if len(inp.split()) > 200:
        too_long += 1
    if re.search(r'\[.*?\]', out):
        has_placeholders += 1
    if inp.lower() == out.lower():
        identical += 1
    # Count heavy markdown usage
    md_markers = out.count('**') + out.count('##') + out.count('| ')
    if md_markers > 4:
        markdown_heavy += 1

print(f'Empty pairs: {empty}')
print(f'Too short (<5 words): {too_short}')
print(f'Too long (>200 words): {too_long}')
print(f'Placeholder brackets: {has_placeholders}')
print(f'Identical in/out: {identical}')
print(f'Heavy markdown: {markdown_heavy} ({markdown_heavy/total*100:.1f}%)')

# Similarity distribution
sims = []
for p in random.sample(pairs, min(1000, total)):
    s = SequenceMatcher(None, p['input'].lower(), p['output'].lower()).ratio()
    sims.append(s)
avg_sim = sum(sims) / len(sims)
print(f'Avg similarity (sample): {avg_sim:.2f}')

# Show 5 random samples
print()
print('=== RANDOM SAMPLES ===')
for p in random.sample(pairs, 5):
    print(f'IN:  {p[\"input\"][:120]}')
    print(f'OUT: {p[\"output\"][:120]}')
    print()

# Overall verdict
problems = empty + too_short + too_long + has_placeholders
problem_pct = problems / total * 100
if problem_pct < 5:
    print(f'VERDICT: PASS ({problem_pct:.1f}% problematic)')
    exit(0)
else:
    print(f'VERDICT: WARNING ({problem_pct:.1f}% problematic - check data)')
    exit(0)  # still proceed, just warn
" 2>&1 | tee -a "$LOG"

echo "$(date): Quality check done. Starting training..." | tee -a "$LOG"

# Start training
python3 train.py \
    --data "$DATA" \
    --epochs 5 \
    --batch-size 16 \
    --output output/chirp-cleanup \
    2>&1 | tee -a "$LOG"

echo "$(date): Training complete!" | tee -a "$LOG"
