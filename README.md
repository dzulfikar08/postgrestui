# PostgresTUI

[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![GitHub release](https://img.shields.io/github/v/release/dzulfikar08/postgrestui?include_prereleases)](https://github.com/dzulfikar08/postgrestui/releases)

A fast, lightweight PostgreSQL database browser with a terminal UI and built-in Web UI — all in a single ~5 MB binary.

## Features

- **TUI + Web UI** — Same binary, two interfaces. Use `--ui` for the web version.
- **Table & View Browser** — Sidebar navigation with row counts, instant name filter, Vim-style keys.
- **Paginated Data View** — 500 rows at a time with horizontal scroll. Copy as TSV/JSON.
- **SQL Query Editor** — Autocomplete for keywords, tables, and columns. Syntax highlighting. Multi-statement support.
- **Schema Inspector** — Column names, types, row counts with DDL highlighting.
- **Multi-Format Export** — CSV (Excel-ready), JSON (API-ready), SQL INSERT statements.
- **Single Binary** — ~5 MB, zero runtime dependencies. macOS, Linux, Windows.

## Quick Start

### Install from source

```bash
cargo install --git https://github.com/dzulfikar08/postgrestui
```

### Build locally

```bash
git clone https://github.com/dzulfikar08/postgrestui.git
cd postgrestui
cargo build --release
```

### Run

```bash
# Terminal UI
./target/release/postgrestui -s localhost -d mydb -u postgres

# Web UI (open http://localhost:5000)
./target/release/postgrestui -s localhost -d mydb -u postgres --ui
```

## Usage

```
postgrestui [OPTIONS]

Options:
  -s, --host <HOST>       PostgreSQL host [default: localhost]
  -p, --port <PORT>       PostgreSQL port [default: 5432]
  -d, --database <DB>     Database name [default: postgres]
  -u, --user <USER>       Username [default: postgres]
  -P, --pass <PASSWORD>   Password (prompted if omitted)
      --ui                Start Web UI instead of TUI
      --listen <ADDR>     Web UI listen port [default: 5000]
  -h, --help              Print help
  -V, --version           Print version
```

### TUI Keybindings

| Key | Action |
|-----|--------|
| `h` / `j` / `k` / `l` | Navigate (left/down/up/right) |
| `Tab` | Focus next panel |
| `Enter` | Select / open |
| `/` | Filter / search |
| `c` | Copy (TSV or JSON) |
| `e` | Export (CSV, JSON, SQL) |
| `q` / `Esc` | Back / quit |
| `[` / `]` | Previous / next result tab |
| `?` | Help |

## Architecture

```
src/
├── main.rs            Entry point, CLI parsing
├── app.rs             Core: DB connection, SQL execution
├── api.rs             Axum REST API (/api/query, /api/tables, etc.)
└── ui/
    └── talbe_view.rs  TUI: sidebar, data view, SQL editor (ratatui)

web/
├── src/App.tsx        React web UI with SQL editor, data table, result tabs
├── src/Tree.tsx       Table/View tree sidebar component
└── dist/              Compiled assets (embedded via rust-embed)
```

The binary embeds the compiled web frontend at build time. The Web UI communicates with the same backend through a local REST API.

## Development

```bash
# Build and run in debug mode
cargo run -- -s localhost -d mydb -u postgres

# Web UI development
cd web && npm install && npm run dev

# Full release build (includes web assets)
cd web && npm run build && cd .. && cargo build --release

# Lint and format
cargo fmt && cargo clippy
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full contribution guide.

## Cross-Platform Builds

```bash
# Using the included release script
bash scripts/release-local.sh
```

Builds for macOS (ARM + Intel), Linux (x86_64), and Windows (x86_64) using `cargo-zigbuild` and `cargo-xwin`.

## License

[GPL-3.0](LICENSE) — free to build from source. Paid pre-built binaries support continued development and are available at [voltrus.id/postgrestui](https://voltrus.id/postgrestui/).

## Sponsor

This project is sponsored by [Voltrus.id](https://voltrus.id) and [JoyoDigitama.com](https://joyodigitama.com).
