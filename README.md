# pg-vault

A cross-platform CLI tool for securely managing PostgreSQL credentials and connections.

## What is pg-vault?

pg-vault helps you store and manage PostgreSQL connection credentials securely, eliminating the need to remember or repeatedly type database connection details. It integrates with your system's keychain (macOS Keychain, etc.) to store passwords safely and provides convenient commands to connect to your databases.

## Features

- **Secure credential storage** - Uses system keychain with file fallback
- **Quick connections** - Connect to saved databases with a single command
- **Quick psql sessions** - Automatically launches psql with your credentials
- **Environment sessions** - Start shell sessions with PostgreSQL environment variables
- **Connection management** - List, store, and remove connection configurations
- **Interactive TUI mode** - Terminal UI for browsing and managing connections
- **Cross-platform** - Works on macOS (Linux and Windows support planned)

## Installation

### From Source

```bash
git clone https://github.com/okeeffed/pg-vault.git
cd pg-vault
./install.sh
```

This will build and install pg-vault to `~/.local/bin/pg-vault`. Make sure `~/.local/bin` is in your PATH.

> **Note:** On macOS, the install script automatically signs the binary. This is required for systems with endpoint protection software (e.g., CrowdStrike).

### From Releases

Download the latest release for your platform from the [releases page](https://github.com/okeeffed/pg-vault/releases).

## Quick Start

### 1. Store a connection

```bash
pg-vault store mydb --host localhost --database myapp --username postgres
# You'll be prompted to enter the password securely
```

### 2. List stored connections

```bash
pg-vault list
```

### 3. Connect to a database

```bash
pg-vault connect mydb
# Launches psql with your stored credentials
```

### 4. Start a shell session with environment variables

```bash
pg-vault session mydb
# Starts a shell with PGHOST, PGUSER, PGPASSWORD, DATABASE_URL, etc.
```

### 5. Launch interactive TUI

```bash
pg-vault tui
# Opens the terminal UI for managing connections
```

### 6. Remove a connection

```bash
pg-vault remove mydb
```

## Commands

- `pg-vault store <name> --host <host> --database <db> --username <user>` - Store database credentials
- `pg-vault list` - List all stored connections
- `pg-vault connect <name>` - Connect to database using psql
- `pg-vault session <name>` - Start shell with PostgreSQL environment variables
- `pg-vault tui` - Launch interactive terminal UI
- `pg-vault remove <name>` - Remove stored credentials
- `pg-vault --help` - Show help information

## Environment Variables Available in Sessions

When using `pg-vault session <name>`, the following environment variables are set:

- `PGHOST` - Database host
- `PGPORT` - Database port
- `PGDATABASE` - Database name
- `PGUSER` - Username
- `PGPASSWORD` - Password
- `DATABASE_URL` - Full PostgreSQL connection URL

## Security

- Passwords are stored in your system's keychain when available
- Falls back to encrypted local files if keychain is unavailable
- Connection metadata is stored in `~/.config/pg-vault/connections.json`
- No credentials are stored in plain text in configuration files

## Requirements

- Rust (for building from source)
- PostgreSQL client tools (`psql` command)
- macOS (for keychain integration)

## License

MIT License - see LICENSE file for details.
