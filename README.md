# capm

`capm` manages API provider profiles for coding-agent CLIs such as Codex and Claude Code.

## Install

From source:

```sh
cargo install --path .
```

Homebrew tap installation is intended to use this shape after release:

```sh
brew install turinmouse/tap/capm
```

Homebrew maps `turinmouse/tap` to this public tap repository:

```text
https://github.com/turinmouse/homebrew-tap
```

That tap repository must contain `Formula/capm.rb`.

## Shell Setup

Add this line to `~/.zshrc`:

```sh
eval "$(capm init zsh)"
```

`capm init zsh` prints shell commands for the currently selected profile. It unsets supported provider variables first, clears any previously registered `codex` shell function, then exports the variables for the active target.

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

- `codex`: sets `OPENAI_BASE_URL` and `OPENAI_API_KEY`; non-`openai` profiles also register a `codex` shell function that injects the selected profile as a Codex `model_provider` and forwards all arguments unchanged
- `claude`: sets `ANTHROPIC_BASE_URL` and `ANTHROPIC_AUTH_TOKEN`

Built-in profiles:

- `openai` under `codex`
- `anthropic` under `claude`

These built-in profiles have empty values. Each target has its own current profile, so `openai` and `anthropic` are both selected by default and appear with `*` in `capm list`. Switching to either built-in leaves the related provider variables unset after `capm init zsh` runs. Switching Codex back to `openai` also leaves the real `codex` command unwrapped.

Profiles are stored in `~/.config/capm/config.toml`.

## Release

Tagging a version creates prebuilt release archives and updates the Homebrew tap formula:

```sh
git tag v0.1.4
git push origin v0.1.4
```

The release workflow expects this repository secret:

```text
HOMEBREW_TAP_TOKEN
```

The token needs permission to push to `turinmouse/homebrew-tap`.
