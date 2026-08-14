//! Glyph palettes for terminals with and without Nerd Font support.

use crate::config::IconStyle;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IconSet {
    pub(crate) play: &'static str,
    pub(crate) pause: &'static str,
    pub(crate) heart: &'static str,
    pub(crate) shuffle: &'static str,
    pub(crate) repeat_list: &'static str,
    pub(crate) repeat_one: &'static str,
    pub(crate) sequential: &'static str,
    pub(crate) volume: &'static str,
    pub(crate) volume_full: &'static str,
    pub(crate) volume_empty: &'static str,
    pub(crate) search: &'static str,
}

const UNICODE: IconSet = IconSet {
    play: "▶",
    pause: "⏸",
    heart: "♥",
    shuffle: "⇆",
    repeat_list: "↺",
    repeat_one: "↺¹",
    sequential: "→",
    volume: "♪",
    volume_full: "●",
    volume_empty: "○",
    search: "⌕",
};

const NERD: IconSet = IconSet {
    play: "\u{f04b}",
    pause: "\u{f04c}",
    heart: "\u{f02d1}",
    shuffle: "\u{f074}",
    repeat_list: "\u{f01e}",
    repeat_one: "\u{f0458}",
    sequential: "\u{f061}",
    volume: "\u{f028}",
    volume_full: "\u{f111}",
    volume_empty: "\u{f10c}",
    search: "\u{f002}",
};

pub(crate) const fn for_style(style: IconStyle) -> &'static IconSet {
    match style {
        IconStyle::Unicode => &UNICODE,
        IconStyle::Nerd => &NERD,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyphs(icons: &IconSet) -> [&str; 11] {
        [
            icons.play,
            icons.pause,
            icons.heart,
            icons.shuffle,
            icons.repeat_list,
            icons.repeat_one,
            icons.sequential,
            icons.volume,
            icons.volume_full,
            icons.volume_empty,
            icons.search,
        ]
    }

    #[test]
    fn unicode_palette_does_not_require_private_use_glyphs() {
        assert!(glyphs(for_style(IconStyle::Unicode))
            .into_iter()
            .flat_map(str::chars)
            .all(|glyph| !(('\u{e000}'..='\u{f8ff}').contains(&glyph))));
    }

    #[test]
    fn nerd_palette_uses_private_use_glyphs() {
        let is_private_use = |glyph: char| {
            ('\u{e000}'..='\u{f8ff}').contains(&glyph)
                || ('\u{f0000}'..='\u{ffffd}').contains(&glyph)
                || ('\u{100000}'..='\u{10fffd}').contains(&glyph)
        };
        assert!(glyphs(for_style(IconStyle::Nerd))
            .into_iter()
            .all(|icon| icon.chars().all(is_private_use)));
    }

    #[test]
    fn both_palettes_use_the_requested_solid_heart() {
        assert_eq!(for_style(IconStyle::Unicode).heart, "♥");
        assert_eq!(for_style(IconStyle::Nerd).heart, "\u{f02d1}");
    }
}
