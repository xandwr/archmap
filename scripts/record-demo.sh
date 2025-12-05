#!/bin/bash
# Record an asciinema demo of archmap basic usage
#
# Usage: ./scripts/record-demo.sh
#
# Prerequisites:
#   - asciinema installed (apt install asciinema / brew install asciinema)
#   - archmap built (cargo build --release)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="$PROJECT_ROOT/demo_footage"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
OUTPUT_FILE="$OUTPUT_DIR/archmap_demo_$TIMESTAMP.cast"

# Ensure output directory exists
mkdir -p "$OUTPUT_DIR"

# Ensure archmap is built
if [[ ! -x "$PROJECT_ROOT/target/release/archmap" ]]; then
    echo "Building archmap..."
    cargo build --release --manifest-path "$PROJECT_ROOT/Cargo.toml"
fi

# Create a temporary demo script that will be executed inside the recording
DEMO_SCRIPT=$(mktemp)
cat > "$DEMO_SCRIPT" << 'DEMO_EOF'
#!/bin/bash

# Helper function to simulate typing with realistic delays
type_cmd() {
    echo ""
    echo -n "$ "
    for (( i=0; i<${#1}; i++ )); do
        echo -n "${1:$i:1}"
        sleep 0.04
    done
    echo ""
    sleep 0.3
}

# Helper to pause for reading
pause() {
    sleep "${1:-1.5}"
}

clear

echo "╔════════════════════════════════════════════════════════════╗"
echo "║              archmap - Architectural Analysis              ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""
pause 2

# Show help
type_cmd "archmap --help"
archmap --help
pause 3

# Basic analysis
type_cmd "archmap analyze"
archmap analyze
pause 4

# AI context generation
type_cmd "archmap ai --tokens 2000"
archmap ai --tokens 2000
pause 4

# Impact analysis
type_cmd "archmap impact src/model/mod.rs --tree"
archmap impact src/model/mod.rs --tree
pause 4

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Demo complete! See 'archmap --help' for more commands."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
pause 2

DEMO_EOF

chmod +x "$DEMO_SCRIPT"

echo "Recording demo to: $OUTPUT_FILE"
echo ""

# Record the demo
cd "$PROJECT_ROOT"
asciinema rec "$OUTPUT_FILE" \
    --title "archmap Demo" \
    --command "bash $DEMO_SCRIPT" \
    --cols 80 \
    --rows 24 \
    --overwrite

# Cleanup
rm -f "$DEMO_SCRIPT"

echo ""
echo "Demo recorded successfully!"
echo "Output: $OUTPUT_FILE"
echo ""
echo "To play back: asciinema play $OUTPUT_FILE"
echo "To upload:    asciinema upload $OUTPUT_FILE"
echo "To convert to GIF: agg $OUTPUT_FILE demo.gif"
