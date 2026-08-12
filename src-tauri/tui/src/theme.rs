//! Theme = one retro palette; the cover quantizes to the same colors as
//! the UI (that unification is the product's visual identity).

use ratatui::style::Color;

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub dim: Color,
    pub faint: Color,
    pub accent: Color,
    pub accent2: Color,
    pub sel: Color,
    /// Full palette the pixel cover quantizes against.
    pub palette: &'static [(u8, u8, u8)],
}

pub const DB16: &[(u8, u8, u8)] = &[
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

fn rgb((r, g, b): (u8, u8, u8)) -> Color {
    Color::Rgb(r, g, b)
}

impl Theme {
    pub fn by_name(name: &str) -> Self {
        match name {
            // Theme files land in a later stage; every unknown name is DB16.
            _ => Self::db16(),
        }
    }

    pub fn db16() -> Self {
        Self {
            bg: rgb(DB16[0]),
            fg: rgb(DB16[15]),
            dim: rgb(DB16[10]),
            faint: rgb(DB16[7]),
            accent: rgb(DB16[14]),
            accent2: rgb(DB16[9]),
            sel: rgb(DB16[12]),
            palette: DB16,
        }
    }
}
