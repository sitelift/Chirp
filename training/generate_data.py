"""
Generate synthetic training pairs for the Chirp cleanup model using Claude API.

The cleanup model receives text AFTER regex preprocessing has already handled:
- Filler word removal (um, uh, like, you know, basically)
- Spoken punctuation conversion (period → ., comma → ,, etc.)
- Basic capitalization (first letter, after sentences, "I")
- Simple number formatting in context
- Percentage/email formatting
- List detection for obvious patterns

The model's job is everything regex CAN'T do — making dictated text
read like it was typed, not spoken. This includes:
- Sentence restructuring (rambling speech → concise prose)
- Course correction / false starts ("let's do 2 actually 3" → "let's do 3")
- Run-on sentence splitting (detecting natural boundaries)
- Redundancy compression ("really really really" → "really")
- Rich text formatting (lists, paragraphs, headers, bold, quotes)
- Topic-shift paragraph breaks
- Natural written tone (removing verbal padding phrases)
- Date/time normalization
- Table/structured data detection

Usage:
    export ANTHROPIC_API_KEY=sk-ant-...
    python generate_data.py --pairs 25000 --output data/training_pairs.jsonl
"""

import anthropic
import json
import argparse
import time
import random
import re
from pathlib import Path
from difflib import SequenceMatcher

CATEGORIES = [
    {
        "name": "restructuring",
        "weight": 3,
        "description": "Rambling, wordy speech that needs to be tightened into concise written prose. The input has already been through filler removal and basic punctuation, but still reads like someone talking, not typing.",
        "examples": [
            (
                "So what I was trying to say is that we need to update the homepage. And the reason for that is because the current one is outdated.",
                "We need to update the homepage — the current one is outdated."
            ),
            (
                "I think the thing is that basically the problem comes down to the fact that we don't have enough testing in place.",
                "The problem is that we don't have enough testing in place."
            ),
            (
                "What I want to say is I had a really great time at the conference and I learned a lot of things and I think we should send more people next year.",
                "I had a great time at the conference and learned a lot. We should send more people next year."
            ),
        ]
    },
    {
        "name": "course_correction",
        "weight": 2,
        "description": "Speech where the speaker changes their mind mid-sentence, restates something, or corrects themselves. The model should keep only the final intent.",
        "examples": [
            (
                "Let's schedule it for Tuesday, actually no, Wednesday at 3 PM.",
                "Let's schedule it for Wednesday at 3 PM."
            ),
            (
                "The budget is 50,000, wait no, I think it's 45,000 for this quarter.",
                "The budget is $45,000 for this quarter."
            ),
            (
                "We should use React, or actually, I think Vue might be better for this project since it's smaller.",
                "I think Vue might be better for this project since it's smaller."
            ),
        ]
    },
    {
        "name": "run_on_splitting",
        "weight": 3,
        "description": "Long run-on speech that needs to be split into proper sentences with correct punctuation. Input may have some periods from regex but misses many natural sentence boundaries.",
        "examples": [
            (
                "The server went down at 3 AM and the team was notified but nobody responded until 7 and by then we had lost about 4 hours of data and the clients were already complaining.",
                "The server went down at 3 AM. The team was notified, but nobody responded until 7. By then, we had lost about 4 hours of data, and clients were already complaining."
            ),
            (
                "I went to the store and picked up some groceries and then I stopped by the pharmacy and then I came home and started cooking dinner.",
                "I went to the store and picked up some groceries, then stopped by the pharmacy. I came home and started cooking dinner."
            ),
        ]
    },
    {
        "name": "lists_formatting",
        "weight": 3,
        "description": "Content that should be formatted as numbered lists, bullet points, or structured items. The speaker is clearly enumerating things but the text is flat.",
        "examples": [
            (
                "For the release we need to do a few things. We need to update the changelog and run the full test suite and bump the version number and then create the tag and deploy to staging first.",
                "For the release, we need to:\n1. Update the changelog\n2. Run the full test suite\n3. Bump the version number\n4. Create the tag\n5. Deploy to staging first"
            ),
            (
                "The pros are it's faster and cheaper and easier to maintain. The cons are it has less features and the community is smaller.",
                "**Pros:**\n- Faster\n- Cheaper\n- Easier to maintain\n\n**Cons:**\n- Fewer features\n- Smaller community"
            ),
            (
                "My top priorities this week are finishing the API integration and fixing the login bug and writing tests for the new payment flow.",
                "My top priorities this week:\n1. Finish the API integration\n2. Fix the login bug\n3. Write tests for the new payment flow"
            ),
        ]
    },
    {
        "name": "paragraph_structure",
        "weight": 2,
        "description": "Longer dictation that needs paragraph breaks at topic shifts. The text should be broken into logical paragraphs with proper spacing.",
        "examples": [
            (
                "The project kicked off last Monday and so far things are going smoothly. The team has been really productive. On the technical side, we've finished the database migration and the new API is almost ready. We're expecting to have it done by Friday. The one concern I have is the timeline for the frontend. We lost a developer last week and we haven't found a replacement yet.",
                "The project kicked off last Monday and so far things are going smoothly. The team has been really productive.\n\nOn the technical side, we've finished the database migration and the new API is almost ready. We're expecting to have it done by Friday.\n\nThe one concern I have is the timeline for the frontend. We lost a developer last week and we haven't found a replacement yet."
            ),
        ]
    },
    {
        "name": "email_structure",
        "weight": 2,
        "description": "Email dictation that needs proper email formatting — greeting, body paragraphs, sign-off, proper spacing and tone.",
        "examples": [
            (
                "Hey Mike, just wanted to follow up on the proposal we discussed last week. I've made the changes you suggested and attached the updated version. Let me know if you want to schedule a call to walk through it. Thanks, Sarah.",
                "Hey Mike,\n\nJust wanted to follow up on the proposal we discussed last week. I've made the changes you suggested and attached the updated version.\n\nLet me know if you want to schedule a call to walk through it.\n\nThanks,\nSarah"
            ),
            (
                "Hi team, a few quick updates. The deployment went smoothly and we're seeing good metrics. Also, don't forget the all-hands meeting is moved to Thursday this week. Best, James.",
                "Hi team,\n\nA few quick updates:\n- The deployment went smoothly and we're seeing good metrics\n- The all-hands meeting is moved to Thursday this week\n\nBest,\nJames"
            ),
        ]
    },
    {
        "name": "emphasis_headers",
        "weight": 2,
        "description": "Content where the speaker emphasizes key points that should be bold, or introduces sections/topics that should be headers.",
        "examples": [
            (
                "The most important thing to remember here is never deploy on Fridays. That's the number one rule.",
                "The most important thing to remember: **never deploy on Fridays.** That's the number one rule."
            ),
            (
                "Okay, moving on to the budget section. We allocated 200,000 for Q1 and we're on track to spend about 180,000.",
                "## Budget\n\nWe allocated $200,000 for Q1 and we're on track to spend about $180,000."
            ),
            (
                "So the key takeaway from the meeting is that we're pushing the launch to April and the critical blocker is the security audit.",
                "**Key takeaway:** We're pushing the launch to April. The critical blocker is the security audit."
            ),
        ]
    },
    {
        "name": "quotes_attribution",
        "weight": 1,
        "description": "Speech that contains quotes or paraphrases of what other people said, which should be properly formatted with quotation marks.",
        "examples": [
            (
                "And then Sarah said we need to push the deadline back by two weeks. And I was like, I don't think the client will be okay with that.",
                'Sarah said, "We need to push the deadline back by two weeks." I responded that I didn\'t think the client would be okay with that.'
            ),
            (
                "The error message says connection refused and then it shows the port number.",
                'The error message says "connection refused" and then shows the port number.'
            ),
        ]
    },
    {
        "name": "redundancy_compression",
        "weight": 2,
        "description": "Speech with verbal repetition, redundant phrases, or unnecessary padding that should be compressed into clean prose.",
        "examples": [
            (
                "The thing is is that we've already basically done this exact same thing before in the past and it didn't really work out very well at all.",
                "We've done this before and it didn't work out well."
            ),
            (
                "It's really really important that everyone makes sure to double check and verify that the tests are all passing and working correctly.",
                "It's important that everyone verifies the tests are passing."
            ),
            (
                "At the end of the day, the bottom line is that what it really comes down to is cost.",
                "The bottom line is cost."
            ),
        ]
    },
    {
        "name": "dates_times_numbers",
        "weight": 1,
        "description": "Content with dates, times, and numbers that need consistent, clean formatting. Input may have partial formatting from regex but needs normalization.",
        "examples": [
            (
                "The deadline is March 15th twenty twenty six and the budget meeting is the following Tuesday at 2.",
                "The deadline is March 15, 2026, and the budget meeting is the following Tuesday at 2:00 PM."
            ),
            (
                "We processed about twelve thousand three hundred orders last month which is up about 15% from the month before.",
                "We processed about 12,300 orders last month, up ~15% from the previous month."
            ),
        ]
    },
    {
        "name": "table_structured",
        "weight": 1,
        "description": "Data that the speaker is listing in a way that would be better as a structured format — comparison, key-value pairs, or tabular data.",
        "examples": [
            (
                "The small plan costs 10 a month and gives you 5 users. The medium plan is 25 a month for 20 users. And the enterprise plan is 100 a month with unlimited users.",
                "| Plan | Price | Users |\n|------|-------|-------|\n| Small | $10/mo | 5 |\n| Medium | $25/mo | 20 |\n| Enterprise | $100/mo | Unlimited |"
            ),
            (
                "John is handling the frontend, Maria is doing the backend, and Alex is on DevOps.",
                "- **John** — Frontend\n- **Maria** — Backend\n- **Alex** — DevOps"
            ),
        ]
    },
    {
        "name": "passthrough_clean",
        "weight": 1,
        "description": "Text that is already clean and well-formed after regex processing. The model should learn to leave good text alone, returning it unchanged or with only minimal tweaks.",
        "examples": [
            (
                "The meeting is at 3 PM tomorrow.",
                "The meeting is at 3 PM tomorrow."
            ),
            (
                "I'll send the report by end of day.",
                "I'll send the report by end of day."
            ),
        ]
    },
]

GENERATION_PROMPT = """You are generating training data for a text cleanup AI model. This model sits in a voice-to-text pipeline AFTER a regex stage has already handled basic cleanup (filler removal, spoken punctuation conversion, basic capitalization, simple number formatting).

The model's job is to transform text that still "sounds spoken" into text that "reads like it was typed." Think of it as the difference between a raw transcription and what the person would have written if they were typing instead of talking.

Generate exactly {batch_size} training pairs as a JSON array. Each pair has:

- "input": Text that has already been through basic regex cleanup. It will have:
  - Capitalization at the start of sentences
  - Basic punctuation (periods at end, some commas)
  - Filler words already removed
  - But it still READS LIKE SPEECH: wordy, rambling, run-on, flat structure, no rich formatting

- "output": The same content transformed into polished written text:
  - Restructured for conciseness (cut verbal padding and wordy constructions)
  - Course corrections resolved (keep only final intent, drop false starts)
  - Run-on sentences split at natural boundaries
  - Redundancy compressed ("really really" → "really")
  - Rich formatting applied where appropriate:
    - Numbered/bullet lists for enumerated items
    - Paragraph breaks at topic shifts
    - **Bold** for key terms or emphasis
    - ## Headers for section introductions
    - Proper quote formatting
    - Tables for comparative/structured data
    - Email structure (greeting, body, sign-off) when dictating emails
  - Dates/times in clean, consistent format
  - Natural written tone throughout

Category focus: {category_name}
Description: {category_description}

Example pairs for reference:
{examples}

CRITICAL RULES:
1. The INPUT is POST-REGEX — it already has basic punctuation and capitalization. Do NOT include filler words (um, uh, like, you know) in inputs.
2. Keep inputs between 15-100 words (mix of lengths, lean toward longer)
3. Make inputs sound like transcribed speech that has been lightly cleaned, not like written text
4. The output should be noticeably better than the input — not just minor tweaks
5. Each pair must be unique and realistic — imagine real people dictating real work
6. Don't over-format — only use rich formatting (lists, headers, bold, tables) when it genuinely improves readability
7. For the passthrough category: output should be identical or nearly identical to input
8. Vary the domain: work emails, meeting notes, personal messages, technical discussions, creative writing, to-do lists, documentation, reports
9. Output ONLY the JSON array, no other text

Output format:
[
  {{"input": "...", "output": "..."}},
  ...
]"""


def validate_pair(pair):
    """Validate a single training pair for quality."""
    if not isinstance(pair, dict):
        return False
    inp = pair.get("input", "").strip()
    out = pair.get("output", "").strip()

    if not inp or not out:
        return False

    # Input too short or too long
    word_count = len(inp.split())
    if word_count < 5 or word_count > 200:
        return False

    # Input still has filler words (regex should have removed these)
    filler_pattern = re.compile(r'\b(um|uh|like you know|i mean,|basically,)\b', re.IGNORECASE)
    if filler_pattern.search(inp):
        return False

    # Output shouldn't be longer than 2x input (model is cleaning, not expanding)
    if len(out) > len(inp) * 2.5:
        return False

    # Input and output shouldn't be identical unless passthrough category
    # (we check this loosely — very similar is also suspect)
    similarity = SequenceMatcher(None, inp.lower(), out.lower()).ratio()
    if similarity == 1.0:
        # Exact match — only okay for ~10% of data (passthrough)
        pass

    return True


def generate_batch(client, category, batch_size=25):
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
    rejected = 0
    for pair in pairs:
        if validate_pair(pair):
            valid.append({"input": pair["input"].strip(), "output": pair["output"].strip()})
        else:
            rejected += 1

    if rejected > 0:
        print(f"(rejected {rejected})", end=" ", flush=True)

    return valid


def weighted_choice(categories):
    """Pick a category weighted by its weight field."""
    total = sum(c["weight"] for c in categories)
    r = random.uniform(0, total)
    cumulative = 0
    for cat in categories:
        cumulative += cat["weight"]
        if r <= cumulative:
            return cat
    return categories[-1]


def main():
    parser = argparse.ArgumentParser(description="Generate training data with Claude API")
    parser.add_argument("--pairs", type=int, default=25000, help="Total pairs to generate")
    parser.add_argument("--batch-size", type=int, default=25, help="Pairs per API call")
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

    # Show category distribution
    total_weight = sum(c["weight"] for c in CATEGORIES)
    print(f"Generating {remaining} pairs ({args.pairs} total target)")
    print(f"Batch size: {args.batch_size}")
    print(f"Output: {output_path}")
    print(f"\nCategory weights:")
    for cat in CATEGORIES:
        pct = cat["weight"] / total_weight * 100
        print(f"  {cat['name']}: {pct:.0f}%")
    print()

    with open(output_path, mode) as f:
        while total_generated < args.pairs:
            category = weighted_choice(CATEGORIES)
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
