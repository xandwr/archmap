#!/bin/bash
# Install archmap as a global cargo binary

set -e

echo "Installing archmap..."

# Install from current directory
cargo install --path .

echo ""
echo "archmap installed successfully!"
echo ""
echo "Usage:"
echo "  archmap .                    # Analyze current directory"
echo "  archmap ai --signatures      # AI-optimized output with signatures"
echo "  archmap impact src/main.rs   # Change impact analysis"
echo "  archmap snapshot --save b.json  # Save baseline"
echo "  archmap diff b.json          # Compare against baseline"
echo "  archmap graph --serve        # Interactive visualization"
