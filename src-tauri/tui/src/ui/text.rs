use unicode_width::UnicodeWidthStr;

pub fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

pub fn pad_display(s: &str, width: usize) -> String {
    let current_width = display_width(s);
    if current_width <= width {
        return format!("{s}{}", " ".repeat(width - current_width));
    }
    if width == 0 {
        return String::new();
    }

    let prefix_limit = width - 1;
    let mut prefix_end = 0;
    for (index, ch) in s.char_indices() {
        let next_end = index + ch.len_utf8();
        if display_width(&s[..next_end]) > prefix_limit {
            break;
        }
        prefix_end = next_end;
    }

    let prefix = &s[..prefix_end];
    let gap = prefix_limit - display_width(prefix);
    format!("{prefix}{}…", " ".repeat(gap))
}

#[cfg(test)]
mod tests {
    use super::{display_width, pad_display};

    #[test]
    fn pads_and_truncates_ascii() {
        assert_eq!(display_width("track"), 5);
        assert_eq!(pad_display("track", 7), "track  ");
        assert_eq!(pad_display("tracks", 5), "trac…");
    }

    #[test]
    fn counts_chinese_characters_as_two_columns() {
        assert_eq!(display_width("中文"), 4);
        assert_eq!(pad_display("中文", 6), "中文  ");
    }

    #[test]
    fn pads_mixed_width_text() {
        assert_eq!(display_width("A中B"), 4);
        assert_eq!(pad_display("A中B", 6), "A中B  ");
    }

    #[test]
    fn leaves_exact_boundary_unchanged() {
        assert_eq!(pad_display("歌名A", 5), "歌名A");
    }

    #[test]
    fn truncates_cjk_by_display_width() {
        assert_eq!(pad_display("中文测试", 5), "中文…");
        assert_eq!(pad_display("アルバム", 5), "アル…");
    }

    #[test]
    fn keeps_the_requested_width_when_a_wide_character_does_not_fit() {
        assert_eq!(pad_display("中文", 4), "中文");
        assert_eq!(pad_display("中文", 2), " …");
        assert_eq!(pad_display("中文", 1), "…");
        assert_eq!(pad_display("中文", 0), "");
    }
}
