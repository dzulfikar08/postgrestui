# Contributing to PostgresTUI

Thanks for your interest in contributing! Here's how to get started.

## Prerequisites

- **Rust** 1.75+ (`rustup.rs`)
- **Node.js** 18+ (for Web UI development)
- **PostgreSQL** instance to test against

## Setup

```bash
git clone https://github.com/dzulfikar08/postgrestui.git
cd postgrestui
cargo build
```

## Development

### Rust (TUI + API)

```bash
# Run in debug mode
cargo run -- -s localhost -d mydb -u postgres

# Run tests
cargo test

# Check formatting
cargo fmt --check

# Lint
cargo clippy
```

### Web UI

```bash
cd web
npm install
npm run dev       # Dev server at localhost:5173
npm run build     # Build to web/dist/ (embedded in binary)
```

The Web UI requires the Rust backend running with `--ui` flag, or you can use the Vite dev proxy.

### Full build with web assets

```bash
cd web && npm run build && cd ..
cargo build --release
```

## Making Changes

1. **Fork** the repository
2. **Create a branch** from `main`: `git checkout -b my-feature`
3. **Make your changes** with clear, focused commits
4. **Test** your changes against a real PostgreSQL database
5. **Run** `cargo fmt && cargo clippy` — fix any warnings
6. **Push** and open a Pull Request

## Pull Request Guidelines

- **One concern per PR** — bug fix, feature, or refactor, not mixed
- **Describe the change** — what and why, not just how
- **Test manually** — connect to a real database and exercise your change
- **Keep commits clean** — squash WIP commits before requesting review

## Reporting Issues

- Use **GitHub Issues** for bugs and feature requests
- Include: OS, Rust version, PostgreSQL version, steps to reproduce
- For bugs: include the error output or a screenshot

## Code Style

- Follow `cargo fmt` conventions for Rust
- Follow existing patterns in the codebase
- Keep the single-binary architecture — web assets are embedded via `rust-embed`

## License

By contributing, you agree that your contributions will be licensed under the [GPL-3.0](LICENSE).
