use crate::app::App;
use colors::*;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Stylize,
    text::{Line, Span},
    widgets::{Block, BorderType},
    Frame,
};
use talbe_view::TableView;

pub mod colors;
pub mod help_view;
pub mod string_list;
pub mod talbe_view;
pub mod utils;

const APP_NAME: &str = " PostgresTUI by Voltrus ";

pub struct Ui {
    pub table_view: TableView,
    show_help: bool,
}

impl Ui {
    pub fn new() -> Self {
        Self {
            table_view: TableView::default(),
            show_help: false,
        }
    }

    pub fn ui(&mut self, frame: &mut Frame, app: &mut App) {
        let lay = Layout::horizontal([Constraint::Fill(1)])
            .margin(1)
            .split(frame.area());
        draw_outer_frame(frame, app, lay[0]);

        if let Some(db) = &app.current_db {
            self.table_view.draw(frame, db);
        } else {
            draw_connecting_screen(frame, lay[0]);
        }

        if self.show_help {
            help_view::draw_help_window(frame, lay[0]);
        }
    }

    pub async fn handle_input(
        &mut self,
        key: &KeyEvent,
        app: &mut App,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(_db) = &app.current_db {
            self.table_view.handle_input(key, app).await?;
        }
        if key.code == KeyCode::Char('?') {
            self.show_help = !self.show_help;
        }
        Ok(())
    }
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            table_view: TableView::default(),
            show_help: false,
        }
    }
}

fn draw_outer_frame(frame: &mut Frame, app: &App, area: Rect) {
    let mut key_binds: Vec<Span> = Vec::default();
    append_keybinds(app, &mut key_binds);
    frame.render_widget(new_outer_frame(app, key_binds), area);
}

fn append_keybinds(app: &App, key_binds: &mut Vec<Span>) {
    if app.current_db.is_some() {
        let spans: Vec<Span> = vec![
            " Help ".into(),
            "[?]".fg(HIGHLIGHTED_COLOR),
            " Quit ".into(),
            "[Ctrl+Q]".fg(HIGHLIGHTED_COLOR),
        ];
        key_binds.extend(spans);
    } else {
        let spans: Vec<Span> = vec![
            " Connecting... ".into(),
        ];
        key_binds.extend(spans);
    }
}

fn new_outer_frame<'a>(app: &App, key_binds: Vec<Span<'a>>) -> Block<'a> {
    let key_instruction = Line::from(key_binds).fg(DIM_COLOR).centered();

    let title_line = if let Some(db) = &app.current_db {
        Line::from(vec![
            " postgrestui ".fg(SECONDARY_COLOR).bold(),
            "│".fg(DIM_COLOR),
            format!(" {}:{} ", db.host, db.database).fg(TEXT_COLOR),
            "│".fg(DIM_COLOR),
            format!(
                " {} tables · {} views ",
                db.tables.len(),
                db.views.len()
            )
            .fg(DIM_COLOR),
        ])
        .centered()
    } else {
        Line::from(APP_NAME).fg(SECONDARY_COLOR).bold().centered()
    };

    Block::bordered()
        .title(title_line)
        .title_bottom(key_instruction)
        .fg(PRIMARY_COLOR)
        .border_type(BorderType::Rounded)
}

fn draw_connecting_screen(frame: &mut Frame, area: Rect) {
    use utils::center;
    let text = Line::from("Connecting to PostgreSQL...").fg(DIM_COLOR).centered();
    let centered = center(area, Constraint::Length(30), Constraint::Length(3));
    frame.render_widget(
        ratatui::widgets::Paragraph::new(text).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .fg(PRIMARY_COLOR),
        ),
        centered,
    );
}
