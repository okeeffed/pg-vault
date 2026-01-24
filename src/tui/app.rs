use anyhow::{Context, Result};
use std::collections::HashMap;
use std::process::{Command, Stdio};

use crate::aws::{generate_iam_token, list_aws_profiles};
use crate::config::{load_connections, save_connections, ConnectionInfo};
use crate::credentials::{get_password, remove_password, store_password};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppMode {
    List,
    Actions,
    AddForm,
    ProfileSelector,
    #[allow(dead_code)]
    Connecting,
    ConfirmDelete,
    ConfirmQuit,
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    Connect,
    IamConnect,
    Session,
    Delete,
}

impl Action {
    pub fn label(&self) -> &'static str {
        match self {
            Action::Connect => "Connect (psql)",
            Action::IamConnect => "IAM Connect",
            Action::Session => "Session (shell with env vars)",
            Action::Delete => "Delete",
        }
    }

    pub fn available_actions(is_iam: bool) -> Vec<Action> {
        if is_iam {
            vec![Action::IamConnect, Action::Session, Action::Delete]
        } else {
            vec![Action::Connect, Action::Session, Action::Delete]
        }
    }
}

#[derive(Debug, Default)]
pub struct FormState {
    pub name: String,
    pub host: String,
    pub port: String,
    pub database: String,
    pub username: String,
    pub password: String,
    pub iam: bool,
    pub current_field: usize,
}

impl FormState {
    pub fn reset(&mut self) {
        self.name.clear();
        self.host.clear();
        self.port = "5432".to_string();
        self.database.clear();
        self.username.clear();
        self.password.clear();
        self.iam = false;
        self.current_field = 0;
    }

    pub fn next_field(&mut self) {
        let max_field = if self.iam { 6 } else { 7 }; // Skip password if IAM
        self.current_field = (self.current_field + 1).min(max_field);

        // Skip password field if IAM is enabled
        if self.iam && self.current_field == 6 {
            self.current_field = 7; // Jump to submit
        }
    }

    pub fn prev_field(&mut self) {
        if self.current_field > 0 {
            self.current_field -= 1;

            // Skip password field if IAM is enabled
            if self.iam && self.current_field == 6 {
                self.current_field = 5; // Jump back to IAM checkbox
            }
        }
    }

    pub fn handle_char(&mut self, c: char) {
        match self.current_field {
            0 => self.name.push(c),
            1 => self.host.push(c),
            2 => {
                if c.is_ascii_digit() {
                    self.port.push(c);
                }
            }
            3 => self.database.push(c),
            4 => self.username.push(c),
            6 if !self.iam => self.password.push(c),
            _ => {}
        }
    }

    pub fn handle_backspace(&mut self) {
        match self.current_field {
            0 => { self.name.pop(); }
            1 => { self.host.pop(); }
            2 => { self.port.pop(); }
            3 => { self.database.pop(); }
            4 => { self.username.pop(); }
            6 if !self.iam => { self.password.pop(); }
            _ => {}
        }
    }

    pub fn field_labels() -> &'static [&'static str] {
        &["Name", "Host", "Port", "Database", "Username", "IAM Auth", "Password", "Submit"]
    }

    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            anyhow::bail!("Name is required");
        }
        if self.host.is_empty() {
            anyhow::bail!("Host is required");
        }
        if self.port.is_empty() {
            anyhow::bail!("Port is required");
        }
        if self.database.is_empty() {
            anyhow::bail!("Database is required");
        }
        if self.username.is_empty() {
            anyhow::bail!("Username is required");
        }
        if !self.iam && self.password.is_empty() {
            anyhow::bail!("Password is required for non-IAM connections");
        }
        Ok(())
    }
}

pub enum PendingAction {
    Psql(Box<dyn FnOnce() -> Result<()>>),
    IamConnect {
        connection_info: ConnectionInfo,
        profile: Option<String>,
    },
}

pub struct App {
    pub connections: HashMap<String, ConnectionInfo>,
    pub connection_names: Vec<String>,
    pub selected_index: usize,
    pub mode: AppMode,
    pub selected_action: usize,
    pub form_state: FormState,
    pub aws_profiles: Vec<String>,
    pub selected_profile: usize,
    pub status_message: Option<String>,
    pub should_quit: bool,
    pub pending_action: Option<PendingAction>,
    pub search_query: String,
    pub search_matches: Vec<usize>,
    pub search_match_index: usize,
    pub profile_search_query: String,
    pub profile_search_matches: Vec<usize>,
    pub profile_search_active: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        let connections = load_connections()?;
        let mut connection_names: Vec<String> = connections.keys().cloned().collect();
        connection_names.sort();

        let aws_profiles = list_aws_profiles();

        Ok(Self {
            connections,
            connection_names,
            selected_index: 0,
            mode: AppMode::List,
            selected_action: 0,
            form_state: FormState::default(),
            aws_profiles,
            selected_profile: 0,
            status_message: None,
            should_quit: false,
            pending_action: None,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_match_index: 0,
            profile_search_query: String::new(),
            profile_search_matches: Vec::new(),
            profile_search_active: false,
        })
    }

    pub fn reload_connections(&mut self) -> Result<()> {
        self.connections = load_connections()?;
        self.connection_names = self.connections.keys().cloned().collect();
        self.connection_names.sort();

        // Adjust selected index if needed
        if self.selected_index >= self.connection_names.len() && !self.connection_names.is_empty() {
            self.selected_index = self.connection_names.len() - 1;
        }
        Ok(())
    }

    pub fn selected_connection(&self) -> Option<(&String, &ConnectionInfo)> {
        self.connection_names
            .get(self.selected_index)
            .and_then(|name| self.connections.get(name).map(|info| (name, info)))
    }

    pub fn next_connection(&mut self) {
        if !self.connection_names.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.connection_names.len();
        }
    }

    pub fn prev_connection(&mut self) {
        if !self.connection_names.is_empty() {
            self.selected_index = self.selected_index
                .checked_sub(1)
                .unwrap_or(self.connection_names.len() - 1);
        }
    }

    pub fn available_actions(&self) -> Vec<Action> {
        if let Some((_, info)) = self.selected_connection() {
            Action::available_actions(info.iam_auth)
        } else {
            vec![]
        }
    }

    pub fn next_action(&mut self) {
        let actions = self.available_actions();
        if !actions.is_empty() {
            self.selected_action = (self.selected_action + 1) % actions.len();
        }
    }

    pub fn prev_action(&mut self) {
        let actions = self.available_actions();
        if !actions.is_empty() {
            self.selected_action = self.selected_action
                .checked_sub(1)
                .unwrap_or(actions.len() - 1);
        }
    }

    pub fn next_profile(&mut self) {
        if !self.aws_profiles.is_empty() {
            self.selected_profile = (self.selected_profile + 1) % self.aws_profiles.len();
        }
    }

    pub fn prev_profile(&mut self) {
        if !self.aws_profiles.is_empty() {
            self.selected_profile = self.selected_profile
                .checked_sub(1)
                .unwrap_or(self.aws_profiles.len() - 1);
        }
    }

    pub fn update_search_matches(&mut self) {
        let query = self.search_query.to_lowercase();
        self.search_matches = self
            .connection_names
            .iter()
            .enumerate()
            .filter(|(_, name)| name.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();

        // Jump to first match if there are any
        if !self.search_matches.is_empty() {
            self.search_match_index = 0;
            self.selected_index = self.search_matches[0];
        }
    }

    pub fn next_match(&mut self) {
        if !self.search_matches.is_empty() {
            self.search_match_index = (self.search_match_index + 1) % self.search_matches.len();
            self.selected_index = self.search_matches[self.search_match_index];
        }
    }

    pub fn prev_match(&mut self) {
        if !self.search_matches.is_empty() {
            self.search_match_index = self
                .search_match_index
                .checked_sub(1)
                .unwrap_or(self.search_matches.len() - 1);
            self.selected_index = self.search_matches[self.search_match_index];
        }
    }

    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_matches.clear();
        self.search_match_index = 0;
    }

    pub fn update_profile_search_matches(&mut self) {
        let query = self.profile_search_query.to_lowercase();
        self.profile_search_matches = self
            .aws_profiles
            .iter()
            .enumerate()
            .filter(|(_, name)| name.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();

        // Jump to first match if there are any
        if !self.profile_search_matches.is_empty() {
            self.selected_profile = self.profile_search_matches[0];
        }
    }

    pub fn clear_profile_search(&mut self) {
        self.profile_search_query.clear();
        self.profile_search_matches.clear();
        self.profile_search_active = false;
    }

    pub fn execute_action(&mut self) -> Result<()> {
        let actions = self.available_actions();
        let action = actions.get(self.selected_action).copied();

        let Some((name, info)) = self.selected_connection() else {
            return Ok(());
        };
        let name = name.clone();
        let info = info.clone();

        match action {
            Some(Action::Connect) => {
                self.mode = AppMode::List;
                match get_password(&name) {
                    Ok(password) => {
                        self.pending_action = Some(PendingAction::Psql(Box::new(move || {
                            spawn_psql(&info, &password)
                        })));
                    }
                    Err(e) => {
                        self.status_message = Some(format!(
                            "Error: Could not retrieve password for '{}': {}",
                            name, e
                        ));
                    }
                }
            }
            Some(Action::IamConnect) => {
                // Show profile selector
                self.selected_profile = 0;
                self.mode = AppMode::ProfileSelector;
            }
            Some(Action::Session) => {
                self.mode = AppMode::List;

                let password_result = if info.iam_auth {
                    // For IAM connections, generate token with default profile
                    generate_iam_token(&info.host, info.port, &info.username, None)
                } else {
                    get_password(&name)
                };

                match password_result {
                    Ok(password) => {
                        self.pending_action = Some(PendingAction::Psql(Box::new(move || {
                            spawn_session(&info, &password)
                        })));
                    }
                    Err(e) => {
                        self.status_message = Some(format!(
                            "Error: Could not retrieve credentials for '{}': {}",
                            name, e
                        ));
                    }
                }
            }
            Some(Action::Delete) => {
                self.mode = AppMode::ConfirmDelete;
            }
            None => {}
        }
        Ok(())
    }

    pub fn connect_with_profile(&mut self) -> Result<()> {
        let Some((_name, info)) = self.selected_connection() else {
            return Ok(());
        };
        let info = info.clone();
        let profile = self.aws_profiles.get(self.selected_profile).cloned();

        self.mode = AppMode::List;
        self.status_message = Some(format!(
            "Connecting with profile '{}'...",
            profile.as_deref().unwrap_or("default")
        ));

        // Defer IAM token generation to the spawn handler so we can show loading state
        self.pending_action = Some(PendingAction::IamConnect {
            connection_info: info,
            profile,
        });

        Ok(())
    }

    pub fn retry_iam_connect(&mut self, info: ConnectionInfo, profile: Option<String>) {
        self.status_message = Some("Retrying IAM connection...".to_string());
        self.pending_action = Some(PendingAction::IamConnect {
            connection_info: info,
            profile,
        });
    }

    pub fn delete_selected_connection(&mut self) -> Result<()> {
        let Some((name, _)) = self.selected_connection() else {
            return Ok(());
        };
        let name = name.clone();

        self.connections.remove(&name);
        save_connections(&self.connections)?;

        // Try to remove password, but don't fail if it doesn't exist
        let _ = remove_password(&name);

        self.reload_connections()?;
        self.status_message = Some(format!("Connection '{}' deleted", name));
        Ok(())
    }

    pub fn submit_form(&mut self) -> Result<()> {
        self.form_state.validate()?;

        let port: u16 = self.form_state.port.parse()
            .context("Invalid port number")?;

        let info = ConnectionInfo {
            host: self.form_state.host.clone(),
            port,
            database: self.form_state.database.clone(),
            username: self.form_state.username.clone(),
            iam_auth: self.form_state.iam,
        };

        let name = self.form_state.name.clone();
        let password = self.form_state.password.clone();

        self.connections.insert(name.clone(), info);
        save_connections(&self.connections)?;

        if !self.form_state.iam {
            store_password(&name, &password)?;
        }

        self.reload_connections()?;
        self.mode = AppMode::List;
        self.form_state.reset();
        Ok(())
    }
}

fn spawn_psql(info: &ConnectionInfo, password: &str) -> Result<()> {
    let mut cmd = Command::new("psql");
    cmd.arg(format!(
        "postgres://{}:{}@{}:{}/{}",
        info.username,
        password,
        info.host,
        info.port,
        info.database
    ))
    .env("PGPASSWORD", password)
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

fn spawn_session(info: &ConnectionInfo, password: &str) -> Result<()> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

    let mut cmd = Command::new(&shell);
    cmd.env("PGHOST", &info.host)
        .env("PGPORT", info.port.to_string())
        .env("PGDATABASE", &info.database)
        .env("PGUSER", &info.username)
        .env("PGPASSWORD", password)
        .env(
            "DATABASE_URL",
            format!(
                "postgres://{}:{}@{}:{}/{}",
                info.username,
                password,
                info.host,
                info.port,
                info.database
            ),
        )
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = cmd.status().context("Failed to start shell session")?;

    if !status.success() {
        anyhow::bail!("Shell session exited with error code: {:?}", status.code());
    }
    Ok(())
}
