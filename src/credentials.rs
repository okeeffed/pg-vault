use anyhow::{Context, Result};
use keyring::Entry;

pub fn store_password(name: &str, password: &str) -> Result<()> {
    let entry = Entry::new("pg-vault", name).context("Could not create keyring entry")?;

    entry
        .set_password(password)
        .map_err(|e| anyhow::Error::from(e))
        .context("Could not store password in keyring")?;

    Ok(())
}

pub fn get_password(name: &str) -> Result<String> {
    let entry = Entry::new("pg-vault", name).context("Could not create keyring entry")?;
    let password = entry
        .get_password()
        .context("Could not retrieve password from keyring")?;
    Ok(password)
}

pub fn remove_password(name: &str) -> Result<()> {
    let entry = Entry::new("pg-vault", name).context("Could not create keyring entry")?;
    entry
        .delete_credential()
        .context("Could not remove password from keyring")?;
    Ok(())
}
