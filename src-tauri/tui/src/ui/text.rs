use unicode_width::UnicodeWidthStr;

const MARQUEE_PAUSE_FRAMES: u64 = 9;

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

pub fn marquee_offset(content_width: usize, viewport_width: usize, frame: u64) -> usize {
    let max_offset = content_width.saturating_sub(viewport_width);
    if max_offset == 0 {
        return 0;
    }
    let cycle = MARQUEE_PAUSE_FRAMES * 2 + max_offset as u64;
    let position = frame % cycle;
    if position < MARQUEE_PAUSE_FRAMES {
        0
    } else if position < MARQUEE_PAUSE_FRAMES + max_offset as u64 {
        (position - MARQUEE_PAUSE_FRAMES + 1) as usize
    } else {
        max_offset
    }
}

pub fn display_window(s: &str, width: usize, offset: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let window_end = offset.saturating_add(width);
    let span = ratatui::text::Span::raw(s);
    let mut column = 0_usize;
    let mut output = String::new();
    let mut output_width = 0_usize;
    for grapheme in span.styled_graphemes(ratatui::style::Style::new()) {
        let grapheme_width = display_width(grapheme.symbol);
        let grapheme_end = column.saturating_add(grapheme_width);
        if grapheme_end <= offset {
            column = grapheme_end;
            continue;
        }
        if column >= window_end {
            break;
        }
        let visible_start = column.max(offset);
        let visible_end = grapheme_end.min(window_end);
        let visible_width = visible_end.saturating_sub(visible_start);
        if visible_width == grapheme_width {
            output.push_str(grapheme.symbol);
        } else {
            output.push_str(&" ".repeat(visible_width));
        }
        output_width += visible_width;
        column = grapheme_end;
    }
    output.push_str(&" ".repeat(width.saturating_sub(output_width)));
    output
}

pub fn pad_or_marquee(s: &str, width: usize, selected: bool, frame: u64) -> String {
    if !selected || display_width(s) <= width {
        return pad_display(s, width);
    }
    let offset = marquee_offset(display_width(s), width, frame);
    display_window(s, width, offset)
}

#[cfg(test)]
mod tests {
    use super::{display_width, display_window, marquee_offset, pad_display, pad_or_marquee};

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

    #[test]
    fn marquee_pauses_scrolls_one_column_and_pauses_at_the_end() {
        let offsets = (0..25)
            .map(|frame| marquee_offset(8, 5, frame))
            .collect::<Vec<_>>();

        assert_eq!(&offsets[..9], &[0; 9]);
        assert_eq!(&offsets[9..12], &[1, 2, 3]);
        assert_eq!(&offsets[12..21], &[3; 9]);
        assert_eq!(offsets[21], 0);
        assert_eq!(marquee_offset(5, 5, 99), 0);
    }

    #[test]
    fn marquee_window_moves_through_cjk_one_display_column_at_a_time() {
        assert_eq!(display_window("中文测试", 5, 0), "中文 ");
        assert_eq!(display_window("中文测试", 5, 1), " 文测");
        assert_eq!(display_window("中文测试", 5, 2), "文测 ");
        assert_eq!(display_window("中文测试", 5, 3), " 测试");
        for offset in 0..=3 {
            assert_eq!(display_width(&display_window("中文测试", 5, offset)), 5);
        }
        assert_eq!(display_window("abc", 0, 1), "");
    }

    #[test]
    fn only_the_selected_overflowing_text_uses_the_marquee_window() {
        assert_eq!(pad_or_marquee("abcdefgh", 5, false, 9), "abcd…");
        assert_eq!(pad_or_marquee("abcdefgh", 5, true, 9), "bcdef");
        assert_eq!(pad_or_marquee("abc", 5, true, 99), "abc  ");
    }
}
