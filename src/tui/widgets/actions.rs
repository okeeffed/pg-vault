use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

use crate::tui::app::App;
use crate::tui::ui::centered_rect;

pub fn render_actions_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(40, 40, f.area());

    let actions = app.available_actions();
    let items: Vec<ListItem> = actions
        .iter()
        .map(|action| {
            let style = match action {
                crate::tui::app::Action::Delete => Style::default().fg(Color::Red),
                _ => Style::default(),
            };
            ListItem::new(Line::from(Span::styled(action.label(), style)))
        })
        .collect();

    let title = app
        .selected_connection()
        .map(|(name, _)| format!(" {} ", name))
        .unwrap_or_else(|| " Actions ".to_string());

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state = ListState::default();
    state.select(Some(app.selected_action));

    f.render_widget(Clear, area);
    f.render_stateful_widget(list, area, &mut state);
}
