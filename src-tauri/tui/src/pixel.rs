use anyhow::{ensure, Result};
use image::imageops::{resize, FilterType};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;

type Rgb = (u8, u8, u8);

const BAYER_4X4: [[i16; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
const DITHER_RANGE: i16 = 14;
// Below this squared distance the undithered match is faithful enough;
// dithering there would only speckle smooth gradients (the dirty-logo bug).
const CLEAN_MATCH_SQ: i32 = 2800;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PixelCell {
    pub upper: Rgb,
    pub lower: Rgb,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PixelCover {
    pub width: u16,
    pub height: u16,
    pub cells: Vec<PixelCell>,
}

/// Built-in idle art: a procedural vinyl record quantized to the theme
/// palette — no bundled asset, and it recolors with the theme like covers.
pub fn vinyl(palette: &[Rgb], cell_width: u16, cell_height: u16) -> PixelCover {
    let last = palette.len().saturating_sub(1);
    let bg = palette[0];
    let disc = palette[1.min(last)];
    let ring = palette[last / 2];
    let shine = palette[last * 5 / 8];
    let label = palette[last.saturating_sub(1).max(1)];

    let width_px = cell_width as f64;
    let height_px = (cell_height * 2) as f64;
    let (cx, cy) = (width_px / 2.0, height_px / 2.0);
    let radius = width_px.min(height_px) / 2.0 - 0.5;

    let mut cells = Vec::with_capacity(cell_width as usize * cell_height as usize);
    let color_at = |x: u32, y: u32| -> Rgb {
        let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
        let r = (dx * dx + dy * dy).sqrt();
        if r > radius {
            return bg;
        }
        if r > radius - 1.0 {
            return ring;
        }
        if r < radius * 0.08 {
            return bg; // spindle hole
        }
        if r < radius * 0.32 {
            return label;
        }
        // Groove zone: dark disc, sparse lighter rings, one specular wedge.
        let angle = dy.atan2(dx);
        let in_shine = (-2.5..-1.9).contains(&angle) || (0.6..1.2).contains(&angle);
        if in_shine && r > radius * 0.45 {
            shine
        } else if (r as u32).is_multiple_of(4) {
            ring
        } else {
            disc
        }
    };
    for cell_y in 0..cell_height as u32 {
        for x in 0..cell_width as u32 {
            cells.push(PixelCell {
                upper: color_at(x, cell_y * 2),
                lower: color_at(x, cell_y * 2 + 1),
            });
        }
    }
    PixelCover {
        width: cell_width,
        height: cell_height,
        cells,
    }
}

pub fn from_image_bytes(
    bytes: &[u8],
    palette: &[Rgb],
    cell_width: u16,
    cell_height: u16,
) -> Result<PixelCover> {
    ensure!(!palette.is_empty(), "pixel cover palette cannot be empty");
    ensure!(
        cell_width > 0 && cell_height > 0,
        "pixel cover dimensions must be non-zero"
    );

    let source = image::load_from_memory(bytes)?.to_rgb8();
    let target_width = u32::from(cell_width);
    let target_height = u32::from(cell_height) * 2;
    let (resized_width, resized_height) =
        fitted_dimensions(source.width(), source.height(), target_width, target_height);
    let resized = resize(&source, resized_width, resized_height, FilterType::Triangle);
    let offset_x = (target_width - resized_width) / 2;
    let offset_y = (target_height - resized_height) / 2;

    let background = palette[0];
    let mut pixels = vec![background; target_width as usize * target_height as usize];
    for y in 0..resized_height {
        for x in 0..resized_width {
            let [red, green, blue] = resized.get_pixel(x, y).0;
            let target_x = x + offset_x;
            let target_y = y + offset_y;
            let index = target_y as usize * target_width as usize + target_x as usize;
            let plain = nearest_color((red, green, blue), palette);
            let error = color_distance_sq((red, green, blue), plain);
            pixels[index] = if error <= CLEAN_MATCH_SQ {
                plain
            } else {
                let offset = bayer_offset(target_x, target_y);
                nearest_color(
                    (
                        apply_offset(red, offset),
                        apply_offset(green, offset),
                        apply_offset(blue, offset),
                    ),
                    palette,
                )
            };
        }
    }

    let mut cells = Vec::with_capacity(cell_width as usize * cell_height as usize);
    for cell_y in 0..cell_height {
        let upper_row = usize::from(cell_y) * 2 * usize::from(cell_width);
        let lower_row = upper_row + usize::from(cell_width);
        for x in 0..usize::from(cell_width) {
            cells.push(PixelCell {
                upper: pixels[upper_row + x],
                lower: pixels[lower_row + x],
            });
        }
    }

    Ok(PixelCover {
        width: cell_width,
        height: cell_height,
        cells,
    })
}

impl Widget for &PixelCover {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let draw_width = area.width.min(self.width);
        let draw_height = area.height.min(self.height);
        let destination_x = area.x + area.width.saturating_sub(self.width) / 2;
        let destination_y = area.y + area.height.saturating_sub(self.height) / 2;

        for y in 0..draw_height {
            for x in 0..draw_width {
                let cover_cell =
                    self.cells[usize::from(y) * usize::from(self.width) + usize::from(x)];
                let Some(buffer_cell) = buf.cell_mut((destination_x + x, destination_y + y)) else {
                    continue;
                };
                buffer_cell
                    .set_symbol("▀")
                    .set_fg(to_color(cover_cell.upper))
                    .set_bg(to_color(cover_cell.lower));
            }
        }
    }
}

fn fitted_dimensions(
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
) -> (u32, u32) {
    let source_width_64 = u64::from(source_width);
    let source_height_64 = u64::from(source_height);
    let target_width_64 = u64::from(target_width);
    let target_height_64 = u64::from(target_height);

    if source_width_64 * target_height_64 >= source_height_64 * target_width_64 {
        let height = ((source_height_64 * target_width_64 + source_width_64 / 2) / source_width_64)
            .clamp(1, target_height_64);
        (target_width, height as u32)
    } else {
        let width = ((source_width_64 * target_height_64 + source_height_64 / 2)
            / source_height_64)
            .clamp(1, target_width_64);
        (width as u32, target_height)
    }
}

fn color_distance_sq(a: Rgb, b: Rgb) -> i32 {
    let dr = a.0 as i32 - b.0 as i32;
    let dg = a.1 as i32 - b.1 as i32;
    let db = a.2 as i32 - b.2 as i32;
    dr * dr + dg * dg + db * db
}

fn bayer_offset(x: u32, y: u32) -> i16 {
    let threshold = BAYER_4X4[y as usize % 4][x as usize % 4];
    let numerator = (threshold * 2 + 1 - 16) * DITHER_RANGE;
    if numerator >= 0 {
        (numerator + 16) / 32
    } else {
        (numerator - 16) / 32
    }
}

fn apply_offset(channel: u8, offset: i16) -> u8 {
    (i16::from(channel) + offset).clamp(0, 255) as u8
}

fn nearest_color(color: Rgb, palette: &[Rgb]) -> Rgb {
    let mut nearest = palette[0];
    let mut shortest_distance = u32::MAX;
    for &candidate in palette {
        let red = i32::from(color.0) - i32::from(candidate.0);
        let green = i32::from(color.1) - i32::from(candidate.1);
        let blue = i32::from(color.2) - i32::from(candidate.2);
        let distance = (red * red + green * green + blue * blue) as u32;
        if distance < shortest_distance {
            nearest = candidate;
            shortest_distance = distance;
        }
    }
    nearest
}

fn to_color((red, green, blue): Rgb) -> Color {
    Color::Rgb(red, green, blue)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{DynamicImage, ImageFormat, Rgb as ImageRgb, RgbImage};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Color;
    use ratatui::widgets::Widget;

    use super::{from_image_bytes, PixelCell, PixelCover, Rgb};

    const BLACK: Rgb = (0, 0, 0);
    const WHITE: Rgb = (255, 255, 255);

    #[test]
    fn quantizes_a_two_by_two_image_to_known_colors() {
        let bytes = png_bytes(2, 2, &[(255, 0, 0), (0, 255, 0), (0, 0, 255), WHITE]);
        let palette = [(255, 0, 0), (0, 255, 0), (0, 0, 255), WHITE];

        let cover = from_image_bytes(&bytes, &palette, 2, 1).unwrap();

        assert_eq!(
            cover.cells,
            vec![
                PixelCell {
                    upper: (255, 0, 0),
                    lower: (0, 0, 255),
                },
                PixelCell {
                    upper: (0, 255, 0),
                    lower: WHITE,
                },
            ]
        );
    }

    #[test]
    fn bayer_dithering_is_deterministic() {
        let bytes = png_bytes(4, 4, &[(128, 128, 128); 16]);
        let palette = [BLACK, WHITE];

        let first = from_image_bytes(&bytes, &palette, 4, 2).unwrap();
        let second = from_image_bytes(&bytes, &palette, 4, 2).unwrap();
        // Snapshot under DITHER_RANGE=14 / CLEAN_MATCH_SQ=2800: mid-gray
        // still checkers, with one Bayer cell tipping to white.
        let top = vec![
            PixelCell {
                upper: BLACK,
                lower: WHITE,
            },
            PixelCell {
                upper: WHITE,
                lower: BLACK,
            },
            PixelCell {
                upper: BLACK,
                lower: WHITE,
            },
            PixelCell {
                upper: WHITE,
                lower: BLACK,
            },
        ];
        let bottom = vec![
            PixelCell {
                upper: BLACK,
                lower: WHITE,
            },
            PixelCell {
                upper: WHITE,
                lower: WHITE,
            },
            PixelCell {
                upper: BLACK,
                lower: WHITE,
            },
            PixelCell {
                upper: WHITE,
                lower: BLACK,
            },
        ];

        assert_eq!(first, second);
        assert_eq!(first.cells, [top, bottom].concat());
    }

    #[test]
    fn fills_non_square_image_letterbox_with_first_palette_color() {
        let bytes = png_bytes(4, 2, &[(255, 0, 0); 8]);
        let red = (255, 0, 0);

        let cover = from_image_bytes(&bytes, &[BLACK, red], 4, 2).unwrap();

        assert_eq!(
            cover.cells,
            [
                vec![
                    PixelCell {
                        upper: BLACK,
                        lower: red,
                    };
                    4
                ],
                vec![
                    PixelCell {
                        upper: red,
                        lower: BLACK,
                    };
                    4
                ],
            ]
            .concat()
        );
    }

    #[test]
    fn widget_writes_centered_half_blocks_with_rgb_colors() {
        let cover = PixelCover {
            width: 2,
            height: 1,
            cells: vec![
                PixelCell {
                    upper: (1, 2, 3),
                    lower: (4, 5, 6),
                },
                PixelCell {
                    upper: (7, 8, 9),
                    lower: (10, 11, 12),
                },
            ],
        };
        let mut buffer = Buffer::with_lines(["....", "....", "...."]);

        (&cover).render(Rect::new(0, 0, 4, 3), &mut buffer);

        let first = buffer.cell((1, 1)).unwrap();
        assert_eq!(first.symbol(), "▀");
        assert_eq!(first.fg, Color::Rgb(1, 2, 3));
        assert_eq!(first.bg, Color::Rgb(4, 5, 6));
        let second = buffer.cell((2, 1)).unwrap();
        assert_eq!(second.symbol(), "▀");
        assert_eq!(second.fg, Color::Rgb(7, 8, 9));
        assert_eq!(second.bg, Color::Rgb(10, 11, 12));
        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), ".");
    }

    #[test]
    fn widget_clips_when_the_area_is_smaller_than_the_cover() {
        let cover = PixelCover {
            width: 2,
            height: 1,
            cells: vec![
                PixelCell {
                    upper: (1, 2, 3),
                    lower: (4, 5, 6),
                },
                PixelCell {
                    upper: (7, 8, 9),
                    lower: (10, 11, 12),
                },
            ],
        };
        let mut buffer = Buffer::with_lines(["."]);

        (&cover).render(Rect::new(0, 0, 1, 1), &mut buffer);

        let cell = buffer.cell((0, 0)).unwrap();
        assert_eq!(cell.symbol(), "▀");
        assert_eq!(cell.fg, Color::Rgb(1, 2, 3));
        assert_eq!(cell.bg, Color::Rgb(4, 5, 6));
    }

    fn png_bytes(width: u32, height: u32, pixels: &[Rgb]) -> Vec<u8> {
        let image = RgbImage::from_fn(width, height, |x, y| {
            let (red, green, blue) = pixels[(y * width + x) as usize];
            ImageRgb([red, green, blue])
        });
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }
}
