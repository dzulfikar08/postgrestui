use ratatui::style::{Color, Modifier, Style};

pub const PRIMARY_COLOR: Color = Color::Blue;
pub const SECONDARY_COLOR: Color = Color::Rgb(0, 230, 118);
pub const TEXT_COLOR: Color = Color::Gray;
pub const HIGHLIGHTED_COLOR: Color = Color::Rgb(0, 230, 118);
pub const DIM_COLOR: Color = Color::DarkGray;

pub const HIGHLIGHT_STYLE: Style = Style::new()
    .fg(HIGHLIGHTED_COLOR)
    .add_modifier(Modifier::BOLD);
pub const NULL_STYLE: Style = Style::new().fg(DIM_COLOR).add_modifier(Modifier::ITALIC);
pub const BLOB_STYLE: Style = Style::new().fg(DIM_COLOR);
pub const ROW_NUM_STYLE: Style = Style::new().fg(DIM_COLOR);
