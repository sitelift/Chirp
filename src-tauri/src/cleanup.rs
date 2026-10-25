//! The deterministic text pass: what Chirp does to the recognizer's output
//! without a model.
//!
//! Parakeet already punctuates and capitalizes, so this is deliberately small:
//!
//! 1. [`join_segments`]: join the VAD segments' transcripts, repairing the
//!    artifacts a mid-sentence VAD split leaves (an orphan period, a
//!    capitalized continuation, a duplicated boundary word).
//! 2. [`convert_spoken_commands`]: turn spoken punctuation and layout
//!    commands ("comma", "new line", ...) into the characters they ask for.
//!
//! Everything else (fillers, numbers, self-corrections, register) is left to
//! the cleanup model; with the provider Off the recognizer's words are kept.
use regex::Regex;
use std::sync::OnceLock;

struct BoundaryRegexes {
    boundary_then_than: Regex,
    boundary_safe_cap: Regex,
}

fn regexes() -> &'static BoundaryRegexes {
    static REGEXES: OnceLock<BoundaryRegexes> = OnceLock::new();
    REGEXES.get_or_init(|| BoundaryRegexes {
        boundary_then_than: Regex::new(r"(?i)\b(?P<then>and\s+then)\s+than\s+(?P<gerund>[a-z]+ing)\b").unwrap(),
        boundary_safe_cap: Regex::new(r"\b(?P<prefix>[A-Za-z']+)(?P<gap>\s+)(?P<word>[A-Z][a-z]+(?:'[a-z]+)?)\b").unwrap(),
    })
}

/// Compile the boundary regexes ahead of the first dictation.
pub fn warm_up() {
    let _ = spoken_text(&["warm up comma the deterministic pass".to_string()]);
}

/// The deterministic text for a dictation: segments joined, spoken commands
/// converted. This is what the router judges, what the cleanup model gets,
/// the fallback on any model error, and the result when the provider is Off.
pub fn spoken_text(segments: &[String]) -> String {
    convert_spoken_commands(&join_segments(segments))
}

// ── Spoken commands ─────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum Command {
    /// Punctuation that attaches to the previous word and replaces whatever
    /// punctuation the recognizer guessed there.
    Mark(&'static str),
    /// A line or paragraph break; keeps the recognizer's punctuation before it.
    Break(&'static str),
    /// "(": attaches to the next word.
    Open,
    /// ")": attaches to the previous word, keeps punctuation on both sides.
    Close,
    /// "-": joins the previous and next word.
    Hyphen,
}

/// Spoken commands, longest first. "dash" is deliberately absent: it is a
/// hyphen in domains and flags ("getting dash started", "dash dash release")
/// and an em dash in prose, and the cleanup prompt relies on seeing the word.
const COMMANDS: &[(&[&str], Command)] = &[
    (&["new", "paragraph"], Command::Break("\n\n")),
    (&["new", "line"], Command::Break("\n")),
    (&["newline"], Command::Break("\n")),
    (&["question", "mark"], Command::Mark("?")),
    (&["exclamation", "mark"], Command::Mark("!")),
    (&["exclamation", "point"], Command::Mark("!")),
    (&["full", "stop"], Command::Mark(".")),
    (&["open", "parenthesis"], Command::Open),
    (&["open", "paren"], Command::Open),
    (&["close", "parenthesis"], Command::Close),
    (&["close", "paren"], Command::Close),
    (&["semi", "colon"], Command::Mark(";")),
    (&["semicolon"], Command::Mark(";")),
    (&["comma"], Command::Mark(",")),
    (&["period"], Command::Mark(".")),
    (&["colon"], Command::Mark(":")),
    (&["hyphen"], Command::Hyphen),
];

/// A previous word that makes the command a noun being talked about ("a new
/// line of products", "the Oxford comma", "cues like new paragraph").
fn blocks_command(prev: &str) -> bool {
    matches!(
        prev,
        "a" | "an" | "the" | "this" | "that" | "these" | "those" | "each" | "every" | "any"
            | "some" | "no" | "my" | "your" | "our" | "their" | "his" | "her" | "its"
            | "which" | "what" | "whose" | "another" | "one" | "of" | "about" | "like"
            | "word" | "called" | "named" | "say" | "says" | "said" | "saying" | "oxford"
            | "serial"
    )
}

/// A next word that makes the command a noun ("period of time",
/// "comma separated").
fn blocks_command_after(next: &str) -> bool {
    matches!(next, "of" | "separated" | "delimited")
}

/// "Period" is also an ordinary noun; these words in front of it mean it is
/// one ("trial period", "a two week period").
fn period_is_noun(prev: &str) -> bool {
    matches!(
        prev,
        "trial" | "grace" | "waiting" | "notice" | "probation" | "probationary" | "billing"
            | "reporting" | "transition" | "transitional" | "holding" | "warranty" | "cooling"
            | "rest" | "free" | "time" | "test" | "testing" | "review" | "vesting" | "payment"
            | "lock" | "lockup" | "evaluation" | "study" | "summer" | "winter" | "spring"
            | "autumn" | "holiday" | "christmas" | "busy" | "quiet" | "peak" | "launch"
            | "same" | "whole" | "entire" | "short" | "long" | "initial" | "final" | "given"
            | "certain" | "specific" | "extended" | "limited" | "day" | "week" | "month"
            | "year" | "hour" | "minute" | "school" | "class" | "sample" | "sampling"
            | "retention" | "measurement" | "recovery" | "development" | "menstrual"
    )
}

/// Words it is safe to lowercase after a spoken comma, colon or semicolon
/// when the recognizer capitalized them as a sentence start.
fn lowercase_after_clause_mark(word: &str) -> bool {
    is_safe_to_lowercase(word)
        || matches!(word, "how" | "why" | "who" | "please" | "thanks" | "there" | "here" | "not")
}

/// Leading punctuation, the word, trailing punctuation.
fn split_token(token: &str) -> (&str, &str, &str) {
    let Some(start) = token.find(|c: char| c.is_alphanumeric()) else {
        return (token, "", "");
    };
    let end = token
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_alphanumeric())
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(token.len());
    (&token[..start], &token[start..end], &token[end..])
}

fn is_sentence_punct(c: char) -> bool {
    matches!(c, ',' | '.' | ';' | ':' | '!' | '?')
}

fn is_terminal(mark: &str) -> bool {
    matches!(mark, "." | "?" | "!")
}

/// Match a command starting at token `i`. Returns the command and how many
/// tokens it spans.
fn match_command(tokens: &[&str], i: usize) -> Option<(Command, usize)> {
    'commands: for (words, command) in COMMANDS {
        if i + words.len() > tokens.len() {
            continue;
        }
        for (k, word) in words.iter().enumerate() {
            let (lead, core, trail) = split_token(tokens[i + k]);
            let last = k + 1 == words.len();
            if !lead.is_empty() || (!last && !trail.is_empty()) || !core.eq_ignore_ascii_case(word) {
                continue 'commands;
            }
        }
        // Only a previous word with nothing after it modifies this one;
        // "the. Comma" is a sentence end followed by a command.
        if i > 0 {
            let (_, prev, prev_trail) = split_token(tokens[i - 1]);
            let prev = prev.to_lowercase();
            if prev_trail.is_empty()
                && (blocks_command(&prev) || (*command == Command::Mark(".") && words[0] == "period" && period_is_noun(&prev)))
            {
                return None;
            }
        }
        let (_, _, trail) = split_token(tokens[i + words.len() - 1]);
        if trail.is_empty() {
            if let Some(next) = tokens.get(i + words.len()) {
                let (lead, next, _) = split_token(next);
                if lead.is_empty() && blocks_command_after(&next.to_lowercase()) {
                    return None;
                }
            }
        }
        return Some((*command, words.len()));
    }
    None
}

/// Builds the converted text, gluing punctuation to the right words.
#[derive(Default)]
struct Writer {
    out: String,
    /// The next word follows "(", "-" or a break: no space before it.
    glue: bool,
    capitalize: bool,
    lowercase: bool,
    /// The text ends in a mark a command inserted (not the recognizer's).
    command_mark: Option<&'static str>,
}

impl Writer {
    fn word(&mut self, token: &str) {
        if !self.out.is_empty() && !self.glue {
            self.out.push(' ');
        }
        let (lead, core, trail) = split_token(token);
        self.out.push_str(lead);
        let mut chars = core.chars();
        match chars.next() {
            Some(first) if self.capitalize && first.is_lowercase() => {
                self.out.extend(first.to_uppercase());
                self.out.push_str(chars.as_str());
            }
            Some(first)
                if self.lowercase
                    && is_title_case_word(core)
                    && lowercase_after_clause_mark(&core.to_lowercase()) =>
            {
                self.out.extend(first.to_lowercase());
                self.out.push_str(chars.as_str());
            }
            _ => self.out.push_str(core),
        }
        self.out.push_str(trail);
        self.glue = false;
        self.capitalize = false;
        self.lowercase = false;
        self.command_mark = None;
    }

    /// Drop the recognizer's punctuation at the end of the text.
    fn strip_guessed_punct(&mut self) {
        while self.out.ends_with(is_sentence_punct) {
            self.out.pop();
        }
    }

    /// Text that followed the command word ("comma," or "mark.\"") minus the
    /// recognizer's own punctuation guesses.
    fn rest(&mut self, trail: &str) {
        self.out.extend(trail.chars().filter(|c| !is_sentence_punct(*c)));
    }

    fn command(&mut self, command: Command, trail: &str) {
        match command {
            Command::Mark(mark) => {
                // Two marks in a row ("comma period"): terminal wins over a
                // clause mark; otherwise the first stays.
                if let Some(prev) = self.command_mark {
                    if is_terminal(prev) || !is_terminal(mark) {
                        self.rest(trail);
                        return;
                    }
                }
                self.strip_guessed_punct();
                self.out.push_str(mark);
                self.rest(trail);
                self.glue = false;
                self.capitalize = is_terminal(mark);
                self.lowercase = !is_terminal(mark);
                self.command_mark = Some(mark);
            }
            Command::Break(brk) => {
                while self.out.ends_with(' ') {
                    self.out.pop();
                }
                self.out.push_str(brk);
                self.rest(trail);
                self.glue = true;
                self.capitalize = true;
                self.lowercase = false;
                self.command_mark = None;
            }
            Command::Open => {
                if !self.out.is_empty() && !self.glue {
                    self.out.push(' ');
                }
                self.out.push('(');
                self.glue = true;
                self.command_mark = None;
            }
            Command::Close => {
                self.out.push(')');
                self.out.push_str(trail);
                self.glue = false;
                self.command_mark = None;
            }
            Command::Hyphen => {
                self.strip_guessed_punct();
                self.out.push('-');
                self.glue = true;
                self.capitalize = false;
                self.lowercase = false;
                self.command_mark = None;
            }
        }
    }
}

/// Convert spoken punctuation and layout commands: "new line", "new
/// paragraph", "comma", "period" / "full stop", "question mark", "exclamation
/// mark" / "point", "colon", "semicolon", "hyphen", "open paren" / "close
/// paren".
///
/// Conservative by design: a command preceded by an article, possessive or
/// quoting word ("a new line", "the Oxford comma", "like new paragraph"), or
/// followed by "of" ("period of time"), is kept as words, as is "period" after
/// a word that makes it a noun ("trial period"). The punctuation the recognizer
/// guessed around a command ("Hello, comma, how") gives way to the command.
pub fn convert_spoken_commands(text: &str) -> String {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut writer = Writer::default();
    let mut i = 0;
    while i < tokens.len() {
        if let Some((command, len)) = match_command(&tokens, i) {
            let (_, _, trail) = split_token(tokens[i + len - 1]);
            writer.command(command, trail);
            i += len;
        } else {
            writer.word(tokens[i]);
            i += 1;
        }
    }
    writer.out
}

// ── Segment joining ─────────────────────────────────────────────────────

/// Join the recognizer's per-segment transcripts into one text.
///
/// VAD splits at silence, and a speaker may pause inside a sentence, so the
/// recognizer sees a fragment as a whole utterance: "...just. See if it's
/// viable." Sentences are split aggressively and merged back when the first
/// ends in a word no sentence ends with (article, preposition, modal, "just")
/// or the next starts with a continuation ("and", "but", "because", ...) or a
/// lowercase letter. Deterministic and idempotent.
pub fn join_segments(segments: &[String]) -> String {
    let parts: Vec<&str> = segments.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return String::new();
    }
    let sentences = split_into_sentences(&parts.join(" "));
    let merged = repair_vad_boundary_artifacts(&smart_merge_sentences(&sentences));
    merged.split_whitespace().collect::<Vec<_>>().join(" ")
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
/// `join_segments` to decide whether to strip a terminal period.
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
        "um" | "umm" | "uh" | "uhh" | "hmm" | "mhm" | "mmhmm"
    )
}

/// Words that, when they appear at the START of a segment, strongly suggest
/// the segment is a CONTINUATION of the previous sentence rather than a new
/// one. Used by `join_segments` to decide whether to strip the
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
/// nouns. Used by `join_segments` when merging at a segment boundary.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(text: &str) -> String {
        convert_spoken_commands(text)
    }

    // ── spoken commands ────────────────────────────────────────────────

    #[test]
    fn plain_text_is_unchanged() {
        for text in [
            "Hello, how are you?",
            "Um, I think we should, uh, ship it.",
            "It costs 3.14 dollars, about fifty percent of the budget.",
            "Mail me at pieter at sitelift dot net.",
            "Run cargo build dash dash release.",
            "",
        ] {
            assert_eq!(cmd(text), text);
        }
    }

    #[test]
    fn punctuation_commands() {
        assert_eq!(cmd("hello comma how are you question mark"), "hello, how are you?");
        assert_eq!(cmd("I'm done period"), "I'm done.");
        assert_eq!(cmd("I'm done full stop"), "I'm done.");
        assert_eq!(cmd("that is amazing exclamation mark"), "that is amazing!");
        assert_eq!(cmd("that is amazing exclamation point"), "that is amazing!");
        assert_eq!(cmd("to do colon keep the mic warm"), "to do: keep the mic warm");
        assert_eq!(cmd("first semicolon second"), "first; second");
        assert_eq!(cmd("first semi colon second"), "first; second");
    }

    #[test]
    fn commands_are_case_insensitive() {
        assert_eq!(cmd("Hello Comma how are you Question Mark"), "Hello, how are you?");
        assert_eq!(cmd("Done. Period."), "Done.");
    }

    #[test]
    fn command_replaces_the_recognizers_punctuation() {
        // Parakeet punctuates around the command words.
        assert_eq!(cmd("Hello, comma, how are you?"), "Hello, how are you?");
        assert_eq!(cmd("Hello comma. How are you?"), "Hello, how are you?");
        assert_eq!(cmd("Can someone test it before I merge? Question mark."), "Can someone test it before I merge?");
        assert_eq!(cmd("This morning, period. We're almost ready."), "This morning. We're almost ready.");
        assert_eq!(cmd("Is it ready, question mark?"), "Is it ready?");
    }

    #[test]
    fn terminal_command_capitalizes_the_next_word() {
        assert_eq!(cmd("Thanks for the call period we're almost ready."), "Thanks for the call. We're almost ready.");
        assert_eq!(cmd("Is it ready question mark let me know."), "Is it ready? Let me know.");
    }

    #[test]
    fn clause_command_keeps_names_capitalized() {
        assert_eq!(cmd("Kind regards comma Pieter"), "Kind regards, Pieter");
        assert_eq!(cmd("Thanks comma Anna will join."), "Thanks, Anna will join.");
        // Common words the recognizer capitalized after its own period.
        assert_eq!(cmd("Hello comma. The plan is ready."), "Hello, the plan is ready.");
    }

    #[test]
    fn adjacent_commands_collapse() {
        assert_eq!(cmd("hey comma period world"), "hey. World");
        assert_eq!(cmd("hey period comma world"), "hey. World");
        assert_eq!(cmd("foo comma comma bar"), "foo, bar");
    }

    #[test]
    fn line_and_paragraph_breaks() {
        assert_eq!(cmd("Buy milk new line buy eggs"), "Buy milk\nBuy eggs");
        assert_eq!(cmd("Buy milk newline buy eggs"), "Buy milk\nBuy eggs");
        assert_eq!(cmd("Rough plan. New paragraph. We'd start with booking."), "Rough plan.\n\nWe'd start with booking.");
        assert_eq!(
            cmd("hi Marieke comma new paragraph thanks for the call period new paragraph kind regards comma Pieter"),
            "hi Marieke,\n\nThanks for the call.\n\nKind regards, Pieter"
        );
        // A break keeps the recognizer's punctuation before it.
        assert_eq!(cmd("Hello, new line, how are you?"), "Hello,\nHow are you?");
    }

    #[test]
    fn hyphen_and_parentheses() {
        assert_eq!(cmd("a well hyphen known fact"), "a well-known fact");
        assert_eq!(cmd("see the docs open paren page two close paren for details"), "see the docs (page two) for details");
        assert_eq!(cmd("see the docs open parenthesis page two close parenthesis."), "see the docs (page two).");
    }

    #[test]
    fn command_at_the_edges() {
        assert_eq!(cmd("period"), ".");
        assert_eq!(cmd("New paragraph. Thanks for waiting."), "\n\nThanks for waiting.");
        assert_eq!(cmd("Thanks new line"), "Thanks\n");
    }

    #[test]
    fn talking_about_punctuation_is_kept() {
        for text in [
            "Add a comma after the name.",
            "I always use the Oxford comma.",
            "Put a question mark there.",
            "The colon is part of the large intestine.",
            "We launched a new line of products.",
            "Start on a new line.",
            "It has formatting cues like new paragraph or bullet point.",
            "Use the word period sparingly.",
            "A list of comma separated values.",
            "The car came to a full stop.",
            "Insert a hyphen here.",
        ] {
            assert_eq!(cmd(text), text);
        }
    }

    #[test]
    fn period_as_a_noun_is_kept() {
        for text in [
            "The trial period ends tomorrow.",
            "There's a two week waiting period.",
            "That was a busy period for us.",
            "Over a period of time.",
            "During the summer period.",
            "The grace period",
        ] {
            assert_eq!(cmd(text), text);
        }
        // A determiner two words back does not block: "sign the contract period".
        assert_eq!(cmd("Please sign the contract period"), "Please sign the contract.");
    }

    #[test]
    fn partial_words_do_not_match() {
        assert_eq!(cmd("The periods and commas are fine."), "The periods and commas are fine.");
        assert_eq!(cmd("Colonel Mustard did it."), "Colonel Mustard did it.");
        assert_eq!(cmd("The newlines are escaped."), "The newlines are escaped.");
    }

    #[test]
    fn whitespace_is_normalized() {
        assert_eq!(cmd("  hello   comma  world  "), "hello, world");
    }

    #[test]
    fn spoken_text_joins_then_converts() {
        let s = segs(&["Thanks for the call period.", "And we're almost ready new paragraph", "kind regards comma Pieter"]);
        assert_eq!(spoken_text(&s), "Thanks for the call. And we're almost ready\n\nKind regards, Pieter");
    }

    #[test]
    fn spoken_text_keeps_fillers_for_the_model() {
        let s = segs(&["Um, so I think", "we should ship it."]);
        assert_eq!(spoken_text(&s), "Um, so I think we should ship it.");
    }

    // ── join_segments ──────────────────────────────────────────────────

    fn segs(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn join_empty() {
        assert_eq!(join_segments(&[]), "");
        assert_eq!(join_segments(&segs(&["", "  ", "\n"])), "");
    }

    #[test]
    fn join_single_segment_passthrough() {
        assert_eq!(join_segments(&segs(&["Hello world."])), "Hello world.");
    }

    #[test]
    fn join_real_sentence_boundary_kept() {
        let s = segs(&["I went to the store.", "I bought milk."]);
        assert_eq!(join_segments(&s), "I went to the store. I bought milk.");
    }

    #[test]
    fn join_stub_end_just_merges() {
        let s = segs(&["Don't do any coding right now. Just.", "See if it's viable."]);
        assert_eq!(join_segments(&s), "Don't do any coding right now. Just see if it's viable.");
    }

    #[test]
    fn join_stub_end_preposition_merges() {
        assert_eq!(join_segments(&segs(&["I went to.", "the store."])), "I went to the store.");
    }

    #[test]
    fn join_stub_end_modal_merges() {
        assert_eq!(join_segments(&segs(&["I will.", "make it work."])), "I will make it work.");
    }

    #[test]
    fn join_continuation_word_merges() {
        let s = segs(&["I went to the store.", "And bought milk."]);
        assert_eq!(join_segments(&s), "I went to the store and bought milk.");
        let s = segs(&["I tried to fix it.", "But it didn't work."]);
        assert_eq!(join_segments(&s), "I tried to fix it but it didn't work.");
    }

    #[test]
    fn join_lowercase_start_merges() {
        let s = segs(&["I went to the store.", "and bought milk."]);
        assert_eq!(join_segments(&s), "I went to the store and bought milk.");
    }

    #[test]
    fn join_preserves_proper_nouns_and_i() {
        assert_eq!(join_segments(&segs(&["I went to.", "Paris last summer."])), "I went to Paris last summer.");
        let result = join_segments(&segs(&["I went to.", "I think we should."]));
        assert!(result.contains("I think"), "{result}");
    }

    #[test]
    fn join_three_segments_with_two_merges() {
        let s = segs(&[
            "Don't do any coding at this point in time. Just.",
            "See if it's viable. And if we can.",
            "Make it work.",
        ]);
        let result = join_segments(&s);
        assert!(result.contains("Just see if it's viable and if we can make it work."), "{result}");
    }

    #[test]
    fn join_filler_as_stub_end_merges() {
        let s = segs(&["Don't do any coding right now. Just um.", "See if it's viable."]);
        assert_eq!(join_segments(&s), "Don't do any coding right now. Just um see if it's viable.");
    }

    #[test]
    fn join_drops_empty_segments() {
        assert_eq!(join_segments(&segs(&["First.", "", "  ", "Second."])), "First. Second.");
    }

    #[test]
    fn join_stub_end_merge_across_segments() {
        let s = segs(&[
            "This is a test of the Chirp app after we just did a huge pipeline overhaul for the.",
            "Text injection and other pipelines.",
        ]);
        let result = join_segments(&s);
        assert!(result.to_lowercase().contains("overhaul for the text injection"), "{result}");
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
}
