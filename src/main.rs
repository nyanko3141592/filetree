mod app;
mod file_ops;
mod file_tree;
mod git_status;
mod input;
mod ui;

use std::env;
use std::io::{self, stdout};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use app::App;

fn main() -> Result<()> {
    // Get the path to browse (default: current directory)
    let path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let path = path.canonicalize().unwrap_or(path);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new(&path)?;
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableBracketedPaste
    )?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    let mut visible_height = 20usize;

    loop {
        terminal.draw(|f| {
            app.tree_area_height = f.area().height.saturating_sub(5) as usize;
            visible_height = ui::draw(f, app);
        })?;

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    input::handle_key_event(app, key, visible_height);
                }
                Event::Mouse(mouse) => {
                    input::handle_mouse_event(app, mouse);
                }
                Event::Paste(text) => {
                    app.handle_drop(&text);
                }
                _ => {}
            }
        }

        // Check drop buffer timeout
        app.check_drop_buffer();

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
