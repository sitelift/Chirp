//! Router: decide whether a dictation needs the cloud LLM cleanup pass.
//!
//! Input is the text after the deterministic pass (`cleanup::cleanup_text` +
//! vocabulary replacements) and the active app name. The LLM is skipped only
//! when the dictation is short, plain, and headed somewhere casual, because
//! then the deterministic text is already what the user meant and skipping
//! saves the whole network round trip (~500-700 ms p50).
//!
//! Everything here is plain string scanning: no regex, no allocation beyond
//! one lowercase copy, well under a millisecond. Thresholds were tuned against
//! `bench/corpus/manifest.json` (see the tests at the bottom).

/// Dictations with at most this many words can skip the LLM. Every non-short
/// corpus entry is 12+ words; real short replies ("sounds good, thanks",
/// "I'll be five minutes late") are 2-6. Ten leaves headroom for slightly
/// longer plain replies without letting multi-clause rambles through.
pub const MAX_SKIP_WORDS: usize = 10;

/// Returns `true` when the text should go to the LLM cleanup pass.
pub fn needs_llm(text: &str, app: &str) -> bool {
    let text = text.trim();
    if text.is_empty() {
        return false;
    }
    if word_count(text) > MAX_SKIP_WORDS {
        return true;
    }
    if structured_app(app) {
        return true;
    }
    let lower = format!(" {} ", normalize(text));
    // "Sorry, running late" / "Actually, I can make it" open a message; they
    // only signal a correction after something has been said.
    let after_first = lower[1..].find(' ').map_or("", |i| &lower[i + 1..]);
    has_any(&lower, SELF_CORRECTION)
        || has_any(after_first, MIDSENTENCE_CORRECTION)
        || has_any(&lower, FORMATTING_CUES)
        || has_any(&lower, LEFTOVER_FILLERS)
        || has_any(&lower, SPOKEN_NUMBERS)
        || has_repeated_word(&lower)
        || text.contains('\u{2014}') // em dash: the deterministic pass turned a spoken "dash" into one
}

fn word_count(text: &str) -> usize {
    text.split_whitespace().filter(|w| w.chars().any(char::is_alphanumeric)).count()
}

/// Lowercase, with every non-alphanumeric char (except apostrophes) turned
/// into a space, so phrase matching is " no wait " regardless of commas.
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_space = true;
    for c in text.chars() {
        if c.is_alphanumeric() || c == '\'' || c == '\u{2019}' {
            out.extend(c.to_lowercase());
            last_space = false;
        } else if !last_space {
            out.push(' ');
            last_space = true;
        }
    }
    out.trim_end().to_string()
}

/// Phrases are matched as whole words: every entry is padded with spaces and
/// `lower` is padded too.
fn has_any(lower: &str, phrases: &[&str]) -> bool {
    phrases.iter().any(|p| {
        let mut padded = String::with_capacity(p.len() + 2);
        padded.push(' ');
        padded.push_str(p);
        padded.push(' ');
        lower.contains(&padded)
    })
}

/// "a a Rust function", "the the": a stutter the deterministic pass left in.
fn has_repeated_word(lower: &str) -> bool {
    let mut prev = "";
    for w in lower.split_whitespace() {
        if w == prev && !matches!(w, "very" | "no" | "bye" | "ha" | "haha" | "really" | "so") {
            return true;
        }
        prev = w;
    }
    false
}

/// Apps where even a short dictation implies structure or code formatting:
/// email clients, document editors, code editors and terminals.
fn structured_app(app: &str) -> bool {
    has_any(&format!(" {} ", normalize(app)), STRUCTURED_APPS)
}

const STRUCTURED_APPS: &[&str] = &[
    // email
    "outlook", "thunderbird", "mail", "superhuman", "gmail",
    // documents
    "word", "notion", "obsidian", "google docs", "onenote", "pages", "libreoffice",
    // code and terminals
    "visual studio", "code", "cursor", "windsurf", "zed", "intellij", "pycharm",
    "webstorm", "rider", "clion", "rustrover", "goland", "sublime", "notepad++",
    "terminal", "powershell", "cmd", "command prompt", "wezterm", "alacritty", "warp",
];

const SELF_CORRECTION: &[&str] = &[
    "no wait", "wait no", "scratch that", "i mean", "no no",
    "or rather", "let me rephrase", "delete that", "make that", "never mind",
    "nevermind", "correction", "rather", "i meant", "not that",
];

/// Correction words that only count after the first word.
const MIDSENTENCE_CORRECTION: &[&str] = &["actually", "sorry", "no"];

const FORMATTING_CUES: &[&str] = &[
    "new line", "newline", "new paragraph", "next line", "bullet", "bullet point",
    "number one", "number two", "number 1", "number 2", "first", "second", "third",
    "firstly", "secondly", "comma", "period", "full stop", "question mark",
    "exclamation mark", "exclamation point", "colon", "semicolon", "open quote",
    "close quote", "end quote", "quote", "unquote", "open paren", "close paren",
    "dash", "hyphen", "slash", "backslash", "underscore", "dot", "at sign",
    "hashtag", "ellipsis", "equals", "all caps", "camel case",
];

/// Fillers `cleanup::remove_fillers` intentionally leaves alone because they
/// are ambiguous; the LLM can tell filler from meaning.
const LEFTOVER_FILLERS: &[&str] = &["you know", "like", "kind of", "sort of", "okay so", "so yeah"];

/// Number words the deterministic pass does not convert: times ("three
/// thirty"), money, units, and anything bigger than counting numbers.
const SPOKEN_NUMBERS: &[&str] = &[
    "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen",
    "eighteen", "nineteen", "twenty", "thirty", "forty", "fifty", "sixty",
    "seventy", "eighty", "ninety", "hundred", "thousand", "million", "billion",
    "percent", "dollars", "euros", "euro", "pounds", "bucks", "o'clock", "pm",
    "point",
];

#[cfg(test)]
mod tests {
    use super::*;

    // Deterministic-pass output for the corpus `short-*` entries
    // (bench/corpus/manifest.json): all skip.
    #[test]
    fn corpus_short_replies_skip() {
        for (text, app) in [
            ("Sounds good, thanks", "Slack"),
            ("On my way", "WhatsApp"),
            ("Yep, merging now", "Slack"),
            ("Thanks, that fixed it", "Discord"),
            ("I'll be five minutes late", "Microsoft Teams"),
            ("Let me check", "Slack"),
        ] {
            assert!(!needs_llm(text, app), "{text:?} in {app} should skip");
        }
    }

    #[test]
    fn empty_skips() {
        assert!(!needs_llm("", "Slack"));
        assert!(!needs_llm("   ", "Slack"));
    }

    #[test]
    fn long_dictation_goes_to_llm() {
        let text = "Quick update on Van der Berg, the Mollie integration is done, payments work in test mode";
        assert!(needs_llm(text, "Slack"));
    }

    #[test]
    fn word_threshold_boundary() {
        let ten = "one two three four five six seven eight nine ten";
        let eleven = "one two three four five six seven eight nine ten more";
        // "one two three" etc. are counting words, not cues.
        assert!(!needs_llm(ten, "Slack"));
        assert!(needs_llm(eleven, "Slack"));
    }

    #[test]
    fn self_corrections_go_to_llm() {
        assert!(needs_llm("Tuesday, no wait, Wednesday", "Slack"));
        assert!(needs_llm("Call Anna, scratch that, call Bram", "Slack"));
        assert!(needs_llm("Six people, actually eight", "Slack"));
        assert!(needs_llm("Two thousand, sorry, more", "WhatsApp"));
    }

    #[test]
    fn formatting_cues_go_to_llm() {
        assert!(needs_llm("Buy milk new line buy eggs", "Slack"));
        assert!(needs_llm("Bullet point eggs", "Slack"));
        assert!(needs_llm("Mail me at pieter at sitelift dot net", "Slack"));
        assert!(needs_llm("Is it ready question mark", "Slack"));
        assert!(needs_llm("First eggs, second milk", "Slack"));
    }

    #[test]
    fn em_dash_artifact_goes_to_llm() {
        assert!(needs_llm("Cargo build,  —  — release", "Slack"));
    }

    #[test]
    fn spoken_numbers_go_to_llm() {
        assert!(needs_llm("See you at three thirty", "Slack"));
        assert!(needs_llm("It's twenty five euros", "WhatsApp"));
        assert!(needs_llm("Version twelve point four", "Discord"));
    }

    #[test]
    fn leftover_fillers_and_stutters_go_to_llm() {
        assert!(needs_llm("It's like really good", "Slack"));
        assert!(needs_llm("You know, it works", "Slack"));
        assert!(needs_llm("Write a, a function", "Slack"));
    }

    #[test]
    fn structured_apps_go_to_llm_even_when_short() {
        assert!(needs_llm("Thanks for the update", "Outlook"));
        assert!(needs_llm("Sounds good", "Notion"));
        assert!(needs_llm("Use state false", "Visual Studio Code"));
        assert!(needs_llm("Cargo build", "Windows Terminal"));
        assert!(needs_llm("Meeting notes", "Obsidian"));
    }

    #[test]
    fn casual_apps_and_unknown_skip_short_plain_text() {
        assert!(!needs_llm("Sounds good", "Google Chrome"));
        assert!(!needs_llm("Sounds good", ""));
        assert!(!needs_llm("Lunch at the usual place?", "WhatsApp"));
    }

    #[test]
    fn opening_sorry_or_actually_is_not_a_correction() {
        assert!(!needs_llm("Sorry, running late", "Slack"));
        assert!(!needs_llm("Actually, I can make it", "Slack"));
        assert!(needs_llm("Six, no, eight people", "Slack"));
    }

    #[test]
    fn app_names_match_whole_words() {
        // "1Password" contains "word", "Codex" contains "code".
        assert!(!needs_llm("Sounds good", "1Password"));
        assert!(!needs_llm("Sounds good", "Codex"));
        assert!(needs_llm("Sounds good", "Microsoft Word"));
    }

    #[test]
    fn whole_word_matching() {
        // "dotted", "colonel", "seconds" and "likes" must not trip the cue lists.
        assert!(!needs_llm("She likes the dotted one", "Slack"));
        assert!(!needs_llm("Give me two seconds", "Slack"));
        assert!(!needs_llm("Colonel Mustard did it", "Slack"));
    }

    #[test]
    fn allowed_repeats_skip() {
        assert!(!needs_llm("Bye bye", "Slack"));
        assert!(!needs_llm("Very very nice", "Slack"));
    }
}
