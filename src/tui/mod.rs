pub mod app;
pub mod ui;
pub mod widgets;

use anyhow::{Context, Result};
use crossterm::{
    cursor::Show,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use signal_hook::consts::SIGINT;
use signal_hook::flag;
use ratatui::prelude::*;
use std::io::{self, stdout, Write};
use std::panic;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use app::{App, AppMode, PendingAction};
use ui::draw;

use crate::aws::{generate_iam_token, needs_sso_login};

pub fn run() -> Result<()> {
    // Set up panic hook to restore terminal on panic
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        original_hook(panic_info);
    }));

    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new()?;

    // Main event loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    restore_terminal()?;

    result
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, Show)?;
    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;

        // Poll for events with a timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Handle Ctrl+C globally - show quit confirmation
                    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                        app.mode = AppMode::ConfirmQuit;
                        continue;
                    }

                    match app.mode {
                        AppMode::List => handle_list_input(app, key.code)?,
                        AppMode::Actions => handle_actions_input(app, key.code)?,
                        AppMode::AddForm => handle_form_input(app, key.code),
                        AppMode::ProfileSelector => handle_profile_input(app, key.code)?,
                        AppMode::Connecting => {}
                        AppMode::ConfirmDelete => handle_confirm_delete_input(app, key.code)?,
                        AppMode::ConfirmQuit => handle_confirm_quit_input(app, key.code),
                        AppMode::Search => handle_search_input(app, key.code),
                    }

                    if app.should_quit {
                        return Ok(());
                    }

                    // Handle pending actions (spawning external processes, IAM connections, etc.)
                    if let Some(pending) = app.pending_action.take() {
                        handle_pending_action(terminal, app, pending)?;
                    }
                }
            }
        }
    }
}

fn handle_pending_action<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    pending: PendingAction,
) -> Result<()> {
    match pending {
        PendingAction::Psql(action) => {
            // Simple spawn - suspend TUI and run
            suspend_and_run(terminal, action, app)?;
        }
        PendingAction::IamConnect { connection_info, profile } => {
            // Show loading message before suspending
            restore_terminal()?;
            print!("\x1B[2J\x1B[H");
            println!("Generating IAM authentication token...");
            println!("Profile: {}", profile.as_deref().unwrap_or("default"));
            println!();
            io::stdout().flush()?;

            // Generate IAM token (this is the slow part)
            match generate_iam_token(
                &connection_info.host,
                connection_info.port,
                &connection_info.username,
                profile.as_deref(),
            ) {
                Ok(iam_token) => {
                    println!("Token generated successfully. Connecting to PostgreSQL...");
                    println!();
                    io::stdout().flush()?;

                    // Ignore SIGINT while child process runs
                    let sigint_flag = Arc::new(AtomicBool::new(false));
                    let _ = flag::register(SIGINT, Arc::clone(&sigint_flag));

                    // Spawn psql with IAM token
                    let result = spawn_psql_iam(&connection_info, &iam_token);
                    sigint_flag.store(false, Ordering::Relaxed);

                    // Resume TUI
                    enable_raw_mode()?;
                    execute!(io::stdout(), EnterAlternateScreen)?;
                    terminal.clear()?;

                    if let Err(e) = result {
                        app.status_message = Some(format!("Error: {}", e));
                    }
                }
                Err(e) => {
                    let error_str = e.to_string();
                    if needs_sso_login(&error_str) {
                        // SSO login needed
                        println!("SSO login required. Opening browser...");
                        println!();
                        io::stdout().flush()?;

                        // Ignore SIGINT while SSO login runs
                        let sigint_flag = Arc::new(AtomicBool::new(false));
                        let _ = flag::register(SIGINT, Arc::clone(&sigint_flag));

                        let sso_result = spawn_sso_login(profile.as_deref());
                        sigint_flag.store(false, Ordering::Relaxed);

                        // Resume TUI
                        enable_raw_mode()?;
                        execute!(io::stdout(), EnterAlternateScreen)?;
                        terminal.clear()?;

                        if sso_result.is_ok() {
                            // SSO login succeeded - automatically retry
                            app.status_message = Some("SSO login successful. Retrying connection...".to_string());
                            app.retry_iam_connect(connection_info, profile);
                        } else {
                            app.status_message = Some(format!("SSO login failed: {}", sso_result.unwrap_err()));
                        }
                    } else {
                        // Other error - just show message
                        enable_raw_mode()?;
                        execute!(io::stdout(), EnterAlternateScreen)?;
                        terminal.clear()?;
                        app.status_message = Some(format!("Error: Failed to generate IAM token: {}", e));
                    }
                }
            }
        }
    }
    Ok(())
}

fn suspend_and_run<B: Backend>(
    terminal: &mut Terminal<B>,
    action: Box<dyn FnOnce() -> Result<()>>,
    app: &mut App,
) -> Result<()> {
    // Suspend TUI
    restore_terminal()?;

    // Clear screen and reset cursor for clean handoff
    print!("\x1B[2J\x1B[H");
    io::stdout().flush()?;

    // Ignore SIGINT while child process runs so Ctrl+C goes to child only
    let sigint_flag = Arc::new(AtomicBool::new(false));
    let _ = flag::register(SIGINT, Arc::clone(&sigint_flag));

    // Execute the action
    let result = action();

    // Clear the flag (we don't care if Ctrl+C was pressed)
    sigint_flag.store(false, Ordering::Relaxed);

    // Resume TUI
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    terminal.clear()?;

    if let Err(e) = result {
        app.status_message = Some(format!("Error: {}", e));
    }

    Ok(())
}

fn spawn_psql_iam(info: &crate::config::ConnectionInfo, iam_token: &str) -> Result<()> {
    use std::process::{Command, Stdio};
    use urlencoding::encode;

    let encoded_token = encode(iam_token);
    let mut cmd = Command::new("psql");
    cmd.arg(format!(
        "postgres://{}:{}@{}:{}/{}?sslmode=require",
        info.username,
        encoded_token,
        info.host,
        info.port,
        info.database
    ))
    .env("PGPASSWORD", iam_token)
    .stdin(Stdio::inherit())
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit());

    // Only set pager if user hasn't configured one (respect user preferences)
    if std::env::var("PSQL_PAGER").is_err() && std::env::var("PAGER").is_err() {
        cmd.env("PAGER", "less -S -i -X");
    }

    let status = cmd.status().context(
        "Failed to execute psql command. Make sure psql is installed and in your PATH.",
    )?;

    if !status.success() {
        anyhow::bail!("psql exited with error code: {:?}", status.code());
    }
    Ok(())
}

fn spawn_sso_login(profile: Option<&str>) -> Result<()> {
    use std::process::{Command, Stdio};

    let mut cmd = Command::new("aws");
    cmd.arg("sso").arg("login");

    if let Some(profile_name) = profile {
        cmd.args(["--profile", profile_name]);
    }

    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = cmd.status().context(
        "Failed to execute AWS SSO login. Make sure AWS CLI v2 is installed.",
    )?;

    if !status.success() {
        anyhow::bail!("AWS SSO login exited with error code: {:?}", status.code());
    }
    Ok(())
}

fn handle_list_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
        KeyCode::Char('q') => app.mode = AppMode::ConfirmQuit,
        KeyCode::Char('j') | KeyCode::Down => app.next_connection(),
        KeyCode::Char('k') | KeyCode::Up => app.prev_connection(),
        KeyCode::Enter => {
            if !app.connection_names.is_empty() {
                app.mode = AppMode::Actions;
                app.selected_action = 0;
            }
        }
        KeyCode::Char('a') => {
            app.mode = AppMode::AddForm;
            app.form_state.reset();
        }
        KeyCode::Char('d') => {
            if !app.connection_names.is_empty() {
                app.mode = AppMode::ConfirmDelete;
            }
        }
        KeyCode::Char('/') => {
            app.clear_search();
            app.mode = AppMode::Search;
        }
        KeyCode::Char('n') => app.next_match(),
        KeyCode::Char('N') => app.prev_match(),
        KeyCode::Esc => app.clear_search(),
        _ => {}
    }
    Ok(())
}

fn handle_actions_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
        KeyCode::Esc => app.mode = AppMode::List,
        KeyCode::Char('j') | KeyCode::Down => app.next_action(),
        KeyCode::Char('k') | KeyCode::Up => app.prev_action(),
        KeyCode::Enter => app.execute_action()?,
        _ => {}
    }
    Ok(())
}

fn handle_form_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Esc => {
            app.mode = AppMode::List;
            app.form_state.reset();
        }
        KeyCode::Tab => app.form_state.next_field(),
        KeyCode::BackTab => app.form_state.prev_field(),
        KeyCode::Enter => {
            if app.form_state.current_field == 6 {
                // Submit button
                if let Err(e) = app.submit_form() {
                    app.status_message = Some(format!("Error: {}", e));
                } else {
                    app.status_message = Some("Connection added successfully".to_string());
                }
            }
        }
        KeyCode::Char(' ') if app.form_state.current_field == 5 => {
            // IAM checkbox toggle
            app.form_state.iam = !app.form_state.iam;
        }
        KeyCode::Char(c) => app.form_state.handle_char(c),
        KeyCode::Backspace => app.form_state.handle_backspace(),
        _ => {}
    }
}

fn handle_profile_input(app: &mut App, key: KeyCode) -> Result<()> {
    if app.profile_search_active {
        // Search mode within profile selector
        match key {
            KeyCode::Esc => {
                app.clear_profile_search();
            }
            KeyCode::Enter => {
                app.profile_search_active = false;
                // Keep the filter active but exit search input mode
            }
            KeyCode::Backspace => {
                app.profile_search_query.pop();
                app.update_profile_search_matches();
            }
            KeyCode::Char(c) => {
                app.profile_search_query.push(c);
                app.update_profile_search_matches();
            }
            _ => {}
        }
    } else {
        // Normal profile selector mode
        match key {
            KeyCode::Esc => {
                app.clear_profile_search();
                app.mode = AppMode::List;
            }
            KeyCode::Char('j') | KeyCode::Down => app.next_profile(),
            KeyCode::Char('k') | KeyCode::Up => app.prev_profile(),
            KeyCode::Enter => {
                app.clear_profile_search();
                app.connect_with_profile()?;
            }
            KeyCode::Char('/') => {
                app.profile_search_query.clear();
                app.profile_search_matches.clear();
                app.profile_search_active = true;
            }
            _ => {}
        }
    }
    Ok(())
}

fn handle_confirm_delete_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
        KeyCode::Esc | KeyCode::Char('n') => app.mode = AppMode::List,
        KeyCode::Char('y') | KeyCode::Enter => {
            app.delete_selected_connection()?;
            app.mode = AppMode::List;
        }
        _ => {}
    }
    Ok(())
}

fn handle_confirm_quit_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Esc | KeyCode::Char('n') => app.mode = AppMode::List,
        KeyCode::Char('y') | KeyCode::Enter | KeyCode::Char('q') => app.should_quit = true,
        _ => {}
    }
}

fn handle_search_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Esc => {
            app.clear_search();
            app.mode = AppMode::List;
        }
        KeyCode::Enter => {
            // Keep search active but go back to list mode
            app.mode = AppMode::List;
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            app.update_search_matches();
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            app.update_search_matches();
        }
        _ => {}
    }
}
