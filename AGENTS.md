# Repository Guidelines

## Project Structure & Module Organization

`lazycc` is a Rust 2024 CLI for managing coding-agent API provider profiles. The application entry point is `src/main.rs`, which defines the Clap command surface and dispatches to feature modules. Configuration loading, profile storage, shell script generation, and table rendering live in `src/config.rs`; shared error types are in `src/error.rs`; the interactive terminal UI is under `src/tui/`, split by runner, state, layout, theme, and view responsibilities. Tests are currently inline in Rust modules with `#[cfg(test)]`. Packaging and release automation live in `.github/workflows/release.yml`, with Homebrew-related files under `Formula/`.

## Build, Test, and Development Commands

- `cargo build`: compile the debug binary during development.
- `cargo run -- list`: run the CLI locally with a subcommand.
- `cargo run -- tui`: launch the TUI profile manager.
- `cargo test`: run all unit tests, including inline module tests.
- `cargo fmt --check`: verify Rust formatting before submitting changes.
- `cargo clippy --all-targets --all-features`: catch common Rust issues and lints.
- `cargo build --release --locked`: reproduce the release-style optimized build using `Cargo.lock`.

## Coding Style & Naming Conventions

Use standard `rustfmt` formatting and four-space indentation. Keep modules focused by domain: command dispatch in `main.rs`, persistent config behavior in `config.rs`, and terminal UI state/rendering in `tui/`. When adding or expanding behavior, preserve this separation of concerns instead of placing unrelated logic in the same file; create or extend focused modules when a feature crosses command, config, UI, or error boundaries. Prefer descriptive enum variants and structs such as `Target`, `Shell`, `Profile`, and `LazyccError`. CLI subcommands should remain short, lowercase verbs (`add`, `del`, `use`, `list`) and long flags should use kebab-case when added.

## Testing Guidelines

Add unit tests next to the code they exercise inside `#[cfg(test)] mod tests`. Name tests by behavior, for example `add_rejects_duplicate_name_for_same_target`. Cover config mutations, shell output, CLI-visible behavior, and TUI state transitions when changing those areas. Run `cargo test` before opening a PR; use focused tests only while iterating.

## Commit & Pull Request Guidelines

Recent history uses short imperative commits such as `Fix Claude auth token env var`, release commits like `Release 0.1.6`, and occasional scoped docs commits like `docs: update homebrew tap install path`. Keep commits concise and specific. Pull requests should describe the user-visible change, list test commands run, link related issues when available, and include terminal screenshots or recordings for TUI changes.

## Security & Configuration Tips

Do not commit real provider API keys or generated user config. Runtime profiles are stored outside the repository at `~/.config/lazycc/config.toml`. Release automation depends on the `HOMEBREW_TAP_TOKEN` secret, so changes to `.github/workflows/release.yml` should be reviewed carefully.
