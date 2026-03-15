use ndarray::Array2;
use ort::session::Session;
use ort::value::Value;
use regex::Regex;
use rust_tokenizers::tokenizer::{T5Tokenizer, Tokenizer, TruncationStrategy};
use std::path::Path;

use crate::settings::models_dir;

const TASK_PREFIX: &str = "Fix the text: ";
const MAX_INPUT_LEN: usize = 256;
const MAX_OUTPUT_LEN: usize = 256;
const PAD_TOKEN_ID: i64 = 0;
const EOS_TOKEN_ID: i64 = 1;

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

/// Load the T5 tokenizer from spiece.model
pub fn load_tokenizer() -> Result<T5Tokenizer, String> {
    let spiece_path = models_dir().join("cleanup").join("spiece.model");
    T5Tokenizer::from_file(spiece_path.to_str().unwrap_or(""), false)
        .map_err(|e| format!("Failed to load tokenizer: {e}"))
}

/// Run T5 model inference: encode input, greedy decode output
fn model_cleanup(
    text: &str,
    encoder: &mut Session,
    decoder: &mut Session,
    tokenizer: &T5Tokenizer,
) -> Result<String, String> {
    let input_text = format!("{TASK_PREFIX}{text}");

    // Tokenize
    let encoded = tokenizer.encode(
        &input_text,
        None,
        MAX_INPUT_LEN,
        &TruncationStrategy::LongestFirst,
        0,
    );
    let token_ids: Vec<i64> = encoded.token_ids.iter().map(|&id| id).collect();
    let seq_len = token_ids.len();

    if seq_len == 0 {
        return Ok(text.to_string());
    }

    // Build encoder inputs
    let input_ids = Array2::from_shape_vec((1, seq_len), token_ids.clone())
        .map_err(|e| format!("input_ids shape error: {e}"))?;
    let attention_mask = Array2::from_shape_vec(
        (1, seq_len),
        vec![1i64; seq_len],
    )
    .map_err(|e| format!("attention_mask shape error: {e}"))?;

    // Run encoder
    let enc_input_ids = Value::from_array(input_ids.clone())
        .map_err(|e| format!("encoder input error: {e}"))?;
    let enc_attn_mask = Value::from_array(attention_mask.clone())
        .map_err(|e| format!("encoder attn error: {e}"))?;

    let encoder_outputs = encoder
        .run(ort::inputs!["input_ids" => enc_input_ids, "attention_mask" => enc_attn_mask])
        .map_err(|e| format!("Encoder run failed: {e}"))?;

    let (hidden_shape, hidden_data) = encoder_outputs["last_hidden_state"]
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("Failed to extract hidden states: {e}"))?;
    let hidden_shape_vec: Vec<i64> = hidden_shape.iter().copied().collect();
    let hidden_data_vec: Vec<f32> = hidden_data.to_vec();

    // Greedy decode
    let mut generated_ids: Vec<i64> = vec![PAD_TOKEN_ID]; // decoder_start_token_id = 0

    for _ in 0..MAX_OUTPUT_LEN {
        let dec_seq_len = generated_ids.len();
        let dec_input_ids = Array2::from_shape_vec(
            (1, dec_seq_len),
            generated_ids.clone(),
        )
        .map_err(|e| format!("dec input shape error: {e}"))?;

        let dec_ids_val = Value::from_array(dec_input_ids)
            .map_err(|e| format!("dec ids error: {e}"))?;
        let dec_attn_val = Value::from_array(attention_mask.clone())
            .map_err(|e| format!("dec attn error: {e}"))?;
        let dec_hidden_val = Value::from_array((hidden_shape_vec.clone(), hidden_data_vec.clone()))
            .map_err(|e| format!("dec hidden error: {e}"))?;

        let decoder_outputs = decoder
            .run(ort::inputs![
                "encoder_attention_mask" => dec_attn_val,
                "input_ids" => dec_ids_val,
                "encoder_hidden_states" => dec_hidden_val,
            ])
            .map_err(|e| format!("Decoder run failed: {e}"))?;

        let (logits_shape, logits_data) = decoder_outputs["logits"]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract logits: {e}"))?;

        // Get logits for the last position
        let vocab_size = logits_shape[2usize] as usize;
        let last_pos = dec_seq_len - 1;
        let offset = last_pos * vocab_size;

        // Argmax over vocabulary
        let mut max_id = 0i64;
        let mut max_val = f32::NEG_INFINITY;
        for v in 0..vocab_size {
            let val = logits_data[offset + v];
            if val > max_val {
                max_val = val;
                max_id = v as i64;
            }
        }

        if max_id == EOS_TOKEN_ID {
            break;
        }

        generated_ids.push(max_id);
    }

    // Decode tokens back to text (skip the start token)
    let output_ids: Vec<i64> = generated_ids[1..].to_vec();
    let decoded = tokenizer.decode(&output_ids, true, true);

    Ok(decoded)
}

/// Full cleanup pipeline: filler removal → regex formatting → model cleanup
pub fn cleanup_text(
    text: &str,
    smart_formatting: bool,
    encoder: Option<&mut Session>,
    decoder: Option<&mut Session>,
    tokenizer: Option<&T5Tokenizer>,
) -> String {
    if text.is_empty() {
        return String::new();
    }

    // Step 1: Remove filler words
    let cleaned = remove_fillers(text);

    if !smart_formatting {
        return capitalize_first(&cleaned);
    }

    // Step 2: Regex-based formatting (spoken punctuation, numbers, etc.)
    let formatted = smart_format(&cleaned);

    // Step 3: Model cleanup if all pieces are available
    if let (Some(enc), Some(dec), Some(tok)) = (encoder, decoder, tokenizer) {
        match model_cleanup(&formatted, enc, dec, tok) {
            Ok(result) if !result.trim().is_empty() => return result,
            Ok(_) => log::warn!("Model returned empty output, using regex result"),
            Err(e) => log::warn!("Model cleanup failed, using regex result: {e}"),
        }
    }

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
        let result = cleanup_text("um i want to uh send an email to bob at test dot com", true, None, None, None::<&T5Tokenizer>);
        assert!(result.starts_with("I"));
        assert!(result.contains("bob@test.com"));
        assert!(!result.contains("um"));
        assert!(!result.contains("uh"));
    }
}
