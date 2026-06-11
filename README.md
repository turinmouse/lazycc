# lazycc

Switch coding-agent provider profiles without re-opening your shell.

`lazycc` is a small Rust CLI/TUI for managing API profiles used by Codex, Claude Code, and related coding-agent tools. It keeps each tool's active profile separate, generates shell exports, and refreshes your current shell after profile changes.

## Highlights

- Manage Codex and Claude Code provider profiles from one place.
- Use a terminal UI for daily profile switching.
- Keep one current profile per target.
- Mask API keys in list/detail views.
- Wrap `codex` only when a non-default Codex profile is selected.
- Store config locally at `~/.config/lazycc/config.toml`.

## Install

Install from the Homebrew tap:

```sh
brew install turinmouse/tap/lazycc
```

Or install from source:

```sh
cargo install --path .
```

The Homebrew tap lives at:

```text
https://github.com/turinmouse/homebrew-tap
```

## Quick Start

Add `lazycc` to your shell startup file:

```sh
eval "$(lazycc init zsh)"
```

Then open the TUI:

```sh
lazycc tui
```

Add a profile, select it, and your current shell will be refreshed automatically when the TUI exits after a successful profile change.

## Terminal UI

The TUI has two main areas:

- Left: `[1] Tools - Targets - Profiles`
- Right: `[0] Configuration`

Navigation is hierarchical. Select a target first, enter its profiles, then choose the active profile.

| Key | Action |
| --- | --- |
| `Up` / `Down` | Move within the current list |
| `Enter` / `Space` on a target | Open profiles for that target |
| `Enter` / `Space` on a profile | Use the selected profile |
| `Esc` in profiles | Return to targets |
| `1` | Focus the tools area |
| `2` | Focus profiles |
| `0` | Focus profile details |
| `Left` / `Right` | Move between targets and profiles |
| `n` / `a` | Add a profile for the selected target |
| `e` | Edit the selected custom profile |
| `d` | Delete the selected custom profile |
| `t` | Toggle theme |
| `q` | Quit |

In add/edit forms, `Tab` or arrow keys move between fields, `Enter` saves, and `Esc` cancels.

Built-in `openai` and `anthropic` profiles are read-only and cannot be deleted.

## CLI

The command interface is useful for scripts and quick changes:

```sh
lazycc add work
lazycc add work --target codex
lazycc list
lazycc use work
lazycc use work --target claude
lazycc del work
lazycc del work --target codex
```

Running `lazycc` without a subcommand prints help. Use `lazycc tui` for the profile manager.

## Targets

| Target | Environment |
| --- | --- |
| `codex` | `OPENAI_BASE_URL`, `OPENAI_API_KEY` |
| `claude` | `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN` |

For Codex, selecting any profile other than the built-in `openai` profile also registers a shell `codex` wrapper. The wrapper injects the selected profile as a Codex `model_provider`, applies the profile `model` when set, and forwards all command arguments unchanged.

## Built-In Profiles

`lazycc` creates two empty built-in profiles:

- `openai` for `codex`
- `anthropic` for `claude`

Each target has its own active profile, so both built-ins are selected by default and appear with `*` in `lazycc list`.

Using a built-in profile leaves the related provider variables unset after `lazycc init zsh` runs. Using Codex profile `openai` also leaves the real `codex` command unwrapped.

## Config

Profiles are stored in:

```text
~/.config/lazycc/config.toml
```

Each profile contains:

- `name`
- `target`
- `base_url`
- `api_key`
- `model`

Do not commit real provider keys or generated user config.

## Release

Tagging a version creates prebuilt release archives and updates the Homebrew tap formula:

```sh
git tag v0.2.2
git push origin v0.2.2
```

The release workflow expects this repository secret:

```text
HOMEBREW_TAP_TOKEN
```

The token needs permission to push to `turinmouse/homebrew-tap`.
