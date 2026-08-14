//! Theme = one palette plus role mappings shared by the UI and covers.

use std::fs;
use std::path::{Path, PathBuf};

use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;

type Rgb = (u8, u8, u8);

/// Selected rows lift twelve percent from the foreground without becoming a chip.
const SELECTION_FOREGROUND_PERCENT: u16 = 12;
const COLOR_PERCENT_SCALE: u16 = 100;

pub const BUILTIN_NAMES: &[&str] = &[
    "db16",
    "pico8",
    "gameboy",
    "everforest",
    "tokyo-night",
    "tokyo-night-storm",
    "one-dark",
    "dracula",
    "one-dark-pro",
    "synthwave84",
    "laserwave",
    "fairyfloss",
    "ultraviolence",
    "transparent",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub dim: Color,
    pub faint: Color,
    pub accent: Color,
    pub accent2: Color,
    pub sel: Color,
    /// Full palette the pixel cover quantizes against.
    pub palette: &'static [Rgb],
}

pub const DB16: &[Rgb] = &[
    (0x14, 0x0c, 0x1c),
    (0x44, 0x24, 0x34),
    (0x30, 0x34, 0x6d),
    (0x4e, 0x4a, 0x4e),
    (0x85, 0x4c, 0x30),
    (0x34, 0x65, 0x24),
    (0xd0, 0x46, 0x48),
    (0x75, 0x71, 0x61),
    (0x59, 0x7d, 0xce),
    (0xd2, 0x7d, 0x2c),
    (0x85, 0x95, 0xa1),
    (0x6d, 0xaa, 0x2c),
    (0xd2, 0xaa, 0x99),
    (0x6d, 0xc2, 0xca),
    (0xda, 0xd4, 0x5e),
    (0xde, 0xee, 0xd6),
];

pub const PICO8: &[Rgb] = &[
    (0x00, 0x00, 0x00),
    (0x1d, 0x2b, 0x53),
    (0x7e, 0x25, 0x53),
    (0x00, 0x87, 0x51),
    (0xab, 0x52, 0x36),
    (0x5f, 0x57, 0x4f),
    (0xc2, 0xc3, 0xc7),
    (0xff, 0xf1, 0xe8),
    (0xff, 0x00, 0x4d),
    (0xff, 0xa3, 0x00),
    (0xff, 0xec, 0x27),
    (0x00, 0xe4, 0x36),
    (0x29, 0xad, 0xff),
    (0x83, 0x76, 0x9c),
    (0xff, 0x77, 0xa8),
    (0xff, 0xcc, 0xaa),
];

pub const GAMEBOY: &[Rgb] = &[
    (0x0f, 0x38, 0x0f),
    (0x30, 0x62, 0x30),
    (0x8b, 0xac, 0x0f),
    (0x9b, 0xbc, 0x0f),
];

/// Everforest dark, medium contrast.
pub const EVERFOREST: &[Rgb] = &[
    (0x2d, 0x35, 0x3b),
    (0x23, 0x2a, 0x2e),
    (0x34, 0x3f, 0x44),
    (0x3d, 0x48, 0x4d),
    (0x47, 0x52, 0x58),
    (0x4f, 0x58, 0x5e),
    (0x56, 0x63, 0x5f),
    (0xd3, 0xc6, 0xaa),
    (0x7a, 0x84, 0x78),
    (0x85, 0x92, 0x89),
    (0x9d, 0xa9, 0xa0),
    (0xe6, 0x7e, 0x80),
    (0xdb, 0xbc, 0x7f),
    (0xa7, 0xc0, 0x80),
    (0x83, 0xc0, 0x92),
    (0x7f, 0xbb, 0xb3),
    (0xd6, 0x99, 0xb6),
];

/// Tokyo Night's official night palette.
pub const TOKYO_NIGHT: &[Rgb] = &[
    (0x1a, 0x1b, 0x26),
    (0x16, 0x16, 0x1e),
    (0x29, 0x2e, 0x42),
    (0x41, 0x48, 0x68),
    (0x56, 0x5f, 0x89),
    (0x73, 0x7a, 0xa2),
    (0xa9, 0xb1, 0xd6),
    (0xc0, 0xca, 0xf5),
    (0x7a, 0xa2, 0xf7),
    (0x2a, 0xc3, 0xde),
    (0x7d, 0xcf, 0xff),
    (0x9e, 0xce, 0x6a),
    (0xe0, 0xaf, 0x68),
    (0xff, 0x9e, 0x64),
    (0xf7, 0x76, 0x8e),
    (0xbb, 0x9a, 0xf7),
];

/// Tokyo Night's official storm palette.
pub const TOKYO_NIGHT_STORM: &[Rgb] = &[
    (0x24, 0x28, 0x3b),
    (0x1f, 0x23, 0x35),
    (0x29, 0x2e, 0x42),
    (0x41, 0x48, 0x68),
    (0x56, 0x5f, 0x89),
    (0x73, 0x7a, 0xa2),
    (0xa9, 0xb1, 0xd6),
    (0xc0, 0xca, 0xf5),
    (0x7a, 0xa2, 0xf7),
    (0x2a, 0xc3, 0xde),
    (0x7d, 0xcf, 0xff),
    (0x9e, 0xce, 0x6a),
    (0xe0, 0xaf, 0x68),
    (0xff, 0x9e, 0x64),
    (0xf7, 0x76, 0x8e),
    (0xbb, 0x9a, 0xf7),
];

/// Atom One Dark's reference palette.
pub const ONE_DARK: &[Rgb] = &[
    (0x28, 0x2c, 0x34),
    (0x2c, 0x32, 0x3c),
    (0x3b, 0x40, 0x48),
    (0x3e, 0x44, 0x52),
    (0x4b, 0x52, 0x63),
    (0x5c, 0x63, 0x70),
    (0x82, 0x89, 0x97),
    (0xab, 0xb2, 0xbf),
    (0xe0, 0x6c, 0x75),
    (0xbe, 0x50, 0x46),
    (0x98, 0xc3, 0x79),
    (0xe5, 0xc0, 0x7b),
    (0xd1, 0x9a, 0x66),
    (0x61, 0xaf, 0xef),
    (0xc6, 0x78, 0xdd),
    (0x56, 0xb6, 0xc2),
];

pub const ONE_DARK_PRO: &[Rgb] = &[
    (0x28, 0x2c, 0x34),
    (0xab, 0xb2, 0xbf),
    (0x5c, 0x63, 0x70),
    (0x3e, 0x44, 0x51),
    (0x61, 0xaf, 0xef),
    (0xe0, 0x6c, 0x75),
];

pub const DRACULA: &[Rgb] = &[
    (0x28, 0x2a, 0x36),
    (0xf8, 0xf8, 0xf2),
    (0x62, 0x72, 0xa4),
    (0x44, 0x47, 0x5a),
    (0xbd, 0x93, 0xf9),
    (0xff, 0x79, 0xc6),
];

pub const SYNTHWAVE84: &[Rgb] = &[
    (0x26, 0x23, 0x35),
    (0xf0, 0xef, 0xf1),
    (0x49, 0x54, 0x95),
    (0x34, 0x29, 0x4f),
    (0xff, 0x7e, 0xdb),
    (0x36, 0xf9, 0xf6),
];

pub const LASERWAVE: &[Rgb] = &[
    (0x27, 0x21, 0x2e),
    (0xf2, 0xec, 0xf7),
    (0x91, 0x88, 0x9b),
    (0x71, 0x63, 0x85),
    (0xeb, 0x64, 0xb9),
    (0x74, 0xdf, 0xc4),
];

pub const FAIRYFLOSS: &[Rgb] = &[
    (0x5a, 0x54, 0x75),
    (0xf8, 0xf8, 0xf2),
    (0xa8, 0xa1, 0xc7),
    (0x80, 0x77, 0xa8),
    (0xc5, 0xa3, 0xff),
    (0xff, 0xb8, 0xd1),
];

pub const ULTRAVIOLENCE: &[Rgb] = &[
    (0x19, 0x11, 0x14),
    (0xe9, 0xdf, 0xd2),
    (0xa0, 0x8d, 0x80),
    (0x55, 0x45, 0x4a),
    (0xc9, 0xa7, 0x6f),
    (0xc9, 0x8a, 0x8e),
];

#[derive(Clone, Copy)]
struct RoleIndices {
    bg: usize,
    fg: usize,
    dim: usize,
    faint: usize,
    accent: usize,
    accent2: usize,
    sel: usize,
}

#[derive(Debug, Deserialize)]
struct ThemeFile {
    palette: Vec<String>,
    #[serde(default)]
    roles: FileRoles,
}

#[derive(Debug, Default, Deserialize)]
struct FileRoles {
    bg: Option<usize>,
    fg: Option<usize>,
    dim: Option<usize>,
    faint: Option<usize>,
    accent: Option<usize>,
    accent2: Option<usize>,
    sel: Option<usize>,
}

impl Theme {
    pub const fn selection_fg(&self) -> Color {
        if matches!(self.bg, Color::Reset) {
            Color::Black
        } else {
            self.bg
        }
    }

    /// Resolve the background only for color arithmetic. Drawing still uses
    /// `self.bg` so the transparent theme keeps the terminal's native alpha.
    pub(crate) const fn resolved_background(
        &self,
        terminal_background: Option<Color>,
    ) -> Option<Color> {
        match self.bg {
            Color::Rgb(..) => Some(self.bg),
            Color::Reset => match terminal_background {
                Some(color @ Color::Rgb(..)) => Some(color),
                _ => None,
            },
            _ => None,
        }
    }

    /// Selected rows use a subtle RGB lift when possible and terminal-native
    /// reversal when OSC 11 could not reveal a transparent background.
    pub(crate) fn selection_style(&self, terminal_background: Option<Color>) -> Style {
        let Some(Color::Rgb(bg_r, bg_g, bg_b)) = self.resolved_background(terminal_background)
        else {
            return Style::new().add_modifier(Modifier::REVERSED);
        };
        let (fg_r, fg_g, fg_b) = match self.fg {
            Color::Rgb(red, green, blue) => (red, green, blue),
            _ if contrasting_foreground_is_light(bg_r, bg_g, bg_b) => (255, 255, 255),
            _ => (0, 0, 0),
        };
        Style::new().bg(Color::Rgb(
            selection_channel(bg_r, fg_r),
            selection_channel(bg_g, fg_g),
            selection_channel(bg_b, fg_b),
        ))
    }

    pub fn by_name(name: &str) -> Self {
        if let Some(theme) = Self::builtin(name) {
            return theme;
        }
        let Some(home) = dirs::home_dir() else {
            return Self::db16();
        };
        Self::by_name_in_dir(name, &themes_dir(&home))
    }

    pub fn db16() -> Self {
        Self::from_palette(
            DB16,
            RoleIndices {
                bg: 0,
                fg: 15,
                dim: 10,
                faint: 7,
                accent: 14,
                accent2: 9,
                sel: 12,
            },
        )
    }

    fn pico8() -> Self {
        Self::from_palette(
            PICO8,
            RoleIndices {
                bg: 0,
                fg: 7,
                dim: 6,
                faint: 5,
                accent: 10,
                accent2: 14,
                sel: 9,
            },
        )
    }

    fn gameboy() -> Self {
        Self::from_palette(
            GAMEBOY,
            RoleIndices {
                bg: 0,
                fg: 3,
                dim: 2,
                faint: 1,
                accent: 3,
                accent2: 2,
                sel: 2,
            },
        )
    }

    fn everforest() -> Self {
        Self::from_palette(
            EVERFOREST,
            RoleIndices {
                bg: 0,
                fg: 7,
                dim: 9,
                faint: 4,
                accent: 13,
                accent2: 11,
                sel: 13,
            },
        )
    }

    fn tokyo_night() -> Self {
        Self::from_palette(
            TOKYO_NIGHT,
            RoleIndices {
                bg: 0,
                fg: 7,
                dim: 4,
                faint: 3,
                accent: 8,
                accent2: 14,
                sel: 8,
            },
        )
    }

    fn tokyo_night_storm() -> Self {
        Self::from_palette(
            TOKYO_NIGHT_STORM,
            RoleIndices {
                bg: 0,
                fg: 7,
                dim: 5,
                faint: 4,
                accent: 8,
                accent2: 15,
                sel: 8,
            },
        )
    }

    fn one_dark() -> Self {
        Self::from_palette(
            ONE_DARK,
            RoleIndices {
                bg: 0,
                fg: 7,
                dim: 6,
                faint: 5,
                accent: 13,
                accent2: 14,
                sel: 13,
            },
        )
    }

    fn one_dark_pro() -> Self {
        Self::six_role_palette(ONE_DARK_PRO)
    }

    fn dracula() -> Self {
        Self::six_role_palette(DRACULA)
    }

    fn synthwave84() -> Self {
        Self::six_role_palette(SYNTHWAVE84)
    }

    fn laserwave() -> Self {
        Self::six_role_palette(LASERWAVE)
    }

    fn fairyfloss() -> Self {
        Self::six_role_palette(FAIRYFLOSS)
    }

    fn ultraviolence() -> Self {
        Self::six_role_palette(ULTRAVIOLENCE)
    }

    fn builtin(name: &str) -> Option<Self> {
        match name {
            "db16" => Some(Self::db16()),
            "pico8" => Some(Self::pico8()),
            "gameboy" => Some(Self::gameboy()),
            "everforest" => Some(Self::everforest()),
            "tokyo-night" => Some(Self::tokyo_night()),
            "tokyo-night-storm" => Some(Self::tokyo_night_storm()),
            "one-dark" => Some(Self::one_dark()),
            "dracula" => Some(Self::dracula()),
            "one-dark-pro" => Some(Self::one_dark_pro()),
            "synthwave84" => Some(Self::synthwave84()),
            "laserwave" => Some(Self::laserwave()),
            "fairyfloss" => Some(Self::fairyfloss()),
            "ultraviolence" => Some(Self::ultraviolence()),
            "transparent" => Some(Self::transparent()),
            _ => None,
        }
    }

    /// Terminal-native theme: Reset = the terminal's own default colors,
    /// the TUI equivalent of a transparent background. Accents and the
    /// cover palette stay DB16 so pixel art keeps its contrast; best on
    /// dark terminals (light-terminal users should write a light theme
    /// file instead).
    fn transparent() -> Self {
        Self {
            bg: Color::Reset,
            fg: Color::Reset,
            dim: rgb(DB16[10]),
            faint: rgb(DB16[7]),
            accent: rgb(DB16[14]),
            accent2: rgb(DB16[9]),
            sel: rgb(DB16[12]),
            palette: DB16,
        }
    }

    fn by_name_in_dir(name: &str, directory: &Path) -> Self {
        if let Some(theme) = Self::builtin(name) {
            return theme;
        }
        load_theme(&theme_path(directory, name)).unwrap_or_else(Self::db16)
    }

    fn from_palette(palette: &'static [Rgb], roles: RoleIndices) -> Self {
        Self {
            bg: rgb(palette[roles.bg]),
            fg: rgb(palette[roles.fg]),
            dim: rgb(palette[roles.dim]),
            faint: rgb(palette[roles.faint]),
            accent: rgb(palette[roles.accent]),
            accent2: rgb(palette[roles.accent2]),
            sel: rgb(palette[roles.sel]),
            palette,
        }
    }

    fn six_role_palette(palette: &'static [Rgb]) -> Self {
        Self::from_palette(
            palette,
            RoleIndices {
                bg: 0,
                fg: 1,
                dim: 2,
                faint: 3,
                accent: 4,
                accent2: 5,
                sel: 4,
            },
        )
    }
}

fn themes_dir(home: &Path) -> PathBuf {
    home.join(".config").join("ypm").join("themes")
}

fn theme_path(directory: &Path, name: &str) -> PathBuf {
    directory.join(format!("{name}.toml"))
}

fn load_theme(path: &Path) -> Option<Theme> {
    let source = fs::read_to_string(path).ok()?;
    let file: ThemeFile = toml::from_str(&source).ok()?;
    if !(2..=64).contains(&file.palette.len()) {
        return None;
    }
    let palette = file
        .palette
        .iter()
        .map(|color| parse_hex_color(color))
        .collect::<Option<Vec<_>>>()?;
    let roles = resolve_roles(file.roles, palette.len())?;
    // Cover-fetch threads require 'static; startup loads one tiny palette, so this leak is bounded.
    let palette: &'static [Rgb] = Box::leak(palette.into_boxed_slice());
    Some(Theme::from_palette(palette, roles))
}

fn resolve_roles(file: FileRoles, palette_len: usize) -> Option<RoleIndices> {
    let defaults = default_roles(palette_len);
    let roles = RoleIndices {
        bg: file.bg.unwrap_or(defaults.bg),
        fg: file.fg.unwrap_or(defaults.fg),
        dim: file.dim.unwrap_or(defaults.dim),
        faint: file.faint.unwrap_or(defaults.faint),
        accent: file.accent.unwrap_or(defaults.accent),
        accent2: file.accent2.unwrap_or(defaults.accent2),
        sel: file.sel.unwrap_or(defaults.sel),
    };
    let indices = [
        roles.bg,
        roles.fg,
        roles.dim,
        roles.faint,
        roles.accent,
        roles.accent2,
        roles.sel,
    ];
    indices
        .into_iter()
        .all(|index| index < palette_len)
        .then_some(roles)
}

fn default_roles(palette_len: usize) -> RoleIndices {
    let last = palette_len - 1;
    RoleIndices {
        bg: 0,
        fg: last,
        // Scale DB16's role positions to palettes of any supported size.
        dim: scaled_role(last, 2, 3),
        faint: scaled_role(last, 1, 2),
        accent: last.saturating_sub(1).max(1),
        accent2: scaled_role(last, 3, 5),
        sel: scaled_role(last, 4, 5),
    }
}

fn scaled_role(last: usize, numerator: usize, denominator: usize) -> usize {
    (last * numerator / denominator).max(1)
}

fn parse_hex_color(value: &str) -> Option<Rgb> {
    let bytes = value.as_bytes();
    if bytes.len() != 7 || bytes[0] != b'#' {
        return None;
    }
    Some((
        hex_byte(bytes[1], bytes[2])?,
        hex_byte(bytes[3], bytes[4])?,
        hex_byte(bytes[5], bytes[6])?,
    ))
}

fn hex_byte(high: u8, low: u8) -> Option<u8> {
    Some(hex_nibble(high)? * 16 + hex_nibble(low)?)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn rgb((red, green, blue): Rgb) -> Color {
    Color::Rgb(red, green, blue)
}

const fn selection_channel(background: u8, foreground: u8) -> u8 {
    let foreground_weight = foreground as u16 * SELECTION_FOREGROUND_PERCENT;
    let background_weight =
        background as u16 * (COLOR_PERCENT_SCALE - SELECTION_FOREGROUND_PERCENT);
    ((background_weight + foreground_weight + COLOR_PERCENT_SCALE / 2) / COLOR_PERCENT_SCALE) as u8
}

const fn contrasting_foreground_is_light(red: u8, green: u8, blue: u8) -> bool {
    let luminance = red as u32 * 299 + green as u32 * 587 + blue as u32 * 114;
    luminance < 128_000
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use ratatui::style::{Color, Modifier};
    use tempfile::tempdir;

    use super::{parse_hex_color, Theme, BUILTIN_NAMES, DB16};

    #[test]
    fn builtins_use_their_documented_background_and_accent() {
        let directory = tempdir().unwrap();

        let db16 = Theme::by_name_in_dir("db16", directory.path());
        assert_eq!(db16.bg, Color::Rgb(0x14, 0x0c, 0x1c));
        assert_eq!(db16.accent, Color::Rgb(0xda, 0xd4, 0x5e));

        let pico8 = Theme::by_name_in_dir("pico8", directory.path());
        assert_eq!(pico8.bg, Color::Rgb(0x00, 0x00, 0x00));
        assert_eq!(pico8.accent, Color::Rgb(0xff, 0xec, 0x27));

        let gameboy = Theme::by_name_in_dir("gameboy", directory.path());
        assert_eq!(gameboy.bg, Color::Rgb(0x0f, 0x38, 0x0f));
        assert_eq!(gameboy.accent, Color::Rgb(0x9b, 0xbc, 0x0f));

        let everforest = Theme::by_name_in_dir("everforest", directory.path());
        assert_eq!(everforest.bg, Color::Rgb(0x2d, 0x35, 0x3b));
        assert_eq!(everforest.accent, Color::Rgb(0xa7, 0xc0, 0x80));

        let storm = Theme::by_name_in_dir("tokyo-night-storm", directory.path());
        assert_eq!(storm.bg, Color::Rgb(0x24, 0x28, 0x3b));
        assert_eq!(storm.accent, Color::Rgb(0x7a, 0xa2, 0xf7));

        let one_dark = Theme::by_name_in_dir("one-dark", directory.path());
        assert_eq!(one_dark.bg, Color::Rgb(0x28, 0x2c, 0x34));
        assert_eq!(one_dark.accent, Color::Rgb(0x61, 0xaf, 0xef));
    }

    #[test]
    fn requested_builtin_themes_have_exact_role_colors() {
        let directory = tempdir().unwrap();
        let expected = [
            (
                "tokyo-night",
                (
                    Color::Rgb(0x1a, 0x1b, 0x26),
                    Color::Rgb(0xc0, 0xca, 0xf5),
                    Color::Rgb(0x56, 0x5f, 0x89),
                    Color::Rgb(0x41, 0x48, 0x68),
                    Color::Rgb(0x7a, 0xa2, 0xf7),
                    Color::Rgb(0xf7, 0x76, 0x8e),
                ),
            ),
            (
                "dracula",
                (
                    Color::Rgb(0x28, 0x2a, 0x36),
                    Color::Rgb(0xf8, 0xf8, 0xf2),
                    Color::Rgb(0x62, 0x72, 0xa4),
                    Color::Rgb(0x44, 0x47, 0x5a),
                    Color::Rgb(0xbd, 0x93, 0xf9),
                    Color::Rgb(0xff, 0x79, 0xc6),
                ),
            ),
            (
                "one-dark-pro",
                (
                    Color::Rgb(0x28, 0x2c, 0x34),
                    Color::Rgb(0xab, 0xb2, 0xbf),
                    Color::Rgb(0x5c, 0x63, 0x70),
                    Color::Rgb(0x3e, 0x44, 0x51),
                    Color::Rgb(0x61, 0xaf, 0xef),
                    Color::Rgb(0xe0, 0x6c, 0x75),
                ),
            ),
            (
                "everforest",
                (
                    Color::Rgb(0x2d, 0x35, 0x3b),
                    Color::Rgb(0xd3, 0xc6, 0xaa),
                    Color::Rgb(0x85, 0x92, 0x89),
                    Color::Rgb(0x47, 0x52, 0x58),
                    Color::Rgb(0xa7, 0xc0, 0x80),
                    Color::Rgb(0xe6, 0x7e, 0x80),
                ),
            ),
            (
                "synthwave84",
                (
                    Color::Rgb(0x26, 0x23, 0x35),
                    Color::Rgb(0xf0, 0xef, 0xf1),
                    Color::Rgb(0x49, 0x54, 0x95),
                    Color::Rgb(0x34, 0x29, 0x4f),
                    Color::Rgb(0xff, 0x7e, 0xdb),
                    Color::Rgb(0x36, 0xf9, 0xf6),
                ),
            ),
            (
                "laserwave",
                (
                    Color::Rgb(0x27, 0x21, 0x2e),
                    Color::Rgb(0xf2, 0xec, 0xf7),
                    Color::Rgb(0x91, 0x88, 0x9b),
                    Color::Rgb(0x71, 0x63, 0x85),
                    Color::Rgb(0xeb, 0x64, 0xb9),
                    Color::Rgb(0x74, 0xdf, 0xc4),
                ),
            ),
            (
                "fairyfloss",
                (
                    Color::Rgb(0x5a, 0x54, 0x75),
                    Color::Rgb(0xf8, 0xf8, 0xf2),
                    Color::Rgb(0xa8, 0xa1, 0xc7),
                    Color::Rgb(0x80, 0x77, 0xa8),
                    Color::Rgb(0xc5, 0xa3, 0xff),
                    Color::Rgb(0xff, 0xb8, 0xd1),
                ),
            ),
            (
                "ultraviolence",
                (
                    Color::Rgb(0x19, 0x11, 0x14),
                    Color::Rgb(0xe9, 0xdf, 0xd2),
                    Color::Rgb(0xa0, 0x8d, 0x80),
                    Color::Rgb(0x55, 0x45, 0x4a),
                    Color::Rgb(0xc9, 0xa7, 0x6f),
                    Color::Rgb(0xc9, 0x8a, 0x8e),
                ),
            ),
        ];

        for (name, expected_roles) in expected {
            assert!(BUILTIN_NAMES.contains(&name), "{name} is not selectable");
            let theme = Theme::by_name_in_dir(name, directory.path());
            assert_eq!(
                (
                    theme.bg,
                    theme.fg,
                    theme.dim,
                    theme.faint,
                    theme.accent,
                    theme.accent2,
                ),
                expected_roles,
                "{name} roles changed"
            );
        }
    }

    #[test]
    fn transparent_inherits_terminal_foreground_and_background() {
        let directory = tempdir().unwrap();
        let transparent = Theme::by_name_in_dir("transparent", directory.path());

        assert_eq!(transparent.bg, Color::Reset);
        assert_eq!(transparent.fg, Color::Reset);
        assert_eq!(transparent.selection_fg(), Color::Black);
        assert!(transparent
            .selection_style(None)
            .add_modifier
            .contains(Modifier::REVERSED));
        assert_ne!(transparent.accent, Color::Reset);
        assert_eq!(transparent.palette, DB16);
    }

    #[test]
    fn selection_background_is_a_twelve_percent_foreground_lift() {
        let theme = Theme::db16();

        assert_eq!(theme.selection_style(None).bg, Some(Color::Rgb(44, 39, 50)));
        assert_ne!(theme.selection_style(None).bg, Some(theme.sel));
    }

    #[test]
    fn transparent_selection_uses_the_detected_terminal_background() {
        let theme = Theme::by_name("transparent");

        assert_eq!(
            theme.selection_style(Some(Color::Rgb(16, 32, 48))).bg,
            Some(Color::Rgb(45, 59, 73))
        );
        assert!(!theme
            .selection_style(Some(Color::Rgb(16, 32, 48)))
            .add_modifier
            .contains(Modifier::REVERSED));
    }

    #[test]
    fn loads_valid_palette_and_role_indices() {
        let directory = tempdir().unwrap();
        write_theme(
            directory.path(),
            "custom",
            r##"
palette = ["#010203", "#102030", "#abcdef", "#ffffff"]

[roles]
bg = 1
fg = 3
dim = 2
faint = 0
accent = 3
accent2 = 1
sel = 2
"##,
        );

        let theme = Theme::by_name_in_dir("custom", directory.path());

        assert_eq!(
            theme.palette,
            &[
                (0x01, 0x02, 0x03),
                (0x10, 0x20, 0x30),
                (0xab, 0xcd, 0xef),
                (0xff, 0xff, 0xff),
            ]
        );
        assert_eq!(theme.bg, Color::Rgb(0x10, 0x20, 0x30));
        assert_eq!(theme.fg, Color::Rgb(0xff, 0xff, 0xff));
        assert_eq!(theme.dim, Color::Rgb(0xab, 0xcd, 0xef));
        assert_eq!(theme.faint, Color::Rgb(0x01, 0x02, 0x03));
        assert_eq!(theme.accent2, Color::Rgb(0x10, 0x20, 0x30));
        assert_eq!(theme.sel, Color::Rgb(0xab, 0xcd, 0xef));
    }

    #[test]
    fn omitted_roles_use_scaled_db16_defaults() {
        let directory = tempdir().unwrap();
        write_theme(
            directory.path(),
            "defaults",
            r##"palette = ["#000000", "#111111", "#222222", "#333333", "#444444", "#ffffff"]"##,
        );

        let theme = Theme::by_name_in_dir("defaults", directory.path());

        assert_eq!(theme.bg, Color::Rgb(0x00, 0x00, 0x00));
        assert_eq!(theme.fg, Color::Rgb(0xff, 0xff, 0xff));
        assert_eq!(theme.dim, Color::Rgb(0x33, 0x33, 0x33));
        assert_eq!(theme.faint, Color::Rgb(0x22, 0x22, 0x22));
        assert_eq!(theme.accent, Color::Rgb(0x44, 0x44, 0x44));
        assert_eq!(theme.accent2, Color::Rgb(0x33, 0x33, 0x33));
        assert_eq!(theme.sel, Color::Rgb(0x44, 0x44, 0x44));
    }

    #[test]
    fn invalid_hex_out_of_range_role_and_missing_file_fall_back_to_db16() {
        let directory = tempdir().unwrap();
        write_theme(
            directory.path(),
            "invalid-hex",
            r##"palette = ["#000000", "#12zz56"]"##,
        );
        write_theme(
            directory.path(),
            "invalid-role",
            r##"
palette = ["#000000", "#ffffff"]
[roles]
accent = 2
"##,
        );

        assert_db16(Theme::by_name_in_dir("invalid-hex", directory.path()));
        assert_db16(Theme::by_name_in_dir("invalid-role", directory.path()));
        assert_db16(Theme::by_name_in_dir("missing", directory.path()));
    }

    #[test]
    fn parses_mixed_case_hex_and_white_boundary() {
        assert_eq!(parse_hex_color("#aBcDeF"), Some((0xab, 0xcd, 0xef)));
        assert_eq!(parse_hex_color("#ffffff"), Some((0xff, 0xff, 0xff)));
        assert_eq!(parse_hex_color("#FFFFFF"), Some((0xff, 0xff, 0xff)));
    }

    fn write_theme(directory: &Path, name: &str, source: &str) {
        fs::write(directory.join(format!("{name}.toml")), source).unwrap();
    }

    fn assert_db16(theme: Theme) {
        assert_eq!(theme, Theme::db16());
        assert_eq!(theme.palette, DB16);
    }
}
