# archmap

Architectural analysis for codebases. Understands dependencies, detects coupling issues, and generates context for AI agents.

**Why?** Before modifying code, you should understand what depends on it. Before refactoring, you should know where the coupling problems are. Before asking an AI to help, it should understand your architecture. `archmap` does all of this.

## Install

```bash
cargo install archmap
```

or, locally:

```bash
cargo install --path .
```

Or build from source:

```bash
git clone https://github.com/xandwr/archmap
cd archmap
cargo build --release
./target/release/archmap --help
```

## Quick Start

```bash
# Analyze current directory
archmap analyze

# AI-optimized output (compact, structured for LLM consumption)
archmap ai

# Check impact before changing a file
archmap impact src/core/database.rs --tree

# Interactive dependency graph
archmap graph --serve --open
```

## Commands

### `analyze` — Architectural Analysis

Detects coupling issues, circular dependencies, boundary violations, and god objects.

```bash
archmap analyze                     # Markdown output
archmap analyze -f json             # JSON output
archmap analyze --watch             # Re-analyze on file changes
archmap analyze --min-severity warn # Filter by severity
archmap analyze -x tests -x vendor  # Exclude directories
archmap analyze --lang rust,typescript  # Specific languages
```

**Options:**
| Flag | Description |
|------|-------------|
| `-f, --format <FORMAT>` | Output format: `markdown`, `json` |
| `-o, --output <FILE>` | Write to file instead of stdout |
| `--min-severity <LEVEL>` | Minimum severity: `info`, `warn`, `error` |
| `--lang <LANGS>` | Languages to analyze (comma-separated) |
| `-w, --watch` | Re-analyze on file changes |
| `-x, --exclude <DIR>` | Exclude directories (repeatable) |
| `--max-depth <N>` | Max dependency chain depth (default: 5) |
| `--min-cohesion <N>` | Min cohesion score 0.0-1.0 (default: 0.3) |

Example output:
```
## Issues Found

### High Coupling
- `src/model/mod.rs` - Imported by 15 other modules

### Boundary Violations
**Filesystem** crossed in 21 locations:
- `src/config.rs:81` - `let content = std::fs::read_to_string(...)`
  → Consider centralizing file operations
```

### `ai` — AI-Optimized Context

Generates compact module summaries optimized for LLM consumption. Fits architectural context into token budgets.

```bash
archmap ai                          # Full context
archmap ai --tokens 4000            # Fit within token budget
archmap ai --signatures             # Public API surface only
archmap ai --topo-order             # Dependencies before dependents
archmap ai -f json                  # JSON format
archmap ai -f xml                   # XML format
archmap ai --priority fan-in        # Prioritize most-imported modules
```

**Options:**
| Flag | Description |
|------|-------------|
| `--tokens <N>` | Maximum tokens (uses tiktoken for accuracy) |
| `--signatures` | Output only public API surface |
| `--topo-order` | Topological ordering (deps before dependents) |
| `-f, --format <FORMAT>` | Output format: `markdown`, `json`, `xml` |
| `-o, --output <FILE>` | Write to file instead of stdout |
| `--priority <STRATEGY>` | Prioritization: `fan-in`, `fan-out`, `combined` |
| `--lang <LANGS>` | Languages to analyze (comma-separated) |

### `impact` — Change Impact Analysis

Shows which files are affected when you modify a specific file. Essential before refactoring.

```bash
archmap impact src/model/mod.rs           # List affected files
archmap impact src/model/mod.rs --tree    # ASCII tree visualization
archmap impact src/model/mod.rs -d 2      # Limit traversal depth
```

**Options:**
| Flag | Description |
|------|-------------|
| `--tree` | Show ASCII tree visualization |
| `-d, --depth <N>` | Maximum traversal depth |
| `-f, --format <FORMAT>` | Output format: `markdown`, `json` |
| `-o, --output <FILE>` | Write to file instead of stdout |
| `--path <PATH>` | Project path (default: current directory) |
| `--lang <LANGS>` | Languages to analyze (comma-separated) |

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

### `graph` — Interactive Visualization

Launch a web UI to explore the dependency graph.

```bash
archmap graph --serve               # Start server on port 3000
archmap graph --serve --open        # Start and open browser
archmap graph --serve --port 8080   # Custom port
archmap graph --serve --watch       # Live-reload on changes
archmap graph --export graph.html   # Export static HTML
```

**Options:**
| Flag | Description |
|------|-------------|
| `--serve` | Start HTTP server |
| `--open` | Open browser automatically |
| `--port <PORT>` | Server port (default: 3000) |
| `-w, --watch` | Live-reload on file changes |
| `--export <FILE>` | Export as static HTML |
| `--lang <LANGS>` | Languages to analyze (comma-separated) |

By default, the server starts without opening a browser—ideal for CI/scripts or remote machines.

### `snapshot` & `diff` — Track Architectural Drift

Save snapshots and compare against baselines. Useful for CI pipelines.

```bash
# Save current state
archmap snapshot --save baseline.json

# Compare against baseline
archmap diff baseline.json
archmap diff baseline.json --fail-on-regression  # Exit non-zero on regression
archmap diff baseline.json -f json               # JSON output
```

**snapshot options:**
| Flag | Description |
|------|-------------|
| `--save <FILE>` | Save snapshot to file (required) |
| `--lang <LANGS>` | Languages to analyze (comma-separated) |

**diff options:**
| Flag | Description |
|------|-------------|
| `--fail-on-regression` | Exit with error if regressions found |
| `-f, --format <FORMAT>` | Output format: `markdown`, `json` |
| `-o, --output <FILE>` | Write to file instead of stdout |
| `--lang <LANGS>` | Languages to analyze (comma-separated) |

### `mcp` — AI Assistant Integration

Start an MCP (Model Context Protocol) server for integration with AI assistants like Claude.

```bash
archmap mcp                         # Start MCP server (stdio transport)
archmap mcp /path/to/project        # Analyze specific directory
```

To get the MCP manifest for client configuration:
```bash
archmap --mcp-manifest
```

This outputs JSON configuration that can be used to register archmap with MCP-compatible AI assistants.

### `init` — Generate Config

```bash
archmap init  # Creates .archmap.toml with defaults
```

## Configuration

Create `.archmap.toml` to customize thresholds and define architectural boundaries:

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

[boundaries.filesystem]
name = "Filesystem"
indicators = ["std::fs::", "tokio::fs::", "fs.readFile"]
suggestion = "Consider centralizing file operations"
```

## Supported Languages

- Rust
- TypeScript/JavaScript
- Python

## Performance

archmap uses parallel file walking and thread-local tree-sitter parsers:

| Codebase | Files | Time |
|----------|-------|------|
| archmap itself | ~30 | 10ms |
| Medium project | ~500 | 200ms |
| Large monorepo | ~5000 | 2s |

For large codebases, exclude test and vendor directories:

```bash
archmap analyze -x tests -x node_modules -x vendor -x target
```

## Use Cases

**Before modifying a core module:**
```bash
archmap impact src/database/connection.rs --tree
```

**Generate context for an AI assistant:**
```bash
archmap ai --tokens 8000 -o context.md
```

**CI pipeline — fail on architectural regression:**
```bash
archmap snapshot --save baseline.json  # Run once, commit to repo
archmap diff baseline.json --fail-on-regression  # Run in CI
```

**Explore an unfamiliar codebase:**
```bash
archmap graph --serve --open
```

**Integrate with Claude or other MCP-compatible AI:**
```bash
archmap --mcp-manifest > mcp-config.json
# Add to your AI assistant's MCP configuration
```

## License

MIT
