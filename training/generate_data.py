"""
Generate synthetic (raw transcript -> clean text) training pairs using Claude API.

Usage:
    export ANTHROPIC_API_KEY=sk-ant-...
    python generate_data.py --pairs 15000 --output data/training_pairs.jsonl

Generates diverse transcript cleanup examples across categories:
- Casual dictation, emails, messages
- Technical descriptions, code talk
- Lists and structured content
- Numbers, currencies, percentages
- Emails, URLs, phone numbers spoken out
- Filler-heavy speech
- Multi-sentence paragraphs
- Questions, mixed tone
"""

import anthropic
import json
import argparse
import time
import random
from pathlib import Path

CATEGORIES = [
    {
        "name": "casual_email",
        "description": "Casual email or message dictation",
        "examples": [
            ("um hey sarah i wanted to follow up on our meeting yesterday comma i think we should move forward with option two period let me know what you think", "Hey Sarah,\n\nI wanted to follow up on our meeting yesterday. I think we should move forward with option two. Let me know what you think."),
            ("so like i was thinking we could grab lunch on friday at that new place on fifth street question mark", "I was thinking we could grab lunch on Friday at that new place on 5th Street?"),
        ]
    },
    {
        "name": "technical",
        "description": "Technical descriptions, bug reports, code discussions",
        "examples": [
            ("the api endpoint returns a four oh four when you pass an invalid user id um we need to add validation in the middleware before it hits the database", "The API endpoint returns a 404 when you pass an invalid user ID. We need to add validation in the middleware before it hits the database."),
            ("basically the function takes two parameters first the input array and second the callback and it returns a promise", "The function takes two parameters: first, the input array, and second, the callback. It returns a promise."),
        ]
    },
    {
        "name": "lists",
        "description": "Content with lists, steps, or enumerated items",
        "examples": [
            ("okay so the steps are first clone the repository second install dependencies with npm install third create a dot env file and fourth run npm start", "The steps are:\n1. Clone the repository\n2. Install dependencies with npm install\n3. Create a .env file\n4. Run npm start"),
            ("i need to buy um milk eggs bread and uh some chicken for dinner tonight", "I need to buy milk, eggs, bread, and some chicken for dinner tonight."),
        ]
    },
    {
        "name": "numbers_money",
        "description": "Text with numbers, currencies, percentages, measurements",
        "examples": [
            ("the project budget is around fifty thousand dollars and we've spent about thirty percent so far", "The project budget is around $50,000 and we've spent about 30% so far."),
            ("um the meeting is at two thirty pm on march fifteenth twenty twenty six", "The meeting is at 2:30 PM on March 15th, 2026."),
        ]
    },
    {
        "name": "contact_info",
        "description": "Emails, URLs, phone numbers spoken out",
        "examples": [
            ("you can reach me at john dot smith at gmail dot com or call me at five five five dash one two three four", "You can reach me at john.smith@gmail.com or call me at 555-1234."),
            ("check out the docs at https colon slash slash docs dot example dot com slash getting started", "Check out the docs at https://docs.example.com/getting-started."),
        ]
    },
    {
        "name": "filler_heavy",
        "description": "Speech with lots of filler words, false starts, repetitions",
        "examples": [
            ("so um yeah i was i was thinking that uh you know we should probably like reconsider the the design because basically it's not it's not working", "I was thinking that we should probably reconsider the design because it's not working."),
            ("i mean like honestly um the the performance is is kind of like really bad right now you know", "Honestly, the performance is really bad right now."),
        ]
    },
    {
        "name": "paragraphs",
        "description": "Multi-sentence paragraphs and longer dictation",
        "examples": [
            ("the project is going well period new paragraph we finished the backend last week and the frontend is about eighty percent done period the main thing left is testing and documentation period", "The project is going well.\n\nWe finished the backend last week and the frontend is about 80% done. The main thing left is testing and documentation."),
        ]
    },
    {
        "name": "questions_exclamations",
        "description": "Questions, exclamations, mixed tone",
        "examples": [
            ("wait are you serious question mark that's amazing exclamation point when did this happen", "Wait, are you serious? That's amazing! When did this happen?"),
            ("can you send me the report by end of day question mark i need it for the board meeting tomorrow", "Can you send me the report by end of day? I need it for the board meeting tomorrow."),
        ]
    },
    {
        "name": "spoken_punctuation",
        "description": "Text with spoken punctuation commands mixed with natural speech",
        "examples": [
            ("dear team comma new paragraph i wanted to share some updates on the project period first comma we've completed the migration period second comma the new system is live period new paragraph please let me know if you have any questions period", "Dear team,\n\nI wanted to share some updates on the project. First, we've completed the migration. Second, the new system is live.\n\nPlease let me know if you have any questions."),
        ]
    },
    {
        "name": "mixed_casual",
        "description": "Everyday casual speech, notes to self, quick thoughts",
        "examples": [
            ("remind me to um call the dentist tomorrow and also pick up the prescription from walgreens", "Remind me to call the dentist tomorrow and also pick up the prescription from Walgreens."),
            ("note to self colon look into upgrading the server to the new version before friday", "Note to self: look into upgrading the server to the new version before Friday."),
        ]
    },
]

GENERATION_PROMPT = """You are generating training data for a speech-to-text cleanup model. The model takes raw, messy transcripts and produces clean, well-formatted text.

Generate exactly {batch_size} training pairs as a JSON array. Each pair has:
- "input": A raw transcript as it would come from a speech recognition system. Include realistic imperfections:
  - Filler words (um, uh, like, you know, basically, I mean, sort of, kind of)
  - No capitalization or inconsistent capitalization
  - Missing or no punctuation
  - Spoken punctuation commands (period, comma, question mark, exclamation point/mark, colon, semicolon, new line, new paragraph, dash, hyphen, open/close paren)
  - Spoken numbers instead of digits (sometimes)
  - Repeated words or false starts (the the, I I, we we should)
  - Run-on sentences
  - Spoken-out emails (at, dot com), URLs, phone numbers
  - Spoken percentages (fifty percent), money (twenty dollars)

- "output": The clean, properly formatted version:
  - Remove ALL filler words
  - Remove false starts and repetitions
  - Proper capitalization (sentences, "I", proper nouns)
  - Proper punctuation (periods, commas, question marks, etc.)
  - Convert spoken punctuation to actual punctuation
  - Format numbers appropriately (digits for numbers, $, %, etc.)
  - Format emails, URLs properly
  - Add paragraph breaks where "new paragraph" was spoken
  - Format lists with numbers or bullets when enumerated
  - Natural, clean prose

Category focus for this batch: {category_name}
Description: {category_description}

Example pairs for reference:
{examples}

IMPORTANT RULES:
- Keep inputs between 10-80 words (varying lengths)
- Make inputs sound like REAL spoken English, not written text read aloud
- Vary the density of fillers (some light, some heavy)
- Include a mix of simple and complex formatting needs
- Each pair should be unique and different from the examples
- Output ONLY the JSON array, no other text
- The input should NEVER contain proper punctuation — it's raw speech output

Output format:
[
  {{"input": "...", "output": "..."}},
  ...
]"""


def generate_batch(client, category, batch_size=50):
    examples_str = "\n".join(
        f'  Input:  "{inp}"\n  Output: "{out}"'
        for inp, out in category["examples"]
    )

    prompt = GENERATION_PROMPT.format(
        batch_size=batch_size,
        category_name=category["name"],
        category_description=category["description"],
        examples=examples_str,
    )

    response = client.messages.create(
        model="claude-sonnet-4-20250514",
        max_tokens=8192,
        messages=[{"role": "user", "content": prompt}],
    )

    text = response.content[0].text.strip()

    # Extract JSON array from response
    start = text.find("[")
    end = text.rfind("]") + 1
    if start == -1 or end == 0:
        raise ValueError(f"No JSON array found in response: {text[:200]}")

    pairs = json.loads(text[start:end])

    # Validate pairs
    valid = []
    for pair in pairs:
        if isinstance(pair, dict) and "input" in pair and "output" in pair:
            inp = pair["input"].strip()
            out = pair["output"].strip()
            if inp and out and inp != out:
                valid.append({"input": inp, "output": out})

    return valid


def main():
    parser = argparse.ArgumentParser(description="Generate training data with Claude API")
    parser.add_argument("--pairs", type=int, default=5000, help="Total pairs to generate")
    parser.add_argument("--batch-size", type=int, default=50, help="Pairs per API call")
    parser.add_argument("--output", type=str, default="data/training_pairs.jsonl")
    parser.add_argument("--resume", action="store_true", help="Resume from existing file")
    args = parser.parse_args()

    client = anthropic.Anthropic()
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    # Count existing pairs if resuming
    existing = 0
    if args.resume and output_path.exists():
        with open(output_path) as f:
            existing = sum(1 for _ in f)
        print(f"Resuming from {existing} existing pairs")

    remaining = args.pairs - existing
    if remaining <= 0:
        print(f"Already have {existing} pairs, target is {args.pairs}. Done!")
        return

    mode = "a" if args.resume else "w"
    total_generated = existing

    print(f"Generating {remaining} pairs ({args.pairs} total target)")
    print(f"Batch size: {args.batch_size}")
    print(f"Output: {output_path}")
    print()

    with open(output_path, mode) as f:
        while total_generated < args.pairs:
            # Pick a random category, weighted slightly toward harder ones
            category = random.choice(CATEGORIES)
            batch_target = min(args.batch_size, args.pairs - total_generated)

            try:
                print(f"  [{total_generated}/{args.pairs}] Generating {batch_target} '{category['name']}' pairs...", end=" ", flush=True)
                pairs = generate_batch(client, category, batch_target)

                for pair in pairs:
                    f.write(json.dumps(pair) + "\n")
                f.flush()

                total_generated += len(pairs)
                print(f"got {len(pairs)}")

                # Small delay to avoid rate limits
                time.sleep(0.5)

            except Exception as e:
                print(f"Error: {e}")
                print("  Retrying in 10s...")
                time.sleep(10)

    print(f"\nDone! Generated {total_generated} total pairs in {output_path}")


if __name__ == "__main__":
    main()
