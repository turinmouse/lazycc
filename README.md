# capm

`capm` manages API provider profiles for coding-agent CLIs such as Codex and Claude Code.

## Install

From source:

```sh
cargo install --path .
```

Homebrew tap installation is intended to use this shape after release:

```sh
brew install <tap-owner>/capm/capm
```

## Shell Setup

Add this line to `~/.zshrc`:

```sh
eval "$(capm init zsh)"
```

`capm init zsh` prints shell commands for the currently selected profile. It unsets supported provider variables first, then exports the variables for the active target.

## Commands

```sh
capm add work
capm add work --target codex
capm list
capm switch work
capm switch work --target claude
capm del work
capm del work --target codex
```

Supported targets:

- `codex`: sets `OPENAI_BASE_URL` and `OPENAI_API_KEY`
- `claude`: sets `ANTHROPIC_BASE_URL` and `ANTHROPIC_API_KEY`

Built-in profiles:

- `openai` under `codex`
- `anthropic` under `claude`

These built-in profiles have empty values. Each target has its own current profile, so `openai` and `anthropic` are both selected by default and appear with `*` in `capm list`. Switching to either built-in leaves the related provider variables unset after `capm init zsh` runs.

Profiles are stored in `~/.config/capm/config.toml`.
