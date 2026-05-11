use super::{
    colors::HIGHLIGHTED_COLOR,
    string_list::{self, StringList},
    BLOB_STYLE, DIM_COLOR, HIGHLIGHT_STYLE, NULL_STYLE, PRIMARY_COLOR, ROW_NUM_STYLE,
    SECONDARY_COLOR, TEXT_COLOR,
};
use crate::app::{self, App, CellType, Db};
use arboard::Clipboard;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols,
    text::Line,
    widgets::{
        Block, BorderType, Borders, Cell, Padding, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Table, TableState, Tabs, Widget, Wrap,
    },
    Frame,
};
use strum::{Display, EnumIter, IntoEnumIterator};

#[derive(Clone, Copy, Default, Debug, Display, EnumIter, PartialEq)]
pub enum SelectedTableTab {
    #[default]
    #[strum(to_string = "Browse")]
    Browse,
    #[strum(to_string = "Schema")]
    Schema,
    #[strum(to_string = "Query")]
    Query,
}

impl SelectedTableTab {
    pub fn next(&self) -> SelectedTableTab {
        let len = Self::iter().len();
        Self::iter()
            .nth((*self as usize + 1) % len)
            .unwrap_or(*self)
    }

    pub fn previous(&self) -> SelectedTableTab {
        let len = Self::iter().len();
        Self::iter()
            .nth((*self as usize + len - 1) % len)
            .unwrap_or(*self)
    }
}

impl Widget for SelectedTableTab {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let labels: Vec<Line> = vec![
            Line::from(vec![
                " Browse ".fg(SECONDARY_COLOR).bold(),
                "[L]".fg(HIGHLIGHTED_COLOR),
            ]),
            Line::from(vec![
                " Schema ".fg(SECONDARY_COLOR).bold(),
                "[H]".fg(HIGHLIGHTED_COLOR),
            ]),
            Line::from(vec![
                " Query ".fg(SECONDARY_COLOR).bold(),
                "[;]".fg(HIGHLIGHTED_COLOR),
            ]),
        ];
        Tabs::new(labels)
            .divider(symbols::DOT)
            .highlight_style(
                Style::default()
                    .underlined()
                    .underline_color(HIGHLIGHTED_COLOR),
            )
            .padding(" ", " ")
            .select(self as usize)
            .block(Block::default().borders(Borders::LEFT))
            .render(area, buf);
    }
}

#[derive(Clone, Copy, Default, Debug, Display, EnumIter)]
pub enum NavigationTab {
    #[default]
    #[strum(to_string = "Tables")]
    Tables,
    #[strum(to_string = "Views")]
    Views,
}

impl NavigationTab {
    pub fn next(&self) -> NavigationTab {
        let len = Self::iter().len();
        Self::iter()
            .nth((*self as usize + 1) % len)
            .unwrap_or(*self)
    }

    pub fn previous(&self) -> NavigationTab {
        let len = Self::iter().len();
        Self::iter()
            .nth((*self as usize + len - 1) % len)
            .unwrap_or(*self)
    }
}

impl Widget for NavigationTab {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let labels: Vec<Line> = vec![
            Line::from(vec![
                " Tables ".fg(SECONDARY_COLOR).bold(),
                "[q]".fg(HIGHLIGHTED_COLOR),
            ]),
            Line::from(vec![
                " Views ".fg(SECONDARY_COLOR).bold(),
                "[e]".fg(HIGHLIGHTED_COLOR),
            ]),
        ];
        Tabs::new(labels)
            .divider(symbols::DOT)
            .highlight_style(
                Style::default()
                    .underlined()
                    .underline_color(HIGHLIGHTED_COLOR),
            )
            .padding(" ", " ")
            .select(self as usize)
            .block(Block::default())
            .render(area, buf);
    }
}

const SQL_KEYWORDS: &[&str] = &[
    "SELECT", "FROM", "WHERE", "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE",
    "CREATE", "TABLE", "DROP", "ALTER", "ADD", "COLUMN", "INDEX", "VIEW", "TRIGGER",
    "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "CROSS", "ON", "AS", "AND", "OR", "NOT",
    "NULL", "IS", "IN", "BETWEEN", "LIKE", "ORDER", "BY", "GROUP", "HAVING", "LIMIT",
    "OFFSET", "UNION", "ALL", "DISTINCT", "CASE", "WHEN", "THEN", "ELSE", "END",
    "EXISTS", "PRIMARY", "KEY", "FOREIGN", "REFERENCES", "DEFAULT", "CHECK", "UNIQUE",
    "IF", "BEGIN", "COMMIT", "ROLLBACK", "TRANSACTION", "WITH", "RECURSIVE",
    "OVER", "PARTITION", "ROW_NUMBER", "RANK", "DENSE_RANK", "COUNT", "SUM", "AVG",
    "MIN", "MAX", "COALESCE", "CAST", "EXPLAIN", "ANALYZE", "VACUUM", "REINDEX",
    "RETURNING", "CONFLICT", "NOTHING", "UPSERT", "BOOLEAN", "INTEGER", "BIGINT",
    "SERIAL", "BIGSERIAL", "TIMESTAMP", "INTERVAL", "JSONB", "ARRAY", "UUID",
    "TRUE", "FALSE", "TYPE", "ENUM", "SEQUENCE", "SCHEMA", "DATABASE",
];

fn highlight_sql(sql: &str) -> Line<'static> {
    let mut spans: Vec<ratatui::text::Span> = Vec::new();
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                let mut s = String::from("'");
                while let Some(&nc) = chars.peek() {
                    if nc == '\'' {
                        s.push(chars.next().unwrap());
                        if chars.peek() == Some(&'\'') {
                            s.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    } else {
                        s.push(chars.next().unwrap());
                    }
                }
                spans.push(s.fg(Color::Green));
            }
            '"' => {
                let mut s = String::from("\"");
                while let Some(&nc) = chars.peek() {
                    s.push(chars.next().unwrap());
                    if nc == '"' {
                        break;
                    }
                }
                spans.push(s.fg(Color::Magenta));
            }
            '-' if chars.peek() == Some(&'-') => {
                chars.next();
                let rest: String = chars.collect();
                spans.push(format!("--{}", rest).fg(DIM_COLOR));
                break;
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut s = String::from("/*");
                while let Some(&nc) = chars.peek() {
                    s.push(chars.next().unwrap());
                    if nc == '*' && chars.peek() == Some(&'/') {
                        s.push(chars.next().unwrap());
                        break;
                    }
                }
                spans.push(s.fg(DIM_COLOR));
            }
            c if c.is_ascii_digit() => {
                let mut s = String::from(c);
                while let Some(&nc) = chars.peek() {
                    if nc.is_ascii_digit() || nc == '.' {
                        s.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                spans.push(s.fg(Color::Yellow));
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut s = String::from(c);
                while let Some(&nc) = chars.peek() {
                    if nc.is_ascii_alphabetic() || nc == '_' || nc.is_ascii_digit() {
                        s.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                if SQL_KEYWORDS.contains(&s.to_uppercase().as_str()) {
                    spans.push(s.fg(SECONDARY_COLOR).bold());
                } else {
                    spans.push(s.fg(TEXT_COLOR));
                }
            }
            '=' | '<' | '>' | '!' | '|' => {
                let mut s = String::from(c);
                if let Some(&nc) = chars.peek() {
                    if nc == '=' || (c == '|' && nc == '|') {
                        s.push(chars.next().unwrap());
                    }
                }
                spans.push(s.fg(HIGHLIGHTED_COLOR));
            }
            '(' | ')' | ',' | ';' => {
                spans.push(c.to_string().fg(DIM_COLOR));
            }
            '.' => {
                spans.push(c.to_string().fg(TEXT_COLOR));
            }
            _ => {
                spans.push(c.to_string().fg(TEXT_COLOR));
            }
        }
    }
    Line::from(spans)
}

pub struct TableView {
    pub tables_list: StringList,
    pub view_list: StringList,
    pub selected_table_tab: SelectedTableTab,
    pub table_nav_tab: NavigationTab,
    pub data: (Vec<String>, Vec<Vec<CellType>>),
    pub table_state: TableState,
    table_scroll_height: u16,
    clipboard: Option<Clipboard>,
    last_loaded_table: Option<String>,
    scroll_state: ScrollbarState,
    search_query: String,
    search_mode: bool,
    filtered_list: StringList,
    sql_input: String,
    sql_result: Option<(Vec<String>, Vec<Vec<CellType>>)>,
    sql_error: Option<String>,
    copy_menu_open: bool,
    copy_menu_state: ratatui::widgets::ListState,
    suggestions: Vec<String>,
    suggestion_selected: Option<usize>,
    db_table_names: Vec<String>,
    db_view_names: Vec<String>,
    db_columns: Vec<(String, Vec<String>)>,
    page: usize,
    page_size: usize,
    h_scroll_amount: u16,
}

impl Default for TableView {
    fn default() -> Self {
        Self {
            tables_list: StringList::default(),
            view_list: StringList::default(),
            selected_table_tab: SelectedTableTab::default(),
            table_nav_tab: NavigationTab::default(),
            data: (Vec::default(), Vec::default()),
            table_state: TableState::default(),
            table_scroll_height: 10,
            clipboard: Clipboard::new().ok(),
            last_loaded_table: None,
            scroll_state: ScrollbarState::default(),
            search_query: String::new(),
            search_mode: false,
            filtered_list: StringList::default(),
            sql_input: String::new(),
            sql_result: None,
            sql_error: None,
            copy_menu_open: false,
            copy_menu_state: ratatui::widgets::ListState::default(),
            suggestions: Vec::new(),
            suggestion_selected: None,
            db_table_names: Vec::new(),
            db_view_names: Vec::new(),
            db_columns: Vec::new(),
            page: 0,
            page_size: 500,
            h_scroll_amount: 20,
        }
    }
}

impl TableView {
    pub fn load_nav(&mut self, db: &Db) {
        let (table_names, table_displays): (Vec<String>, Vec<String>) = db
            .tables
            .iter()
            .map(|t| {
                let count = t.row_count.map(|c| c.to_string()).unwrap_or("?".into());
                let display = if t.schema == "public" {
                    format!("{}  {}", t.name, count)
                } else {
                    format!("{}.{}  {}", t.schema, t.name, count)
                };
                (format!("{}.{}", t.schema, t.name), display)
            })
            .unzip();
        self.tables_list
            .load_items_with_display(table_names, table_displays);

        let (view_names, view_displays): (Vec<String>, Vec<String>) = db
            .views
            .iter()
            .map(|v| {
                let count = v.row_count.map(|c| c.to_string()).unwrap_or("?".into());
                let display = if v.schema == "public" {
                    format!("{}  {}", v.name, count)
                } else {
                    format!("{}.{}  {}", v.schema, v.name, count)
                };
                (format!("{}.{}", v.schema, v.name), display)
            })
            .unzip();
        self.view_list
            .load_items_with_display(view_names, view_displays);

        self.db_table_names = db.tables.iter().map(|t| format!("{}.{}", t.schema, t.name)).collect();
        self.db_view_names = db.views.iter().map(|v| format!("{}.{}", v.schema, v.name)).collect();
        self.db_columns = db
            .tables
            .iter()
            .map(|t| (format!("{}.{}", t.schema, t.name), t.columns.clone()))
            .collect();

        self.last_loaded_table = None;
        self.search_query.clear();
        self.search_mode = false;
    }

    pub fn reset(&mut self) {
        self.last_loaded_table = None;
        self.data = (Vec::default(), Vec::default());
        self.table_state.select_cell(Some((0, 1)));
        self.search_query.clear();
        self.search_mode = false;
        self.sql_input.clear();
        self.sql_result = None;
        self.sql_error = None;
    }

    fn is_searching(&self) -> bool {
        self.search_mode
    }

    pub fn is_input_mode(&self) -> bool {
        self.search_mode || self.selected_table_tab == SelectedTableTab::Query
    }

    fn active_list(&self) -> &StringList {
        match self.table_nav_tab {
            NavigationTab::Tables => &self.tables_list,
            NavigationTab::Views => &self.view_list,
        }
    }

    fn active_list_mut(&mut self) -> &mut StringList {
        match self.table_nav_tab {
            NavigationTab::Tables => &mut self.tables_list,
            NavigationTab::Views => &mut self.view_list,
        }
    }

    fn current_list(&self) -> &StringList {
        if self.is_searching() {
            &self.filtered_list
        } else {
            self.active_list()
        }
    }

    fn update_filter(&mut self) {
        let (items, displays) = {
            let source = self.active_list();
            let query = self.search_query.to_lowercase();
            source
                .items
                .iter()
                .zip(source.display_text().iter())
                .filter(|(item, _)| query.is_empty() || item.to_lowercase().contains(&query))
                .map(|(item, display)| (item.clone(), display.clone()))
                .unzip::<String, String, Vec<_>, Vec<_>>()
        };
        self.filtered_list.load_items_with_display(items, displays);
    }

    pub fn draw(&mut self, frame: &mut Frame, db: &Db) {
        let [l, r] = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(4)])
            .margin(2)
            .areas(frame.area());

        let nav_tab_h = 1u16;
        let search_h = if self.is_searching() { 1u16 } else { 0u16 };
        let hints_h = 2u16;

        let vertical_constraints = {
            let mut c = vec![Constraint::Length(nav_tab_h)];
            if self.is_searching() {
                c.push(Constraint::Length(search_h));
            }
            c.push(Constraint::Fill(1));
            c.push(Constraint::Length(hints_h));
            c
        };
        let left_areas = Layout::vertical(vertical_constraints).split(l);
        let mut area_idx = 0;

        let nav_tab_area = left_areas[area_idx];
        area_idx += 1;

        let search_area = if self.is_searching() {
            let a = left_areas[area_idx];
            area_idx += 1;
            Some(a)
        } else {
            None
        };

        let nav_list_area = left_areas[area_idx];
        area_idx += 1;

        let nav_hints_area = left_areas[area_idx];

        let [right_tab, right_body, right_hints] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(2),
        ])
        .areas(r);

        frame.render_widget(self.table_nav_tab, nav_tab_area);

        if let Some(sa) = search_area {
            let search_text = if self.search_query.is_empty() {
                Line::from(vec!["/ ".fg(HIGHLIGHTED_COLOR), "type to filter...".fg(DIM_COLOR)])
            } else {
                Line::from(vec![
                    "/ ".fg(HIGHLIGHTED_COLOR),
                    self.search_query.clone().fg(TEXT_COLOR),
                    "█".fg(HIGHLIGHTED_COLOR),
                ])
            };
            frame.render_widget(
                Paragraph::new(search_text).block(Block::default().borders(Borders::BOTTOM)),
                sa,
            );
        }

        self.draw_nav_lists(frame, nav_list_area);

        let mut nav_hint_spans: Vec<ratatui::text::Span> = vec![];
        if !self.is_searching() {
            nav_hint_spans.push(" Search ".into());
            nav_hint_spans.push("[/]".fg(HIGHLIGHTED_COLOR));
        } else {
            nav_hint_spans.push(" Clear ".into());
            nav_hint_spans.push("[Esc]".fg(HIGHLIGHTED_COLOR));
            nav_hint_spans.push(" Navigate ".into());
            nav_hint_spans.push("↑↓".fg(HIGHLIGHTED_COLOR));
        }
        let nav_hint = Line::from(nav_hint_spans).fg(TEXT_COLOR);
        frame.render_widget(
            Paragraph::new(nav_hint).block(Block::default().borders(Borders::TOP)),
            nav_hints_area,
        );

        frame.render_widget(self.selected_table_tab, right_tab);

        match self.selected_table_tab {
            SelectedTableTab::Browse => {
                if let Some(table) = self.get_selected_table(db) {
                    self.draw_body(frame, table, right_body);
                }
            }
            SelectedTableTab::Schema => {
                if let Some(table) = self.get_selected_table(db) {
                    let lay = Layout::vertical([Constraint::Fill(1)])
                        .margin(2)
                        .split(right_body);
                    let schema_text = if table.sql.is_empty() {
                        "(schema definition not available)"
                    } else {
                        table.sql.trim()
                    };
                    let p = Paragraph::new(schema_text)
                        .wrap(Wrap { trim: true })
                        .fg(TEXT_COLOR);
                    frame.render_widget(p, lay[0]);
                }
            }
            SelectedTableTab::Query => {
                self.draw_query(frame, right_body);
            }
        }

        let table_hint = Line::from(vec![
            " Scroll↑↓ ".into(),
            "[i/k]".fg(HIGHLIGHTED_COLOR),
            " Scroll←→ ".into(),
            "[j/l]".fg(HIGHLIGHTED_COLOR),
            " Page ".into(),
            "[u/d]".fg(HIGHLIGHTED_COLOR),
            " Copy ".into(),
            "[c]".fg(HIGHLIGHTED_COLOR),
        ])
        .fg(TEXT_COLOR);
        frame.render_widget(
            Paragraph::new(table_hint).block(Block::default().borders(Borders::TOP)),
            right_hints,
        );

        if self.copy_menu_open {
            self.draw_copy_menu(frame);
        }
    }

    fn copy_options(&self) -> Vec<&'static str> {
        let mut opts = vec![
            "Value (current cell)",
            "Row (tab-separated)",
            "Row with headers",
            "All results (tab-separated)",
            "All results with headers",
        ];
        if self.selected_table_tab == SelectedTableTab::Schema {
            opts.push("CREATE statement (SQL)");
        }
        if self.selected_table_tab == SelectedTableTab::Query {
            opts.push("SQL query");
        }
        opts
    }

    fn draw_copy_menu(&mut self, frame: &mut Frame) {
        let options = self.copy_options();
        let items: Vec<ratatui::widgets::ListItem> = options
            .iter()
            .map(|o| {
                ratatui::widgets::ListItem::new(Line::from(*o).fg(TEXT_COLOR))
            })
            .collect();
        let list = ratatui::widgets::List::new(items)
            .highlight_style(HIGHLIGHT_STYLE)
            .highlight_symbol("▸")
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .fg(PRIMARY_COLOR)
                    .title(Line::from(" Copy ").fg(SECONDARY_COLOR).bold().centered()),
            );
        let area = super::utils::center(
            frame.area(),
            Constraint::Length(36),
            Constraint::Length((options.len() as u16) + 2),
        );
        frame.render_widget(ratatui::widgets::Clear, area);
        frame.render_stateful_widget(list, area, &mut self.copy_menu_state);
    }

    fn do_copy(&mut self, index: usize, db: &Db) -> Result<(), Box<dyn std::error::Error>> {
        let text = match index {
            0 => {
                if let Some((row, col)) = self.table_state.selected_cell() {
                    let dc = col.saturating_sub(1);
                    self.data.1.get(row).and_then(|r| r.get(dc)).map(|c| c.display_text().to_string()).unwrap_or_default()
                } else {
                    String::new()
                }
            }
            1 => {
                if let Some((row, _)) = self.table_state.selected_cell() {
                    self.data.1.get(row)
                        .map(|r| r.iter().map(|c| c.display_text()).collect::<Vec<_>>().join("\t"))
                        .unwrap_or_default()
                } else {
                    String::new()
                }
            }
            2 => {
                if let Some((row, _)) = self.table_state.selected_cell() {
                    let header = self.data.0.join("\t");
                    let vals = self.data.1.get(row)
                        .map(|r| r.iter().map(|c| c.display_text()).collect::<Vec<_>>().join("\t"))
                        .unwrap_or_default();
                    format!("{}\n{}", header, vals)
                } else {
                    String::new()
                }
            }
            3 => {
                self.data.1.iter()
                    .map(|r| r.iter().map(|c| c.display_text()).collect::<Vec<_>>().join("\t"))
                    .collect::<Vec<_>>().join("\n")
            }
            4 => {
                let header = self.data.0.join("\t");
                let rows = self.data.1.iter()
                    .map(|r| r.iter().map(|c| c.display_text()).collect::<Vec<_>>().join("\t"))
                    .collect::<Vec<_>>().join("\n");
                format!("{}\n{}", header, rows)
            }
            5 => {
                let list = self.current_list();
                if let Some(name) = list.get_selected() {
                    let tables = match self.table_nav_tab {
                        NavigationTab::Tables => &db.tables,
                        NavigationTab::Views => &db.views,
                    };
                    tables.iter().find(|t| format!("{}.{}", t.schema, t.name) == name).map(|t| t.sql.clone()).unwrap_or_default()
                } else {
                    String::new()
                }
            }
            6 => self.sql_input.clone(),
            _ => String::new(),
        };
        if !text.is_empty() {
            if let Some(ref mut clipboard) = self.clipboard {
                clipboard.set_text(text)?;
            }
        }
        Ok(())
    }

    fn draw_nav_lists(&mut self, frame: &mut Frame, area: Rect) {
        if self.is_searching() {
            let items = self.filtered_list.display_text().to_vec();
            frame.render_stateful_widget(
                string_list::to_widget(&items),
                area,
                &mut self.filtered_list.list_state,
            );
        } else {
            match self.table_nav_tab {
                NavigationTab::Tables => {
                    let items = self.tables_list.display_text().to_vec();
                    frame.render_stateful_widget(
                        string_list::to_widget(&items),
                        area,
                        &mut self.tables_list.list_state,
                    );
                }
                NavigationTab::Views => {
                    let items = self.view_list.display_text().to_vec();
                    frame.render_stateful_widget(
                        string_list::to_widget(&items),
                        area,
                        &mut self.view_list.list_state,
                    );
                }
            }
        }
    }

    fn get_selected_table<'a>(&self, db: &'a Db) -> Option<&'a app::Table> {
        let list = self.current_list();
        match self.table_nav_tab {
            NavigationTab::Tables => list
                .get_selected()
                .and_then(|name| db.tables.iter().find(|x| format!("{}.{}", x.schema, x.name) == name)),
            NavigationTab::Views => list
                .get_selected()
                .and_then(|name| db.views.iter().find(|x| format!("{}.{}", x.schema, x.name) == name)),
        }
    }

    fn draw_body(&mut self, frame: &mut Frame, table: &app::Table, r: Rect) {
        let total_rows = table.row_count.unwrap_or(self.data.1.len());
        let lay = Layout::vertical([Constraint::Fill(1), Constraint::Length(3)])
            .margin(2)
            .split(r);

        let [table_area, scroll_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(1)]).areas(lay[0]);

        self.draw_table(frame, table_area, table);
        self.draw_scrollbar(frame, scroll_area, total_rows);
        self.draw_preview(frame, lay[1]);
    }

    fn draw_scrollbar(&mut self, frame: &mut Frame, area: Rect, total_rows: usize) {
        let row = self.table_state.selected_cell().map(|(r, _)| r).unwrap_or(0);
        let virtual_pos = self.page * self.page_size + row;
        self.scroll_state = ScrollbarState::new(total_rows).position(virtual_pos);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .thumb_style(Style::new().fg(DIM_COLOR))
                .track_style(Style::new().fg(DIM_COLOR)),
            area,
            &mut self.scroll_state,
        );
    }

    fn draw_preview(&mut self, frame: &mut Frame, table_inner: Rect) {
        if let Some((row, col)) = self.table_state.selected_cell() {
            let data_col = col.saturating_sub(1);
            let col_name = self.data.0.get(data_col).map(|s| s.as_str()).unwrap_or("");

            if let Some(data_row) = self.data.1.get(row) {
                if let Some(cell) = data_row.get(data_col) {
                    let display_text = cell.display_text();
                    let value_style = match cell {
                        CellType::Null => NULL_STYLE,
                        CellType::Blob => BLOB_STYLE,
                        CellType::Text(_) => Style::new().fg(TEXT_COLOR),
                    };

                    let title = Line::from(vec![
                        " Preview ".fg(SECONDARY_COLOR).bold(),
                        format!("({},{}) ", self.page * self.page_size + row + 1, data_col + 1).fg(DIM_COLOR),
                        "│ ".fg(DIM_COLOR),
                        col_name.to_string().fg(HIGHLIGHTED_COLOR),
                    ])
                    .left_aligned();

                    let p = Paragraph::new(display_text)
                        .wrap(Wrap { trim: true })
                        .style(value_style)
                        .block(
                            Block::bordered()
                                .border_type(BorderType::Rounded)
                                .fg(PRIMARY_COLOR)
                                .title(title),
                        );
                    frame.render_widget(p, table_inner);
                }
            }
        }
    }

    fn draw_query(&mut self, frame: &mut Frame, area: Rect) {
        let areas = Layout::vertical([Constraint::Length(5), Constraint::Fill(1)])
            .margin(2)
            .split(area);
        let input_area = areas[0];
        let result_area = areas[1];

        let input_title = Line::from(vec![
            " SQL ".fg(SECONDARY_COLOR).bold(),
            " Ctrl+S run ".fg(DIM_COLOR),
            " Tab accept ".fg(DIM_COLOR),
        ])
        .left_aligned();
        let input_text = if self.sql_input.is_empty() {
            Line::from(vec!["SELECT * FROM ...;".fg(DIM_COLOR)])
        } else {
            let mut highlighted = highlight_sql(&self.sql_input);
            highlighted.spans.push("█".fg(HIGHLIGHTED_COLOR));
            highlighted
        };
        frame.render_widget(
            Paragraph::new(input_text).block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .fg(PRIMARY_COLOR)
                    .title(input_title),
            ),
            input_area,
        );

        if !self.suggestions.is_empty() {
            let max_visible = self.suggestions.len().min(6);
            let popup_h = max_visible as u16 + 2;
            let popup_w = self.suggestions.iter().map(|s| s.len()).max().unwrap_or(10).max(16) as u16 + 4;
            let input_inner = Layout::new(ratatui::layout::Direction::Vertical, [Constraint::Length(3)])
                .margin(1)
                .split(input_area);
            let suggestion_area = Rect {
                x: input_inner[0].x,
                y: input_inner[0].y + 1,
                width: popup_w.min(input_inner[0].width),
                height: popup_h,
            };
            let items: Vec<ratatui::widgets::ListItem> = self.suggestions.iter().enumerate().map(|(i, s)| {
                let style = if self.suggestion_selected == Some(i) {
                    HIGHLIGHT_STYLE
                } else {
                    Style::new().fg(TEXT_COLOR)
                };
                ratatui::widgets::ListItem::new(Line::from(format!(" {} ", s)).style(style))
            }).collect();
            let list = ratatui::widgets::List::new(items).block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .fg(PRIMARY_COLOR)
                    .style(Style::default().bg(Color::Rgb(16, 16, 24))),
            );
            frame.render_widget(ratatui::widgets::Clear, suggestion_area);
            frame.render_widget(list, suggestion_area);
        }

        if let Some(err) = &self.sql_error {
            let p = Paragraph::new(err.as_str())
                .fg(Color::Red)
                .block(
                    Block::bordered()
                        .border_type(BorderType::Rounded)
                        .fg(PRIMARY_COLOR)
                        .title(" Error "),
                );
            frame.render_widget(p, result_area);
        } else if let Some(ref result) = self.sql_result {
            let (columns, data) = result;
            let num_rows = data.len();
            let rn_width = if num_rows > 0 { format!("{}", num_rows).len() } else { 1 };
            let mut widths: Vec<usize> = vec![rn_width];
            for col_name in columns.iter() {
                widths.push(col_name.len());
            }
            for row_data in data.iter() {
                for (j, cell) in row_data.iter().enumerate() {
                    let len = match cell {
                        CellType::Text(s) => s.len(),
                        CellType::Null => 4,
                        CellType::Blob => 6,
                    };
                    if j + 1 < widths.len() {
                        widths[j + 1] = widths[j + 1].max(len);
                    }
                }
            }
            let mut header_cells: Vec<Cell> = vec![Cell::from("#").style(ROW_NUM_STYLE)];
            for col_name in columns.iter() {
                header_cells.push(Cell::from(col_name.as_str()));
            }
            let rows: Vec<Row> = data.iter().enumerate().map(|(i, row_data)| {
                let mut cells: Vec<Cell> = vec![Cell::from(format!("{}", i + 1)).style(ROW_NUM_STYLE)];
                for cell in row_data.iter() {
                    cells.push(match cell {
                        CellType::Text(s) => Cell::from(s.as_str()),
                        CellType::Null => Cell::from("null").style(NULL_STYLE),
                        CellType::Blob => Cell::from("[Blob]").style(BLOB_STYLE),
                    });
                }
                let mut row_style = Style::new();
                if i % 2 != 0 { row_style = row_style.bg(Color::Rgb(24, 24, 32)); }
                Row::new(cells).style(row_style)
            }).collect();
            let constraints: Vec<Constraint> = widths.iter().map(|w| Constraint::Min(*w as u16)).collect();
            let title = Line::from(vec![
                " Result ".fg(SECONDARY_COLOR).bold(),
                format!("── {} rows · {} cols ", num_rows, columns.len()).fg(DIM_COLOR),
            ]).left_aligned();
            let table_widget = Table::new(rows, constraints)
                .column_spacing(2)
                .style(Style::new().fg(TEXT_COLOR))
                .header(Row::new(header_cells).underlined().bold())
                .block(Block::bordered().padding(Padding::uniform(1)).border_type(BorderType::Rounded).fg(PRIMARY_COLOR).title(title))
                .cell_highlight_style(Style::new().reversed());
            frame.render_widget(table_widget, result_area);
        } else {
            frame.render_widget(
                Paragraph::new("Run a query to see results here")
                    .fg(DIM_COLOR)
                    .block(
                        Block::bordered()
                            .border_type(BorderType::Rounded)
                            .fg(PRIMARY_COLOR),
                    ),
                result_area,
            );
        }
    }

    pub fn draw_table(&mut self, frame: &mut Frame, area: Rect, table: &app::Table) {
        let (columns, data) = &self.data;
        let num_rows = data.len();
        let num_cols = columns.len();
        let page_offset = self.page * self.page_size;

        let rn_width = if num_rows > 0 {
            format!("{}", page_offset + num_rows).len()
        } else {
            1
        };
        let mut widths: Vec<usize> = vec![rn_width];
        for col_name in columns.iter() {
            widths.push(col_name.len());
        }
        for row_data in data.iter() {
            for (j, cell) in row_data.iter().enumerate() {
                let len = match cell {
                    CellType::Text(s) => s.len(),
                    CellType::Null => 4,
                    CellType::Blob => 6,
                };
                if j + 1 < widths.len() {
                    widths[j + 1] = widths[j + 1].max(len);
                }
            }
        }

        let mut header_cells: Vec<Cell> = vec![Cell::from("#").style(ROW_NUM_STYLE)];
        for col_name in columns.iter() {
            header_cells.push(Cell::from(col_name.as_str()));
        }

        let rows: Vec<Row> = data
            .iter()
            .enumerate()
            .map(|(i, row_data)| {
                let mut cells: Vec<Cell> =
                    vec![Cell::from(format!("{}", page_offset + i + 1)).style(ROW_NUM_STYLE)];
                for cell in row_data.iter() {
                    let c = match cell {
                        CellType::Text(s) => Cell::from(s.as_str()),
                        CellType::Null => Cell::from("null").style(NULL_STYLE),
                        CellType::Blob => Cell::from("[Blob]").style(BLOB_STYLE),
                    };
                    cells.push(c);
                }
                let mut row_style = Style::new();
                if i % 2 != 0 {
                    row_style = row_style.bg(Color::Rgb(24, 24, 32));
                }
                Row::new(cells).style(row_style)
            })
            .collect();

        let constraints: Vec<Constraint> = widths
            .iter()
            .map(|w| Constraint::Min(*w as u16))
            .collect();

        let total_rows = table.row_count.unwrap_or(num_rows);
        let display_name = if table.schema == "public" {
            table.name.clone()
        } else {
            format!("{}.{}", table.schema, table.name)
        };
        let row_start = if num_rows > 0 { page_offset + 1 } else { 0 };
        let row_end = page_offset + num_rows;
        let title = Line::from(vec![
            format!(" {} ", display_name).fg(SECONDARY_COLOR).bold(),
            format!("── {}-{} of {} rows · {} cols ", row_start, row_end, total_rows, num_cols).fg(DIM_COLOR),
        ])
        .left_aligned();

        let table_widget = Table::new(rows, constraints)
            .column_spacing(2)
            .style(Style::new().fg(TEXT_COLOR))
            .header(Row::new(header_cells).underlined().bold())
            .block(
                Block::bordered()
                    .padding(Padding::uniform(1))
                    .border_type(BorderType::Rounded)
                    .fg(PRIMARY_COLOR)
                    .title(title),
            )
            .cell_highlight_style(Style::new().reversed());

        frame.render_stateful_widget(table_widget, area, &mut self.table_state);
        self.table_scroll_height = area.height / 2;
    }

    pub async fn handle_input(
        &mut self,
        key: &KeyEvent,
        app: &mut App,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(db) = &app.current_db {
            if self.copy_menu_open {
                match key.code {
                    KeyCode::Esc => {
                        self.copy_menu_open = false;
                        return Ok(());
                    }
                    KeyCode::Up => {
                        self.copy_menu_state.select_previous();
                        return Ok(());
                    }
                    KeyCode::Down => {
                        self.copy_menu_state.select_next();
                        return Ok(());
                    }
                    KeyCode::Enter => {
                        let idx = self.copy_menu_state.selected().unwrap_or(0);
                        self.do_copy(idx, db)?;
                        self.copy_menu_open = false;
                        return Ok(());
                    }
                    _ => return Ok(()),
                }
            }

            if self.is_searching() {
                match key.code {
                    KeyCode::Esc => {
                        self.search_query.clear();
                        self.search_mode = false;
                        self.filtered_list = StringList::default();
                        return Ok(());
                    }
                    KeyCode::Enter => {
                        self.search_mode = false;
                        self.load_table_data(app).await?;
                        return Ok(());
                    }
                    KeyCode::Backspace => {
                        self.search_query.pop();
                        self.update_filter();
                        return Ok(());
                    }
                    KeyCode::Char(c) => {
                        self.search_query.push(c);
                        self.update_filter();
                        return Ok(());
                    }
                    KeyCode::Up => {
                        self.filtered_list.list_state.select_previous();
                        return Ok(());
                    }
                    KeyCode::Down => {
                        self.filtered_list.list_state.select_next();
                        return Ok(());
                    }
                    _ => return Ok(()),
                }
            }

            if self.selected_table_tab == SelectedTableTab::Query {
                match key.code {
                    KeyCode::Esc => {
                        self.suggestions.clear();
                        self.suggestion_selected = None;
                        self.sql_input.clear();
                        self.sql_result = None;
                        self.sql_error = None;
                        self.selected_table_tab = SelectedTableTab::Browse;
                        self.load_table_data(app).await?;
                        return Ok(());
                    }
                    KeyCode::Char('s') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                        self.suggestions.clear();
                        self.suggestion_selected = None;
                        self.run_sql(app).await;
                        return Ok(());
                    }
                    KeyCode::Enter => {
                        if !self.suggestions.is_empty() {
                            self.apply_suggestion();
                        }
                        return Ok(());
                    }
                    KeyCode::Tab => {
                        self.apply_suggestion();
                        return Ok(());
                    }
                    KeyCode::Up => {
                        if !self.suggestions.is_empty() {
                            if let Some(idx) = self.suggestion_selected {
                                if idx > 0 {
                                    self.suggestion_selected = Some(idx - 1);
                                }
                            }
                            return Ok(());
                        }
                    }
                    KeyCode::Down => {
                        if !self.suggestions.is_empty() {
                            if let Some(idx) = self.suggestion_selected {
                                if idx < self.suggestions.len() - 1 {
                                    self.suggestion_selected = Some(idx + 1);
                                }
                            }
                            return Ok(());
                        }
                    }
                    KeyCode::Backspace => {
                        self.sql_input.pop();
                        self.update_suggestions();
                        return Ok(());
                    }
                    KeyCode::Char('u') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                        self.sql_input.clear();
                        self.update_suggestions();
                        return Ok(());
                    }
                    KeyCode::Char('w') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                        self.sql_input.truncate(
                            self.sql_input.trim_end().len()
                                - self.sql_input.trim_end().split_whitespace().last().map(|w| w.len()).unwrap_or(0)
                        );
                        self.update_suggestions();
                        return Ok(());
                    }
                    KeyCode::Char(c) if !key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                        self.sql_input.push(c);
                        self.update_suggestions();
                        return Ok(());
                    }
                    _ => return Ok(()),
                }
            }

            if key.code == KeyCode::Esc {
                self.selected_table_tab = SelectedTableTab::Browse;
                self.load_table_data(app).await?;
                return Ok(());
            }

            if key.code == KeyCode::Char('/') {
                self.search_mode = true;
                self.search_query.clear();
                self.update_filter();
                return Ok(());
            } else if key.code == KeyCode::Char('u') {
                if self.page > 0 {
                    self.page -= 1;
                    self.load_current_page(app).await?;
                }
                return Ok(());
            } else if key.code == KeyCode::Char('d') {
                let total = self.total_rows_for_current(app);
                if (self.page + 1) * self.page_size < total {
                    self.page += 1;
                    self.load_current_page(app).await?;
                }
                return Ok(());
            } else if key.code == KeyCode::Char('e') {
                self.table_nav_tab = self.table_nav_tab.next();
            } else if key.code == KeyCode::Char('q') {
                self.table_nav_tab = self.table_nav_tab.previous();
            } else if key.code == KeyCode::Char('L') {
                self.selected_table_tab = self.selected_table_tab.next();
            } else if key.code == KeyCode::Char('H') {
                self.selected_table_tab = self.selected_table_tab.previous();
            } else if key.code == KeyCode::Char(';') {
                self.selected_table_tab = SelectedTableTab::Query;
            } else if key.code == KeyCode::Char('K') || key.code == KeyCode::Up {
                self.active_list_mut().list_state.select_previous();
            } else if key.code == KeyCode::Char('J') || key.code == KeyCode::Down {
                self.active_list_mut().list_state.select_next();
            } else if key.code == KeyCode::Char('y') {
                self.yank_cell()?;
                return Ok(());
            } else if key.code == KeyCode::Char('c') {
                self.copy_menu_open = true;
                self.copy_menu_state.select(Some(0));
                return Ok(());
            } else if key.code == KeyCode::Char('j') || key.code == KeyCode::Left {
                self.table_state.scroll_left_by(self.h_scroll_amount);
                return Ok(());
            } else if key.code == KeyCode::Char('l') || key.code == KeyCode::Right {
                self.table_state.scroll_right_by(self.h_scroll_amount);
                return Ok(());
            } else if key.code == KeyCode::Char('i') {
                self.move_cell(-1, 0);
                return Ok(());
            } else if key.code == KeyCode::Char('k') {
                self.move_cell(1, 0);
                return Ok(());
            } else {
                return Ok(());
            }
            self.load_table_data(app).await?;
        }
        Ok(())
    }

    async fn run_sql(&mut self, app: &App) {
        if self.sql_input.trim().is_empty() {
            return;
        }
        match app.execute_sql(&self.sql_input).await {
            Ok(result) => {
                self.sql_result = Some(result);
                self.sql_error = None;
            }
            Err(e) => {
                self.sql_error = Some(format!("{}", e));
                self.sql_result = None;
            }
        }
    }

    fn update_suggestions(&mut self) {
        self.suggestions.clear();
        self.suggestion_selected = None;
        let input = &self.sql_input;
        if input.is_empty() {
            return;
        }
        let last_word = input
            .rsplit(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.')
            .next()
            .unwrap_or("")
            .to_uppercase();
        if last_word.is_empty() {
            return;
        }
        let mut matches: Vec<String> = Vec::new();
        for kw in SQL_KEYWORDS {
            if kw.starts_with(&last_word) {
                matches.push(kw.to_string());
            }
        }
        for name in &self.db_table_names {
            if name.to_uppercase().starts_with(&last_word) {
                matches.push(name.clone());
            }
        }
        for name in &self.db_view_names {
            if name.to_uppercase().starts_with(&last_word) {
                matches.push(name.clone());
            }
        }
        for (table_name, columns) in &self.db_columns {
            for col in columns {
                if col.to_uppercase().starts_with(&last_word) {
                    let entry = format!("{}.{}", table_name, col);
                    if !matches.contains(&entry) {
                        matches.push(entry);
                    }
                }
            }
        }
        self.suggestions = matches;
        if !self.suggestions.is_empty() {
            self.suggestion_selected = Some(0);
        }
    }

    fn apply_suggestion(&mut self) {
        if let Some(idx) = self.suggestion_selected {
            if let Some(suggestion) = self.suggestions.get(idx).cloned() {
                let prefix = self.sql_input
                    .rsplit(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.')
                    .next()
                    .unwrap_or("")
                    .len();
                self.sql_input.truncate(self.sql_input.len() - prefix);
                self.sql_input.push_str(&suggestion);
                self.suggestions.clear();
                self.suggestion_selected = None;
            }
        }
    }

    fn move_cell(&mut self, drow: i32, dcol: i32) {
        let num_rows = self.data.1.len();
        let num_data_cols = self.data.0.len();
        if num_rows == 0 || num_data_cols == 0 {
            return;
        }
        let (row, col) = self.table_state.selected_cell().unwrap_or((0, 1));
        let new_row = (row as i32 + drow).clamp(0, (num_rows - 1) as i32) as usize;
        let new_col = (col as i32 + dcol).clamp(1, num_data_cols as i32) as usize;
        self.table_state.select_cell(Some((new_row, new_col)));
    }

    fn yank_cell(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some((row, col)) = self.table_state.selected_cell() {
            let data_col = col.saturating_sub(1);
            if let Some(data_row) = self.data.1.get(row) {
                if let Some(cell) = data_row.get(data_col) {
                    if let Some(ref mut clipboard) = self.clipboard {
                        clipboard.set_text(cell.display_text().to_string())?;
                    }
                }
            }
        }
        Ok(())
    }

    fn total_rows_for_current(&self, app: &App) -> usize {
        if let Some(name) = &self.last_loaded_table {
            if let Some(db) = &app.current_db {
                let table = match self.table_nav_tab {
                    NavigationTab::Tables => db.tables.iter().find(|x| format!("{}.{}", x.schema, x.name) == name.as_str()),
                    NavigationTab::Views => db.views.iter().find(|x| format!("{}.{}", x.schema, x.name) == name.as_str()),
                };
                if let Some(table) = table {
                    return table.row_count.unwrap_or(0);
                }
            }
        }
        0
    }

    pub async fn load_table_data(&mut self, app: &App) -> Result<(), Box<dyn std::error::Error>> {
        if self.selected_table_tab != SelectedTableTab::Browse {
            return Ok(());
        }
        let list = self.current_list();
        let table_name = list.get_selected().map(|s| s.to_string());
        if table_name.as_deref() == self.last_loaded_table.as_deref() {
            return Ok(());
        }
        self.page = 0;
        self.table_state.select_cell(Some((0, 1)));
        self.last_loaded_table = table_name.clone();
        if let Some(name) = table_name {
            if let Some(db) = &app.current_db {
                let table = match self.table_nav_tab {
                    NavigationTab::Tables => db.tables.iter().find(|x| format!("{}.{}", x.schema, x.name) == name),
                    NavigationTab::Views => db.views.iter().find(|x| format!("{}.{}", x.schema, x.name) == name),
                };
                if let Some(table) = table {
                    let total = table.row_count.unwrap_or(0);
                    if total > self.page_size {
                        self.data = app.select_page(table, 0, self.page_size).await?;
                    } else {
                        self.data = app.select(table).await?;
                    }
                }
            }
        }
        Ok(())
    }

    async fn load_current_page(&mut self, app: &App) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(name) = &self.last_loaded_table {
            if let Some(db) = &app.current_db {
                let table = match self.table_nav_tab {
                    NavigationTab::Tables => db.tables.iter().find(|x| format!("{}.{}", x.schema, x.name) == name.as_str()),
                    NavigationTab::Views => db.views.iter().find(|x| format!("{}.{}", x.schema, x.name) == name.as_str()),
                };
                if let Some(table) = table {
                    let offset = self.page * self.page_size;
                    self.data = app.select_page(table, offset, self.page_size).await?;
                    self.table_state.select_cell(Some((0, 1)));
                }
            }
        }
        Ok(())
    }
}
