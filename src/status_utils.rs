use unicode_segmentation::UnicodeSegmentation;

/// Extract base name from tab name.
/// Status is ANY first grapheme cluster followed by a space.
/// Handles complex emoji like flags and skin tones.
///
/// # Examples
/// - "🤖 Working" -> "Working"
/// - "! Alert" -> "Alert"
/// - "A Tab" -> "Tab"
/// - "Working" -> "Working" (no space after first char)
pub fn extract_base_name(name: &str) -> &str {
    let mut graphemes = name.graphemes(true);
    if let Some(_first_grapheme) = graphemes.next() {
        let rest = graphemes.as_str();
        if let Some(stripped) = rest.strip_prefix(' ') {
            return stripped;
        }
    }
    name
}

/// Extract status from tab name.
/// Status is ANY first grapheme cluster followed by a space.
///
/// # Examples
/// - "🤖 Working" -> "🤖"
/// - "! Alert" -> "!"
/// - "A Tab" -> "A"
/// - "Working" -> "" (no space after first char)
pub fn extract_status(name: &str) -> &str {
    let mut graphemes = name.graphemes(true);
    if let Some(first_grapheme) = graphemes.next() {
        let rest = graphemes.as_str();
        if rest.starts_with(' ') {
            return first_grapheme;
        }
    }
    ""
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== extract_base_name tests ====================

    #[test]
    fn test_base_name_with_emoji() {
        assert_eq!(extract_base_name("🤖 Working"), "Working");
        assert_eq!(extract_base_name("🇺🇸 USA"), "USA");
        assert_eq!(extract_base_name("✅ Done"), "Done");
    }

    #[test]
    fn test_base_name_with_punctuation() {
        assert_eq!(extract_base_name("! Alert"), "Alert");
        assert_eq!(extract_base_name("? Question"), "Question");
        assert_eq!(extract_base_name("* Important"), "Important");
    }

    #[test]
    fn test_base_name_with_letter() {
        // Letter + space IS treated as status now
        assert_eq!(extract_base_name("A Tab"), "Tab");
        assert_eq!(extract_base_name("X Marks"), "Marks");
    }

    #[test]
    fn test_base_name_no_space() {
        // No space after first char = no status
        assert_eq!(extract_base_name("Working"), "Working");
        assert_eq!(extract_base_name("MyProject"), "MyProject");
        assert_eq!(extract_base_name("!Alert"), "!Alert");
    }

    #[test]
    fn test_base_name_repeated_status_strips_one() {
        // When status is applied twice, extracting base strips one layer
        assert_eq!(extract_base_name("! ! Tab"), "! Tab");
        assert_eq!(extract_base_name("🤖 🤖 Tab"), "🤖 Tab");
    }

    #[test]
    fn test_base_name_empty() {
        assert_eq!(extract_base_name(""), "");
    }

    #[test]
    fn test_base_name_single_char() {
        assert_eq!(extract_base_name("X"), "X");
        assert_eq!(extract_base_name("🤖"), "🤖");
    }

    // ==================== extract_status tests ====================

    #[test]
    fn test_status_with_emoji() {
        assert_eq!(extract_status("🤖 Working"), "🤖");
        assert_eq!(extract_status("🇺🇸 USA"), "🇺🇸");
        assert_eq!(extract_status("✅ Done"), "✅");
    }

    #[test]
    fn test_status_with_punctuation() {
        assert_eq!(extract_status("! Alert"), "!");
        assert_eq!(extract_status("? Question"), "?");
        assert_eq!(extract_status("* Important"), "*");
    }

    #[test]
    fn test_status_with_letter() {
        // Letter + space IS treated as status now
        assert_eq!(extract_status("A Tab"), "A");
        assert_eq!(extract_status("X Marks"), "X");
    }

    #[test]
    fn test_status_no_space() {
        // No space after first char = no status
        assert_eq!(extract_status("Working"), "");
        assert_eq!(extract_status("MyProject"), "");
        assert_eq!(extract_status("!Alert"), "");
    }

    #[test]
    fn test_status_empty() {
        assert_eq!(extract_status(""), "");
    }
}
