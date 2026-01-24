use anyhow::{Context, Result};
use clap::Subcommand;
use rpassword::read_password;
use std::io::{self, Write};
use std::process::{Command, Stdio};
use tabled::{Table, Tabled};
use urlencoding::encode;

use crate::aws::generate_iam_token;
use crate::config::{load_connections, save_connections, ConnectionInfo};
use crate::credentials::{get_password, remove_password, store_password};

#[derive(Subcommand)]
pub enum Commands {
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
        #[arg(long, help = "AWS profile to use")]
        profile: Option<String>,
    },
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

pub fn run_command(command: Commands) -> Result<()> {
    match command {
        Commands::Store {
            name,
            host,
            port,
            database,
            username,
            iam,
        } => cmd_store(name, host, port, database, username, iam),
        Commands::List => cmd_list(),
        Commands::Connect { name } => cmd_connect(&name),
        Commands::Remove { name } => cmd_remove(&name),
        Commands::Session { name } => cmd_session(&name),
        Commands::Iam { name, profile } => cmd_iam(&name, profile.as_deref()),
    }
}

fn cmd_store(
    name: String,
    host: String,
    port: u16,
    database: String,
    username: String,
    iam: bool,
) -> Result<()> {
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
        println!(
            "IAM connection '{}' stored successfully for user '{}'",
            name, username
        );
        println!("  Note: This connection will use AWS IAM authentication (no password stored)");
    } else {
        print!("Enter password for {}: ", username);
        io::stdout().flush()?;
        let password = read_password()?;

        match store_password(&name, &password) {
            Ok(()) => println!("Credentials stored successfully for '{}'", name),
            Err(e) => {
                println!("Failed to store password: {}", e);
                println!(
                    "Connection metadata saved, but you may need to enter the password each time."
                );
            }
        }
    }
    Ok(())
}

fn cmd_list() -> Result<()> {
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
            auth_type: if info.iam_auth {
                "IAM".to_string()
            } else {
                "Password".to_string()
            },
        })
        .collect();

    let table = Table::new(display_connections);
    println!("{}", table);
    Ok(())
}

fn cmd_connect(name: &str) -> Result<()> {
    let connections = load_connections()?;
    let connection_info = connections
        .get(name)
        .context(format!("Connection '{}' not found", name))?;

    if connection_info.iam_auth {
        anyhow::bail!(
            "Connection '{}' is configured for IAM authentication. Use 'pg-vault iam {}' instead of 'pg-vault connect {}'.",
            name, name, name
        );
    }

    let password = get_password(name).context(format!(
        "Could not retrieve password for '{}'. You may need to store the credentials again.",
        name
    ))?;

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
    Ok(())
}

fn cmd_remove(name: &str) -> Result<()> {
    let mut connections = load_connections()?;

    if !connections.contains_key(name) {
        println!("Connection '{}' not found.", name);
        return Ok(());
    }

    connections.remove(name);
    save_connections(&connections)?;

    match remove_password(name) {
        Ok(()) => println!("Credentials removed successfully for '{}'", name),
        Err(_) => {
            println!(
                "Connection metadata removed for '{}' (password may have already been removed)",
                name
            );
        }
    }
    Ok(())
}

fn cmd_session(name: &str) -> Result<()> {
    let connections = load_connections()?;
    let connection_info = connections
        .get(name)
        .context(format!("Connection '{}' not found", name))?;

    let password = get_password(name)?;

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
    Ok(())
}

fn cmd_iam(name: &str, profile: Option<&str>) -> Result<()> {
    let connections = load_connections()?;
    let connection_info = connections
        .get(name)
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

    let iam_token = generate_iam_token(
        &connection_info.host,
        connection_info.port,
        &connection_info.username,
        profile,
    )?;

    println!("IAM token generated successfully");
    println!("Connecting to PostgreSQL using IAM authentication...");

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
    Ok(())
}
