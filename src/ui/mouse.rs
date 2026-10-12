use ratatui::layout::Rect;

pub fn is_inside(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.left() && x < rect.right() && y >= rect.top() && y < rect.bottom()
}

pub fn relative_x_ratio(rect: Rect, x: u16) -> f32 {
    if rect.width <= 1 {
        return 0.0;
    }
    let clamped_x = x.clamp(rect.left(), rect.right().saturating_sub(1));
    (clamped_x - rect.left()) as f32 / (rect.width as f32 - 1.0)
}
