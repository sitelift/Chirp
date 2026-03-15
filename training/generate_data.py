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
import random
import re
import asyncio
import time
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
                "Pros: it's faster, cheaper, and easier to maintain. Cons: fewer features and a smaller community."
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
                "Budget: We allocated $200,000 for Q1 and we're on track to spend about $180,000."
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
                "Small plan: $10/mo for 5 users. Medium: $25/mo for 20 users. Enterprise: $100/mo, unlimited users."
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

GENERATION_PROMPT = """You are generating training data for a voice-to-text cleanup model. The model makes dictated text read like it was typed — clean, natural, and invisible. It should feel like magic: the user dictates and gets back exactly what they would have typed themselves.

The model sits AFTER a regex stage that already handled filler removal, spoken punctuation, basic capitalization, and simple number formatting.

DESIGN PHILOSOPHY:
- The #1 goal is CLEAN PROSE. Most output should just be well-written plain text.
- The user is dictating into text fields, chat apps, emails, documents — not a markdown editor.
- Formatting (lists, bold, headers, tables) is a RARE treat, not the default. Only use it when the content is so obviously structured that plain text would be worse. Think: 5+ enumerated items, a clear comparison, or a long document with distinct sections.
- When in doubt, output plain text. A clean sentence is always better than unnecessary formatting.
- NEVER make the output feel like an AI wrote it. No corporate jargon, no over-polished language. Keep the speaker's voice and personality.
- Short inputs should get short outputs. Don't inflate or over-process simple messages.
- The model should be a JOY to use — never a hindrance. If a user would have to undo the model's changes, the training pair is bad.

Generate exactly {batch_size} training pairs as a JSON array. Each pair has:

- "input": Text that has been through basic regex cleanup (has capitalization, basic punctuation, no filler words) but still READS LIKE SPEECH — wordy, rambly, run-on, repetitive.

- "output": The same content cleaned up as the person would have typed it:
  - Tighten wordy constructions (cut verbal padding, but keep their voice)
  - Resolve course corrections (keep only the final intent)
  - Split run-on sentences at natural boundaries
  - Compress redundancy ("really really" → "really")
  - Clean up dates/times/numbers to natural written format
  - Add paragraph breaks for long text with topic shifts
  - Format emails properly (greeting, body, sign-off) when clearly dictating an email
  - Use bullet/numbered lists ONLY when there are 4+ clearly enumerated items
  - Use bold ONLY for genuinely critical emphasis (rare)
  - Use headers ONLY in long, multi-section content (rare)
  - Use tables ONLY when data is clearly comparative with 3+ rows (rare)
  - Use proper quotation marks when someone is clearly quoting another person
  - For already-clean text: return it unchanged or with minimal tweaks

Category focus: {category_name}
Description: {category_description}

Example pairs for reference:
{examples}

CRITICAL RULES:
1. The INPUT is POST-REGEX — no filler words (um, uh, like, you know) in inputs
2. Keep inputs between 15-100 words (mix of lengths, lean toward longer)
3. Inputs should sound like transcribed speech that has been lightly cleaned
4. ~60% of outputs should be PLAIN TEXT with no markdown formatting at all
5. Keep the speaker's natural voice — don't make everything sound corporate or robotic
6. Each pair must be unique and realistic — real people dictating real things
7. Vary the domain: work messages, personal texts, emails, meeting notes, technical discussions, creative writing, to-do lists, documentation
8. Short casual inputs should stay short and casual — don't over-process them
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

    # Reject outputs with placeholder brackets (hallucinated templates)
    if re.search(r'\[.*?\]', out):
        return False

    # Input and output shouldn't be identical unless passthrough category
    # (we check this loosely — very similar is also suspect)
    similarity = SequenceMatcher(None, inp.lower(), out.lower()).ratio()
    if similarity == 1.0:
        # Exact match — only okay for ~10% of data (passthrough)
        pass

    return True


async def generate_batch(client, category, batch_size=50):
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

    response = await client.messages.create(
        model="claude-haiku-4-5-20251001",
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

    return valid, rejected


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


async def worker(worker_id, client, task_queue, results, batch_size, rate_lock):
    """Worker that pulls tasks from the queue and generates batches."""
    while True:
        try:
            category = task_queue.get_nowait()
        except asyncio.QueueEmpty:
            break

        try:
            # Stagger requests to stay under rate limit
            async with rate_lock:
                await asyncio.sleep(1.0)
            pairs, rejected = await generate_batch(client, category, batch_size)
            rej_str = f" (rejected {rejected})" if rejected > 0 else ""
            print(f"  [worker {worker_id}] '{category['name']}' → {len(pairs)} pairs{rej_str}", flush=True)
            results.extend(pairs)
        except Exception as e:
            if "429" in str(e) or "rate_limit" in str(e):
                print(f"  [worker {worker_id}] Rate limited, waiting 30s...", flush=True)
                await asyncio.sleep(30)
            else:
                print(f"  [worker {worker_id}] Error on '{category['name']}': {e}", flush=True)
                await asyncio.sleep(5)
            # Re-queue the failed task
            await task_queue.put(category)


async def run(args):
    client = anthropic.AsyncAnthropic()
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

    # Show category distribution
    total_weight = sum(c["weight"] for c in CATEGORIES)
    print(f"Generating {remaining} pairs ({args.pairs} total target)")
    print(f"Batch size: {args.batch_size}, Workers: {args.workers}")
    print(f"Output: {output_path}")
    print(f"\nCategory weights:")
    for cat in CATEGORIES:
        pct = cat["weight"] / total_weight * 100
        print(f"  {cat['name']}: {pct:.0f}%")
    print()

    total_generated = existing
    start_time = time.time()

    with open(output_path, mode) as f:
        while total_generated < args.pairs:
            # Build a queue of tasks for this round
            # Each worker batch is batch_size pairs, run workers * 1 tasks per round
            tasks_this_round = min(
                args.workers * 2,  # 2 tasks per worker per round
                max(1, (args.pairs - total_generated + args.batch_size - 1) // args.batch_size),
            )

            task_queue = asyncio.Queue()
            for _ in range(tasks_this_round):
                task_queue.put_nowait(weighted_choice(CATEGORIES))

            results = []
            rate_lock = asyncio.Lock()
            workers = [
                worker(i, client, task_queue, results, args.batch_size, rate_lock)
                for i in range(args.workers)
            ]

            await asyncio.gather(*workers)

            # Write results
            for pair in results:
                f.write(json.dumps(pair) + "\n")
            f.flush()

            total_generated += len(results)
            elapsed = time.time() - start_time
            rate = (total_generated - existing) / elapsed if elapsed > 0 else 0
            eta = (args.pairs - total_generated) / rate if rate > 0 else 0
            print(f"  Progress: {total_generated}/{args.pairs} ({rate:.0f} pairs/sec, ETA {eta/60:.1f}min)\n", flush=True)

    elapsed = time.time() - start_time
    print(f"\nDone! Generated {total_generated} total pairs in {output_path}")
    print(f"Time: {elapsed/60:.1f} minutes")


def main():
    parser = argparse.ArgumentParser(description="Generate training data with Claude API")
    parser.add_argument("--pairs", type=int, default=25000, help="Total pairs to generate")
    parser.add_argument("--batch-size", type=int, default=50, help="Pairs per API call")
    parser.add_argument("--workers", type=int, default=4, help="Concurrent API workers")
    parser.add_argument("--output", type=str, default="data/training_pairs.jsonl")
    parser.add_argument("--resume", action="store_true", help="Resume from existing file")
    args = parser.parse_args()

    asyncio.run(run(args))


if __name__ == "__main__":
    main()
