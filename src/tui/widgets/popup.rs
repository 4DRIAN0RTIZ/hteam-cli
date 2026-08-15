use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear},
    Frame,
};

/// Carves a centered `percent_x` × `percent_y` rectangle out of `area` — the
/// layout math every popup used to size itself.
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}

/// Clears `area`, draws a bordered block titled `title` in `border_color`,
/// and returns the inner content rect — the `Clear` + `Block` boilerplate
/// every popup repeated by hand.
pub fn draw_frame(frame: &mut Frame, area: Rect, title: impl Into<String>, border_color: Color) -> Rect {
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(title.into())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}
