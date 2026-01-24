use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::tui::app::App;
use crate::tui::ui::centered_rect;

fn highlight_profile_match<'a>(name: &'a str, query: &str, is_default: bool) -> Line<'a> {
    let suffix = if is_default { " (default)" } else { "" };
    let base_style = if is_default {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    if query.is_empty() {
        return Line::from(Span::styled(format!("{}{}", name, suffix), base_style));
    }

    let name_lower = name.to_lowercase();
    let query_lower = query.to_lowercase();

    if let Some(start) = name_lower.find(&query_lower) {
        let end = start + query.len();
        let mut spans = Vec::new();

        if start > 0 {
            spans.push(Span::styled(name[..start].to_string(), base_style));
        }

        spans.push(Span::styled(
            name[start..end].to_string(),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

        if end < name.len() {
            spans.push(Span::styled(name[end..].to_string(), base_style));
        }

        spans.push(Span::styled(suffix.to_string(), base_style));
        Line::from(spans)
    } else {
        Line::from(Span::styled(format!("{}{}", name, suffix), base_style))
    }
}

pub fn render_profile_selector(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 60, f.area());

    // Split area for search bar and list
    let show_search = app.profile_search_active || !app.profile_search_query.is_empty();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if show_search {
            vec![Constraint::Length(3), Constraint::Min(0), Constraint::Length(2)]
        } else {
            vec![Constraint::Min(0), Constraint::Length(2)]
        })
        .split(area);

    f.render_widget(Clear, area);

    let (list_area, footer_area) = if show_search {
        // Render search bar
        let search_area = chunks[0];
        let cursor = if app.profile_search_active { "_" } else { "" };
        let match_info = if app.profile_search_matches.is_empty() && !app.profile_search_query.is_empty() {
            Span::styled(" (no matches)", Style::default().fg(Color::Red))
        } else if !app.profile_search_matches.is_empty() {
            Span::styled(
                format!(" ({} matches)", app.profile_search_matches.len()),
                Style::default().fg(Color::Green),
            )
        } else {
            Span::raw("")
        };

        let search = Paragraph::new(Line::from(vec![
            Span::styled("/", Style::default().fg(Color::Yellow)),
            Span::raw(&app.profile_search_query),
            Span::styled(cursor, Style::default().add_modifier(Modifier::SLOW_BLINK)),
            match_info,
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if app.profile_search_active {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::DarkGray)
                }),
        );
        f.render_widget(search, search_area);

        (chunks[1], chunks[2])
    } else {
        (chunks[0], chunks[1])
    };

    // Build list items
    let items: Vec<ListItem> = if app.aws_profiles.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "No AWS profiles found",
            Style::default().fg(Color::DarkGray),
        )))]
    } else {
        app.aws_profiles
            .iter()
            .map(|profile| {
                let is_default = profile == "default";
                ListItem::new(highlight_profile_match(profile, &app.profile_search_query, is_default))
            })
            .collect()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Select AWS Profile ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state = ListState::default();
    if !app.aws_profiles.is_empty() {
        state.select(Some(app.selected_profile));
    }

    f.render_stateful_widget(list, list_area, &mut state);

    // Footer with keybindings
    let footer_text = if app.profile_search_active {
        "Esc: Cancel  Enter: Confirm"
    } else {
        "j/k: Navigate  /: Search  Enter: Select  Esc: Back"
    };
    let footer = Paragraph::new(Span::styled(
        footer_text,
        Style::default().fg(Color::DarkGray),
    ))
    .centered();
    f.render_widget(footer, footer_area);
}
