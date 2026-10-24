use regex::Regex;
use std::sync::OnceLock;

use crate::state::VocabEntry;

/// Pre-compiled regex patterns for text cleanup
struct CleanupRegexes {
    fillers: Vec<Regex>,
    dangling_comma: Regex,
    leading_comma: Regex,
    whitespace: Regex,
    sentence_end: Regex,
    standalone_i: Regex,
    /// Collapses adjacent punctuation separated by optional whitespace. Used
    /// to fix artifacts like "hey comma period" → "Hey,." where two spoken
    /// punctuation commands leave their marks glued together after
    /// `space_before_punct` strips the whitespace between them. Terminal
    /// punct (`.!?`) wins over clause punct (`,;:`); ties keep the first.
    adjacent_punct: Regex,
    punctuation: Vec<(Regex, &'static str)>,
    /// Dedicated handler for "dash" → em dash conversion. Handled separately
    /// from `punctuation` because we need a closure to skip cases like
    /// "em dash" / "en dash" where the user is talking ABOUT the punctuation
    /// rather than asking to insert it. Captures: (1) optional "em "/"en "
    /// prefix, (2) the literal "dash" word.
    dash: Regex,
    space_before_punct: Regex,
    no_space_after: Regex,
    email: Regex,
    numeric_contexts: Vec<Regex>,
    number_words: Vec<&'static str>,
    percentage: Regex,
    hundred_pct: Regex,
    period_and: Regex,
    boundary_then_than: Regex,
    boundary_safe_cap: Regex,
}

fn regexes() -> &'static CleanupRegexes {
    static REGEXES: OnceLock<CleanupRegexes> = OnceLock::new();
    REGEXES.get_or_init(|| {
        let filler_patterns = [
            r"(?i)\bum+\b",
            r"(?i)\buh+\b",
            r"(?i)\buh huh\b",
            r"(?i)\bmm+ ?hmm+\b",
            r"(?i)\bhmm+\b",
            r"(?i)\byou know\b(?=\s*,?\s)",
            r"(?i)\blike\b(?=\s+(the|a|an|i|we|they|he|she|it|my|our|this|that)\b)",
            r"(?i)\bbasically\b(?=\s*,)",
            r"(?i)\bactually\b(?=\s*,)",
            r"(?i)\bso\b(?=\s*,\s)",
            r"(?i)\bi mean\b(?=\s*,)",
            r"(?i)\bkind of\b(?=\s+(like|a|the)\b)",
            r"(?i)\bsort of\b(?=\s+(like|a|the)\b)",
            r"(?i)\bright\s*\?\s*(?=\b)",
        ];

        let number_word_patterns = [
            (r"\b(?i)zero\b", "0"),
            (r"\b(?i)one\b", "1"),
            (r"\b(?i)two\b", "2"),
            (r"\b(?i)three\b", "3"),
            (r"\b(?i)four\b", "4"),
            (r"\b(?i)five\b", "5"),
            (r"\b(?i)six\b", "6"),
            (r"\b(?i)seven\b", "7"),
            (r"\b(?i)eight\b", "8"),
            (r"\b(?i)nine\b", "9"),
            (r"\b(?i)ten\b", "10"),
        ];

        let numeric_context_patterns = [
            r"(?i)\b(number|step|item|option|version|v|chapter|page|line|row|column|level|grade|score|count|total)\s+",
            r"(?i)\b(is|are|was|were|equals?|=)\s+",
            r"(?i)\b(about|around|approximately|roughly|nearly|over|under)\s+",
        ];

        // Pre-compile combined numeric context + number word patterns
        let mut compiled_contexts = Vec::new();
        let mut compiled_numbers = Vec::new();
        for ctx_pattern in &numeric_context_patterns {
            for (word_pattern, digit) in &number_word_patterns {
                let combined = format!("({ctx_pattern})({word_pattern})");
                if let Ok(re) = Regex::new(&combined) {
                    compiled_contexts.push(re);
                    compiled_numbers.push(*digit);
                }
            }
        }

        let punctuation_map: Vec<(Regex, &'static str)> = [
            (r"(?i)\bperiod\b", "."),
            (r"(?i)\bcomma\b", ","),
            (r"(?i)\bquestion mark\b", "?"),
            (r"(?i)\bexclamation (?:mark|point)\b", "!"),
            (r"(?i)\bcolon\b", ":"),
            (r"(?i)\bsemicolon\b", ";"),
            // "dash" handled separately via the `dash` field — see CleanupRegexes
            (r"(?i)\bhyphen\b", "-"),
            (r"(?i)\bopen (?:paren|parenthesis)\b", "("),
            (r"(?i)\bclose (?:paren|parenthesis)\b", ")"),
            // "new line" and "new paragraph" handled by LLM, not regex
        ]
        .iter()
        .filter_map(|(p, r)| Regex::new(p).ok().map(|re| (re, *r)))
        .collect();

        // Store pre-compiled context+number pairs as parallel vecs in the struct
        // We'll use numeric_contexts for the compiled combined regexes
        // and number_words for the corresponding digit strings
        CleanupRegexes {
            fillers: filler_patterns
                .iter()
                .filter_map(|p| Regex::new(p).ok())
                .collect(),
            dangling_comma: Regex::new(r",\s*,").unwrap(),
            leading_comma: Regex::new(r"^\s*,\s*").unwrap(),
            whitespace: Regex::new(r"\s{2,}").unwrap(),
            sentence_end: Regex::new(r#"([.!?:])(["')\]]*)(\s+)(["'(\[]*)([a-z])"#).unwrap(),
            standalone_i: Regex::new(r"\bi\b").unwrap(),
            adjacent_punct: Regex::new(r"([.!?,;:])\s*([.!?,;:])").unwrap(),
            punctuation: punctuation_map,
            // Match the literal word "dash", optionally preceded by "em" or
            // "en". Group 1 captures the prefix (or is empty); group 2 is
            // always "dash". Replacement skips when group 1 is non-empty so
            // "em dash" / "en dash" stay as content phrases.
            dash: Regex::new(r"(?i)\b(em |en )?(dash)\b").unwrap(),
            space_before_punct: Regex::new(r"\s+([.,!?;:)])").unwrap(),
            no_space_after: Regex::new(r"([.,!?;:])([A-Za-z])").unwrap(),
            email: Regex::new(r"(?i)\b(\w+)\s+at\s+(\w+)\s+dot\s+(com|org|net|io|dev|co)\b").unwrap(),
            numeric_contexts: compiled_contexts,
            number_words: compiled_numbers,
            percentage: Regex::new(r"(?i)\b(twenty|thirty|forty|fifty|sixty|seventy|eighty|ninety)\s+percent\b").unwrap(),
            hundred_pct: Regex::new(r"(?i)\b(one )?hundred percent\b").unwrap(),
            period_and: Regex::new(r"\.\s+And\b").unwrap(),
            boundary_then_than: Regex::new(r"(?i)\b(?P<then>and\s+then)\s+than\s+(?P<gerund>[a-z]+ing)\b").unwrap(),
            boundary_safe_cap: Regex::new(r"\b(?P<prefix>[A-Za-z']+)(?P<gap>\s+)(?P<word>[A-Z][a-z]+(?:'[a-z]+)?)\b").unwrap(),
        }
    })
}

/// Full cleanup pipeline: filler removal → capitalization → regex formatting.
///
/// Self-correction handling is now performed by the LLM cleanup pass
/// (chirp-cleanup-v2), which is fine-tuned for it and applies semantic
/// context. The previous regex-based `strip_corrections` step was removed
/// because its `.*` greedy match across sentence boundaries silently deleted
/// large portions of dictations whenever a discourse filler like "I mean" /
/// "actually" / "sorry" appeared mid-monologue.
pub fn cleanup_text(text: &str, smart_formatting: bool) -> String {
    if text.is_empty() {
        return String::new();
    }

    if !smart_formatting {
        return text.to_string();
    }

    // Remove fillers (um, uh, filler "like", etc.)
    let result = remove_fillers(text);
    if result.is_empty() {
        return String::new();
    }

    // Capitalize first letter (filler removal may have stripped a leading "Um,")
    let result = capitalize_first(&result);

    // Re-capitalize the first letter of intra-text sentences. Filler removal
    // at sentence start (e.g. "...up. Um there's..." -> "...up. there's...")
    // would otherwise leave the next word lowercase. Uses the pre-compiled
    // sentence_end regex which was defined for this exact case but never
    // wired in.
    let result = fix_sentence_capitalization(&result);

    // Regex-based formatting (spoken punctuation, numbers, etc.)
    let result = smart_format(&result);

    // Re-apply sentence capitalization. `smart_format` may have exposed a new
    // sentence boundary by collapsing adjacent punctuation (e.g. ",." → "."
    // reveals a lowercase word that was previously hidden mid-phrase).
    fix_sentence_capitalization(&result)
}

/// After filler removal, re-capitalize the first letter of any sentence that
/// now starts with a lowercase word. Preserves the original whitespace
/// between the punctuation and the next word.
fn fix_sentence_capitalization(text: &str) -> String {
    let re = regexes();
    re.sentence_end
        .replace_all(text, |caps: &regex::Captures| {
            let punct = &caps[1];
            let closers = &caps[2];
            let ws = &caps[3];
            let prefix = &caps[4];
            let letter = caps[5].to_uppercase();
            format!("{punct}{closers}{ws}{prefix}{letter}")
        })
        .to_string()
}

/// Remove common filler words from transcript
fn remove_fillers(text: &str) -> String {
    let re = regexes();
    let mut result = text.to_string();

    for filler in &re.fillers {
        result = filler.replace_all(&result, "").to_string();
    }

    // Clean up extra whitespace and dangling commas from removal
    result = re.dangling_comma.replace_all(&result, ",").to_string();
    result = re.leading_comma.replace(&result, "").to_string();
    re.whitespace.replace_all(result.trim(), " ").to_string()
}

/// Capitalize the first character of a string
fn capitalize_first(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    for (idx, ch) in trimmed.char_indices() {
        if ch.is_alphabetic() {
            let mut result = String::with_capacity(trimmed.len());
            result.push_str(&trimmed[..idx]);
            result.push_str(&ch.to_uppercase().to_string());
            result.push_str(&trimmed[idx + ch.len_utf8()..]);
            return result;
        }
    }

    trimmed.to_string()
}

/// Smart formatting: punctuation, capitalization, numbers, common patterns
fn smart_format(text: &str) -> String {
    let mut result = text.to_string();

    // Expand spoken numbers to digits for common cases
    result = format_spoken_numbers(&result);

    // Format common spoken patterns
    result = format_spoken_patterns(&result);

    // Capitalize any standalone lowercase "i" — the English pronoun is
    // always "I". This also normalizes contractions like "i'm" → "I'm"
    // because `\b` matches between the letter and the apostrophe.
    let re = regexes();
    result = re.standalone_i.replace_all(&result, "I").to_string();

    result
}

/// Convert spoken number words to digits for common short numbers
fn format_spoken_numbers(text: &str) -> String {
    let re = regexes();
    let mut result = text.to_string();

    // Apply pre-compiled combined context+number patterns
    for (i, ctx_re) in re.numeric_contexts.iter().enumerate() {
        let digit = re.number_words[i];
        result = ctx_re
            .replace_all(&result, |caps: &regex::Captures| {
                format!("{}{}", &caps[1], digit)
            })
            .to_string();
    }

    // Percentages
    result = re
        .percentage
        .replace_all(&result, |caps: &regex::Captures| {
            let num = match caps[1].to_lowercase().as_str() {
                "twenty" => "20",
                "thirty" => "30",
                "forty" => "40",
                "fifty" => "50",
                "sixty" => "60",
                "seventy" => "70",
                "eighty" => "80",
                "ninety" => "90",
                _ => &caps[1],
            };
            format!("{num}%")
        })
        .to_string();

    // "hundred percent" → "100%"
    result = re.hundred_pct.replace_all(&result, "100%").to_string();

    result
}

/// Format common spoken patterns (email, URLs, punctuation commands)
fn format_spoken_patterns(text: &str) -> String {
    let re = regexes();
    let mut result = text.to_string();

    // Spoken punctuation → actual punctuation
    for (pattern, replacement) in &re.punctuation {
        result = pattern.replace_all(&result, *replacement).to_string();
    }

    // "dash" → em-dash, but ONLY when it's a bare word — skip when the
    // user said "em dash" or "en dash" (talking about the punctuation
    // rather than asking to insert it). Identified empirically from a
    // dictation log where "no em dash and uh the little drop" got
    // mangled to "no em  — and the little drop".
    result = re
        .dash
        .replace_all(&result, |caps: &regex::Captures| {
            if caps.get(1).is_some() {
                // "em dash" / "en dash" — return the original match unchanged
                caps.get(0).unwrap().as_str().to_string()
            } else {
                " —".to_string()
            }
        })
        .to_string();

    // Clean up spaces before punctuation
    result = re.space_before_punct.replace_all(&result, "$1").to_string();

    // Collapse adjacent punctuation. Dictating two punctuation commands in a
    // row ("hey comma period") or segment stitching can leave artifacts like
    // ",." or ". ,". Rule: terminal (.!?) beats clause (,;:); within the same
    // class, keep the first. Loop until stable so cascades like ",.," fully
    // collapse.
    loop {
        let next = re
            .adjacent_punct
            .replace_all(&result, |caps: &regex::Captures| {
                let a = &caps[1];
                let b = &caps[2];
                let is_terminal = |s: &str| matches!(s, "." | "?" | "!");
                match (is_terminal(a), is_terminal(b)) {
                    (true, false) => a.to_string(),
                    (false, true) => b.to_string(),
                    _ => a.to_string(),
                }
            })
            .to_string();
        if next == result {
            break;
        }
        result = next;
    }

    // Ensure space after punctuation
    result = re.no_space_after.replace_all(&result, "$1 $2").to_string();

    // Email pattern
    result = re.email.replace_all(&result, "$1@$2.$3").to_string();

    // ". And" → ", and" — Parakeet often terminates a sentence before a
    // conjunction that was actually a mid-sentence continuation.
    result = re.period_and.replace_all(&result, ", and").to_string();

    result
}

/// Smart-join per-segment LLM cleanup outputs into one coherent string.
///
/// Per-segment streaming cleanup gives the model perfect blast-radius isolation
/// (it can't paraphrase across segments because it never sees more than one),
/// but it has three failure modes at join time:
///
///   1. **Mid-sentence VAD breaks.** VAD splits at silence, but a speaker
///      may pause inside a sentence ("...just <pause> see if it's viable").
///      The model sees segment N as a complete utterance and adds a terminal
///      period. Naive `join(" ")` produces "...Just. See if..." with an
///      orphan period in the middle of what was one sentence.
///
///   2. **Internal paragraph breaks.** The fine-tuned model occasionally
///      injects `\n\n` inside a single segment's output (a leftover habit
///      from training data that contained restructuring examples).
///
///   3. **Cross-boundary fillers.** A filler like "um" at the start of a
///      segment looks to the model like a discourse marker and gets
///      preserved. The regex pre-pass already runs per-segment but can't
///      see what's on the other side of the boundary.
///
/// Strategy:
///   - Strip `\n\n` and `\n` inside each segment (Rule 1)
///   - Smart-join with two heuristics (Rules 2 & 3):
///       Rule 2: if segment N's last word is a "stub word" (article,
///         preposition, conjunction, modal, hesitation-end like "just"),
///         it was mid-sentence — strip the period and merge.
///       Rule 3: if segment N+1 starts with a "continuation word" (and,
///         but, so, because, however, ...), strip segment N's period and
///         lowercase the conjunction.
///   - Re-run `cleanup_text` on the joined output to catch cross-boundary
///     fillers and normalize whitespace.
///
/// All deterministic, no LLM calls. Idempotent.
#[cfg(test)]
pub fn join_cleaned_segments(segments: &[String]) -> String {
    join_cleaned_segments_with_formatting(segments, true)
}

pub fn join_cleaned_segments_with_formatting(
    segments: &[String],
    smart_formatting: bool,
) -> String {
    // Step 1: strip internal paragraph breaks the model may have inserted
    // within a single segment, and trim whitespace.
    let cleaned: Vec<String> = segments
        .iter()
        .map(|s| s.replace("\n\n", " ").replace('\n', " ").trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if cleaned.is_empty() {
        return String::new();
    }

    // Step 2: naive join with single space. This gives us one big string
    // that contains both inter-segment and intra-segment sentence boundaries.
    let raw_joined = cleaned.join(" ");

    // Step 3: split into sentences and apply smart merge between adjacent
    // sentences. This handles both inter-segment and intra-segment cases
    // uniformly — the model can produce ". And" inside a single segment too.
    let sentences = split_into_sentences(&raw_joined);
    let merged = repair_vad_boundary_artifacts(&smart_merge_sentences(&sentences));

    // Step 4: re-run regex pre-pass on the merged output. cleanup_text is
    // idempotent for already-cleaned text but catches cross-boundary
    // patterns (a filler "um" that survived because it was at a segment
    // boundary) and normalizes whitespace.
    cleanup_text(&merged, smart_formatting)
}

fn lowercase_first_ascii(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => {
            let mut out = String::with_capacity(word.len());
            out.push(first.to_ascii_lowercase());
            out.push_str(chars.as_str());
            out
        }
        None => String::new(),
    }
}

/// Repair artifacts created when VAD splits mid-phrase and the next segment
/// gets sentence-style capitalization or a duplicated boundary word.
pub fn repair_vad_boundary_artifacts(text: &str) -> String {
    let re = regexes();

    let mut result = re
        .boundary_then_than
        .replace_all(text, |caps: &regex::Captures| {
            format!("{} {}", &caps["then"], &caps["gerund"].to_lowercase())
        })
        .to_string();

    result = re
        .boundary_safe_cap
        .replace_all(&result, |caps: &regex::Captures| {
            let prefix = &caps["prefix"];
            let word = &caps["word"];
            if should_lowercase_boundary_cap(prefix, word) {
                format!("{}{}{}", prefix, &caps["gap"], lowercase_first_ascii(word))
            } else {
                caps[0].to_string()
            }
        })
        .to_string();

    result
}

/// Decide whether a Title Case word is probably capitalization introduced by
/// a VAD/ASR boundary rather than a real proper noun. Parakeet already
/// capitalizes proper nouns, so this stays deliberately conservative:
/// only closed-class/common lowercase words are changed, and only when the
/// previous word makes the phrase grammatically unfinished.
fn should_lowercase_boundary_cap(prefix: &str, word: &str) -> bool {
    if word == "I" || (word.chars().any(|c| c.is_uppercase()) && !is_title_case_word(word)) {
        return false;
    }

    let prefix_lc = prefix.to_lowercase();
    let word_lc = word.to_lowercase();

    is_safe_to_lowercase(&word_lc)
        && (is_boundary_lowercase_prefix(&prefix_lc) || is_continuation_start_word(&word_lc))
}

fn is_title_case_word(word: &str) -> bool {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    first.is_uppercase() && chars.all(|c| !c.is_uppercase())
}

fn is_boundary_lowercase_prefix(word: &str) -> bool {
    is_stub_end_word(word)
        || matches!(
            word,
            // Complementizers / subordinators that commonly introduce an
            // unfinished clause in dictation.
            "that" | "because" | "since" | "while" | "although" | "though" | "when" |
            // Discourse connectors that often survive VAD joins without a
            // real sentence boundary.
            "like" | "also" | "plus" | "different"
        )
}

/// Split text into sentences. A sentence ends at `.`, `!`, or `?` followed
/// by whitespace OR end of string. We split AGGRESSIVELY (don't require the
/// next char to be capital) and let `smart_merge_sentences` decide whether
/// adjacent sentences should be merged back together. This way we catch
/// both "Foo. Bar." (real boundary, kept) and "I went to. the store."
/// (lowercase next, will be merged back).
fn split_into_sentences(text: &str) -> Vec<String> {
    let mut sentences: Vec<String> = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        current.push(chars[i]);
        if matches!(chars[i], '.' | '!' | '?') {
            // Look ahead for whitespace (or end of input).
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            let at_boundary = j >= chars.len() || j > i + 1;
            // Don't split on decimals like "3.14" — period must be followed
            // by whitespace, not another digit.
            let next_is_digit = j < chars.len() && chars[j].is_ascii_digit() && j == i + 1;
            if at_boundary && !next_is_digit {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    sentences.push(trimmed.to_string());
                }
                current.clear();
                i = j;
                continue;
            }
        }
        i += 1;
    }
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        sentences.push(trimmed.to_string());
    }
    sentences
}

/// Apply smart merge between adjacent sentences. If sentence N ends with a
/// stub word OR sentence N+1 starts with a continuation word, merge them
/// (strip terminal punctuation, lowercase the next word if safe).
fn smart_merge_sentences(sentences: &[String]) -> String {
    if sentences.is_empty() {
        return String::new();
    }

    let mut out = sentences[0].clone();

    for sent in &sentences[1..] {
        // Look at the running output's tail.
        let prev_ends_with_terminal = out.trim_end().ends_with(['.', '!', '?']);

        // Previous sentence's last alphanumeric token (lowercased).
        let prev_last_word_lc: String = out
            .trim_end_matches(|c: char| !c.is_alphanumeric() && c != '\'')
            .split_whitespace()
            .last()
            .unwrap_or("")
            .to_lowercase();
        let prev_ends_in_stub = is_stub_end_word(&prev_last_word_lc);

        // New sentence's head word.
        let head_word_raw = sent.split_whitespace().next().unwrap_or("");
        let head_starts_lower = head_word_raw
            .chars()
            .next()
            .map_or(false, |c| c.is_lowercase());
        let head_word_lc: String = head_word_raw
            .trim_end_matches(|c: char| !c.is_alphanumeric() && c != '\'')
            .to_lowercase();
        let head_is_continuation = is_continuation_start_word(&head_word_lc);
        let head_is_safe_to_lowercase = is_safe_to_lowercase(&head_word_lc);

        let should_merge = head_starts_lower
            || (prev_ends_in_stub && prev_ends_with_terminal)
            || (head_is_continuation && prev_ends_with_terminal);

        if should_merge {
            if prev_ends_with_terminal {
                while out.ends_with(|c: char| matches!(c, '.' | '!' | '?' | ' ')) {
                    out.pop();
                }
            }
            out.push(' ');
            if !head_starts_lower && head_is_safe_to_lowercase {
                let mut chars = sent.chars();
                if let Some(c) = chars.next() {
                    out.push(c.to_ascii_lowercase());
                }
                out.push_str(chars.as_str());
            } else {
                out.push_str(sent);
            }
        } else {
            out.push(' ');
            out.push_str(sent);
        }
    }

    out
}

/// Words that, when they appear at the END of a segment, strongly suggest
/// the segment is mid-sentence (VAD broke at a hesitation pause). Used by
/// `join_cleaned_segments` to decide whether to strip a terminal period.
fn is_stub_end_word(word: &str) -> bool {
    // CONSERVATIVE list: only words that essentially never end a real
    // English sentence. Common sentence-final adverbs (now, then, well,
    // really, very, still, even, kind, sort, like) are EXCLUDED because
    // they cause false-merges of legitimate sentence breaks like
    // "Don't do this right now. Start tomorrow." → would lose the period.
    //
    // "just" is the one borderline keeper: it can technically end a
    // sentence ("That's just, you know") but in dictation it's
    // overwhelmingly a hesitation marker mid-sentence.
    matches!(
        word,
        // articles
        "a" | "an" | "the" |
        // conjunctions
        "and" | "or" | "but" | "so" | "if" | "as" | "than" | "nor" |
        // prepositions
        "of" | "to" | "in" | "on" | "at" | "by" | "with" | "for" | "from" |
        "into" | "onto" | "upon" | "about" | "under" | "over" | "between" |
        "through" | "across" |
        // auxiliaries / modals
        "is" | "am" | "are" | "was" | "were" | "be" | "been" | "being" |
        "have" | "has" | "had" |
        "do" | "does" | "did" |
        "will" | "would" | "can" | "could" | "should" | "may" | "might" | "must" |
        // possessive pronouns (need a noun to follow)
        "my" | "our" | "your" | "his" | "its" | "their" |
        // single-keeper hesitation marker
        "just" |
        // fillers — if these are the LAST WORD of a segment, the segment
        // is mid-sentence by definition (no real sentence ends with "um").
        // The regex pre-pass strips them, but smart-merge runs FIRST so
        // including them here lets us strip the orphan period the model
        // added BEFORE the filler is removed (otherwise we leave " .")
        "um" | "umm" | "uh" | "uhh" | "hmm" | "mhm" | "mmhmm"
    )
}

/// Words that, when they appear at the START of a segment, strongly suggest
/// the segment is a CONTINUATION of the previous sentence rather than a new
/// one. Used by `join_cleaned_segments` to decide whether to strip the
/// previous segment's terminal period.
fn is_continuation_start_word(word: &str) -> bool {
    // Conservative list: words that are USUALLY mid-sentence continuations.
    // Excludes "just" (often a sentence-initial imperative: "Just see..."),
    // "so" (often discourse opener: "So what do you think?"), "also"/"plus"
    // (often paragraph starters), "yet"/"nor" (rare and often standalone).
    matches!(
        word,
        "and"
            | "but"
            | "or"
            | "because"
            | "since"
            | "while"
            | "though"
            | "although"
            | "however"
            | "therefore"
            | "thus"
            | "hence"
            | "moreover"
            | "furthermore"
    )
}

/// Words it is ALWAYS safe to lowercase mid-sentence — they're never proper
/// nouns. Used by `join_cleaned_segments` when merging at a segment boundary.
fn is_safe_to_lowercase(word: &str) -> bool {
    // Anything ≤ 3 chars that's clearly a stop word, plus a hand-picked list
    // of common English short verbs/auxiliaries that frequently start
    // mid-sentence clauses.
    matches!(
        word,
        // articles + 2-letter prepositions/conjunctions
        "a" | "an" | "the" | "of" | "to" | "in" | "on" | "at" | "by" |
        "as" | "if" | "or" | "so" | "is" | "am" | "be" | "do" | "we" |
        "us" | "it" | "he" |
        // 3-letter common words
        "and" | "but" | "for" | "nor" | "yet" | "are" | "was" | "had" |
        "has" | "did" | "let" | "all" | "any" | "may" | "can" | "see" |
        "use" | "get" | "got" | "her" | "his" | "you" | "our" | "way" |
        "out" | "off" | "now" | "two" | "one" | "few" | "say" | "set" |
        "run" | "try" | "put" |
        // 4-letter common words
        "this" | "that" | "with" | "from" | "have" | "will" | "make" |
        "just" | "like" | "also" | "then" | "than" | "when" | "they" |
        "them" | "your" | "what" | "some" | "want" | "need" | "been" |
        "were" | "more" | "into" | "give" | "take" | "feel" | "look" |
        "tell" | "show" | "find" | "kind" | "sort" | "well" | "much" |
        "we'll" | "we're" | "we've" | "i'll" | "i'm" | "it's" | "that's" |
        // 5-letter common words
        "would" | "could" | "might" | "since" | "while" | "after" |
        "above" | "below" | "where" | "which" | "their" | "those" |
        "these" | "going" | "doing" | "being" |
        // continuation/discourse markers
        "really" | "though" | "because" | "however" | "before" | "during" |
        "between" | "through" | "across" | "behind" | "should" |
        "although" | "therefore"
    )
}

/// Apply user-configured find/replace from the vocabulary's `replaces` lists.
///
/// For each VocabEntry, every string in `replaces` is matched
/// case-insensitively at word boundaries and substituted with the canonical
/// `term`. This is the deterministic post-ASR fixup layer for things sherpa
/// hotword biasing can't fix — homophones (Pieter/Peter), brand spellings,
/// stable mishearings.
///
/// Runs BEFORE the LLM cleanup pass so the LLM sees the corrected names in
/// context. chirp-cleanup-v2 at temp 0.0 won't reintroduce alternative
/// spellings the input doesn't contain.
///
/// Edge cases:
///   - Self-replacements (case-insensitive `from == to`) are skipped
///   - Empty / whitespace-only `from` strings are skipped
///   - `from` strings are `regex::escape`'d, so literal special chars are safe
///   - Match is case-insensitive; replacement always uses the canonical case
pub fn apply_replacements(text: &str, vocabulary: &[VocabEntry]) -> String {
    let mut result = text.to_string();
    for entry in vocabulary {
        let canonical = entry.term.trim();
        if canonical.is_empty() {
            continue;
        }
        for from in &entry.replaces {
            let from_trim = from.trim();
            if from_trim.is_empty() {
                continue;
            }
            if from_trim.eq_ignore_ascii_case(canonical) {
                continue;
            }
            // Build a case-insensitive regex for the literal `from`. `\b` only
            // matches at the transition between a word and non-word char, so
            // we only anchor with `\b` on sides where the pattern's edge IS a
            // word character. Otherwise the boundary assertion would fail
            // (e.g. `\bco (corp)\b` — the trailing `)` is non-word, the
            // following space is also non-word, so no boundary exists there).
            let starts_with_word = from_trim
                .chars()
                .next()
                .map(|c| c.is_alphanumeric() || c == '_')
                .unwrap_or(false);
            let ends_with_word = from_trim
                .chars()
                .last()
                .map(|c| c.is_alphanumeric() || c == '_')
                .unwrap_or(false);
            let pattern = format!(
                "(?i){}{}{}",
                if starts_with_word { r"\b" } else { "" },
                regex::escape(from_trim),
                if ends_with_word { r"\b" } else { "" },
            );
            match Regex::new(&pattern) {
                Ok(re) => {
                    result = re.replace_all(&result, canonical).to_string();
                }
                Err(e) => {
                    log::warn!("Failed to compile replacement pattern '{from_trim}': {e}");
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spoken_punctuation() {
        let result = smart_format("hello comma how are you question mark");
        assert!(result.contains("hello, how are you?"));
    }

    #[test]
    fn test_percentage() {
        let result = smart_format("it was about fifty percent done");
        assert!(result.contains("50%"));
    }

    #[test]
    fn test_email() {
        let result = smart_format("send it to john at example dot com");
        assert!(result.contains("john@example.com"));
    }

    #[test]
    fn test_new_paragraph_passthrough() {
        // "new paragraph" should pass through to LLM as words, not be converted by regex
        let result = smart_format("hello new paragraph world");
        assert!(result.contains("new paragraph"));
    }

    #[test]
    fn test_period_and_to_comma_and() {
        let result = smart_format("I went to the store. And then I came home.");
        assert_eq!(result, "I went to the store, and then I came home.");
    }

    #[test]
    fn test_full_cleanup() {
        let result = cleanup_text("send an email to bob at test dot com", true);
        assert!(result.contains("bob@test.com"));
    }

    // ── standalone "i" capitalization ──────────────────────────────────

    #[test]
    fn test_standalone_i_capitalized() {
        // A stray lowercase "i" gets uppercased to the pronoun "I".
        let result = cleanup_text("i think i should ask", true);
        assert_eq!(result, "I think I should ask");
    }

    #[test]
    fn test_standalone_i_contraction_capitalized() {
        // `\bi\b` matches the `i` before the apostrophe (apostrophe is a
        // non-word char), so "i'm" becomes "I'm".
        let result = cleanup_text("i'm going", true);
        assert_eq!(result, "I'm going");
    }

    #[test]
    fn test_standalone_i_common_contractions_capitalized() {
        let result = cleanup_text("i'll go because i've got it and i'd like to", true);
        assert_eq!(result, "I'll go because I've got it and I'd like to");
    }

    #[test]
    fn test_capitalizes_after_quotes_and_parens() {
        let result = cleanup_text("he said. \"this works.\" (this also works).", true);
        assert_eq!(result, "He said. \"This works.\" (This also works).");
    }

    #[test]
    fn test_capitalizes_after_colon_and_newline() {
        let result = cleanup_text("note: this should start capitalized.\nnext line too.", true);
        assert_eq!(
            result,
            "Note: This should start capitalized.\nNext line too."
        );
    }

    #[test]
    fn test_capitalizes_initial_quoted_text() {
        let result = cleanup_text("\"hello there,\" he said.", true);
        assert_eq!(result, "\"Hello there,\" he said.");
    }

    #[test]
    fn test_standalone_i_preserves_embedded_i() {
        // Words containing "i" (not standalone) must NOT be affected.
        let result = cleanup_text("the iphone is in my bag", true);
        // "i" inside "iphone"/"is"/"in"/"bag"/"my" stays. Only word-standalone
        // "i" would be uppercased; there are none in this input.
        assert!(result.contains("iphone"));
        assert!(result.contains("is"));
        assert!(result.contains("in"));
    }

    // ── adjacent punctuation collapse ──────────────────────────────────

    #[test]
    fn test_spoken_period_then_comma_collapses() {
        // Dictating "period comma" back-to-back used to produce "Hey,."; now
        // the terminal period wins and the following word is capitalized.
        let result = cleanup_text("hey period comma world", true);
        assert_eq!(result, "Hey. World");
    }

    #[test]
    fn test_spoken_comma_then_period_collapses() {
        // Reverse order — the terminal period still wins.
        let result = cleanup_text("hey comma period world", true);
        assert_eq!(result, "Hey. World");
    }

    #[test]
    fn test_adjacent_punct_period_comma_literal() {
        // Direct ".," in the text (e.g. from upstream) collapses to "." and
        // the next word is re-capitalized.
        let result = cleanup_text("hello.,world", true);
        assert_eq!(result, "Hello. World");
    }

    #[test]
    fn test_adjacent_punct_preserves_decimal() {
        // "3.14" is a digit-period-digit sequence. The adjacent-punct regex
        // must not touch it because the neighbors aren't punctuation.
        let result = cleanup_text("it costs 3.14 dollars", true);
        assert_eq!(result, "It costs 3.14 dollars");
    }

    #[test]
    fn test_adjacent_punct_with_space_between() {
        // A period followed by whitespace then a comma still collapses.
        let result = smart_format("hello . , world");
        // `space_before_punct` collapses leading whitespace, then
        // `adjacent_punct` keeps the terminal period.
        assert_eq!(result, "hello. world");
    }

    #[test]
    fn test_adjacent_punct_double_comma() {
        // Two commas in a row collapse to one (first wins).
        let result = smart_format("foo ,, bar");
        assert_eq!(result, "foo, bar");
    }

    // ── merge must not lowercase "I" ───────────────────────────────────

    #[test]
    fn test_merge_preserves_capital_i() {
        // Previously, `is_safe_to_lowercase` included "i", so merging a
        // stub-end prev segment with a next segment starting with "I" would
        // produce "...to i think...". Must stay capitalized now.
        let s = segs(&["I went to.", "I think we should."]);
        let result = join_cleaned_segments(&s);
        assert!(
            !result.contains(" i think"),
            "expected capital I after merge, got: {result}"
        );
        assert!(
            result.contains("I think"),
            "expected 'I think' preserved, got: {result}"
        );
    }

    fn vocab(entries: &[(&str, &[&str])]) -> Vec<VocabEntry> {
        entries
            .iter()
            .map(|(term, replaces)| VocabEntry {
                term: (*term).to_string(),
                replaces: replaces.iter().map(|s| (*s).to_string()).collect(),
            })
            .collect()
    }

    #[test]
    fn test_apply_replacements_basic() {
        let v = vocab(&[("Pieter", &["Peter"])]);
        assert_eq!(
            apply_replacements("My name is Peter.", &v),
            "My name is Pieter."
        );
    }

    #[test]
    fn test_apply_replacements_case_insensitive() {
        let v = vocab(&[("Pieter", &["Peter"])]);
        assert_eq!(
            apply_replacements("PETER and peter", &v),
            "Pieter and Pieter"
        );
    }

    #[test]
    fn test_apply_replacements_word_boundary() {
        // "petersburg" should NOT become "pietersburg"
        let v = vocab(&[("Pieter", &["Peter"])]);
        assert_eq!(
            apply_replacements("I went to Petersburg.", &v),
            "I went to Petersburg."
        );
    }

    #[test]
    fn test_apply_replacements_apostrophe() {
        // "Peter's" should become "Pieter's" — \b matches between r and '
        let v = vocab(&[("Pieter", &["Peter"])]);
        assert_eq!(apply_replacements("Peter's car.", &v), "Pieter's car.");
    }

    #[test]
    fn test_apply_replacements_multiple_froms() {
        let v = vocab(&[("Lakshamanan", &["Lakshmanan", "lock shamanan"])]);
        assert_eq!(
            apply_replacements("Hi Lakshmanan and lock shamanan", &v),
            "Hi Lakshamanan and Lakshamanan"
        );
    }

    #[test]
    fn test_apply_replacements_self_replacement_skipped() {
        // Replacing "Peter" with "Peter" should be a no-op (no infinite loop)
        let v = vocab(&[("Peter", &["peter", "Peter", "PETER"])]);
        // All three case-insensitively equal to canonical → all skipped
        assert_eq!(apply_replacements("hello peter", &v), "hello peter");
    }

    #[test]
    fn test_apply_replacements_empty_replaces() {
        // Term with no replaces does nothing — purely for ASR biasing
        let v = vocab(&[("Akilan", &[])]);
        assert_eq!(apply_replacements("hello world", &v), "hello world");
    }

    #[test]
    fn test_apply_replacements_special_chars_in_from() {
        // regex::escape handles parens, dots, etc. in the `from` string
        let v = vocab(&[("Co.", &["co (corp)"])]);
        assert_eq!(
            apply_replacements("the co (corp) released a product", &v),
            "the Co. released a product"
        );
    }

    // ── join_cleaned_segments tests ─────────────────────────────────────

    fn segs(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_join_empty() {
        assert_eq!(join_cleaned_segments(&[]), "");
        assert_eq!(join_cleaned_segments(&segs(&["", "  ", "\n"])), "");
    }

    #[test]
    fn test_join_single_segment_passthrough() {
        let s = segs(&["Hello world."]);
        assert_eq!(join_cleaned_segments(&s), "Hello world.");
    }

    #[test]
    fn test_join_respects_smart_formatting_disabled() {
        let s = segs(&["hello period."]);
        assert_eq!(
            join_cleaned_segments_with_formatting(&s, false),
            "hello period."
        );
    }

    #[test]
    fn test_join_strips_internal_paragraph_breaks() {
        // Model sometimes injects \n\n inside a single segment's output.
        let s = segs(&["First line.\n\nSecond line."]);
        assert_eq!(join_cleaned_segments(&s), "First line. Second line.");
    }

    #[test]
    fn test_join_real_sentence_boundary_kept() {
        // Two complete sentences across segments — punctuation must be kept.
        let s = segs(&["I went to the store.", "I bought milk."]);
        assert_eq!(
            join_cleaned_segments(&s),
            "I went to the store. I bought milk."
        );
    }

    #[test]
    fn test_join_stub_end_just_merges() {
        // The session-3 case: segment ends in "Just." → mid-sentence break.
        let s = segs(&[
            "Don't do any coding right now. Just.",
            "See if it's viable.",
        ]);
        assert_eq!(
            join_cleaned_segments(&s),
            "Don't do any coding right now. Just see if it's viable."
        );
    }

    #[test]
    fn test_join_stub_end_preposition_merges() {
        // "I went to." should merge with whatever comes next.
        let s = segs(&["I went to.", "the store."]);
        assert_eq!(join_cleaned_segments(&s), "I went to the store.");
    }

    #[test]
    fn test_join_stub_end_modal_merges() {
        let s = segs(&["I will.", "make it work."]);
        assert_eq!(join_cleaned_segments(&s), "I will make it work.");
    }

    #[test]
    fn test_join_continuation_word_merges() {
        // Segment 2 starts with "And" → continuation, strip & lowercase.
        let s = segs(&["I went to the store.", "And bought milk."]);
        assert_eq!(
            join_cleaned_segments(&s),
            "I went to the store and bought milk."
        );
    }

    #[test]
    fn test_join_continuation_but_merges() {
        let s = segs(&["I tried to fix it.", "But it didn't work."]);
        assert_eq!(
            join_cleaned_segments(&s),
            "I tried to fix it but it didn't work."
        );
    }

    #[test]
    fn test_join_lowercase_starts_merge() {
        // If segment 2 already starts lowercase, treat as continuation.
        let s = segs(&["I went to the store.", "and bought milk."]);
        assert_eq!(
            join_cleaned_segments(&s),
            "I went to the store and bought milk."
        );
    }

    #[test]
    fn test_join_preserves_proper_noun_capitalization() {
        // "I went to." → stub. "Paris" should NOT be lowercased.
        // (Paris is 5 chars and not in the safe-to-lowercase list.)
        let s = segs(&["I went to.", "Paris last summer."]);
        assert_eq!(join_cleaned_segments(&s), "I went to Paris last summer.");
    }

    #[test]
    fn test_join_three_segments_with_two_merges() {
        // Session 3 in full: stub-end "Just." + intra-segment "viable. And"
        // continuation + stub-end "can." + final "Make it work."
        // The sentence-level merge handles ALL of these uniformly.
        let s = segs(&[
            "Don't do any coding at this point in time. Just.",
            "See if it's viable. And if we can.",
            "Make it work.",
        ]);
        let result = join_cleaned_segments(&s);
        // All three should be merged: "Just see", "viable and", "can make"
        assert!(
            result.contains("Just see if it's viable"),
            "expected 'Just see if it's viable' merge, got: {result}"
        );
        assert!(
            result.contains("viable and if we can"),
            "expected 'viable and if we can' merge, got: {result}"
        );
        assert!(
            result.contains("can make it work"),
            "expected 'can make it work' merge, got: {result}"
        );
        // No orphan periods
        assert!(
            !result.contains(". And"),
            "expected no '. And' artifact, got: {result}"
        );
    }

    #[test]
    fn test_join_filler_as_stub_end_merges_then_strips() {
        // The session-3 case: a segment ends with "Just um." (the model
        // added a period after the filler). The smart-merge should
        // recognize "um" as a stub-end word, strip the period, merge with
        // the next segment, and the post-merge regex pre-pass should then
        // strip "um" entirely. No orphan period, no leftover filler.
        let s = segs(&[
            "Don't do any coding right now. Just um.",
            "See if it's viable.",
        ]);
        let result = join_cleaned_segments(&s);
        // "um" should be gone
        assert!(
            !result.to_lowercase().contains(" um "),
            "expected 'um' stripped, got: {result}"
        );
        assert!(
            !result.to_lowercase().contains(" um."),
            "expected 'um.' stripped, got: {result}"
        );
        // No orphan period after "Just"
        assert!(
            !result.contains("Just."),
            "expected no 'Just.' orphan, got: {result}"
        );
        assert!(
            !result.contains("Just ."),
            "expected no 'Just .' with space, got: {result}"
        );
        // The merge should have happened: "Just" → "see" continuation
        assert!(
            result.contains("Just see if it's viable")
                || result.contains("just see if it's viable"),
            "expected 'Just see if it's viable' merge, got: {result}"
        );
    }

    #[test]
    fn test_join_filler_at_segment_boundary_removed() {
        // The session-2 case: a filler "Um" survived the per-segment LLM
        // because it looked like a discourse marker. The post-join regex
        // pre-pass should strip it.
        let s = segs(&[
            "Please analyse if this plan is feasible.",
            "Um I know we'll have to make it.",
        ]);
        let result = join_cleaned_segments(&s);
        assert!(
            !result.to_lowercase().contains(" um "),
            "expected 'um' to be stripped, got: {result}"
        );
        assert!(result.contains("I know we'll have to make it"));
    }

    #[test]
    fn test_join_drops_empty_segments() {
        let s = segs(&["First.", "", "  ", "Second."]);
        assert_eq!(join_cleaned_segments(&s), "First. Second.");
    }

    #[test]
    fn test_repair_boundary_safe_capitalization() {
        let result = repair_vad_boundary_artifacts(
            "The fact that it's that Much like work is not good, but like Have it derived.",
        );
        assert_eq!(
            result,
            "The fact that it's that much like work is not good, but like have it derived."
        );
    }

    #[test]
    fn test_repair_boundary_uses_unfinished_grammar_pattern() {
        let result = repair_vad_boundary_artifacts(
            "This prompt is Also dictated with Chirp. Voice rules should have a It should be local.",
        );
        assert_eq!(
            result,
            "This prompt is also dictated with Chirp. Voice rules should have a it should be local."
        );
    }

    #[test]
    fn test_repair_boundary_then_than_gerund() {
        let result = repair_vad_boundary_artifacts(
            "We should communicate that clearly in wireframing first, and then Than implementing a brand kit.",
        );
        assert_eq!(
            result,
            "We should communicate that clearly in wireframing first, and then implementing a brand kit."
        );
    }

    #[test]
    fn test_repair_boundary_preserves_proper_nouns() {
        let result =
            repair_vad_boundary_artifacts("We can send this to SiteLift and Google tomorrow.");
        assert_eq!(result, "We can send this to SiteLift and Google tomorrow.");
    }

    #[test]
    fn test_repair_boundary_preserves_title_case_names() {
        let result = repair_vad_boundary_artifacts("I read The Verge today.");
        assert_eq!(result, "I read The Verge today.");
    }

    #[test]
    fn test_repair_boundary_preserves_parakeet_proper_nouns() {
        let result = repair_vad_boundary_artifacts(
            "I talked to Claude about SiteLift before sending it to Google.",
        );
        assert_eq!(
            result,
            "I talked to Claude about SiteLift before sending it to Google."
        );
    }

    #[test]
    fn test_join_session_5_blast_radius() {
        // The "Chirp app" test session — even if per-segment cleanup
        // paraphrases within a segment, it should NOT add cross-segment
        // "Chirp app:" prefixes.
        let s = segs(&[
            "This is a test of the Chirp app after we just did a huge pipeline overhaul for the.",
            "Text injection and other pipelines. Just working on making sure we're not duplicating any text.",
        ]);
        let result = join_cleaned_segments(&s);
        // "for the." → stub end, should merge
        assert!(
            result.contains("overhaul for the text injection")
                || result.contains("overhaul for the Text injection"),
            "expected stub-end merge of 'for the.' + 'Text injection', got: {result}"
        );
        // Should not have invented "Chirp app:" prefix because the input
        // doesn't have one and per-segment can't introduce it cross-boundary.
        assert!(
            !result.starts_with("Chirp app:"),
            "expected no invented 'Chirp app:' prefix, got: {result}"
        );
    }
}
