use ort::session::Session;
use regex::Regex;
use std::path::Path;

use crate::settings::models_dir;

/// Check if the ONNX cleanup model files exist
pub fn cleanup_model_exists() -> bool {
    let dir = models_dir().join("cleanup");
    dir.join("encoder_model.onnx").exists() && dir.join("decoder_model.onnx").exists()
}

/// Load ONNX encoder session
pub fn load_encoder() -> Result<Session, String> {
    let path = models_dir().join("cleanup").join("encoder_model.onnx");
    load_onnx_session(&path)
}

/// Load ONNX decoder session
pub fn load_decoder() -> Result<Session, String> {
    let path = models_dir().join("cleanup").join("decoder_model.onnx");
    load_onnx_session(&path)
}

fn load_onnx_session(path: &Path) -> Result<Session, String> {
    Session::builder()
        .and_then(|mut b| b.commit_from_file(path))
        .map_err(|e| format!("Failed to load ONNX model {}: {e}", path.display()))
}

/// Full cleanup pipeline: filler removal → formatting → trim
pub fn cleanup_text(
    text: &str,
    smart_formatting: bool,
    _encoder: Option<&Session>,
    _decoder: Option<&Session>,
) -> String {
    if text.is_empty() {
        return String::new();
    }

    // Step 1: Remove filler words
    let cleaned = remove_fillers(text);

    if !smart_formatting {
        return capitalize_first(&cleaned);
    }

    // Step 2: Smart formatting
    let formatted = smart_format(&cleaned);

    formatted
}

/// Remove common filler words from transcript
fn remove_fillers(text: &str) -> String {
    let fillers = [
        r"\bum+\b",
        r"\buh+\b",
        r"\buh huh\b",
        r"\bmm+ ?hmm+\b",
        r"\bhmm+\b",
        r"\byou know\b(?=\s*,?\s)",
        r"\blike\b(?=\s+(the|a|an|i|we|they|he|she|it|my|our|this|that)\b)",
        r"\bbasically\b(?=\s*,)",
        r"\bactually\b(?=\s*,)",
        r"\bso\b(?=\s*,\s)",
        r"\bi mean\b(?=\s*,)",
        r"\bkind of\b(?=\s+(like|a|the)\b)",
        r"\bsort of\b(?=\s+(like|a|the)\b)",
        r"\bright\s*\?\s*(?=\b)",
    ];

    let mut result = text.to_string();
    for pattern in &fillers {
        if let Ok(re) = Regex::new(&format!("(?i){pattern}")) {
            result = re.replace_all(&result, "").to_string();
        }
    }

    // Clean up extra whitespace and dangling commas from removal
    let dangling_comma = Regex::new(r",\s*,").unwrap();
    result = dangling_comma.replace_all(&result, ",").to_string();
    let leading_comma = Regex::new(r"^\s*,\s*").unwrap();
    result = leading_comma.replace(&result, "").to_string();
    let ws_re = Regex::new(r"\s{2,}").unwrap();
    ws_re.replace_all(result.trim(), " ").to_string()
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
fn smart_format(text: &str) -> String {
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
    let sent_re = Regex::new(r"([.!?])\s+([a-z])").unwrap();
    result = sent_re
        .replace_all(&result, |caps: &regex::Captures| {
            format!("{} {}", &caps[1], caps[2].to_uppercase())
        })
        .to_string();

    // Capitalize "i" as standalone word
    let i_re = Regex::new(r"\bi\b").unwrap();
    result = i_re.replace_all(&result, "I").to_string();
    // Fix "I'M", "I'D", "I'LL", "I'VE" — the I is already uppercase
    // but also handle "i'm" → "I'm"
    let i_contraction = Regex::new(r"\bI'([msdtv])").unwrap();
    result = i_contraction
        .replace_all(&result, |caps: &regex::Captures| {
            format!("I'{}", &caps[1])
        })
        .to_string();

    // Detect list patterns and format as numbered list
    result = format_lists(&result);

    result
}

/// Convert spoken number words to digits for common short numbers
fn format_spoken_numbers(text: &str) -> String {
    let mut result = text.to_string();

    // Map of spoken numbers to digits (only convert when contextually appropriate)
    let number_words = [
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

    // Only convert numbers that appear after numeric context words
    let numeric_contexts = [
        r"(?i)\b(number|step|item|option|version|v|chapter|page|line|row|column|level|grade|score|count|total)\s+",
        r"(?i)\b(is|are|was|were|equals?|=)\s+",
        r"(?i)\b(about|around|approximately|roughly|nearly|over|under)\s+",
    ];

    for ctx_pattern in &numeric_contexts {
        for (word_pattern, digit) in &number_words {
            let combined = format!("({ctx_pattern})({word_pattern})");
            if let Ok(re) = Regex::new(&combined) {
                result = re
                    .replace_all(&result, |caps: &regex::Captures| {
                        format!("{}{}", &caps[1], digit)
                    })
                    .to_string();
            }
        }
    }

    // Percentages: "fifty percent" → "50%", etc.
    let pct_re = Regex::new(r"(?i)\b(twenty|thirty|forty|fifty|sixty|seventy|eighty|ninety)\s+percent\b").unwrap();
    result = pct_re
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
    let hundred_pct = Regex::new(r"(?i)\b(one )?hundred percent\b").unwrap();
    result = hundred_pct.replace_all(&result, "100%").to_string();

    result
}

/// Format common spoken patterns (email, URLs, punctuation commands)
fn format_spoken_patterns(text: &str) -> String {
    let mut result = text.to_string();

    // Spoken punctuation → actual punctuation
    let punctuation_map = [
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
    ];

    for (pattern, replacement) in &punctuation_map {
        if let Ok(re) = Regex::new(pattern) {
            result = re.replace_all(&result, *replacement).to_string();
        }
    }

    // Clean up spaces before punctuation (e.g., "hello ." → "hello.")
    let space_before_punct = Regex::new(r"\s+([.,!?;:)])").unwrap();
    result = space_before_punct.replace_all(&result, "$1").to_string();

    // Ensure space after punctuation (but not before newlines or at end)
    let no_space_after = Regex::new(r"([.,!?;:])([A-Za-z])").unwrap();
    result = no_space_after.replace_all(&result, "$1 $2").to_string();

    // "at" between words with domain-like suffix → @ (email)
    let email_re = Regex::new(r"(?i)\b(\w+)\s+at\s+(\w+)\s+dot\s+(com|org|net|io|dev|co)\b").unwrap();
    result = email_re.replace_all(&result, "$1@$2.$3").to_string();

    result
}

/// Detect and format list patterns
fn format_lists(text: &str) -> String {
    // Pattern: "first ... second ... third ..."
    let list_re = Regex::new(r"(?i)\b(first|one|1)\s+(.+?)\s+(second|two|2)\s+(.+?)(?:\s+(third|three|3)\s+(.+?))?(?:\s+(fourth|four|4)\s+(.+?))?(?:\s+(fifth|five|5)\s+(.+?))?$").unwrap();
    if let Some(caps) = list_re.captures(text) {
        let mut items = Vec::new();
        for i in (1..caps.len()).step_by(2) {
            if let (Some(_keyword), Some(content)) = (caps.get(i), caps.get(i + 1)) {
                let item = content.as_str().trim().trim_end_matches('.').to_string();
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
            return numbered.join("\n");
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
        let result = smart_format("i want to go and i need help");
        assert!(result.contains("I want"));
        assert!(result.contains("I need"));
    }

    #[test]
    fn test_sentence_ending() {
        let result = smart_format("hello world");
        assert!(result.ends_with('.'));
    }

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
    fn test_new_paragraph() {
        let result = smart_format("first paragraph new paragraph second paragraph");
        assert!(result.contains("\n\n"));
    }

    #[test]
    fn test_full_cleanup() {
        let result = cleanup_text("um i want to uh send an email to bob at test dot com", true, None, None);
        assert!(result.starts_with("I"));
        assert!(result.contains("bob@test.com"));
        assert!(!result.contains("um"));
        assert!(!result.contains("uh"));
    }
}
