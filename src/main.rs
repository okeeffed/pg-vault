use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use keyring::Entry;
use rpassword::read_password;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tabled::{Table, Tabled};
use urlencoding::encode;

#[derive(Parser)]
#[command(name = "pg-vault")]
#[command(about = "A CLI tool for managing PostgreSQL credentials")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Store PostgreSQL credentials")]
    Store {
        #[arg(help = "Connection name/alias")]
        name: String,
        #[arg(long, help = "Database host")]
        host: String,
        #[arg(short, long, help = "Database port", default_value = "5432")]
        port: u16,
        #[arg(short, long, help = "Database name")]
        database: String,
        #[arg(short, long, help = "Username")]
        username: String,
        #[arg(long, help = "Store as IAM-authenticated connection (no password required)")]
        iam: bool,
    },
    #[command(about = "List stored connections")]
    List,
    #[command(about = "Connect to a stored PostgreSQL instance")]
    Connect {
        #[arg(help = "Connection name/alias")]
        name: String,
    },
    #[command(about = "Remove stored credentials")]
    Remove {
        #[arg(help = "Connection name/alias")]
        name: String,
    },
    #[command(about = "Start a shell session with PostgreSQL environment variables")]
    Session {
        #[arg(help = "Connection name/alias")]
        name: String,
    },
    #[command(about = "Connect using AWS IAM authentication")]
    Iam {
        #[arg(help = "Connection name/alias")]
        name: String,
    },
}

#[derive(Serialize, Deserialize)]
struct ConnectionInfo {
    host: String,
    port: u16,
    database: String,
    username: String,
    #[serde(default)]
    iam_auth: bool,
}

#[derive(Tabled)]
struct ConnectionDisplay {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Host")]
    host: String,
    #[tabled(rename = "Port")]
    port: u16,
    #[tabled(rename = "Database")]
    database: String,
    #[tabled(rename = "Username")]
    username: String,
    #[tabled(rename = "Auth Type")]
    auth_type: String,
}

fn get_config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir().context("Could not find config directory")?;
    let app_dir = config_dir.join("pg-vault");
    fs::create_dir_all(&app_dir).context("Could not create config directory")?;
    Ok(app_dir.join("connections.json"))
}

fn load_connections() -> Result<HashMap<String, ConnectionInfo>> {
    let config_path = get_config_path()?;
    if !config_path.exists() {
        return Ok(HashMap::new());
    }

    let content = fs::read_to_string(config_path).context("Could not read connections file")?;
    let connections: HashMap<String, ConnectionInfo> =
        serde_json::from_str(&content).context("Could not parse connections file")?;
    Ok(connections)
}

fn save_connections(connections: &HashMap<String, ConnectionInfo>) -> Result<()> {
    let config_path = get_config_path()?;
    let content =
        serde_json::to_string_pretty(connections).context("Could not serialize connections")?;
    fs::write(config_path, content).context("Could not write connections file")?;
    Ok(())
}

fn store_password(name: &str, password: &str) -> Result<()> {
    let entry = Entry::new("pg-vault", name).context("Could not create keyring entry")?;

    match entry.set_password(password) {
        Ok(()) => {
            println!("Password stored successfully!");
            Ok(())
        }
        Err(e) => {
            println!("Failed to store password: {:?}", e);
            Err(anyhow::Error::from(e)).context("Could not store password in keyring")
        }
    }
}

fn get_password(name: &str) -> Result<String> {
    println!("Attempting to get password {}", name);
    let entry = Entry::new("pg-vault", name).context("Could not create keyring entry")?;
    println!("Attempting to fetch password");
    let password = entry
        .get_password()
        .context("Could not retrieve password from keyring")?;
    Ok(password)
}

fn remove_password(name: &str) -> Result<()> {
    let entry = Entry::new("pg-vault", name).context("Could not create keyring entry")?;
    entry
        .delete_credential()
        .context("Could not remove password from keyring")?;
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Store {
            name,
            host,
            port,
            database,
            username,
            iam,
        } => {
            let connection_info = ConnectionInfo {
                host,
                port,
                database,
                username: username.clone(),
                iam_auth: iam,
            };

            let mut connections = load_connections()?;
            connections.insert(name.clone(), connection_info);
            save_connections(&connections)?;

            if iam {
                println!("✓ IAM connection '{}' stored successfully for user '{}'", name, username);
                println!("  Note: This connection will use AWS IAM authentication (no password stored)");
            } else {
                print!("Enter password for {}: ", username);
                io::stdout().flush()?;
                let password = read_password()?;

                match store_password(&name, &password) {
                    Ok(()) => println!("✓ Credentials stored successfully for '{}'", name),
                    Err(e) => {
                        println!("✗ Failed to store password: {}", e);
                        println!(
                            "Connection metadata saved, but you may need to enter the password each time."
                        );
                    }
                }
            }
        }
        Commands::List => {
            let connections = load_connections()?;
            if connections.is_empty() {
                println!("No stored connections found.");
                return Ok(());
            }

            let display_connections: Vec<ConnectionDisplay> = connections
                .iter()
                .map(|(name, info)| ConnectionDisplay {
                    name: name.clone(),
                    host: info.host.clone(),
                    port: info.port,
                    database: info.database.clone(),
                    username: info.username.clone(),
                    auth_type: if info.iam_auth { "IAM".to_string() } else { "Password".to_string() },
                })
                .collect();

            let table = Table::new(display_connections);
            println!("{}", table);
        }
        Commands::Connect { name } => {
            let connections = load_connections()?;
            let connection_info = connections
                .get(&name)
                .context(format!("Connection '{}' not found", name))?;

            if connection_info.iam_auth {
                anyhow::bail!(
                    "Connection '{}' is configured for IAM authentication. Use 'pg-vault iam {}' instead of 'pg-vault connect {}'.",
                    name, name, name
                );
            }

            let password = get_password(&name)
                .context(format!("Could not retrieve password for '{}'. You may need to store the credentials again.", name))?;

            println!(
                "Connecting to {} ({}@{}:{}/{})...",
                name,
                connection_info.username,
                connection_info.host,
                connection_info.port,
                connection_info.database
            );

            let mut cmd = Command::new("psql");
            cmd.arg(format!(
                "postgres://{}:{}@{}:{}/{}",
                connection_info.username,
                password,
                connection_info.host,
                connection_info.port,
                connection_info.database
            ))
            .env("PGPASSWORD", &password)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

            let status = cmd.status().context(
                "Failed to execute psql command. Make sure psql is installed and in your PATH.",
            )?;

            if !status.success() {
                anyhow::bail!("psql exited with error code: {:?}", status.code());
            }
        }
        Commands::Remove { name } => {
            let mut connections = load_connections()?;

            if !connections.contains_key(&name) {
                println!("Connection '{}' not found.", name);
                return Ok(());
            }

            connections.remove(&name);
            save_connections(&connections)?;

            match remove_password(&name) {
                Ok(()) => println!("✓ Credentials removed successfully for '{}'", name),
                Err(_) => {
                    println!(
                        "✓ Connection metadata removed for '{}' (password may have already been removed)",
                        name
                    );
                }
            }
        }
        Commands::Session { name } => {
            let connections = load_connections()?;
            let connection_info = connections
                .get(&name)
                .context(format!("Connection '{}' not found", name))?;

            let password = get_password(&name)?;

            println!(
                "Starting shell session with PostgreSQL environment for '{}'",
                name
            );
            println!("Available environment variables:");
            println!("  PGHOST={}", connection_info.host);
            println!("  PGPORT={}", connection_info.port);
            println!("  PGDATABASE={}", connection_info.database);
            println!("  PGUSER={}", connection_info.username);
            println!("  PGPASSWORD=<hidden>");
            println!(
                "  DATABASE_URL=postgres://{}:<password>@{}:{}/{}",
                connection_info.username,
                connection_info.host,
                connection_info.port,
                connection_info.database
            );
            println!();

            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

            let mut cmd = Command::new(&shell);
            cmd.env("PGHOST", &connection_info.host)
                .env("PGPORT", connection_info.port.to_string())
                .env("PGDATABASE", &connection_info.database)
                .env("PGUSER", &connection_info.username)
                .env("PGPASSWORD", &password)
                .env(
                    "DATABASE_URL",
                    format!(
                        "postgres://{}:{}@{}:{}/{}",
                        connection_info.username,
                        password,
                        connection_info.host,
                        connection_info.port,
                        connection_info.database
                    ),
                )
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());

            let status = cmd.status().context("Failed to start shell session")?;

            if !status.success() {
                anyhow::bail!("Shell session exited with error code: {:?}", status.code());
            }
        }
        Commands::Iam { name } => {
            let connections = load_connections()?;
            let connection_info = connections
                .get(&name)
                .context(format!("Connection '{}' not found", name))?;

            if !connection_info.iam_auth {
                anyhow::bail!(
                    "Connection '{}' is not configured for IAM authentication. Use 'pg-vault store --iam' to create an IAM-enabled connection.",
                    name
                );
            }

            println!(
                "Generating IAM authentication token for {} ({}@{}:{}/{})...",
                name,
                connection_info.username,
                connection_info.host,
                connection_info.port,
                connection_info.database
            );

            // Generate IAM auth token using AWS CLI
            let output = Command::new("aws")
                .args([
                    "rds",
                    "generate-db-auth-token",
                    "--hostname",
                    &connection_info.host,
                    "--port",
                    &connection_info.port.to_string(),
                    "--username",
                    &connection_info.username,
                ])
                .output()
                .context("Failed to execute AWS CLI command. Make sure AWS CLI is installed and configured.")?;

            if !output.status.success() {
                let error_msg = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("AWS CLI command failed: {}", error_msg);
            }

            let iam_token = String::from_utf8(output.stdout)
                .context("Invalid UTF-8 in AWS CLI output")?
                .trim()
                .to_string();

            if iam_token.is_empty() {
                anyhow::bail!("Empty IAM token received from AWS CLI");
            }

            println!("✓ IAM token generated successfully");
            println!("Token length: {} characters", iam_token.len());
            println!("Token preview: {}...", &iam_token[..std::cmp::min(50, iam_token.len())]);
            println!("Connecting to PostgreSQL using IAM authentication...");

            // Connect to PostgreSQL using the IAM token as password
            let encoded_token = encode(&iam_token);
            let mut cmd = Command::new("psql");
            cmd.arg(format!(
                "postgres://{}:{}@{}:{}/{}?sslmode=require",
                connection_info.username,
                encoded_token,
                connection_info.host,
                connection_info.port,
                connection_info.database
            ))
            .env("PGPASSWORD", &iam_token)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

            let status = cmd.status().context(
                "Failed to execute psql command. Make sure psql is installed and in your PATH.",
            )?;

            if !status.success() {
                anyhow::bail!("psql exited with error code: {:?}", status.code());
            }
        }
    }

    Ok(())
}
