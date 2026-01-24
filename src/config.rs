use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct ConnectionInfo {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    #[serde(default)]
    pub iam_auth: bool,
}

pub fn get_config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir().context("Could not find config directory")?;
    let app_dir = config_dir.join("pg-vault");
    fs::create_dir_all(&app_dir).context("Could not create config directory")?;
    Ok(app_dir.join("connections.json"))
}

pub fn load_connections() -> Result<HashMap<String, ConnectionInfo>> {
    let config_path = get_config_path()?;
    if !config_path.exists() {
        return Ok(HashMap::new());
    }

    let content = fs::read_to_string(config_path).context("Could not read connections file")?;
    let connections: HashMap<String, ConnectionInfo> =
        serde_json::from_str(&content).context("Could not parse connections file")?;
    Ok(connections)
}

pub fn save_connections(connections: &HashMap<String, ConnectionInfo>) -> Result<()> {
    let config_path = get_config_path()?;
    let content =
        serde_json::to_string_pretty(connections).context("Could not serialize connections")?;
    fs::write(config_path, content).context("Could not write connections file")?;
    Ok(())
}
