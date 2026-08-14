use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::AppState;

pub const WIDTH: u16 = 26;
pub const HEIGHT: u16 = 13;
const GAP: u16 = 2;

pub fn split_preview(area: Rect, min_list_width: u16) -> (Rect, Option<Rect>) {
    let required_width = min_list_width.saturating_add(GAP).saturating_add(WIDTH);
    if area.width < required_width || area.height < HEIGHT {
        return (area, None);
    }

    let list_width = area.width - GAP - WIDTH;
    let list = Rect {
        width: list_width,
        ..area
    };
    let preview = Rect {
        x: area.x.saturating_add(list_width).saturating_add(GAP),
        y: area.y,
        width: WIDTH,
        height: HEIGHT,
    };
    (list, Some(preview))
}

pub fn draw(frame: &mut Frame, state: &mut AppState, area: Rect) {
    if state.selected_original_is_available() {
        if let Some(cover) = state.selected_pixel_cover() {
            frame.render_widget(cover, area);
        } else {
            frame.render_widget(state.preview_placeholder(), area);
        }
        state.render_selected_original(frame, area);
    } else if let Some(cover) = state.selected_pixel_cover() {
        frame.render_widget(cover, area);
    } else {
        frame.render_widget(state.preview_placeholder(), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_preview_appears_at_the_exact_width_and_height_boundary() {
        let too_narrow = Rect::new(7, 3, 79, 13);
        assert_eq!(split_preview(too_narrow, 52), (too_narrow, None));

        let too_short = Rect::new(7, 3, 80, 12);
        assert_eq!(split_preview(too_short, 52), (too_short, None));

        let area = Rect::new(7, 3, 80, 13);
        assert_eq!(
            split_preview(area, 52),
            (
                Rect::new(7, 3, 52, 13),
                Some(Rect::new(61, 3, WIDTH, HEIGHT)),
            )
        );
    }

    #[test]
    fn search_preview_needs_one_more_list_column() {
        let too_narrow = Rect::new(0, 0, 80, 20);
        assert_eq!(split_preview(too_narrow, 53), (too_narrow, None));

        let area = Rect::new(0, 0, 81, 20);
        assert_eq!(
            split_preview(area, 53),
            (
                Rect::new(0, 0, 53, 20),
                Some(Rect::new(55, 0, WIDTH, HEIGHT)),
            )
        );
    }

    #[test]
    fn spare_width_stays_with_the_list() {
        let area = Rect::new(4, 2, 87, 20);
        let (list, preview) = split_preview(area, 52);

        assert_eq!(list, Rect::new(4, 2, 59, 20));
        assert_eq!(preview, Some(Rect::new(65, 2, WIDTH, HEIGHT)));
    }
}
