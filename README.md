# PostgresTUI

A fast, lightweight PostgreSQL database browser that runs in your terminal. Browse tables, run SQL with autocomplete, export data as CSV/JSON/SQL — all from a single compiled binary. Built-in Web UI for when you want a graphical view.

**License:** GPL-3.0 — free to build from source.
**Pre-built binary:** $4.99 one-time at [voltrus.id/postgrestui](https://voltrus.id/postgrestui/)

## Features

- **TUI + Web UI** — Same binary, two interfaces. Launch with `--ui` for the web version.
- **Table & View Browser** — Sidebar navigation with row counts, instant name filter, Vim-style keys.
- **Paginated Data View** — 500 rows at a time with Prev/Next. Horizontal scroll for wide tables. Copy TSV/JSON.
- **SQL Query Editor** — Autocomplete for keywords, tables, and columns. Syntax highlighting. Multi-statement support with per-statement error reporting. Run scripts from files.
- **Schema Inspector** — Column names, types, row counts. DDL syntax highlighting.
- **Multi-Format Export** — CSV (Excel-ready), JSON (API-ready), SQL INSERT statements.
- **Single Binary** — ~5 MB, zero dependencies. macOS, Linux, Windows.

## Quick Start

### Build from source (free)

```bash
cargo build --release
./target/release/postgrestui -s localhost -p 5432 -d mydb -u postgres
```

### Web UI

```bash
cargo build --release
./target/release/postgrestui -s localhost -d mydb -u postgres --ui
# Open http://localhost:5000
```

### Pre-built binary ($4.99)

Download from [voltrus.id/postgrestui](https://voltrus.id/postgrestui/) after purchase.

## Usage

```
postgrestui [OPTIONS] --host <HOST> --database <DATABASE> --user <USER>

Options:
  -s, --host <HOST>       PostgreSQL host
  -p, --port <PORT>       PostgreSQL port [default: 5432]
  -d, --database <DB>     Database name
  -u, --user <USER>       Username
  -W, --password <PW>     Password (prompt if omitted)
      --ui                Start Web UI instead of TUI
      --listen <ADDR>     Web UI listen address [default: 0.0.0.0:5000]
```

### TUI Keybindings

| Key | Action |
|-----|--------|
| `i` / `j` / `k` / `l` | Navigate (right/down/up/left) |
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
├── app.rs          # Core: connect, execute_sql, execute_script, split_sql
├── api.rs          # Axum web API: /api/query, /api/script, /api/tables, etc.
├── ui/
│   ├── talbe_view.rs  # TUI: sidebar, data view, SQL editor (ratatui)
│   └── ...
web/
├── src/App.tsx     # React web UI with SQL editor, data table, result tabs
├── src/Tree.tsx    # Table/View tree sidebar component
└── dist/           # Compiled web assets (embedded via rust-embed)
cloudflare-worker/  # Download distribution: R2 + D1
scripts/
└── release-local.sh    # Build + upload to R2/D1
```

## License

GPL-3.0. See [LICENSE](LICENSE) (or [github.com/dzulfikar08/postgrestui](https://github.com/dzulfikar08/postgrestui)).

Paid pre-built binaries support continued development of open-source tooling.
