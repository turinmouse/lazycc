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

use crate::config::Config;
use crate::error::LazyccError;

use super::state::{TuiAction, TuiApp};
use super::view::draw_tui;

pub(crate) fn run_tui(path: &Path) -> Result<(), LazyccError> {
    let config = Config::load(path)?;
    let mut app = TuiApp::new(config);
    app.refresh_mcp_servers();
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
    loop {
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
    }
}
