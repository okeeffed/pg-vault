use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

use crate::tui::app::App;

fn highlight_match<'a>(name: &'a str, query: &str) -> Line<'a> {
    if query.is_empty() {
        return Line::from(name.to_string());
    }

    let name_lower = name.to_lowercase();
    let query_lower = query.to_lowercase();

    if let Some(start) = name_lower.find(&query_lower) {
        let end = start + query.len();
        let mut spans = Vec::new();

        if start > 0 {
            spans.push(Span::raw(name[..start].to_string()));
        }

        spans.push(Span::styled(
            name[start..end].to_string(),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));

        if end < name.len() {
            spans.push(Span::raw(name[end..].to_string()));
        }

        Line::from(spans)
    } else {
        Line::from(name.to_string())
    }
}

pub fn render_connection_list(f: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(vec![
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Auth").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().fg(Color::Cyan))
    .bottom_margin(1);

    let rows: Vec<Row> = if app.connection_names.is_empty() {
        vec![Row::new(vec![
            Cell::from("No connections stored. Press 'a' to add one.")
                .style(Style::default().fg(Color::DarkGray)),
            Cell::from(""),
        ])]
    } else {
        app.connection_names
            .iter()
            .map(|name| {
                let info = app.connections.get(name).unwrap();
                let auth_cell = if info.iam_auth {
                    Cell::from("IAM").style(
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Cell::from("PWD").style(Style::default().fg(Color::Green))
                };

                let name_cell = Cell::from(highlight_match(name, &app.search_query));
                Row::new(vec![name_cell, auth_cell])
            })
            .collect()
    };

    let widths = [Constraint::Percentage(80), Constraint::Percentage(20)];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .title(" Connections ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White)),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state = TableState::default();
    if !app.connection_names.is_empty() {
        state.select(Some(app.selected_index));
    }

    f.render_stateful_widget(table, area, &mut state);
}
