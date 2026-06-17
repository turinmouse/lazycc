mod config;
mod error;
mod template;
mod tools;
mod tui;

use clap::{CommandFactory, Parser, Subcommand};
use inquire::{Password, PasswordDisplayMode, Text};

use config::{Config, Profile, Shell, Target, config_path};
use error::LazyccError;
use tui::run_tui;

fn main() {
    if let Err(error) = run() {
        eprintln!("lazycc: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), LazyccError> {
    let cli = Cli::parse();
    let path = config_path()?;

    match cli.command {
        None => {
            Cli::command().print_help()?;
            println!();
        }
        Some(Command::Tui) => run_tui(&path)?,
        Some(Command::Init { shell }) => {
            let config = Config::load(&path)?;
            print!("{}", config.init_script(shell));
        }
        Some(Command::List) => {
            let config = Config::load(&path)?;
            println!("{}", config.render_table());
        }
        Some(Command::Add { name, target }) => {
            let target = match target {
                Some(target) => target,
                None => Target::prompt()?,
            };
            let base_url = Text::new("Base URL:").prompt()?;
            let api_key = Password::new("API key:")
                .with_display_mode(PasswordDisplayMode::Masked)
                .without_confirmation()
                .prompt()?;
            let model = Text::new("Model:").prompt()?;

            let mut config = Config::load(&path)?;
            config.add(Profile {
                name,
                target,
                base_url,
                api_key,
                model,
            })?;
            config.save(&path)?;
        }
        Some(Command::Del { name, target }) => {
            let mut config = Config::load(&path)?;
            config.delete(&name, target)?;
            config.save(&path)?;
        }
        Some(Command::Use { name, target }) => {
            let mut config = Config::load(&path)?;
            config.use_profile(&name, target)?;
            config.save(&path)?;
        }
    }

    Ok(())
}

#[derive(Parser, Debug)]
#[command(
    name = "lazycc",
    version,
    about = "Manage coding-agent API provider profiles"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    Tui,
    Init {
        shell: Shell,
    },
    #[command(alias = "ls")]
    List,
    Add {
        name: String,
        #[arg(long)]
        target: Option<Target>,
    },
    Del {
        name: String,
        #[arg(long)]
        target: Option<Target>,
    },
    Use {
        name: String,
        #[arg(long)]
        target: Option<Target>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use ratatui::layout::Rect;

    use crate::config::{
        CurrentProfile, config_path_from, default_current_profile, default_current_profiles,
        default_profiles, mask_api_key,
    };
    use crate::tools::Plugin;
    use crate::tui::{
        FocusPane, McpServer, ProfileForm, TuiAction, TuiApp, TuiMode, TuiOperation,
        TuiOperationResult,
    };

    fn profile(name: &str, target: Target, api_key: &str) -> Profile {
        Profile {
            name: name.to_string(),
            target,
            base_url: format!("https://{}.example.test", target),
            api_key: api_key.to_string(),
            model: String::new(),
        }
    }

    fn plugin(target: Target, name: &str) -> Plugin {
        Plugin {
            target,
            name: name.to_string(),
            selector: format!("{name}@test"),
            marketplace: Some("test".to_string()),
            installed: false,
            enabled: false,
            details: "test plugin".to_string(),
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn mouse_down(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn add_rejects_duplicate_name_for_same_target() {
        let mut config = Config::default();
        config
            .add(profile("work", Target::Codex, "sk-first"))
            .expect("first profile should be added");

        let error = config
            .add(profile("work", Target::Codex, "sk-second"))
            .expect_err("duplicate target/name should fail");

        assert!(matches!(
            error,
            LazyccError::DuplicateProfile {
                target: Target::Codex,
                name
            } if name == "work"
        ));
    }

    #[test]
    fn add_allows_same_name_for_different_targets() {
        let mut config = Config::default();
        config
            .add(profile("work", Target::Codex, "sk-codex"))
            .expect("codex profile should be added");
        config
            .add(profile("work", Target::Claude, "sk-claude"))
            .expect("claude profile should be added");

        assert_eq!(config.profiles.len(), 4);
    }

    #[test]
    fn use_profile_requires_target_for_ambiguous_name() {
        let mut config = Config {
            current: default_current_profiles(),
            profiles: vec![
                profile("work", Target::Codex, "sk-codex"),
                profile("work", Target::Claude, "sk-claude"),
            ],
        };

        let error = config
            .use_profile("work", None)
            .expect_err("ambiguous profile use should fail");

        assert!(matches!(error, LazyccError::AmbiguousProfile { name } if name == "work"));
    }

    #[test]
    fn use_profile_accepts_target_for_ambiguous_name() {
        let mut config = Config {
            current: default_current_profiles(),
            profiles: vec![
                profile("work", Target::Codex, "sk-codex"),
                profile("work", Target::Claude, "sk-claude"),
            ],
        };

        config
            .use_profile("work", Some(Target::Claude))
            .expect("target should disambiguate");

        assert_eq!(
            config.current_for_target(Target::Claude),
            Some(&CurrentProfile {
                target: Target::Claude,
                name: "work".to_string()
            })
        );
    }

    #[test]
    fn init_script_wraps_lazycc_use_to_refresh_current_shell() {
        let script = Config::default().init_script(Shell::Zsh);

        assert!(script.contains("unfunction lazycc 2>/dev/null || true\n"));
        assert!(script.contains("lazycc() {\n"));
        assert!(script.contains("  command lazycc \"$@\"\n"));
        assert!(script.contains("  local lazycc_status=$?\n"));
        assert!(script.contains("  if [ $lazycc_status -eq 0 ] && [ \"$1\" = \"use\" ]; then\n"));
        assert!(script.contains("    eval \"$(command lazycc init zsh)\"\n"));
        assert!(script.contains("  elif [ $lazycc_status -eq 0 ] && [ \"$1\" = \"tui\" ]; then\n"));
        assert!(!script.contains("lazycc_before_init"));
        assert!(!script.contains("lazycc_after_init"));
        assert!(script.contains("  return $lazycc_status\n"));
    }

    #[test]
    fn cli_accepts_ls_alias_for_list() {
        let cli = Cli::try_parse_from(["lazycc", "ls"]).expect("ls alias should parse");

        assert!(matches!(cli.command, Some(Command::List)));
    }

    #[test]
    fn tui_keybindings_generate_targets_hints() {
        let app = TuiApp::new(Config::default());

        let keybindings = app.keybindings();

        assert!(keybindings.contains("Open profiles: <space>/enter"));
        assert!(keybindings.contains("MCP: 2"));
        assert!(keybindings.contains("Details: 0"));
        assert!(keybindings.contains("Quit: q"));
        assert!(keybindings.contains(" | "));
    }

    #[test]
    fn tui_new_does_not_request_mcp_refresh() {
        let app = TuiApp::new(Config::default());

        assert_eq!(
            app.mcp_refresh_state_for(Target::Codex),
            tui::McpRefreshState::NotLoaded
        );
        assert_eq!(
            app.mcp_refresh_state_for(Target::Claude),
            tui::McpRefreshState::NotLoaded
        );
        assert!(app.mcp_refresh_requests.is_empty());
        assert!(app.mcp_servers.is_empty());
        assert!(app.installed_plugin_refresh_requests.is_empty());
        assert!(app.plugin_refresh_requests.is_empty());
        assert!(app.plugins.is_empty());
    }

    #[test]
    fn tui_cache_populates_first_paint_data_without_refresh_requests() {
        let mut app = TuiApp::new(Config::default());

        app.apply_cache(tui::TuiCache {
            mcp_servers: vec![McpServer {
                target: Target::Codex,
                name: "context-mode".to_string(),
                details: "context-mode node ./start.mjs".to_string(),
            }],
            installed_plugins: vec![Plugin {
                target: Target::Codex,
                name: "installed-plugin".to_string(),
                selector: "installed-plugin@test".to_string(),
                marketplace: None,
                installed: true,
                enabled: true,
                details: "installed".to_string(),
            }],
        });

        assert!(app.take_mcp_refresh_requests().is_empty());
        assert!(app.take_installed_plugin_refresh_requests().is_empty());
        assert!(app.take_plugin_refresh_requests().is_empty());
        assert_eq!(
            app.mcp_refresh_state_for(Target::Codex),
            tui::McpRefreshState::Loaded
        );
        assert_eq!(
            app.selected_installed_plugin_refresh_state(),
            tui::McpRefreshState::Loaded
        );
        assert_eq!(app.mcp_servers.len(), 1);
        assert_eq!(app.plugins.len(), 1);
        assert!(app.available_plugins.is_empty());
        assert!(!app.plugin_search_loading());
    }

    #[test]
    fn tui_installed_plugin_refresh_is_queued_per_target() {
        let mut app = TuiApp::new(Config::default());

        assert!(app.request_installed_plugin_refresh_for(Target::Codex));

        assert_eq!(
            app.take_installed_plugin_refresh_requests(),
            vec![Target::Codex]
        );
        assert!(app.take_installed_plugin_refresh_requests().is_empty());
        assert_eq!(
            app.selected_installed_plugin_refresh_state(),
            tui::McpRefreshState::Loading
        );
    }

    #[test]
    fn tui_installed_plugin_refresh_replaces_only_matching_target() {
        let mut app = TuiApp::new(Config::default());
        app.plugins = vec![Plugin {
            target: Target::Claude,
            name: "claude-old".to_string(),
            selector: "claude-old".to_string(),
            marketplace: None,
            installed: true,
            enabled: true,
            details: "installed".to_string(),
        }];

        app.finish_installed_plugin_refresh(
            Target::Codex,
            vec![Plugin {
                target: Target::Codex,
                name: "xcode".to_string(),
                selector: "xcode".to_string(),
                marketplace: None,
                installed: true,
                enabled: true,
                details: "installed".to_string(),
            }],
            None,
        );

        assert_eq!(app.plugins.len(), 2);
        assert!(app.plugins.iter().any(|plugin| plugin.name == "xcode"));
        assert!(app.plugins.iter().any(|plugin| plugin.name == "claude-old"));
    }

    #[test]
    fn tui_entering_mcp_requests_async_refresh_once() {
        let mut app = TuiApp::new(Config::default());

        assert_eq!(app.handle_key(key(KeyCode::Char('2'))), TuiAction::None);

        assert_eq!(app.focus, FocusPane::Mcp);
        assert_eq!(
            app.mcp_refresh_state_for(Target::Codex),
            tui::McpRefreshState::Loading
        );
        assert_eq!(app.take_mcp_refresh_requests(), vec![Target::Codex]);
        assert!(app.take_mcp_refresh_requests().is_empty());
        assert_eq!(app.message, "Loading MCP servers for codex...");
    }

    #[test]
    fn tui_keybindings_generate_profiles_hints() {
        let mut app = TuiApp::new(Config::default());
        app.focus = FocusPane::Profiles;

        let keybindings = app.keybindings();

        assert!(keybindings.contains("Use profile: <space>/enter"));
        assert!(keybindings.contains("Add: n"));
        assert!(keybindings.contains("Edit: e"));
        assert!(keybindings.contains("Delete: d"));
        assert!(keybindings.contains("Back: esc"));
        assert!(keybindings.contains("MCP: 2"));
    }

    #[test]
    fn tui_keybindings_generate_editing_hints() {
        let mut app = TuiApp::new(Config::default());
        app.mode = TuiMode::Editing(ProfileForm::add(Target::Codex));

        let keybindings = app.keybindings();

        assert!(keybindings.contains("Next field: tab/down"));
        assert!(keybindings.contains("Previous: shift-tab/up"));
        assert!(keybindings.contains("Save: enter/ctrl-s"));
        assert!(keybindings.contains("Cancel: esc"));
    }

    #[test]
    fn tui_keybindings_generate_delete_confirmation_hints() {
        let mut app = TuiApp::new(Config::default());
        app.mode = TuiMode::ConfirmDelete;

        let keybindings = app.keybindings();

        assert_eq!(keybindings, "Confirm: enter/y | Cancel: esc/n");
    }

    #[test]
    fn tui_enter_switches_selected_profile() {
        let mut config = Config::default();
        config
            .add(profile("xcode", Target::Codex, "sk-codex"))
            .expect("profile should be added");
        let mut app = TuiApp::new(config);
        app.focus = FocusPane::Profiles;
        app.profile_index = app
            .selected_profile_indices()
            .iter()
            .position(|index| app.config.profiles[*index].name == "xcode")
            .expect("xcode should be selectable");

        assert_eq!(app.handle_key(key(KeyCode::Enter)), TuiAction::Save);

        assert_eq!(
            app.config.current_for_target(Target::Codex),
            Some(&CurrentProfile {
                target: Target::Codex,
                name: "xcode".to_string()
            })
        );
    }

    #[test]
    fn tui_space_switches_selected_profile() {
        let mut config = Config::default();
        config
            .add(profile("xcode", Target::Codex, "sk-codex"))
            .expect("profile should be added");
        let mut app = TuiApp::new(config);
        app.focus = FocusPane::Profiles;
        app.profile_index = app
            .selected_profile_indices()
            .iter()
            .position(|index| app.config.profiles[*index].name == "xcode")
            .expect("xcode should be selectable");

        assert_eq!(app.handle_key(key(KeyCode::Char(' '))), TuiAction::Save);

        assert_eq!(
            app.config.current_for_target(Target::Codex),
            Some(&CurrentProfile {
                target: Target::Codex,
                name: "xcode".to_string()
            })
        );
    }

    #[test]
    fn tui_enter_on_target_opens_profiles_tab() {
        let mut app = TuiApp::new(Config::default());
        app.focus = FocusPane::Targets;
        app.target_index = 1;

        assert_eq!(app.handle_key(key(KeyCode::Enter)), TuiAction::None);

        assert_eq!(app.focus, FocusPane::Profiles);
        assert_eq!(app.target_index, 1);
        assert_eq!(app.message, "Profiles for claude");
    }

    #[test]
    fn tui_escape_returns_from_profiles_to_targets() {
        let mut app = TuiApp::new(Config::default());
        app.focus = FocusPane::Targets;

        assert_eq!(app.handle_key(key(KeyCode::Enter)), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Profiles);

        assert_eq!(app.handle_key(key(KeyCode::Esc)), TuiAction::None);

        assert_eq!(app.focus, FocusPane::Targets);
        assert_eq!(app.message, "Back to targets");
    }

    #[test]
    fn tui_keeps_builtin_profiles_read_only() {
        let mut app = TuiApp::new(Config::default());
        app.focus = FocusPane::Profiles;

        assert_eq!(app.handle_key(key(KeyCode::Char('e'))), TuiAction::None);
        assert!(matches!(app.mode, TuiMode::Normal));
        assert_eq!(app.message, "Built-in profiles are read-only");

        assert_eq!(app.handle_key(key(KeyCode::Char('d'))), TuiAction::None);
        assert!(matches!(app.mode, TuiMode::Normal));
        assert_eq!(app.message, "Built-in profiles cannot be deleted");
    }

    #[test]
    fn tui_form_escape_cancels_editing() {
        let mut app = TuiApp::new(Config::default());
        app.focus = FocusPane::Profiles;

        assert_eq!(app.handle_key(key(KeyCode::Char('a'))), TuiAction::None);
        assert!(matches!(app.mode, TuiMode::Editing(_)));

        assert_eq!(app.handle_key(key(KeyCode::Esc)), TuiAction::None);
        assert!(matches!(app.mode, TuiMode::Normal));
    }

    #[test]
    fn tui_number_keys_switch_panes() {
        let mut app = TuiApp::new(Config::default());
        app.plugin_refresh_states = [tui::McpRefreshState::Loaded; 2];

        assert_eq!(app.handle_key(key(KeyCode::Char('2'))), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Mcp);
        assert_eq!(app.handle_key(key(KeyCode::Char('0'))), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Details);
        assert_eq!(app.handle_key(key(KeyCode::Char('1'))), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Targets);

        assert_eq!(app.handle_key(key(KeyCode::Char('3'))), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Plugins);

        assert_eq!(app.handle_key(key(KeyCode::Char('4'))), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Plugins);
        assert_eq!(app.message, "Panel 4 is reserved");
    }

    #[test]
    fn tui_left_right_keys_switch_panes() {
        let mut app = TuiApp::new(Config::default());
        app.plugin_refresh_states = [tui::McpRefreshState::Loaded; 2];

        assert_eq!(app.handle_key(key(KeyCode::Right)), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Targets);
        assert_eq!(app.handle_key(key(KeyCode::Left)), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Targets);

        assert_eq!(app.handle_key(key(KeyCode::Enter)), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Profiles);
        assert_eq!(app.handle_key(key(KeyCode::Right)), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Profiles);
        assert_eq!(app.handle_key(key(KeyCode::Left)), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Profiles);

        assert_eq!(app.handle_key(key(KeyCode::Char('2'))), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Mcp);
        assert_eq!(app.handle_key(key(KeyCode::Right)), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Plugins);
        assert_eq!(app.handle_key(key(KeyCode::Left)), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Mcp);

        assert_eq!(app.handle_key(key(KeyCode::Char('0'))), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Details);
        assert_eq!(app.handle_key(key(KeyCode::Right)), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Targets);
        assert_eq!(app.handle_key(key(KeyCode::Char('0'))), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Details);
        assert_eq!(app.handle_key(key(KeyCode::Left)), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Mcp);
    }

    #[test]
    fn tui_d_opens_mcp_delete_confirmation_when_mcp_is_focused() {
        let mut app = TuiApp::new(Config::default());
        app.focus = FocusPane::Mcp;
        app.mcp_servers = vec![McpServer {
            target: Target::Codex,
            name: "context-mode".to_string(),
            details: "context-mode node ./start.mjs".to_string(),
        }];

        assert_eq!(app.handle_key(key(KeyCode::Char('d'))), TuiAction::None);

        assert!(matches!(app.mode, TuiMode::ConfirmDeleteMcp));
    }

    #[test]
    fn tui_tab_keys_do_not_switch_panes() {
        let mut app = TuiApp::new(Config::default());

        assert_eq!(app.handle_key(key(KeyCode::Tab)), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Targets);
        assert_eq!(app.handle_key(key(KeyCode::BackTab)), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Targets);
    }

    #[test]
    fn tui_adds_profile_from_form() {
        let mut app = TuiApp::new(Config::default());
        let mut form = ProfileForm::add(Target::Codex);
        form.name = "work".to_string();
        form.base_url = "https://api.example.test/v1".to_string();
        form.api_key = "sk-test".to_string();
        form.model = "gpt-5".to_string();

        assert_eq!(app.save_form(&mut form), TuiAction::Save);

        assert!(app.config.profiles.iter().any(|profile| {
            profile.name == "work"
                && profile.target == Target::Codex
                && profile.base_url == "https://api.example.test/v1"
                && profile.api_key == "sk-test"
                && profile.model == "gpt-5"
        }));
    }

    #[test]
    fn tui_form_enter_saves_without_advancing_fields() {
        let mut app = TuiApp::new(Config::default());
        let mut form = ProfileForm::add(Target::Codex);
        form.active_field = 0;
        form.name = "work".to_string();
        form.base_url = "https://api.example.test/v1".to_string();
        form.api_key = "sk-test".to_string();
        form.model = "gpt-5".to_string();
        app.mode = TuiMode::Editing(form);

        assert_eq!(app.handle_key(key(KeyCode::Enter)), TuiAction::Save);

        assert!(matches!(app.mode, TuiMode::Normal));
        assert!(app.config.profiles.iter().any(|profile| {
            profile.name == "work"
                && profile.target == Target::Codex
                && profile.base_url == "https://api.example.test/v1"
        }));
    }

    #[test]
    fn tui_n_opens_add_profile_form() {
        let mut app = TuiApp::new(Config::default());
        app.focus = FocusPane::Profiles;

        assert_eq!(app.handle_key(key(KeyCode::Char('n'))), TuiAction::None);

        assert!(matches!(app.mode, TuiMode::Editing(_)));
    }

    #[test]
    fn tui_n_does_not_create_profile_from_mcp_or_plugins() {
        let mut app = TuiApp::new(Config::default());
        app.focus = FocusPane::Mcp;

        assert_eq!(app.handle_key(key(KeyCode::Char('n'))), TuiAction::None);
        assert!(matches!(app.mode, TuiMode::Normal));
        assert_eq!(app.focus, FocusPane::Mcp);

        app.plugin_refresh_states = [tui::McpRefreshState::Loaded; 2];
        app.focus = FocusPane::Plugins;
        assert_eq!(app.handle_key(key(KeyCode::Char('n'))), TuiAction::None);
        assert!(matches!(app.mode, TuiMode::Normal));
        assert_eq!(app.focus, FocusPane::PluginSearch);
    }

    #[test]
    fn tui_plugin_market_opens_from_plugins_n_shortcut() {
        let mut app = TuiApp::new(Config::default());

        assert_eq!(app.handle_key(key(KeyCode::Char('0'))), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Details);

        assert_eq!(app.handle_key(key(KeyCode::Char('3'))), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Plugins);
        assert_eq!(app.handle_key(key(KeyCode::Char('0'))), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Details);

        assert_eq!(app.handle_key(key(KeyCode::Char('3'))), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Plugins);
        assert_eq!(app.handle_key(key(KeyCode::Char('n'))), TuiAction::None);
        assert_eq!(app.focus, FocusPane::PluginSearch);

        assert_eq!(app.handle_key(key(KeyCode::Char('0'))), TuiAction::None);
        assert_eq!(app.focus, FocusPane::PluginSearch);
        assert_eq!(app.handle_key(key(KeyCode::Esc)), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Plugins);
    }

    #[test]
    fn tui_plugin_search_filters_and_escape_returns_to_plugins() {
        let mut app = TuiApp::new(Config::default());
        app.available_plugins = vec![
            plugin(Target::Codex, "context-mode"),
            plugin(Target::Claude, "review"),
        ];
        app.plugin_search_errors = Vec::new();
        app.plugin_search_loaded = true;
        app.plugin_refresh_states = [tui::McpRefreshState::Loaded; 2];

        app.focus = FocusPane::PluginSearch;
        assert_eq!(app.focus, FocusPane::PluginSearch);
        assert_eq!(app.handle_key(key(KeyCode::Char('r'))), TuiAction::None);
        assert_eq!(app.plugin_search_query, "");
        assert_eq!(app.handle_key(key(KeyCode::Char('/'))), TuiAction::None);
        assert_eq!(app.handle_key(key(KeyCode::Char('r'))), TuiAction::None);
        assert_eq!(app.plugin_search_query, "r");
        assert_eq!(app.handle_key(key(KeyCode::Char('q'))), TuiAction::None);
        assert_eq!(app.focus, FocusPane::PluginSearch);
        assert_eq!(app.plugin_search_query, "rq");
        assert_eq!(app.handle_key(key(KeyCode::Char('0'))), TuiAction::None);
        assert_eq!(app.focus, FocusPane::PluginSearch);
        assert_eq!(app.plugin_search_query, "rq0");
        assert_eq!(app.handle_key(key(KeyCode::Down)), TuiAction::None);
        assert_eq!(app.plugin_search_index, 0);
        assert_eq!(app.handle_key(key(KeyCode::Enter)), TuiAction::None);
        assert!(matches!(app.mode, TuiMode::Normal));
        assert_eq!(app.focus, FocusPane::PluginSearch);
        assert!(!app.plugin_search_editing);

        assert_eq!(app.selected_available_plugin(), None);

        assert_eq!(app.handle_key(key(KeyCode::Esc)), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Plugins);
    }

    #[test]
    fn tui_plugins_selection_only_sees_selected_target_plugins() {
        let mut app = TuiApp::new(Config::default());
        app.plugins = vec![
            plugin(Target::Claude, "claude-review"),
            plugin(Target::Codex, "codex-context"),
            plugin(Target::Codex, "codex-linear"),
        ];

        assert_eq!(app.selected_plugin_indices(), vec![1, 2]);
        assert_eq!(
            app.selected_plugin().map(|plugin| plugin.name.as_str()),
            Some("codex-context")
        );

        app.plugin_index = 1;
        assert_eq!(
            app.selected_plugin().map(|plugin| plugin.name.as_str()),
            Some("codex-linear")
        );

        app.focus = FocusPane::Plugins;
        assert_eq!(app.handle_key(key(KeyCode::Down)), TuiAction::None);
        assert_eq!(app.plugin_index, 1);
        assert_eq!(
            app.selected_plugin().map(|plugin| plugin.name.as_str()),
            Some("codex-linear")
        );
    }

    #[test]
    fn tui_plugin_toggle_queues_async_operation_without_mutating_plugin() {
        let mut app = TuiApp::new(Config::default());
        app.focus = FocusPane::Plugins;
        app.plugins = vec![Plugin {
            target: Target::Codex,
            name: "context-mode".to_string(),
            selector: "context-mode@test".to_string(),
            marketplace: None,
            installed: true,
            enabled: false,
            details: "installed".to_string(),
        }];

        let action = app.handle_key(key(KeyCode::Enter));

        assert!(matches!(
            action,
            TuiAction::Run(TuiOperation::TogglePlugin {
                plugin,
                enabled: true
            }) if plugin.name == "context-mode"
        ));
        assert!(!app.plugins[0].enabled);
        assert_eq!(app.pending_operations.len(), 1);
        assert_eq!(app.message, "Enabling plugin context-mode for codex...");
    }

    #[test]
    fn tui_duplicate_plugin_operation_is_ignored_while_pending() {
        let mut app = TuiApp::new(Config::default());
        app.focus = FocusPane::Plugins;
        app.plugins = vec![Plugin {
            target: Target::Codex,
            name: "context-mode".to_string(),
            selector: "context-mode@test".to_string(),
            marketplace: None,
            installed: true,
            enabled: false,
            details: "installed".to_string(),
        }];

        assert!(matches!(
            app.handle_key(key(KeyCode::Enter)),
            TuiAction::Run(_)
        ));
        assert_eq!(app.handle_key(key(KeyCode::Enter)), TuiAction::None);

        assert_eq!(app.pending_operations.len(), 1);
        assert_eq!(app.message, "Operation already running");
    }

    #[test]
    fn tui_successful_plugin_operation_requests_installed_plugin_refresh() {
        let mut app = TuiApp::new(Config::default());
        app.installed_plugin_refresh_states = [tui::McpRefreshState::Loaded; 2];
        let selected = plugin(Target::Codex, "context-mode");
        let operation = TuiOperation::TogglePlugin {
            plugin: selected,
            enabled: true,
        };
        app.pending_operations.insert(operation.key());

        app.finish_operation(TuiOperationResult {
            operation,
            error: None,
        });

        assert!(app.pending_operations.is_empty());
        assert_eq!(
            app.take_installed_plugin_refresh_requests(),
            vec![Target::Codex]
        );
        assert_eq!(app.message, "Enabled plugin context-mode for codex");
    }

    #[test]
    fn tui_failed_operation_clears_pending_without_refreshing() {
        let mut app = TuiApp::new(Config::default());
        app.installed_plugin_refresh_states = [tui::McpRefreshState::Loaded; 2];
        let selected = plugin(Target::Codex, "context-mode");
        let operation = TuiOperation::InstallPlugin(selected);
        app.pending_operations.insert(operation.key());

        app.finish_operation(TuiOperationResult {
            operation,
            error: Some("codex plugin add timed out".to_string()),
        });

        assert!(app.pending_operations.is_empty());
        assert!(app.take_installed_plugin_refresh_requests().is_empty());
        assert!(app.take_plugin_refresh_requests().is_empty());
        assert_eq!(app.message, "codex plugin add timed out");
    }

    #[test]
    fn tui_plugin_search_filters_to_selected_target() {
        let mut app = TuiApp::new(Config::default());
        app.available_plugins = vec![
            plugin(Target::Codex, "review"),
            plugin(Target::Claude, "review"),
            plugin(Target::Codex, "context-mode"),
        ];
        app.plugin_search_query = "review".to_string();

        assert_eq!(app.filtered_available_plugin_indices(), vec![0]);
        assert_eq!(
            app.selected_available_plugin()
                .map(|plugin| (plugin.target, plugin.name.as_str())),
            Some((Target::Codex, "review"))
        );
    }

    #[test]
    fn tui_switching_target_clamps_plugin_indices() {
        let mut app = TuiApp::new(Config::default());
        app.plugins = vec![
            plugin(Target::Codex, "codex-a"),
            plugin(Target::Codex, "codex-b"),
            plugin(Target::Claude, "claude-a"),
        ];
        app.available_plugins = vec![
            plugin(Target::Codex, "codex-a"),
            plugin(Target::Codex, "codex-b"),
            plugin(Target::Claude, "claude-a"),
        ];
        app.plugin_index = 1;
        app.plugin_search_index = 1;

        assert_eq!(app.handle_key(key(KeyCode::Down)), TuiAction::None);

        assert_eq!(app.target_index, 1);
        assert_eq!(app.plugin_index, 0);
        assert_eq!(app.plugin_search_index, 0);
        assert_eq!(
            app.selected_plugin().map(|plugin| plugin.name.as_str()),
            Some("claude-a")
        );
        assert_eq!(
            app.selected_available_plugin()
                .map(|plugin| plugin.name.as_str()),
            Some("claude-a")
        );
    }

    #[test]
    fn tui_plugin_search_installed_plugin_requires_confirmation() {
        let mut app = TuiApp::new(Config::default());
        app.focus = FocusPane::PluginSearch;
        app.available_plugins = vec![Plugin {
            target: Target::Codex,
            name: "context-mode".to_string(),
            selector: "context-mode@main".to_string(),
            marketplace: Some("main".to_string()),
            installed: true,
            enabled: true,
            details: "1.0.0".to_string(),
        }];
        app.plugin_search_errors = Vec::new();
        app.plugin_search_loaded = true;
        app.plugin_refresh_states = [tui::McpRefreshState::Loaded; 2];

        assert_eq!(app.handle_key(key(KeyCode::Enter)), TuiAction::None);

        assert!(matches!(app.mode, TuiMode::ConfirmUninstallPlugin(_)));
    }

    #[test]
    fn tui_plugin_search_errors_are_not_selectable_plugins() {
        let mut app = TuiApp::new(Config::default());
        app.focus = FocusPane::PluginSearch;
        app.available_plugins = Vec::new();
        app.plugin_search_errors = vec!["Codex: codex plugin list timed out".to_string()];
        app.plugin_search_loaded = true;
        app.plugin_refresh_states = [tui::McpRefreshState::Loaded; 2];

        assert_eq!(app.selected_available_plugin(), None);
        assert_eq!(app.handle_key(key(KeyCode::Enter)), TuiAction::None);
        assert!(matches!(app.mode, TuiMode::Normal));
        assert_eq!(app.message, "No plugin selected");
    }

    #[test]
    fn tui_plugin_refresh_is_lazy_loaded_when_entering_search() {
        let mut app = TuiApp::new(Config::default());

        assert!(app.request_plugin_refresh_for(Target::Codex));
        assert_eq!(app.take_plugin_refresh_requests(), vec![Target::Codex]);
        assert!(app.take_plugin_refresh_requests().is_empty());

        assert_eq!(app.handle_key(key(KeyCode::Char('0'))), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Details);
        assert!(app.take_plugin_refresh_requests().is_empty());

        let mut app = TuiApp::new(Config::default());
        app.focus = FocusPane::Plugins;

        assert_eq!(app.handle_key(key(KeyCode::Char('n'))), TuiAction::None);

        assert_eq!(app.focus, FocusPane::PluginSearch);
        assert_eq!(app.take_plugin_refresh_requests(), vec![Target::Codex]);
    }

    #[test]
    fn tui_plugin_search_refreshes_every_time() {
        let mut app = TuiApp::new(Config::default());
        app.available_plugins = vec![plugin(Target::Codex, "market-plugin")];
        app.plugin_refresh_states = [tui::McpRefreshState::Loaded; 2];
        app.focus = FocusPane::Plugins;

        assert_eq!(app.handle_key(key(KeyCode::Char('n'))), TuiAction::None);

        assert_eq!(app.focus, FocusPane::PluginSearch);
        assert_eq!(app.take_plugin_refresh_requests(), vec![Target::Codex]);
        assert_eq!(
            app.selected_available_plugin()
                .map(|plugin| plugin.name.as_str()),
            Some("market-plugin")
        );
    }

    #[test]
    fn tui_mouse_selects_target_and_profile() {
        let mut config = Config::default();
        config
            .add(profile("xcode", Target::Codex, "sk-codex"))
            .expect("profile should be added");
        let mut app = TuiApp::new(config);
        let area = Rect::new(0, 0, 100, 40);

        assert_eq!(app.handle_mouse(mouse_down(2, 2), area), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Targets);
        assert_eq!(app.target_index, 1);

        assert_eq!(app.handle_mouse(mouse_down(2, 1), area), TuiAction::None);
        assert_eq!(app.target_index, 0);

        assert_eq!(app.handle_key(key(KeyCode::Enter)), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Profiles);

        assert_eq!(app.handle_mouse(mouse_down(2, 2), area), TuiAction::None);
        assert_eq!(app.focus, FocusPane::Profiles);
        assert_eq!(app.profile_index, 1);
    }

    #[test]
    fn tui_mouse_selects_form_field() {
        let mut app = TuiApp::new(Config::default());
        app.mode = TuiMode::Editing(ProfileForm::add(Target::Codex));
        let area = Rect::new(0, 0, 100, 40);

        assert_eq!(app.handle_mouse(mouse_down(16, 23), area), TuiAction::None);

        let TuiMode::Editing(form) = app.mode else {
            panic!("form should remain open");
        };
        assert_eq!(form.active_field, 3);
    }

    #[test]
    fn tui_edits_custom_profile_and_updates_current_name() {
        let mut config = Config::default();
        config
            .add(profile("old", Target::Codex, "sk-old"))
            .expect("profile should be added");
        config
            .use_profile("old", Some(Target::Codex))
            .expect("profile should be current");
        let original = config
            .profiles
            .iter()
            .find(|profile| profile.name == "old" && profile.target == Target::Codex)
            .expect("profile should exist")
            .clone();
        let mut app = TuiApp::new(config);
        let mut form = ProfileForm::edit(&original);
        form.name = "new".to_string();
        form.base_url = "https://new.example.test".to_string();

        assert_eq!(app.save_form(&mut form), TuiAction::Save);

        assert_eq!(
            app.config.current_for_target(Target::Codex),
            Some(&CurrentProfile {
                target: Target::Codex,
                name: "new".to_string()
            })
        );
        assert!(app.config.profiles.iter().any(|profile| {
            profile.name == "new"
                && profile.target == Target::Codex
                && profile.base_url == "https://new.example.test"
        }));
        assert!(
            !app.config
                .profiles
                .iter()
                .any(|profile| profile.name == "old" && profile.target == Target::Codex)
        );
    }

    #[test]
    fn tui_delete_current_custom_profile_falls_back_to_builtin() {
        let mut config = Config::default();
        config
            .add(profile("xcode", Target::Codex, "sk-codex"))
            .expect("profile should be added");
        config
            .use_profile("xcode", Some(Target::Codex))
            .expect("profile should be current");
        let mut app = TuiApp::new(config);
        app.focus = FocusPane::Profiles;
        app.profile_index = app
            .selected_profile_indices()
            .iter()
            .position(|index| app.config.profiles[*index].name == "xcode")
            .expect("xcode should be selectable");

        assert_eq!(app.handle_key(key(KeyCode::Char('d'))), TuiAction::None);
        assert!(matches!(app.mode, TuiMode::ConfirmDelete));
        assert_eq!(app.handle_key(key(KeyCode::Enter)), TuiAction::Save);

        assert_eq!(
            app.config.current_for_target(Target::Codex),
            Some(&default_current_profile(Target::Codex))
        );
        assert!(
            !app.config
                .profiles
                .iter()
                .any(|profile| profile.name == "xcode" && profile.target == Target::Codex)
        );
    }

    #[test]
    fn delete_falls_target_current_back_to_default_profile() {
        let mut config = Config {
            current: vec![CurrentProfile {
                target: Target::Codex,
                name: "work".to_string(),
            }],
            profiles: vec![profile("work", Target::Codex, "sk-codex")],
        };

        config
            .delete("work", None)
            .expect("active profile should be deleted");

        assert_eq!(
            config.current_for_target(Target::Codex),
            Some(&default_current_profile(Target::Codex))
        );
        assert!(
            config
                .profiles
                .iter()
                .any(|profile| { profile.name == "openai" && profile.target == Target::Codex })
        );
    }

    #[test]
    fn init_script_unsets_all_supported_variables_and_exports_current_profile() {
        let config = Config {
            current: vec![CurrentProfile {
                target: Target::Codex,
                name: "xcode".to_string(),
            }],
            profiles: vec![Profile {
                name: "xcode".to_string(),
                target: Target::Codex,
                base_url: "https://api.example.test/v1".to_string(),
                api_key: "sk-test'quote".to_string(),
                model: String::new(),
            }],
        };

        let script = config.init_script(Shell::Zsh);

        assert!(script.contains("unfunction codex 2>/dev/null || true\n"));
        assert!(script.contains("unset OPENAI_BASE_URL\n"));
        assert!(!script.contains("unset OPENAI_MODEL\n"));
        assert!(script.contains("unset ANTHROPIC_AUTH_TOKEN\n"));
        assert!(script.contains("unset ANTHROPIC_MODEL\n"));
        assert!(script.contains("export OPENAI_BASE_URL='https://api.example.test/v1'\n"));
        assert!(script.contains("export OPENAI_API_KEY='sk-test'\\''quote'\n"));
        assert!(!script.contains("export OPENAI_MODEL="));
        assert!(!script.contains("export ANTHROPIC_AUTH_TOKEN"));
    }

    #[test]
    fn init_script_wraps_non_default_codex_profile_and_forwards_arguments() {
        let config = Config {
            current: vec![CurrentProfile {
                target: Target::Codex,
                name: "xcode".to_string(),
            }],
            profiles: vec![Profile {
                name: "xcode".to_string(),
                target: Target::Codex,
                base_url: "https://api.example.test/v1".to_string(),
                api_key: "sk-test".to_string(),
                model: "gpt-4.1".to_string(),
            }],
        };

        let script = config.init_script(Shell::Zsh);

        assert!(script.contains("codex() {\n"));
        assert!(script.contains("  command codex \\\n"));
        assert!(script.contains("    -c 'model_provider=xcode' \\\n"));
        assert!(script.contains("    -c 'model=gpt-4.1' \\\n"));
        assert!(script.contains("    -c 'model_providers.xcode.name=xcode' \\\n"));
        assert!(
            script.contains("    -c 'model_providers.xcode.base_url='\"${OPENAI_BASE_URL}\" \\\n")
        );
        assert!(script.contains("    -c 'model_providers.xcode.env_key=OPENAI_API_KEY' \\\n"));
        assert!(
            script.contains("    -c 'model_providers.xcode.wire_api=responses' \\\n    \"$@\"\n")
        );
    }

    #[test]
    fn init_script_does_not_leave_unrendered_template_markers() {
        let config = Config {
            current: vec![CurrentProfile {
                target: Target::Codex,
                name: "xcode".to_string(),
            }],
            profiles: vec![Profile {
                name: "xcode".to_string(),
                target: Target::Codex,
                base_url: "https://api.example.test/v1".to_string(),
                api_key: "sk-test".to_string(),
                model: "gpt-4.1".to_string(),
            }],
        };

        let script = config.init_script(Shell::Zsh);

        assert!(!script.contains("{{"));
        assert!(!script.contains("}}"));
        assert!(!script.contains("{%"));
        assert!(!script.contains("%}"));
    }

    #[test]
    fn init_script_shell_quotes_codex_wrapper_values() {
        let config = Config {
            current: vec![CurrentProfile {
                target: Target::Codex,
                name: "work'dev".to_string(),
            }],
            profiles: vec![Profile {
                name: "work'dev".to_string(),
                target: Target::Codex,
                base_url: "https://api.example.test/v1".to_string(),
                api_key: "sk-test".to_string(),
                model: "gpt'4.1".to_string(),
            }],
        };

        let script = config.init_script(Shell::Zsh);

        assert!(script.contains("    -c 'model_provider=work'\\''dev' \\\n"));
        assert!(script.contains("    -c 'model=gpt'\\''4.1' \\\n"));
        assert!(script.contains("    -c 'model_providers.work'\\''dev.name=work'\\''dev' \\\n"));
    }

    #[test]
    fn init_script_uses_codex_profile_name_in_provider_config() {
        let config = Config {
            current: vec![CurrentProfile {
                target: Target::Codex,
                name: "workdev".to_string(),
            }],
            profiles: vec![profile("workdev", Target::Codex, "sk-codex")],
        };

        let script = config.init_script(Shell::Zsh);

        assert!(script.contains("    -c 'model_provider=workdev' \\\n"));
        assert!(script.contains("    -c 'model_providers.workdev.name=workdev' \\\n"));
    }

    #[test]
    fn init_script_without_current_profile_only_unsets() {
        let script = Config {
            current: Vec::new(),
            profiles: default_profiles(),
        }
        .init_script(Shell::Zsh);

        assert!(script.contains("unset OPENAI_API_KEY\n"));
        assert!(!script.contains("export "));
    }

    #[test]
    fn default_config_includes_openai_and_anthropic_clear_profiles() {
        let config = Config::default();

        assert_eq!(
            config.current_for_target(Target::Codex),
            Some(&default_current_profile(Target::Codex))
        );
        assert_eq!(
            config.current_for_target(Target::Claude),
            Some(&default_current_profile(Target::Claude))
        );
        assert!(config.profiles.iter().any(|profile| {
            profile.name == "openai"
                && profile.target == Target::Codex
                && profile.base_url.is_empty()
                && profile.api_key.is_empty()
        }));
        assert!(config.profiles.iter().any(|profile| {
            profile.name == "anthropic"
                && profile.target == Target::Claude
                && profile.base_url.is_empty()
                && profile.api_key.is_empty()
        }));
    }

    #[test]
    fn using_default_openai_profile_only_unsets_variables() {
        let mut config = Config::default();
        config
            .use_profile("openai", Some(Target::Codex))
            .expect("default openai profile should exist");

        let script = config.init_script(Shell::Zsh);

        assert!(script.contains("unset OPENAI_BASE_URL\n"));
        assert!(script.contains("unset OPENAI_API_KEY\n"));
        assert!(script.contains("unfunction codex 2>/dev/null || true\n"));
        assert!(!script.contains("export "));
        assert!(!script.contains("codex()"));
    }

    #[test]
    fn claude_profile_does_not_register_codex_wrapper() {
        let config = Config {
            current: vec![CurrentProfile {
                target: Target::Claude,
                name: "work".to_string(),
            }],
            profiles: vec![Profile {
                name: "work".to_string(),
                target: Target::Claude,
                base_url: "https://claude.example.test".to_string(),
                api_key: "sk-claude".to_string(),
                model: "claude-sonnet-4-5".to_string(),
            }],
        };

        let script = config.init_script(Shell::Zsh);

        assert!(script.contains("export ANTHROPIC_BASE_URL='https://claude.example.test'\n"));
        assert!(script.contains("export ANTHROPIC_MODEL='claude-sonnet-4-5'\n"));
        assert!(!script.contains("codex()"));
    }

    #[test]
    fn list_marks_default_current_profile_with_star() {
        let table = Config::default().render_table();

        assert!(table.contains("*"));
        assert!(table.contains("openai"));
        assert!(table.contains("MODEL"));
    }

    #[test]
    fn list_includes_profile_model() {
        let config = Config {
            current: default_current_profiles(),
            profiles: vec![Profile {
                name: "xcode".to_string(),
                target: Target::Codex,
                base_url: "https://api.example.test/v1".to_string(),
                api_key: "sk-test".to_string(),
                model: "gpt-4.1".to_string(),
            }],
        };

        let table = config.render_table();

        assert!(table.contains("gpt-4.1"));
    }

    #[test]
    fn list_sorts_profiles_by_target_then_name() {
        let config = Config {
            current: default_current_profiles(),
            profiles: vec![
                profile("shuinfo", Target::Claude, "sk-claude"),
                profile("xcode", Target::Codex, "sk-codex"),
                profile("anthropic", Target::Claude, ""),
                profile("openai", Target::Codex, ""),
            ],
        };

        let table = config.render_table();

        let openai = table.find("openai").expect("openai should be listed");
        let xcode = table.find("xcode").expect("xcode should be listed");
        let anthropic = table.find("anthropic").expect("anthropic should be listed");
        let shuinfo = table.find("shuinfo").expect("shuinfo should be listed");

        assert!(openai < xcode);
        assert!(xcode < anthropic);
        assert!(anthropic < shuinfo);
    }

    #[test]
    fn mask_api_key_keeps_short_keys_fully_masked() {
        assert_eq!(mask_api_key(""), "");
        assert_eq!(mask_api_key("secret"), "******");
        assert_eq!(mask_api_key("123456789"), "1234*6789");
        assert_eq!(mask_api_key("123456789abcdef"), "1234*******cdef");
    }

    #[test]
    fn config_round_trips_toml() {
        let config = Config {
            current: vec![CurrentProfile {
                target: Target::Claude,
                name: "work".to_string(),
            }],
            profiles: vec![profile("work", Target::Claude, "sk-claude")],
        };

        let serialized = toml::to_string_pretty(&config).expect("config should serialize");
        let parsed: Config = toml::from_str(&serialized).expect("config should deserialize");

        assert_eq!(parsed, config);
    }

    #[test]
    fn config_deserializes_profiles_without_model() {
        let config: Config = toml::from_str(
            r#"
                [[profiles]]
                name = "xcode"
                target = "codex"
                base_url = "https://api.example.test/v1"
                api_key = "sk-test"
            "#,
        )
        .expect("legacy config should deserialize");

        assert_eq!(config.profiles[0].model, "");
    }

    #[test]
    fn config_path_uses_xdg_config_home_when_present() {
        let path = config_path_from(Some(PathBuf::from("/tmp/lazycc-test-config")))
            .expect("path should resolve");

        assert_eq!(
            path,
            PathBuf::from("/tmp/lazycc-test-config/lazycc/config.toml")
        );
    }

    #[cfg(unix)]
    #[test]
    fn save_writes_config_with_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("lazycc-test-{unique}"));
        let path = dir.join("config.toml");
        let config = Config {
            current: Vec::new(),
            profiles: vec![profile("work", Target::Codex, "sk-codex")],
        };

        config.save(&path).expect("config should save");

        let mode = fs::metadata(&path)
            .expect("config file should exist")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);

        fs::remove_dir_all(dir).expect("test dir should be removed");
    }
}
