# archmap

Generate architectural context for AI agents. Analyze codebases for dependency graphs, coupling issues, boundary violations, and change impact.

## Quick Start

```bash
cargo install --path .

# Analyze current directory
archmap analyze

# Generate AI-optimized context
archmap ai

# Check impact before changing a file
archmap impact src/core/database.rs --tree
```

## Commands

### `analyze` — Architectural Analysis

Detects coupling issues, circular dependencies, boundary violations, and god objects.

```bash
archmap analyze                     # Markdown output
archmap analyze -f json             # JSON output
archmap analyze --watch             # Re-analyze on file changes
archmap analyze --min-severity warn # Filter by severity
```

Example output:
```
## Issues Found

### 🟡 High Coupling
- `src/model/mod.rs` - Imported by 15 other modules

### 🟡 Boundary Violations
**Filesystem** crossed in 21 locations:
- `src/config.rs:81` - `let content = std::fs::read_to_string(...)`
→ Consider centralizing file operations
```

### `ai` — AI-Optimized Context

Generates compact module summaries optimized for LLM consumption.

```bash
archmap ai                          # Full context
archmap ai --tokens 4000            # Fit within token budget
archmap ai --signatures             # Public API surface only
archmap ai --topo-order             # Dependencies before dependents
archmap ai -f json                  # JSON format
archmap ai --priority fan-in        # Prioritize most-imported modules
```

### `impact` — Change Impact Analysis

Shows which files are affected when you modify a specific file.

```bash
archmap impact src/model/mod.rs           # List affected files
archmap impact src/model/mod.rs --tree    # ASCII tree visualization
archmap impact src/model/mod.rs -d 2      # Limit traversal depth
```

Example output:
```
## Summary
- **Total Affected Files**: 19
- **Maximum Chain Length**: 2

## Impact Tree
src/model/mod.rs (TARGET)
├── src/cli.rs
│   └── src/output/ai.rs
├── src/config.rs
│   ├── src/analysis/boundary.rs
│   └── src/analysis/mod.rs
└── src/parser/rust.rs
```

### `snapshot` & `diff` — Track Architectural Drift

Save snapshots and compare against baselines.

```bash
# Save current state
archmap snapshot --save baseline.json

# Later: compare against baseline
archmap diff baseline.json
archmap diff baseline.json --fail-on-regression  # CI-friendly
```

### `graph` — Interactive Visualization

Launch a web UI to explore the dependency graph.

```bash
archmap graph --serve               # Start server (no browser opened)
archmap graph --serve --open        # Start server and open browser
archmap graph --serve --port 8080   # Custom port
archmap graph --serve --watch       # Live-reload on changes
archmap graph --export graph.html   # Export static HTML
```

By default, the server starts without opening a browser — ideal for CI/scripts. Pass `--open` to auto-launch.

### `init` — Generate Config

```bash
archmap init  # Creates .archmap.toml with defaults
```

## Configuration

Create `.archmap.toml` to customize thresholds and boundaries:

```toml
[thresholds]
god_object_lines = 500       # Max lines before flagging
coupling_fanin = 5           # Max importers before flagging
max_dependency_depth = 5     # Max chain length A→B→C→D→E
min_cohesion = 0.3           # 0.0-1.0, lower = less focused

[boundaries.persistence]
name = "Persistence"
indicators = ["sqlx::", "diesel::", "SELECT ", "INSERT "]
suggestion = "Consider centralizing in a repository layer"

[boundaries.network]
name = "Network"
indicators = ["reqwest::", "fetch(", "axios."]
suggestion = "Consider centralizing in an API client service"
```

## Supported Languages

- Rust
- TypeScript/JavaScript
- Python

```bash
archmap analyze --lang rust,typescript  # Specific languages
```

## Use Cases

**Before modifying a core module:**
```bash
archmap impact src/database/connection.rs --tree
```

**Generate context for an AI assistant:**
```bash
archmap ai --tokens 8000 > context.md
```

**CI pipeline — fail on architectural regression:**
```bash
archmap diff baseline.json --fail-on-regression
```

**Explore unfamiliar codebase:**
```bash
archmap graph --serve --open
```

**CI pipeline — serve graph without browser:**
```bash
archmap graph --serve --port 8080  # Headless, no browser popup
```
