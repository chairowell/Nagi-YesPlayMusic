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
    pub(crate) volume_muted: &'static str,
    pub(crate) volume_low: &'static str,
    pub(crate) volume_medium: &'static str,
    pub(crate) volume_high: &'static str,
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
    volume_muted: "✕",
    volume_low: "♩",
    volume_medium: "♪",
    volume_high: "♫",
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
    volume_muted: "\u{f075f}",
    volume_low: "\u{f057f}",
    volume_medium: "\u{f0580}",
    volume_high: "\u{f057e}",
    volume_full: "\u{f111}",
    volume_empty: "\u{f10c}",
    search: "\u{f002}",
};

impl IconSet {
    /// The speaker glyph tracks the actual level: muted, below half, above.
    pub(crate) fn volume_at(&self, volume: f32) -> &'static str {
        if volume <= f32::EPSILON {
            self.volume_muted
        } else if volume < 1.0 / 3.0 {
            self.volume_low
        } else if volume < 2.0 / 3.0 {
            self.volume_medium
        } else {
            self.volume_high
        }
    }
}

pub(crate) const fn for_style(style: IconStyle) -> &'static IconSet {
    match style {
        IconStyle::Unicode => &UNICODE,
        IconStyle::Nerd => &NERD,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyphs(icons: &IconSet) -> [&str; 14] {
        [
            icons.play,
            icons.pause,
            icons.heart,
            icons.shuffle,
            icons.repeat_list,
            icons.repeat_one,
            icons.sequential,
            icons.volume_muted,
            icons.volume_low,
            icons.volume_medium,
            icons.volume_high,
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
    fn speaker_glyph_tracks_the_volume_level() {
        let icons = for_style(IconStyle::Nerd);
        assert_eq!(icons.volume_at(0.0), icons.volume_muted);
        assert_eq!(icons.volume_at(0.2), icons.volume_low);
        assert_eq!(icons.volume_at(0.5), icons.volume_medium);
        assert_eq!(icons.volume_at(0.8), icons.volume_high);
    }

    #[test]
    fn both_palettes_use_the_requested_solid_heart() {
        assert_eq!(for_style(IconStyle::Unicode).heart, "♥");
        assert_eq!(for_style(IconStyle::Nerd).heart, "\u{f02d1}");
    }
}
