use regex::Regex;

use crate::state::DictionaryEntry;

/// Apply dictionary replacements: case-insensitive whole-word find and replace
pub fn apply_dictionary(text: &str, entries: &[DictionaryEntry]) -> String {
    let mut result = text.to_string();

    for entry in entries {
        if entry.from.is_empty() {
            continue;
        }
        // Escape regex special characters in the "from" pattern
        let escaped = regex::escape(&entry.from);
        let pattern = format!(r"(?i)\b{escaped}\b");
        if let Ok(re) = Regex::new(&pattern) {
            result = re.replace_all(&result, entry.to.as_str()).to_string();
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_replacement() {
        let entries = vec![DictionaryEntry {
            from: "gonna".into(),
            to: "going to".into(),
        }];
        assert_eq!(
            apply_dictionary("I'm gonna go", &entries),
            "I'm going to go"
        );
    }

    #[test]
    fn test_case_insensitive() {
        let entries = vec![DictionaryEntry {
            from: "javascript".into(),
            to: "JavaScript".into(),
        }];
        assert_eq!(
            apply_dictionary("I love javascript", &entries),
            "I love JavaScript"
        );
    }

    #[test]
    fn test_whole_word() {
        let entries = vec![DictionaryEntry {
            from: "go".into(),
            to: "leave".into(),
        }];
        // "go" should match but "going" should not
        let result = apply_dictionary("I go now but I'm going later", &entries);
        assert!(result.starts_with("I leave"));
    }
}
