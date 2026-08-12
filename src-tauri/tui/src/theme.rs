//! Theme = one retro palette plus role mappings shared by the UI and covers.

use std::fs;
use std::path::{Path, PathBuf};

use ratatui::style::Color;
use serde::Deserialize;

type Rgb = (u8, u8, u8);

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

    fn builtin(name: &str) -> Option<Self> {
        match name {
            "db16" => Some(Self::db16()),
            "pico8" => Some(Self::pico8()),
            "gameboy" => Some(Self::gameboy()),
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use ratatui::style::Color;
    use tempfile::tempdir;

    use super::{parse_hex_color, Theme, DB16};

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
