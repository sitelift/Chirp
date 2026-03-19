#!/usr/bin/env python3
"""
Clean training data for Qwen fine-tuning.
- Remove markdown formatting (**bold**, - bullets)
- Flatten aggressive list restructuring back to sentences
- Add passthrough pairs (clean text in = clean text out)
- Format for Qwen chat fine-tuning
"""

import json
import random
import re
import sys
from pathlib import Path

INPUT_FILE = Path(__file__).parent / "data" / "training_pairs.jsonl"
OUTPUT_FILE = Path(__file__).parent / "data" / "training_pairs_clean.jsonl"
QWEN_OUTPUT = Path(__file__).parent / "data" / "training_qwen.jsonl"

random.seed(42)


def clean_output(text: str) -> str:
    """Remove markdown formatting from output text."""
    result = text

    # Remove **bold** markers
    result = re.sub(r'\*\*(.+?)\*\*', r'\1', result)

    # Remove ## headers
    result = re.sub(r'^#{1,3}\s*', '', result, flags=re.MULTILINE)

    # Convert bullet lists to inline
    # "- item1\n- item2\n- item3" -> "item1, item2, item3"
    bullet_block = re.compile(r'((?:^- .+\n?){2,})', re.MULTILINE)
    def flatten_bullets(m):
        items = [line.lstrip('- ').strip() for line in m.group(0).strip().split('\n') if line.strip()]
        if len(items) <= 5:
            return ', '.join(items) + '.'
        return '\n'.join(f'{i+1}. {item}' for i, item in enumerate(items))
    result = bullet_block.sub(flatten_bullets, result)

    # Clean up any remaining standalone bullet at start
    result = re.sub(r'^- ', '', result)

    return result.strip()


def is_aggressive_list_restructure(inp: str, out: str) -> bool:
    """Check if output aggressively restructured a sentence into a numbered list."""
    inp_has_list_words = bool(re.search(r'\b(first|second|third|fourth|fifth)\b', inp, re.I))
    out_has_numbered = bool(re.search(r'^\d+\.', out, re.MULTILINE))
    # If input was already list-like, numbered output is fine
    if inp_has_list_words and out_has_numbered:
        return False
    # If input was a plain sentence and output is a numbered list, that's aggressive
    if not inp_has_list_words and out_has_numbered and '\n' in out:
        # Check if it turned a single sentence into multiple numbered items
        lines = [l for l in out.split('\n') if l.strip()]
        if len(lines) >= 3 and all(re.match(r'\d+\.', l.strip()) for l in lines):
            return True
    return False


def flatten_numbered_list(text: str) -> str:
    """Convert a numbered list back to a sentence."""
    lines = [l.strip() for l in text.split('\n') if l.strip()]
    if not lines:
        return text

    # Check if all lines are numbered
    items = []
    preamble = ""
    for i, line in enumerate(lines):
        m = re.match(r'\d+\.\s*(.*)', line)
        if m:
            items.append(m.group(1).rstrip('.').strip())
        elif i == 0:
            preamble = line.rstrip(':').strip()
        else:
            return text  # Not a clean numbered list

    if len(items) < 2:
        return text

    if preamble:
        joined = ', '.join(items[:-1]) + ', and ' + items[-1]
        return f"{preamble}: {joined}."
    else:
        joined = ', '.join(items[:-1]) + ', and ' + items[-1]
        return joined[0].upper() + joined[1:] + '.'


# Passthrough source sentences - clean, well-formed text that should come back unchanged
PASSTHROUGH_TEMPLATES = [
    "The meeting is scheduled for 3 PM tomorrow.",
    "Please review the attached document and let me know your thoughts.",
    "I'll be out of the office next Monday and Tuesday.",
    "The project deadline has been moved to March 28.",
    "Can you send me the latest version of the report?",
    "We need to finalize the budget by end of day Friday.",
    "The new feature will be available in the next release.",
    "I've updated the spreadsheet with the latest numbers.",
    "The client approved the proposal this morning.",
    "Let's schedule a follow-up call for next week.",
    "The server migration is complete and everything looks good.",
    "I'll handle the customer support tickets this afternoon.",
    "The quarterly review is on Thursday at 2 PM.",
    "We received 150 new sign-ups last week.",
    "The API documentation has been updated.",
    "I'm working on the bug fix and should have it done by tomorrow.",
    "The design mockups look great.",
    "We need to order more supplies before the end of the month.",
    "The training session went really well.",
    "I'll send you the meeting notes after lunch.",
    "Revenue increased by 12% compared to last quarter.",
    "The new hire starts on Monday.",
    "I've tested the fix and it works correctly now.",
    "The conference call is at 10 AM Pacific time.",
    "We should prioritize the security update.",
    "The marketing campaign launches next Tuesday.",
    "I finished the code review and left some comments.",
    "The database backup completed successfully.",
    "We have enough inventory to last through April.",
    "The customer feedback has been overwhelmingly positive.",
    "I need to update my availability for next week.",
    "The presentation slides are ready for review.",
    "Our response time improved by 30% this month.",
    "The contract renewal is due in two weeks.",
    "I'll coordinate with the design team on the mockups.",
    "The test suite is passing on all platforms.",
    "We should discuss the roadmap at our next standup.",
    "The invoice was sent to the client yesterday.",
    "I've added the new endpoint to the API.",
    "The product launch is on track for Q2.",
    "Please update the status in the project tracker.",
    "The Wi-Fi password is on the whiteboard in the conference room.",
    "I grabbed coffee on my way in this morning.",
    "The weather is supposed to be nice this weekend.",
    "My flight lands at 6:45 PM on Friday.",
    "The recipe calls for two cups of flour and one cup of sugar.",
    "I finished reading that book you recommended.",
    "The kids have soccer practice at 4 PM.",
    "We're thinking about getting a new couch for the living room.",
    "The restaurant on Main Street has great reviews.",
    "I need to renew my driver's license before it expires next month.",
    "The grocery store closes at 9 PM on weekdays.",
    "Happy birthday! Hope you have a great day.",
    "The movie starts at 7:30 so we should leave by 7.",
    "I'll pick up dinner on my way home tonight.",
    "The parking lot is full so I had to park on the street.",
    "Don't forget to water the plants while I'm gone.",
    "The doctor's appointment is at 11 AM on Wednesday.",
    "I ordered the parts and they should arrive by Thursday.",
    "The battery on my laptop is almost dead.",
]


def generate_passthroughs(count: int) -> list:
    """Generate passthrough pairs where clean text should come back unchanged."""
    pairs = []
    for template in PASSTHROUGH_TEMPLATES:
        pairs.append({"input": template, "output": template})

    # Also generate variations
    variations = [
        "I have a meeting with the marketing team at 2 PM today.",
        "The total cost comes to $1,247.50 including tax.",
        "She said the report would be ready by end of day.",
        "We're meeting at the coffee shop on 5th Avenue at noon.",
        "The temperature is supposed to drop to 40 degrees tonight.",
        "I already sent the email with the updated timeline.",
        "The package arrived this morning.",
        "They confirmed the reservation for 8 people at 7 PM.",
        "The deadline for submissions is April 15.",
        "I'll review the pull request first thing tomorrow morning.",
        "The team agreed to move forward with option B.",
        "My phone number is 555-0123.",
        "The gym is closed on Sundays.",
        "We spent about $300 on supplies this month.",
        "The next train leaves in 15 minutes.",
    ]
    for v in variations:
        pairs.append({"input": v, "output": v})

    # Pad to target count by repeating with slight variations
    while len(pairs) < count:
        base = random.choice(PASSTHROUGH_TEMPLATES + variations)
        pairs.append({"input": base, "output": base})

    return pairs[:count]


def format_for_qwen(pair: dict) -> dict:
    """Format a training pair for Qwen chat fine-tuning."""
    system_msg = "Clean up dictated speech. Remove fillers, fix stutters, resolve self-corrections (keep only the final version). Output only the cleaned text."
    return {
        "messages": [
            {"role": "system", "content": system_msg},
            {"role": "user", "content": pair["input"]},
            {"role": "assistant", "content": pair["output"]},
        ]
    }


def main():
    print(f"Loading {INPUT_FILE}...")
    with open(INPUT_FILE) as f:
        pairs = [json.loads(line) for line in f]
    print(f"Loaded {len(pairs)} pairs")

    # Stats before
    bold_before = sum(1 for p in pairs if '**' in p['output'])
    bullets_before = sum(1 for p in pairs if '\n- ' in p['output'] or p['output'].startswith('- '))
    pass_before = sum(1 for p in pairs if p['input'].strip() == p['output'].strip())

    # Clean each pair
    cleaned = []
    removed = 0
    flattened_lists = 0

    for pair in pairs:
        inp = pair['input']
        out = pair['output']

        # Clean markdown from output
        out = clean_output(out)

        # Check for aggressive list restructuring
        if is_aggressive_list_restructure(inp, out):
            flat = flatten_numbered_list(out)
            if flat != out:
                out = flat
                flattened_lists += 1

        # Skip if output is empty after cleaning
        if not out.strip():
            removed += 1
            continue

        # Skip if output is way longer than input (model hallucinated)
        if len(out) > len(inp) * 1.5 and len(inp) > 20:
            removed += 1
            continue

        cleaned.append({"input": inp, "output": out})

    # Add passthroughs (~15% of dataset)
    target_passthroughs = int(len(cleaned) * 0.15)
    existing_passthroughs = sum(1 for p in cleaned if p['input'].strip() == p['output'].strip())
    new_passthroughs = max(0, target_passthroughs - existing_passthroughs)
    passthrough_pairs = generate_passthroughs(new_passthroughs)
    cleaned.extend(passthrough_pairs)

    # Shuffle
    random.shuffle(cleaned)

    # Stats after
    bold_after = sum(1 for p in cleaned if '**' in p['output'])
    bullets_after = sum(1 for p in cleaned if '\n- ' in p['output'] or p['output'].startswith('- '))
    pass_after = sum(1 for p in cleaned if p['input'].strip() == p['output'].strip())

    print(f"\n=== CLEANING RESULTS ===")
    print(f"  Original pairs:     {len(pairs)}")
    print(f"  Removed (bad):      {removed}")
    print(f"  Flattened lists:    {flattened_lists}")
    print(f"  Added passthroughs: {new_passthroughs}")
    print(f"  Final pairs:        {len(cleaned)}")
    print(f"")
    print(f"  **Bold** markup:    {bold_before} -> {bold_after}")
    print(f"  Bullet points:      {bullets_before} -> {bullets_after}")
    print(f"  Passthroughs:       {pass_before} -> {pass_after}")

    # Save cleaned pairs
    with open(OUTPUT_FILE, 'w') as f:
        for pair in cleaned:
            f.write(json.dumps(pair) + '\n')
    print(f"\nSaved to: {OUTPUT_FILE}")

    # Save Qwen chat format
    with open(QWEN_OUTPUT, 'w') as f:
        for pair in cleaned:
            f.write(json.dumps(format_for_qwen(pair)) + '\n')
    print(f"Saved Qwen format to: {QWEN_OUTPUT}")

    # Show some samples
    print(f"\n=== SAMPLE CLEANED PAIRS ===")
    for pair in cleaned[:10]:
        inp = pair['input'][:80]
        out = pair['output'][:80]
        same = " [PASSTHROUGH]" if pair['input'].strip() == pair['output'].strip() else ""
        print(f"  IN:  {inp}")
        print(f"  OUT: {out}{same}")
        print()


if __name__ == "__main__":
    main()
