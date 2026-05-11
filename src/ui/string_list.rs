use ratatui::{
    style::Stylize,
    text::Text,
    widgets::{List, ListItem, ListState},
};

use super::{HIGHLIGHT_STYLE, SECONDARY_COLOR};

#[derive(Debug, Default)]
pub struct StringList {
    pub list_state: ListState,
    pub items: Vec<String>,
    pub display_items: Vec<String>,
}

impl StringList {
    pub fn load_items(&mut self, items: Vec<String>) {
        self.load_items_with_display(items.clone(), items);
    }

    pub fn load_items_with_display(&mut self, items: Vec<String>, display_items: Vec<String>) {
        self.list_state
            .select(if !items.is_empty() { Some(0) } else { None });
        self.items = items;
        self.display_items = display_items;
    }

    pub fn get_selected(&self) -> Option<&str> {
        if let Some(selected) = self.list_state.selected() {
            if let Some(path) = self.items.get(selected) {
                return Some(path.as_str());
            }
        }
        None
    }

    pub fn display_text(&self) -> &[String] {
        if self.display_items.is_empty() {
            &self.items
        } else {
            &self.display_items
        }
    }
}

pub fn to_widget<'a>(items: &[String]) -> List<'a> {
    List::new(
        items
            .iter()
            .map(|x| ListItem::from(Text::from(x.to_string()).fg(SECONDARY_COLOR).bold()))
            .collect::<Vec<ListItem>>(),
    )
    .highlight_style(HIGHLIGHT_STYLE)
    .highlight_symbol("▸")
}
