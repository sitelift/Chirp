use regex::Regex;
use std::sync::OnceLock;

/// Pre-compiled regex patterns for text cleanup
struct CleanupRegexes {
    fillers: Vec<Regex>,
    dangling_comma: Regex,
    leading_comma: Regex,
    whitespace: Regex,
    sentence_end: Regex,
    standalone_i: Regex,
    i_contraction: Regex,
    punctuation: Vec<(Regex, &'static str)>,
    space_before_punct: Regex,
    no_space_after: Regex,
    email: Regex,
    numeric_contexts: Vec<Regex>,
    number_words: Vec<(Regex, &'static str)>,
    percentage: Regex,
    hundred_pct: Regex,
    list_pattern: Regex,
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
            (r"(?i)\bdash\b", " —"),
            (r"(?i)\bhyphen\b", "-"),
            (r"(?i)\bopen (?:paren|parenthesis)\b", "("),
            (r"(?i)\bclose (?:paren|parenthesis)\b", ")"),
            (r"(?i)\bnew line\b", "\n"),
            (r"(?i)\bnew paragraph\b", "\n\n"),
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
            sentence_end: Regex::new(r"([.!?])\s+([a-z])").unwrap(),
            standalone_i: Regex::new(r"\bi\b").unwrap(),
            i_contraction: Regex::new(r"\bI'([msdtv])").unwrap(),
            punctuation: punctuation_map,
            space_before_punct: Regex::new(r"\s+([.,!?;:)])").unwrap(),
            no_space_after: Regex::new(r"([.,!?;:])([A-Za-z])").unwrap(),
            email: Regex::new(r"(?i)\b(\w+)\s+at\s+(\w+)\s+dot\s+(com|org|net|io|dev|co)\b").unwrap(),
            numeric_contexts: compiled_contexts,
            number_words: compiled_numbers.iter().map(|d| {
                // These are just digit placeholders; we store them as (Regex, &str)
                // but for the combined patterns we already have the regex in numeric_contexts
                // Use a dummy regex that never matches - the actual matching is done via numeric_contexts
                (Regex::new("^$").unwrap(), *d)
            }).collect(),
            percentage: Regex::new(r"(?i)\b(twenty|thirty|forty|fifty|sixty|seventy|eighty|ninety)\s+percent\b").unwrap(),
            hundred_pct: Regex::new(r"(?i)\b(one )?hundred percent\b").unwrap(),
            list_pattern: Regex::new(r"(?i)\b(first|one|1)[,:]?\s+(.+?)[,.]?\s+(second|two|2)[,:]?\s+(.+?)(?:[,.]?\s+(third|three|3)[,:]?\s+(.+?))?(?:[,.]?\s+(fourth|four|4)[,:]?\s+(.+?))?(?:[,.]?\s+(fifth|five|5)[,:]?\s+(.+?))?[.]?$").unwrap(),
        }
    })
}

/// Full cleanup pipeline: filler removal → regex formatting
/// When `ai_cleanup` is true, skip list detection and sentence restructuring
/// since the AI model handles those better.
pub fn cleanup_text(text: &str, smart_formatting: bool) -> String {
    cleanup_text_inner(text, smart_formatting, false)
}

/// Same as cleanup_text but with option to skip transforms the AI handles
pub fn cleanup_text_for_ai(text: &str, smart_formatting: bool) -> String {
    cleanup_text_inner(text, smart_formatting, true)
}

fn cleanup_text_inner(text: &str, smart_formatting: bool, skip_ai_overlap: bool) -> String {
    if text.is_empty() {
        return String::new();
    }

    // Step 1: Remove filler words
    let cleaned = remove_fillers(text);

    if !smart_formatting {
        return capitalize_first(&cleaned);
    }

    // Step 2: Regex-based formatting (spoken punctuation, numbers, etc.)
    smart_format(&cleaned, skip_ai_overlap)
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
    let mut chars = trimmed.chars();
    let first = chars.next().unwrap();
    first.to_uppercase().to_string() + chars.as_str()
}

/// Smart formatting: punctuation, capitalization, numbers, common patterns
fn smart_format(text: &str, skip_ai_overlap: bool) -> String {
    let re = regexes();
    let mut result = text.to_string();

    // Expand spoken numbers to digits for common cases
    result = format_spoken_numbers(&result);

    // Format common spoken patterns
    result = format_spoken_patterns(&result);

    // Capitalize first letter
    result = capitalize_first(&result);

    // Add period at end if missing punctuation
    let trimmed = result.trim_end();
    if !trimmed.is_empty() {
        let last = trimmed.chars().last().unwrap();
        if !matches!(last, '.' | '!' | '?' | ':' | ';' | '"' | ')') {
            result = format!("{trimmed}.");
        }
    }

    // Capitalize after sentence-ending punctuation
    result = re
        .sentence_end
        .replace_all(&result, |caps: &regex::Captures| {
            format!("{} {}", &caps[1], caps[2].to_uppercase())
        })
        .to_string();

    // Capitalize "i" as standalone word
    result = re.standalone_i.replace_all(&result, "I").to_string();
    // Fix I contractions
    result = re
        .i_contraction
        .replace_all(&result, |caps: &regex::Captures| {
            format!("I'{}", &caps[1])
        })
        .to_string();

    result
}

/// Convert spoken number words to digits for common short numbers
fn format_spoken_numbers(text: &str) -> String {
    let re = regexes();
    let mut result = text.to_string();

    // Apply pre-compiled combined context+number patterns
    for (i, ctx_re) in re.numeric_contexts.iter().enumerate() {
        let digit = re.number_words[i].1;
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

    // Clean up spaces before punctuation
    result = re.space_before_punct.replace_all(&result, "$1").to_string();

    // Ensure space after punctuation
    result = re.no_space_after.replace_all(&result, "$1 $2").to_string();

    // Email pattern
    result = re.email.replace_all(&result, "$1@$2.$3").to_string();

    result
}

/// Detect and format list patterns
fn format_lists(text: &str) -> String {
    let re = regexes();
    if let Some(caps) = re.list_pattern.captures(text) {
        let mut items = Vec::new();
        for i in (1..caps.len()).step_by(2) {
            if let (Some(_keyword), Some(content)) = (caps.get(i), caps.get(i + 1)) {
                let item = content.as_str().trim().trim_end_matches(['.', ',']).to_string();
                if !item.is_empty() {
                    items.push(item);
                }
            }
        }
        if items.len() >= 2 {
            let numbered: Vec<String> = items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let s = capitalize_first(item);
                    format!("{}. {}", i + 1, s)
                })
                .collect();
            let match_start = caps.get(0).unwrap().start();
            let preamble = text[..match_start].trim();
            if preamble.is_empty() {
                return numbered.join("\n");
            } else {
                return format!("{}\n{}", preamble, numbered.join("\n"));
            }
        }
    }

    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_fillers() {
        let input = "um so, uh I want to go to the store";
        let result = remove_fillers(input);
        assert!(!result.contains("um"));
        assert!(!result.contains("uh"));
        assert!(result.contains("I want to go to the store"));
    }

    #[test]
    fn test_capitalize_i() {
        let result = smart_format("i want to go and i need help", false);
        assert!(result.contains("I want"));
        assert!(result.contains("I need"));
    }

    #[test]
    fn test_sentence_ending() {
        let result = smart_format("hello world", false);
        assert!(result.ends_with('.'));
    }

    #[test]
    fn test_spoken_punctuation() {
        let result = smart_format("hello comma how are you question mark", false);
        assert!(result.contains("Hello, how are you?"));
    }

    #[test]
    fn test_percentage() {
        let result = smart_format("it was about fifty percent done", false);
        assert!(result.contains("50%"));
    }

    #[test]
    fn test_email() {
        let result = smart_format("send it to john at example dot com", false);
        assert!(result.contains("john@example.com"));
    }

    #[test]
    fn test_new_paragraph() {
        let result = smart_format("hello new paragraph world", false);
        assert!(result.contains("\n\n"));
    }

    #[test]
    fn test_full_cleanup() {
        let result = cleanup_text("um i want to uh send an email to bob at test dot com", true);
        assert!(result.starts_with("I"));
        assert!(result.contains("bob@test.com"));
        assert!(!result.contains("um"));
        assert!(!result.contains("uh"));
    }
}
