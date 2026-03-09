use unicode_segmentation::UnicodeSegmentation;

/// Invisible separator (U+2063) used as unambiguous marker for status-block prefix.
/// Format: MARKER + STATUS + SPACE + base_name
pub const MARKER: char = '\u{2063}';

/// Extract the first Unicode grapheme cluster from input.
/// Returns empty string for empty input.
pub fn first_grapheme(input: &str) -> &str {
    input.graphemes(true).next().unwrap_or("")
}

/// Parse a tab name into (status, base_name) if it has a valid status-block.
/// Returns None if no valid MARKER-prefixed status-block is found.
fn parse_status_block(name: &str) -> Option<(&str, &str)> {
    let rest = name.strip_prefix(MARKER)?;
    let mut graphemes = rest.graphemes(true);
    let status = graphemes.next()?;
    let after_status = graphemes.as_str();
    let base = after_status.strip_prefix(' ')?;
    Some((status, base))
}

/// Returns the STATUS portion if a valid status-block is present, empty string otherwise.
pub fn get_status(current_name: &str) -> &str {
    match parse_status_block(current_name) {
        Some((status, _)) => status,
        None => "",
    }
}

/// Returns the base_name if a valid status-block is present, else the full name.
pub fn get_name(current_name: &str) -> &str {
    match parse_status_block(current_name) {
        Some((_, base)) => base,
        None => current_name,
    }
}

/// Sets or replaces the status-block. Takes first grapheme cluster from emoji.
/// If emoji is empty, returns the name unchanged (use clear_status to remove).
pub fn set_status(current_name: &str, emoji: &str) -> String {
    let grapheme = first_grapheme(emoji);
    if grapheme.is_empty() {
        return current_name.to_string();
    }
    let base = get_name(current_name);
    format!("{}{} {}", MARKER, grapheme, base)
}

/// Removes the status-block if present, returning the base_name.
/// If no status-block, returns the name unchanged.
pub fn clear_status(current_name: &str) -> String {
    get_name(current_name).to_string()
}

/// Preserves existing status-block (if any) and replaces the base_name.
pub fn set_name(current_name: &str, new_name: &str) -> String {
    match parse_status_block(current_name) {
        Some((status, _)) => format!("{}{} {}", MARKER, status, new_name),
        None => new_name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== MARKER constant ====================

    #[test]
    fn test_marker_is_u2063() {
        assert_eq!(MARKER, '\u{2063}');
        assert_eq!(MARKER.len_utf8(), 3); // U+2063 = 3 bytes in UTF-8
    }

    // ==================== first_grapheme ====================

    #[test]
    fn test_first_grapheme_emoji() {
        assert_eq!(first_grapheme("🤖hello"), "🤖");
        assert_eq!(first_grapheme("✅ done"), "✅");
    }

    #[test]
    fn test_first_grapheme_flag_emoji() {
        // 🇺🇸 = U+1F1FA U+1F1F8, one grapheme cluster
        assert_eq!(first_grapheme("🇺🇸 USA"), "🇺🇸");
    }

    #[test]
    fn test_first_grapheme_skin_tone() {
        // 👋🏻 = U+1F44B U+1F3FB, one grapheme cluster
        assert_eq!(first_grapheme("👋🏻 hi"), "👋🏻");
    }

    #[test]
    fn test_first_grapheme_ascii() {
        assert_eq!(first_grapheme("hello"), "h");
        assert_eq!(first_grapheme("!alert"), "!");
    }

    #[test]
    fn test_first_grapheme_empty() {
        assert_eq!(first_grapheme(""), "");
    }

    #[test]
    fn test_first_grapheme_single_char() {
        assert_eq!(first_grapheme("X"), "X");
        assert_eq!(first_grapheme("🤖"), "🤖");
    }

    // ==================== get_status ====================

    #[test]
    fn test_get_status_with_marker() {
        let name = format!("{}🤖 Working", MARKER);
        assert_eq!(get_status(&name), "🤖");
    }

    #[test]
    fn test_get_status_ascii() {
        let name = format!("{}! Alert", MARKER);
        assert_eq!(get_status(&name), "!");
    }

    #[test]
    fn test_get_status_flag_emoji() {
        let name = format!("{}🇺🇸 USA", MARKER);
        assert_eq!(get_status(&name), "🇺🇸");
    }

    #[test]
    fn test_get_status_skin_tone() {
        let name = format!("{}👋🏻 Hi", MARKER);
        assert_eq!(get_status(&name), "👋🏻");
    }

    #[test]
    fn test_get_status_no_marker() {
        // Old-style: emoji + space but no MARKER → not detected as status
        assert_eq!(get_status("🤖 Working"), "");
        assert_eq!(get_status("! Alert"), "");
    }

    #[test]
    fn test_get_status_plain_name() {
        assert_eq!(get_status("Working"), "");
        assert_eq!(get_status("My Tab"), "");
    }

    #[test]
    fn test_get_status_empty() {
        assert_eq!(get_status(""), "");
    }

    #[test]
    fn test_get_status_marker_only() {
        let name = format!("{}", MARKER);
        assert_eq!(get_status(&name), "");
    }

    #[test]
    fn test_get_status_marker_no_space() {
        // Malformed: marker + grapheme but no space after
        let name = format!("{}🤖Working", MARKER);
        assert_eq!(get_status(&name), "");
    }

    // ==================== get_name ====================

    #[test]
    fn test_get_name_with_marker() {
        let name = format!("{}🤖 Working", MARKER);
        assert_eq!(get_name(&name), "Working");
    }

    #[test]
    fn test_get_name_ascii_status() {
        let name = format!("{}! Alert", MARKER);
        assert_eq!(get_name(&name), "Alert");
    }

    #[test]
    fn test_get_name_no_marker() {
        assert_eq!(get_name("🤖 Working"), "🤖 Working");
        assert_eq!(get_name("Working"), "Working");
    }

    #[test]
    fn test_get_name_empty() {
        assert_eq!(get_name(""), "");
    }

    #[test]
    fn test_get_name_marker_only() {
        let name = format!("{}", MARKER);
        assert_eq!(get_name(&name), &format!("{}", MARKER));
    }

    #[test]
    fn test_get_name_preserves_spaces_in_base() {
        let name = format!("{}🤖 My Long Tab Name", MARKER);
        assert_eq!(get_name(&name), "My Long Tab Name");
    }

    // ==================== set_status ====================

    #[test]
    fn test_set_status_plain_name() {
        let result = set_status("Working", "🤖");
        assert_eq!(result, format!("{}🤖 Working", MARKER));
    }

    #[test]
    fn test_set_status_replaces_existing() {
        let name = format!("{}🤖 Working", MARKER);
        let result = set_status(&name, "✅");
        assert_eq!(result, format!("{}✅ Working", MARKER));
    }

    #[test]
    fn test_set_status_idempotent() {
        let name = format!("{}🤖 Working", MARKER);
        let result = set_status(&name, "🤖");
        assert_eq!(result, name);
    }

    #[test]
    fn test_set_status_double_application_no_stacking() {
        let result1 = set_status("Tab", "🤖");
        let result2 = set_status(&result1, "✅");
        assert_eq!(get_name(&result2), "Tab");
        assert_eq!(get_status(&result2), "✅");
    }

    #[test]
    fn test_set_status_flag_emoji() {
        let result = set_status("Tab", "🇺🇸");
        assert_eq!(get_status(&result), "🇺🇸");
        assert_eq!(get_name(&result), "Tab");
    }

    #[test]
    fn test_set_status_skin_tone() {
        let result = set_status("Tab", "👋🏻");
        assert_eq!(get_status(&result), "👋🏻");
        assert_eq!(get_name(&result), "Tab");
    }

    #[test]
    fn test_set_status_takes_first_grapheme_only() {
        let result = set_status("Tab", "🤖✅🎉");
        assert_eq!(get_status(&result), "🤖");
        assert_eq!(get_name(&result), "Tab");
    }

    #[test]
    fn test_set_status_ascii() {
        let result = set_status("Tab", "!");
        assert_eq!(result, format!("{}! Tab", MARKER));
    }

    #[test]
    fn test_set_status_empty_emoji_noop() {
        assert_eq!(set_status("Tab", ""), "Tab");
    }

    #[test]
    fn test_set_status_on_old_style_name() {
        // Old-style "🤖 Working" has no marker, so entire string is base_name
        let result = set_status("🤖 Working", "✅");
        assert_eq!(get_status(&result), "✅");
        assert_eq!(get_name(&result), "🤖 Working");
    }

    #[test]
    fn test_set_status_empty_name() {
        let result = set_status("", "🤖");
        assert_eq!(result, format!("{}🤖 ", MARKER));
        assert_eq!(get_status(&result), "🤖");
        assert_eq!(get_name(&result), "");
    }

    // ==================== clear_status ====================

    #[test]
    fn test_clear_status_with_marker() {
        let name = format!("{}🤖 Working", MARKER);
        assert_eq!(clear_status(&name), "Working");
    }

    #[test]
    fn test_clear_status_no_marker() {
        assert_eq!(clear_status("Working"), "Working");
        assert_eq!(clear_status("🤖 Working"), "🤖 Working");
    }

    #[test]
    fn test_clear_status_empty() {
        assert_eq!(clear_status(""), "");
    }

    #[test]
    fn test_clear_status_preserves_base_byte_for_byte() {
        let base = "  spaced  name  ";
        let name = format!("{}🤖 {}", MARKER, base);
        assert_eq!(clear_status(&name), base);
    }

    // ==================== set_name ====================

    #[test]
    fn test_set_name_with_status() {
        let name = format!("{}🤖 Working", MARKER);
        let result = set_name(&name, "Coding");
        assert_eq!(result, format!("{}🤖 Coding", MARKER));
    }

    #[test]
    fn test_set_name_without_status() {
        assert_eq!(set_name("Working", "Coding"), "Coding");
    }

    #[test]
    fn test_set_name_preserves_status() {
        let name = format!("{}✅ Done", MARKER);
        let result = set_name(&name, "Finished");
        assert_eq!(get_status(&result), "✅");
        assert_eq!(get_name(&result), "Finished");
    }

    #[test]
    fn test_set_name_empty_new_name() {
        let name = format!("{}🤖 Working", MARKER);
        let result = set_name(&name, "");
        assert_eq!(get_status(&result), "🤖");
        assert_eq!(get_name(&result), "");
    }

    // ==================== Malformed marker prefix ====================

    #[test]
    fn test_malformed_marker_no_grapheme_after() {
        // Just marker char, nothing after
        let name = format!("{}", MARKER);
        assert_eq!(get_status(&name), "");
        assert_eq!(get_name(&name), &name);
    }

    #[test]
    fn test_malformed_marker_grapheme_no_space() {
        let name = format!("{}🤖Tab", MARKER);
        assert_eq!(get_status(&name), "");
        assert_eq!(get_name(&name), &name);
    }

    #[test]
    fn test_malformed_marker_space_only() {
        // MARKER + space — the space IS the grapheme, but then no space follows
        // parse: grapheme=" ", rest="", strip_prefix(' ') on "" fails
        // Actually: grapheme=" " and rest="", strip_prefix(' ') on "" → None
        let name = format!("{} ", MARKER);
        assert_eq!(get_status(&name), "");
        assert_eq!(get_name(&name), &name);
    }

    // ==================== Round-trip consistency ====================

    #[test]
    fn test_round_trip_set_get() {
        let original = "My Tab";
        let with_status = set_status(original, "🎉");
        assert_eq!(get_status(&with_status), "🎉");
        assert_eq!(get_name(&with_status), original);
        let cleared = clear_status(&with_status);
        assert_eq!(cleared, original);
    }

    #[test]
    fn test_round_trip_set_name_get_name() {
        let with_status = set_status("Tab1", "🤖");
        let renamed = set_name(&with_status, "Tab2");
        assert_eq!(get_name(&renamed), "Tab2");
        assert_eq!(get_status(&renamed), "🤖");
    }
}
