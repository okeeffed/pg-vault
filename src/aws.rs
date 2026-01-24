use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::process::Command;

pub fn list_aws_profiles() -> Vec<String> {
    let mut profiles = HashSet::new();

    // Parse ~/.aws/credentials for [profile] sections
    if let Some(home) = dirs::home_dir() {
        let credentials_path = home.join(".aws").join("credentials");
        if let Ok(content) = fs::read_to_string(&credentials_path) {
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with('[') && line.ends_with(']') {
                    let profile_name = &line[1..line.len() - 1];
                    profiles.insert(profile_name.to_string());
                }
            }
        }

        // Parse ~/.aws/config for [profile X] sections
        let config_path = home.join(".aws").join("config");
        if let Ok(content) = fs::read_to_string(&config_path) {
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with('[') && line.ends_with(']') {
                    let section = &line[1..line.len() - 1];
                    if section.starts_with("profile ") {
                        let profile_name = section.strip_prefix("profile ").unwrap();
                        profiles.insert(profile_name.to_string());
                    } else if section == "default" {
                        profiles.insert("default".to_string());
                    }
                }
            }
        }
    }

    let mut profiles: Vec<String> = profiles.into_iter().collect();
    profiles.sort();

    // Put "default" first if it exists
    if let Some(pos) = profiles.iter().position(|p| p == "default") {
        profiles.remove(pos);
        profiles.insert(0, "default".to_string());
    }

    profiles
}

pub fn generate_iam_token(
    host: &str,
    port: u16,
    username: &str,
    profile: Option<&str>,
) -> Result<String> {
    let mut cmd = Command::new("aws");
    cmd.args([
        "rds",
        "generate-db-auth-token",
        "--hostname",
        host,
        "--port",
        &port.to_string(),
        "--username",
        username,
    ]);

    if let Some(profile_name) = profile {
        cmd.args(["--profile", profile_name]);
    }

    let output = cmd
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

    Ok(iam_token)
}

#[allow(dead_code)]
pub fn verify_aws_profile(profile: &str) -> Result<()> {
    let mut cmd = Command::new("aws");
    cmd.args(["sts", "get-caller-identity", "--profile", profile]);

    let output = cmd
        .output()
        .context("Failed to execute AWS CLI command")?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("AWS profile '{}' verification failed: {}", profile, error_msg);
    }

    Ok(())
}

pub fn needs_sso_login(error_msg: &str) -> bool {
    let error_lower = error_msg.to_lowercase();
    error_lower.contains("sso")
        || error_lower.contains("token has expired")
        || error_lower.contains("refresh_token")
        || error_lower.contains("the sso session")
        || error_lower.contains("error loading sso")
}
