#!/bin/bash

# Build and install pg-vault locally

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Building pg-vault...${NC}"

# Build the project in release mode
cargo build --release

# Check if build was successful
if [ $? -eq 0 ]; then
    echo -e "${GREEN}Build successful!${NC}"
else
    echo -e "${RED}Build failed!${NC}"
    exit 1
fi

# Create local bin directory if it doesn't exist
LOCAL_BIN="$HOME/.local/bin"
mkdir -p "$LOCAL_BIN"

# Copy binary to local bin
cp target/release/pg-vault "$LOCAL_BIN/"

# Sign the binary for macOS (required for systems with endpoint protection like CrowdStrike)
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo -e "${YELLOW}Signing binary for macOS...${NC}"
    codesign --force --sign - "$LOCAL_BIN/pg-vault"
fi

echo -e "${GREEN}pg-vault installed to $LOCAL_BIN/pg-vault${NC}"
echo -e "${YELLOW}Make sure $LOCAL_BIN is in your PATH${NC}"

# Check if LOCAL_BIN is in PATH
if [[ ":$PATH:" != *":$LOCAL_BIN:"* ]]; then
    echo -e "${YELLOW}To add $LOCAL_BIN to your PATH, add this line to your shell profile:${NC}"
    echo "export PATH=\"\$HOME/.local/bin:\$PATH\""
fi

echo -e "${GREEN}Installation complete! You can now run 'pg-vault --help'${NC}"