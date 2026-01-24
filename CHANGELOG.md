# pg-vault

## 0.2.0

### Minor Changes

- Add interactive TUI mode for managing database connections

  - New TUI interface with connection list, profile selector, and add connection form
  - Refactored codebase into separate modules (cli, config, credentials, aws, tui)
  - Added ratatui and crossterm dependencies for terminal UI rendering
  - Support for keyboard navigation and interactive connection management
