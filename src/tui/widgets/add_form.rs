use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::tui::app::{App, FormState};
use crate::tui::ui::centered_rect;

pub fn render_add_form(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 70, f.area());

    // Clear the area behind the popup
    f.render_widget(Clear, area);

    // Render the form border
    let block = Block::default()
        .title(" Add Connection ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(block, area);

    // Inner area for form fields
    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };

    render_form_fields(f, inner, &app.form_state);
}

fn render_form_fields(f: &mut Frame, area: Rect, form: &FormState) {
    let labels = FormState::field_labels();

    // Calculate field layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(3); 8])
        .split(area);

    // Render each field
    for (i, &label) in labels.iter().enumerate() {
        let is_selected = form.current_field == i;

        // Skip password field if IAM is enabled
        if i == 6 && form.iam {
            let disabled = Paragraph::new(Line::from(vec![
                Span::styled(
                    format!("{}: ", label),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    "(not needed for IAM)",
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                ),
            ]));
            f.render_widget(disabled, chunks[i]);
            continue;
        }

        let style = if is_selected {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };

        let border_style = if is_selected {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        match i {
            5 => {
                // IAM Auth checkbox
                let checkbox = if form.iam { "[x]" } else { "[ ]" };
                let content = Paragraph::new(Line::from(vec![
                    Span::styled(format!("{} ", checkbox), style.add_modifier(Modifier::BOLD)),
                    Span::styled("Use IAM Authentication", style),
                ]))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(border_style)
                        .title(format!(" {} ", label)),
                );
                f.render_widget(content, chunks[i]);
            }
            7 => {
                // Submit button
                let button_style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Cyan)
                };
                let content = Paragraph::new(Line::from(Span::styled(
                    "  [ Submit ]  ",
                    button_style,
                )))
                .centered();
                f.render_widget(content, chunks[i]);
            }
            _ => {
                // Text input field
                let value = match i {
                    0 => &form.name,
                    1 => &form.host,
                    2 => &form.port,
                    3 => &form.database,
                    4 => &form.username,
                    6 => &form.password,
                    _ => unreachable!(),
                };

                let display_value = if i == 6 {
                    // Password field - show asterisks
                    "*".repeat(value.len())
                } else {
                    value.clone()
                };

                let cursor = if is_selected { "_" } else { "" };

                let content = Paragraph::new(Line::from(vec![
                    Span::styled(&display_value, style),
                    Span::styled(cursor, Style::default().add_modifier(Modifier::SLOW_BLINK)),
                ]))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(border_style)
                        .title(format!(" {} ", label)),
                );
                f.render_widget(content, chunks[i]);
            }
        }
    }
}
