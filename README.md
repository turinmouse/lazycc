# lazycc

`lazycc` manages API provider profiles for coding-agent CLIs such as Codex and Claude Code.

## Install

From source:

```sh
cargo install --path .
```

Homebrew tap installation is intended to use this shape after release:

```sh
brew install turinmouse/tap/lazycc
```

Homebrew maps `turinmouse/tap` to this public tap repository:

```text
https://github.com/turinmouse/homebrew-tap
```

That tap repository must contain `Formula/lazycc.rb`.

## Shell Setup

Add this line to `~/.zshrc`:

```sh
eval "$(lazycc init zsh)"
```

`lazycc init zsh` prints shell commands for the currently selected profile. It unsets supported provider variables first, clears any previously registered `lazycc` and `codex` shell functions, then exports the variables for the active target. It also registers a `lazycc` shell function so the TUI (`lazycc`) and `lazycc use ...` refresh the current shell automatically after a successful profile change.

## TUI

Run `lazycc tui` to open the profile manager. Running `lazycc` with no subcommand prints help.

```sh
lazycc tui
```

The TUI is split into numbered panes: tools and profiles on the left, with the selected profile details on the right.

- `1`: focus tools
- `2`: focus profiles
- `0`: focus profile details
- `Tab`: switch between the left-side tools and profiles panes
- `Left` / `Right`: cycle left-side numbered panes
- `Enter`: use the selected profile
- `t`: switch theme
- `n`: add a profile for the selected tool
- `e`: edit the selected custom profile
- In add/edit forms, `Tab` or arrow keys move between fields, `Enter` saves, and `Esc` cancels
- `d`: delete the selected custom profile
- `q` or `Esc`: quit

Built-in `openai` and `anthropic` profiles are read-only and cannot be deleted.

## Commands

The command-style interface remains available for scripts:

```sh
lazycc add work
lazycc add work --target codex
lazycc list
lazycc use work
lazycc use work --target claude
lazycc del work
lazycc del work --target codex
```

Supported targets:

- `codex`: sets `OPENAI_BASE_URL` and `OPENAI_API_KEY`; non-`openai` profiles also register a `codex` shell function that injects the selected profile as a Codex `model_provider`, applies the profile model when set, and forwards all arguments unchanged
- `claude`: sets `ANTHROPIC_BASE_URL` and `ANTHROPIC_AUTH_TOKEN`

Built-in profiles:

- `openai` under `codex`
- `anthropic` under `claude`

These built-in profiles have empty values. Each target has its own current profile, so `openai` and `anthropic` are both selected by default and appear with `*` in `lazycc list`. Using either built-in leaves the related provider variables unset after `lazycc init zsh` runs. Using Codex profile `openai` also leaves the real `codex` command unwrapped.

Profiles store `name`, `target`, `base_url`, `api_key`, and `model` in `~/.config/lazycc/config.toml`.

## Release

Tagging a version creates prebuilt release archives and updates the Homebrew tap formula:

```sh
git tag v0.1.6
git push origin v0.1.6
```

The release workflow expects this repository secret:

```text
HOMEBREW_TAP_TOKEN
```

The token needs permission to push to `turinmouse/homebrew-tap`.
