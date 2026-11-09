//! Sanitizing text that is shown in native UI (tray menu, notifications, dialogs).

/// Control (Cc), format (Cf, incl. bidi overrides and zero-width) and line/paragraph separators (Zl, Zp).
pub fn is_unsafe_char(c: char) -> bool {
    c.is_control()
        || matches!(c,
            '\u{00AD}' | '\u{034F}' | '\u{061C}' | '\u{180E}'
            | '\u{115F}'..='\u{1160}' | '\u{3164}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{E0000}'..='\u{E007F}'
            | '\u{200B}'..='\u{200F}'
            | '\u{2028}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{FEFF}' | '\u{FFF9}'..='\u{FFFB}')
}

/// Removes unsafe characters and truncates to `max_chars`.
pub fn display_safe(s: &str, max_chars: usize) -> String {
    s.chars().filter(|c| !is_unsafe_char(*c)).take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_bidi_zero_width_and_separators() {
        assert_eq!(display_safe("job\u{202E}txt.exe\u{200B}\u{2028}\n", 64), "jobtxt.exe");
        assert_eq!(display_safe("가나다", 2), "가나");
    }
}
