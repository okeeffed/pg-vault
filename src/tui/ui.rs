use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use super::app::{App, AppMode};
use super::widgets::{
    actions::render_actions_popup,
    add_form::render_add_form,
    connection_list::render_connection_list,
    profile_selector::render_profile_selector,
};

pub fn draw(f: &mut Frame, app: &App) {
    let show_search_bar = app.mode == AppMode::Search || !app.search_query.is_empty();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if show_search_bar {
            vec![
                Constraint::Length(3),  // Header
                Constraint::Length(3),  // Search bar
                Constraint::Min(0),     // Content
                Constraint::Length(3),  // Footer
            ]
        } else {
            vec![
                Constraint::Length(3),  // Header
                Constraint::Min(0),     // Content
                Constraint::Length(3),  // Footer
            ]
        })
        .split(f.area());

    // Header
    render_header(f, chunks[0]);

    if show_search_bar {
        // Search bar
        render_search_bar(f, chunks[1], app);
        // Main content - connection list
        render_connection_list(f, chunks[2], app);
        // Footer with keybindings
        render_footer(f, chunks[3], app);
    } else {
        // Main content - connection list
        render_connection_list(f, chunks[1], app);
        // Footer with keybindings
        render_footer(f, chunks[2], app);
    }

    // Render popup overlays based on mode
    match app.mode {
        AppMode::Actions => render_actions_popup(f, app),
        AppMode::AddForm => render_add_form(f, app),
        AppMode::ProfileSelector => render_profile_selector(f, app),
        AppMode::ConfirmDelete => render_confirm_delete(f, app),
        AppMode::ConfirmQuit => render_confirm_quit(f),
        AppMode::List | AppMode::Connecting | AppMode::Search => {}
    }

    // Status message (if any)
    if let Some(ref msg) = app.status_message {
        render_status_message(f, msg);
    }
}

fn render_header(f: &mut Frame, area: Rect) {
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " pg-vault ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("- PostgreSQL Credential Manager"),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(header, area);
}

fn render_search_bar(f: &mut Frame, area: Rect, app: &App) {
    let is_searching = app.mode == AppMode::Search;
    let match_info = if app.search_matches.is_empty() && !app.search_query.is_empty() {
        Span::styled(" (no matches)", Style::default().fg(Color::Red))
    } else if !app.search_matches.is_empty() {
        Span::styled(
            format!(" ({}/{})", app.search_match_index + 1, app.search_matches.len()),
            Style::default().fg(Color::Green),
        )
    } else {
        Span::raw("")
    };

    let cursor = if is_searching { "_" } else { "" };

    let search = Paragraph::new(Line::from(vec![
        Span::styled("/", Style::default().fg(Color::Yellow)),
        Span::raw(&app.search_query),
        Span::styled(cursor, Style::default().add_modifier(Modifier::SLOW_BLINK)),
        match_info,
    ]))
    .block(
        Block::default()
            .title(" Search ")
            .borders(Borders::ALL)
            .border_style(if is_searching {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            }),
    );
    f.render_widget(search, area);
}

fn render_footer(f: &mut Frame, area: Rect, app: &App) {
    let keybindings = match app.mode {
        AppMode::List => {
            if !app.search_matches.is_empty() {
                vec![
                    ("q", "Quit"),
                    ("j/k", "Navigate"),
                    ("n/N", "Next/Prev match"),
                    ("Esc", "Clear search"),
                    ("Enter", "Actions"),
                ]
            } else {
                vec![
                    ("q", "Quit"),
                    ("j/k", "Navigate"),
                    ("/", "Search"),
                    ("Enter", "Actions"),
                    ("a", "Add"),
                    ("d", "Delete"),
                ]
            }
        }
        AppMode::Search => {
            vec![
                ("Esc", "Cancel"),
                ("Enter", "Confirm"),
            ]
        }
        AppMode::Actions => {
            vec![
                ("Esc", "Back"),
                ("j/k", "Navigate"),
                ("Enter", "Execute"),
            ]
        }
        AppMode::AddForm => {
            vec![
                ("Esc", "Cancel"),
                ("Tab", "Next field"),
                ("Shift+Tab", "Prev field"),
                ("Enter", "Submit"),
            ]
        }
        AppMode::ProfileSelector => {
            vec![
                ("Esc", "Cancel"),
                ("j/k", "Navigate"),
                ("Enter", "Select"),
            ]
        }
        AppMode::ConfirmDelete | AppMode::ConfirmQuit => {
            vec![
                ("y/Enter", "Confirm"),
                ("n/Esc", "Cancel"),
            ]
        }
        AppMode::Connecting => {
            vec![("", "Connecting...")]
        }
    };

    let spans: Vec<Span> = keybindings
        .iter()
        .flat_map(|(key, desc)| {
            vec![
                Span::styled(
                    format!(" {} ", key),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan),
                ),
                Span::raw(format!(" {} ", desc)),
            ]
        })
        .collect();

    let footer = Paragraph::new(Line::from(spans))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    f.render_widget(footer, area);
}

fn render_confirm_delete(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 20, f.area());

    let name = app.selected_connection()
        .map(|(n, _)| n.as_str())
        .unwrap_or("unknown");

    let popup = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "Delete Connection?",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("Are you sure you want to delete '{}'?", name)),
        Line::from(""),
        Line::from("This will remove the connection and its stored password."),
        Line::from(""),
        Line::from(vec![
            Span::styled(" y ", Style::default().fg(Color::Black).bg(Color::Red)),
            Span::raw(" Yes  "),
            Span::styled(" n ", Style::default().fg(Color::Black).bg(Color::Green)),
            Span::raw(" No"),
        ]),
    ])
    .block(
        Block::default()
            .title(" Confirm Delete ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red)),
    )
    .centered();

    f.render_widget(Clear, area);
    f.render_widget(popup, area);
}

fn render_confirm_quit(f: &mut Frame) {
    let area = centered_rect(40, 20, f.area());

    let popup = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "Quit pg-vault?",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Are you sure you want to exit?"),
        Line::from(""),
        Line::from(vec![
            Span::styled(" y ", Style::default().fg(Color::Black).bg(Color::Red)),
            Span::raw(" Yes  "),
            Span::styled(" n ", Style::default().fg(Color::Black).bg(Color::Green)),
            Span::raw(" No"),
        ]),
    ])
    .block(
        Block::default()
            .title(" Confirm Quit ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    )
    .centered();

    f.render_widget(Clear, area);
    f.render_widget(popup, area);
}

fn render_status_message(f: &mut Frame, msg: &str) {
    let area = Rect {
        x: 1,
        y: f.area().height.saturating_sub(5),
        width: f.area().width.saturating_sub(2),
        height: 1,
    };

    let is_error = msg.starts_with("Error");
    let style = if is_error {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Green)
    };

    let status = Paragraph::new(Span::styled(msg, style));
    f.render_widget(status, area);
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
