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

/// Full cleanup pipeline: filler removal → model or rule-based formatting → trim
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
        return cleaned;
    }

    // Step 2: Try ONNX model inference (if sessions provided)
    // TODO: Implement T5 encoder→decoder inference loop when model is available
    // For now, always fall through to rule-based formatting

    // Step 3: Rule-based fallback formatting
    rule_based_format(&cleaned)
}

/// Remove common filler words from transcript
fn remove_fillers(text: &str) -> String {
    let fillers = [
        r"\bum\b",
        r"\buh\b",
        r"\buh huh\b",
        r"\bmm hmm\b",
        r"\bhmm\b",
        r"\byou know\b",
        r"\blike\b(?=\s+(the|a|an|i|we|they|he|she|it|my|our|this|that)\b)",
        r"\bbasically\b",
        r"\bactually\b(?=\s*,)",
        r"\bso\b(?=\s*,)",
        r"\bi mean\b(?=\s*,)",
        r"\bkind of\b",
        r"\bsort of\b",
    ];

    let mut result = text.to_string();
    for pattern in &fillers {
        if let Ok(re) = Regex::new(&format!("(?i){pattern}")) {
            result = re.replace_all(&result, "").to_string();
        }
    }

    // Clean up extra whitespace left by removal
    let ws_re = Regex::new(r"\s{2,}").unwrap();
    ws_re.replace_all(result.trim(), " ").to_string()
}

/// Rule-based formatting fallback
fn rule_based_format(text: &str) -> String {
    let mut result = text.to_string();

    // Capitalize first letter
    if let Some(first) = result.chars().next() {
        result = first.to_uppercase().to_string() + &result[first.len_utf8()..];
    }

    // Add period at end if missing punctuation
    let trimmed = result.trim_end();
    if !trimmed.is_empty() {
        let last = trimmed.chars().last().unwrap();
        if !matches!(last, '.' | '!' | '?' | ':' | ';') {
            result = format!("{trimmed}.");
        }
    }

    // Capitalize after sentence-ending punctuation
    let sent_re = Regex::new(r"([.!?])\s+([a-z])").unwrap();
    result = sent_re
        .replace_all(&result, |caps: &regex::Captures| {
            format!(
                "{} {}",
                &caps[1],
                caps[2].to_uppercase()
            )
        })
        .to_string();

    // Capitalize "i" as standalone word
    let i_re = Regex::new(r"\bi\b").unwrap();
    result = i_re.replace_all(&result, "I").to_string();

    // Detect simple list patterns: "first... second... third..."
    let list_re = Regex::new(r"(?i)\b(first|one|1)\s+(.+?)\s+(second|two|2)\s+(.+?)(?:\s+(third|three|3)\s+(.+?))?(?:\s+(fourth|four|4)\s+(.+?))?(?:\s+(fifth|five|5)\s+(.+?))?$").unwrap();
    if let Some(caps) = list_re.captures(&result) {
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
                    let mut s = item.clone();
                    if let Some(first) = s.chars().next() {
                        s = first.to_uppercase().to_string() + &s[first.len_utf8()..];
                    }
                    format!("{}. {}", i + 1, s)
                })
                .collect();
            return numbered.join("\n");
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_fillers() {
        let input = "um so uh I want to go to the store";
        let result = remove_fillers(input);
        assert!(!result.contains("um"));
        assert!(!result.contains("uh"));
        assert!(result.contains("I want to go to the store"));
    }

    #[test]
    fn test_capitalize_i() {
        let result = rule_based_format("i want to go and i need help");
        assert!(result.contains("I want"));
        assert!(result.contains("I need"));
    }

    #[test]
    fn test_sentence_ending() {
        let result = rule_based_format("hello world");
        assert!(result.ends_with('.'));
    }
}
