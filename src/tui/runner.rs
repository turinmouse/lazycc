use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::sync::mpsc::{self, Sender};
use std::thread;
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

use super::state::{TuiAction, TuiApp};
use super::view::draw_tui;

pub(crate) fn run_tui(path: &Path) -> Result<(), LazyccError> {
    let config = Config::load(path)?;
    let mut app = TuiApp::new(config);
    app.request_mcp_refresh();
    app.request_plugin_refresh();
    let mut stdout = io::stdout();

    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let result = {
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        let result = run_tui_loop(&mut terminal, &mut app, path);
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
    path: &Path,
) -> Result<(), LazyccError> {
    let (mcp_sender, mcp_receiver) = mpsc::channel();
    let (plugin_sender, plugin_receiver) = mpsc::channel();
    let mut loading_mcp_targets = HashSet::new();
    let mut loading_plugin_targets = HashSet::new();
    spawn_mcp_refresh_requests(
        app.take_mcp_refresh_requests(),
        &mut loading_mcp_targets,
        &mcp_sender,
    );
    spawn_plugin_refresh_requests(
        app.take_plugin_refresh_requests(),
        &mut loading_plugin_targets,
        &plugin_sender,
    );

    loop {
        while let Ok(result) = mcp_receiver.try_recv() {
            loading_mcp_targets.remove(&result.target);
            app.finish_mcp_refresh(result.target, result.servers, result.error);
        }
        while let Ok(result) = plugin_receiver.try_recv() {
            loading_plugin_targets.remove(&result.target);
            app.finish_plugin_refresh(result.target, result.plugins, result.error);
        }

        terminal.draw(|frame| draw_tui(frame, app))?;

        if app.should_quit {
            return Ok(());
        }

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => match app.handle_key(key) {
                    TuiAction::None => {}
                    TuiAction::Save => app.config.save(path)?,
                    TuiAction::Quit => return Ok(()),
                },
                Event::Mouse(mouse) => {
                    let size = terminal.size()?;
                    let area = Rect::new(0, 0, size.width, size.height);
                    match app.handle_mouse(mouse, area) {
                        TuiAction::None => {}
                        TuiAction::Save => app.config.save(path)?,
                        TuiAction::Quit => return Ok(()),
                    }
                }
                _ => {}
            }
        }

        spawn_mcp_refresh_requests(
            app.take_mcp_refresh_requests(),
            &mut loading_mcp_targets,
            &mcp_sender,
        );
        spawn_plugin_refresh_requests(
            app.take_plugin_refresh_requests(),
            &mut loading_plugin_targets,
            &plugin_sender,
        );
    }
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

fn spawn_mcp_refresh_requests(
    targets: Vec<Target>,
    loading_targets: &mut HashSet<Target>,
    sender: &Sender<McpRefreshResult>,
) {
    for target in targets {
        if !loading_targets.insert(target) {
            continue;
        }

        spawn_mcp_refresh(target, sender.clone());
    }
}

fn spawn_mcp_refresh(target: Target, sender: Sender<McpRefreshResult>) {
    thread::spawn(move || {
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

        let _ = sender.send(McpRefreshResult {
            target,
            servers,
            error,
        });
    });
}

fn spawn_plugin_refresh_requests(
    targets: Vec<Target>,
    loading_targets: &mut HashSet<Target>,
    sender: &Sender<PluginRefreshResult>,
) {
    for target in targets {
        if !loading_targets.insert(target) {
            continue;
        }

        spawn_plugin_refresh(target, sender.clone());
    }
}

fn spawn_plugin_refresh(target: Target, sender: Sender<PluginRefreshResult>) {
    thread::spawn(move || {
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

        let _ = sender.send(PluginRefreshResult {
            target,
            plugins,
            error,
        });
    });
}
