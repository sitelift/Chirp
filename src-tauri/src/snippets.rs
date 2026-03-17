use regex::Regex;

use crate::state::SnippetEntry;

/// Apply snippet expansions: case-insensitive whole-phrase matching.
/// Longest triggers are matched first to avoid partial replacements.
pub fn apply_snippets(text: &str, entries: &[SnippetEntry]) -> String {
    if entries.is_empty() {
        return text.to_string();
    }

    // Sort by trigger length descending (longest match first)
    let mut sorted: Vec<&SnippetEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| b.trigger.len().cmp(&a.trigger.len()));

    let mut result = text.to_string();
    for entry in sorted {
        if entry.trigger.is_empty() {
            continue;
        }
        // Strip punctuation from trigger for flexible matching
        let cleaned: String = entry.trigger.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
        if cleaned.is_empty() {
            continue;
        }
        let escaped = regex::escape(&cleaned);
        let pattern = format!(r"(?i)\b{escaped}\b");
        if let Ok(re) = Regex::new(&pattern) {
            result = re.replace_all(&result, entry.expansion.as_str()).to_string();
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_snippet() {
        let entries = vec![SnippetEntry {
            trigger: "my email".into(),
            expansion: "test@example.com".into(),
        }];
        assert_eq!(
            apply_snippets("send to my email please", &entries),
            "send to test@example.com please"
        );
    }

    #[test]
    fn test_case_insensitive() {
        let entries = vec![SnippetEntry {
            trigger: "My Address".into(),
            expansion: "123 Main St".into(),
        }];
        assert_eq!(
            apply_snippets("ship to my address", &entries),
            "ship to 123 Main St"
        );
    }

    #[test]
    fn test_longest_match_first() {
        let entries = vec![
            SnippetEntry {
                trigger: "my email".into(),
                expansion: "short@test.com".into(),
            },
            SnippetEntry {
                trigger: "my email address".into(),
                expansion: "full@test.com".into(),
            },
        ];
        assert_eq!(
            apply_snippets("send to my email address", &entries),
            "send to full@test.com"
        );
    }

    #[test]
    fn test_empty_entries() {
        assert_eq!(apply_snippets("hello world", &[]), "hello world");
    }
}
