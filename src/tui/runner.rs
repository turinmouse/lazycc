use std::collections::{HashSet, VecDeque};
use std::io;
use std::path::Path;
use std::time::Duration;

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;

use crate::config::{Config, Target};
use crate::error::LazyccError;
use crate::tools::{McpServer, Plugin, tool_for};

use super::cache::TuiCache;
use super::state::{TuiAction, TuiApp, TuiOperation, TuiOperationResult, load_plugins_for};
use super::view::draw_tui;

pub(crate) fn run_tui(path: &Path) -> Result<(), LazyccError> {
    let config = Config::load(path)?;
    let mut app = TuiApp::new(config);
    let mut cache = TuiCache::load();
    app.apply_cache(cache.clone());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let mut stdout = io::stdout();

    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let result = {
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        let result = run_tui_loop(&mut terminal, &mut app, &mut cache, path, &runtime);
        let _ = terminal.show_cursor();
        result
    };

    let restore_result = restore_terminal();
    result.and(restore_result)
}

fn restore_terminal() -> Result<(), LazyccError> {
    disable_raw_mode()?;
    execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
    Ok(())
}

fn run_tui_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut TuiApp,
    cache: &mut TuiCache,
    path: &Path,
    runtime: &tokio::runtime::Runtime,
) -> Result<(), LazyccError> {
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let mut loading_mcp_targets = HashSet::new();
    let mut loading_installed_plugin_targets = HashSet::new();
    let mut loading_plugin_targets = HashSet::new();
    let mut pending_refreshes = VecDeque::new();
    let mut spawned_startup_refreshes = false;

    loop {
        while let Ok(event) = receiver.try_recv() {
            match event {
                TuiEvent::McpRefresh(result) => {
                    loading_mcp_targets.remove(&result.target);
                    if result.error.is_none() {
                        cache.replace_mcp_servers(result.target, result.servers.clone());
                        let _ = cache.save();
                    }
                    app.finish_mcp_refresh(result.target, result.servers, result.error);
                }
                TuiEvent::InstalledPluginRefresh(result) => {
                    loading_installed_plugin_targets.remove(&result.target);
                    if result.error.is_none() {
                        cache.replace_installed_plugins(result.target, result.plugins.clone());
                        let _ = cache.save();
                    }
                    app.finish_installed_plugin_refresh(
                        result.target,
                        result.plugins,
                        result.error,
                    );
                }
                TuiEvent::PluginRefresh(result) => {
                    loading_plugin_targets.remove(&result.target);
                    app.finish_plugin_refresh(result.target, result.plugins, result.error);
                }
                TuiEvent::Operation(result) => app.finish_operation(result),
            }
        }
        if spawned_startup_refreshes {
            queue_pending_refresh_requests(app, &mut pending_refreshes);
        }

        terminal.draw(|frame| draw_tui(frame, app))?;

        if !spawned_startup_refreshes {
            queue_pending_refresh_requests(app, &mut pending_refreshes);
            spawned_startup_refreshes = true;
        }
        spawn_next_refresh_request(
            &mut pending_refreshes,
            &mut loading_mcp_targets,
            &mut loading_installed_plugin_targets,
            &mut loading_plugin_targets,
            &sender,
            runtime,
        );

        if app.should_quit {
            return Ok(());
        }

        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => {
                    if handle_action(app.handle_key(key), app, path, &sender, runtime)? {
                        return Ok(());
                    }
                }
                Event::Mouse(mouse) => {
                    let size = terminal.size()?;
                    let area = Rect::new(0, 0, size.width, size.height);
                    if handle_action(app.handle_mouse(mouse, area), app, path, &sender, runtime)? {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }
    }
}

fn handle_action(
    action: TuiAction,
    app: &mut TuiApp,
    path: &Path,
    sender: &tokio::sync::mpsc::UnboundedSender<TuiEvent>,
    runtime: &tokio::runtime::Runtime,
) -> Result<bool, LazyccError> {
    match action {
        TuiAction::None => Ok(false),
        TuiAction::Run(operation) => {
            spawn_operation(operation, sender.clone(), runtime);
            Ok(false)
        }
        TuiAction::Save => {
            app.config.save(path)?;
            Ok(false)
        }
        TuiAction::Quit => Ok(true),
    }
}

fn queue_pending_refresh_requests(
    app: &mut TuiApp,
    pending_refreshes: &mut VecDeque<RefreshRequest>,
) {
    pending_refreshes.extend(
        app.take_mcp_refresh_requests()
            .into_iter()
            .map(RefreshRequest::Mcp),
    );
    pending_refreshes.extend(
        app.take_installed_plugin_refresh_requests()
            .into_iter()
            .map(RefreshRequest::InstalledPlugin),
    );
    pending_refreshes.extend(
        app.take_plugin_refresh_requests()
            .into_iter()
            .map(RefreshRequest::Plugin),
    );
}

fn spawn_next_refresh_request(
    pending_refreshes: &mut VecDeque<RefreshRequest>,
    loading_mcp_targets: &mut HashSet<Target>,
    loading_installed_plugin_targets: &mut HashSet<Target>,
    loading_plugin_targets: &mut HashSet<Target>,
    sender: &tokio::sync::mpsc::UnboundedSender<TuiEvent>,
    runtime: &tokio::runtime::Runtime,
) {
    let Some(request) = pending_refreshes.pop_front() else {
        return;
    };
    match request {
        RefreshRequest::Mcp(target) => {
            if loading_mcp_targets.insert(target) {
                spawn_mcp_refresh(target, sender.clone(), runtime);
            }
        }
        RefreshRequest::InstalledPlugin(target) => {
            if loading_installed_plugin_targets.insert(target) {
                spawn_installed_plugin_refresh(target, sender.clone(), runtime);
            }
        }
        RefreshRequest::Plugin(target) => {
            if loading_plugin_targets.insert(target) {
                spawn_plugin_refresh(target, sender.clone(), runtime);
            }
        }
    }
}

#[derive(Debug)]
enum TuiEvent {
    McpRefresh(McpRefreshResult),
    InstalledPluginRefresh(InstalledPluginRefreshResult),
    PluginRefresh(PluginRefreshResult),
    Operation(TuiOperationResult),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RefreshRequest {
    Mcp(Target),
    InstalledPlugin(Target),
    Plugin(Target),
}

#[derive(Debug)]
struct McpRefreshResult {
    target: Target,
    servers: Vec<McpServer>,
    error: Option<String>,
}

#[derive(Debug)]
struct PluginRefreshResult {
    target: Target,
    plugins: Vec<Plugin>,
    error: Option<String>,
}

#[derive(Debug)]
struct InstalledPluginRefreshResult {
    target: Target,
    plugins: Vec<Plugin>,
    error: Option<String>,
}

fn spawn_mcp_refresh(
    target: Target,
    sender: tokio::sync::mpsc::UnboundedSender<TuiEvent>,
    runtime: &tokio::runtime::Runtime,
) {
    runtime.spawn(async move {
        let result = tokio::task::spawn_blocking(move || load_mcp_refresh(target))
            .await
            .unwrap_or_else(|error| McpRefreshResult {
                target,
                servers: Vec::new(),
                error: Some(error.to_string()),
            });
        let _ = sender.send(TuiEvent::McpRefresh(result));
    });
}

fn spawn_installed_plugin_refresh(
    target: Target,
    sender: tokio::sync::mpsc::UnboundedSender<TuiEvent>,
    runtime: &tokio::runtime::Runtime,
) {
    runtime.spawn(async move {
        let result = tokio::task::spawn_blocking(move || load_installed_plugin_refresh(target))
            .await
            .unwrap_or_else(|error| InstalledPluginRefreshResult {
                target,
                plugins: Vec::new(),
                error: Some(error.to_string()),
            });
        let _ = sender.send(TuiEvent::InstalledPluginRefresh(result));
    });
}

fn spawn_plugin_refresh(
    target: Target,
    sender: tokio::sync::mpsc::UnboundedSender<TuiEvent>,
    runtime: &tokio::runtime::Runtime,
) {
    runtime.spawn(async move {
        let result = tokio::task::spawn_blocking(move || load_plugin_refresh(target))
            .await
            .unwrap_or_else(|error| PluginRefreshResult {
                target,
                plugins: Vec::new(),
                error: Some(error.to_string()),
            });
        let _ = sender.send(TuiEvent::PluginRefresh(result));
    });
}

fn spawn_operation(
    operation: TuiOperation,
    sender: tokio::sync::mpsc::UnboundedSender<TuiEvent>,
    runtime: &tokio::runtime::Runtime,
) {
    runtime.spawn(async move {
        let fallback_operation = operation.clone();
        let result = tokio::task::spawn_blocking(move || run_operation(operation))
            .await
            .unwrap_or_else(|error| TuiOperationResult {
                operation: fallback_operation,
                error: Some(error.to_string()),
            });
        let _ = sender.send(TuiEvent::Operation(result));
    });
}

fn load_mcp_refresh(target: Target) -> McpRefreshResult {
    let tool = tool_for(target);
    let (servers, error) = match tool.mcp() {
        Some(mcp) => match mcp.list_servers() {
            Ok(servers) => (servers, None),
            Err(current_error) => (Vec::new(), Some(current_error.to_string())),
        },
        None => {
            let message = format!("{} does not support MCP", tool.display_name());
            (Vec::new(), Some(message))
        }
    };

    McpRefreshResult {
        target,
        servers,
        error,
    }
}

fn load_installed_plugin_refresh(target: Target) -> InstalledPluginRefreshResult {
    let tool = tool_for(target);
    let (plugins, error) = match tool.plugin() {
        Some(_) => (load_plugins_for(target), None),
        None => {
            let message = format!("{} does not support plugins", tool.display_name());
            (Vec::new(), Some(message))
        }
    };

    InstalledPluginRefreshResult {
        target,
        plugins,
        error,
    }
}

fn load_plugin_refresh(target: Target) -> PluginRefreshResult {
    let tool = tool_for(target);
    let (plugins, error) = match tool.plugin() {
        Some(plugin) => match plugin.list_available_plugins() {
            Ok(plugins) => (plugins, None),
            Err(current_error) => (Vec::new(), Some(current_error.to_string())),
        },
        None => {
            let message = format!("{} does not support plugins", tool.display_name());
            (Vec::new(), Some(message))
        }
    };

    PluginRefreshResult {
        target,
        plugins,
        error,
    }
}

fn run_operation(operation: TuiOperation) -> TuiOperationResult {
    let error = match &operation {
        TuiOperation::RemoveMcp(server) => match tool_for(server.target).mcp() {
            Some(mcp) => mcp.remove_server(&server.name).err(),
            None => Some(crate::tools::ToolError::CommandFailed {
                command: format!("{} mcp remove {}", server.target, server.name),
                source: format!("{} does not support MCP", server.target),
            }),
        },
        TuiOperation::TogglePlugin { plugin, enabled } => match tool_for(plugin.target).plugin() {
            Some(capability) => capability.set_plugin_enabled(&plugin.name, *enabled).err(),
            None => Some(crate::tools::ToolError::CommandFailed {
                command: format!("{} plugin enable {}", plugin.target, plugin.name),
                source: format!("{} does not support plugins", plugin.target),
            }),
        },
        TuiOperation::RemovePlugin(plugin) => match tool_for(plugin.target).plugin() {
            Some(capability) => capability.remove_plugin(&plugin.name).err(),
            None => Some(crate::tools::ToolError::CommandFailed {
                command: format!("{} plugin remove {}", plugin.target, plugin.name),
                source: format!("{} does not support plugins", plugin.target),
            }),
        },
        TuiOperation::InstallPlugin(plugin) => match tool_for(plugin.target).plugin() {
            Some(capability) => capability.install_plugin(&plugin.selector).err(),
            None => Some(crate::tools::ToolError::CommandFailed {
                command: format!("{} plugin install {}", plugin.target, plugin.selector),
                source: format!("{} does not support plugins", plugin.target),
            }),
        },
        TuiOperation::UninstallPlugin(plugin) => match tool_for(plugin.target).plugin() {
            Some(capability) => capability.remove_plugin(&plugin.selector).err(),
            None => Some(crate::tools::ToolError::CommandFailed {
                command: format!("{} plugin uninstall {}", plugin.target, plugin.selector),
                source: format!("{} does not support plugins", plugin.target),
            }),
        },
    }
    .map(|error| error.to_string());

    TuiOperationResult { operation, error }
}
