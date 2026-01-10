use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::app::{App, ConfirmAction, InputMode};

pub fn handle_key_event(app: &mut App, key: KeyEvent) {
    match &app.input_mode {
        InputMode::Normal => handle_normal_mode(app, key),
        InputMode::Search | InputMode::Rename | InputMode::NewFile | InputMode::NewDir => {
            handle_input_mode(app, key);
        }
        InputMode::Confirm(_) => handle_confirm_mode(app, key),
    }
}

fn handle_normal_mode(app: &mut App, key: KeyEvent) {
    app.message = None;

    match key.code {
        // Quit
        KeyCode::Char('q') => app.should_quit = true,

        // Navigation
        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
        KeyCode::Char('g') => app.move_to_top(),
        KeyCode::Char('G') => app.move_to_bottom(),

        // Expand/Collapse
        KeyCode::Enter | KeyCode::Char('l') => app.expand_current(),
        KeyCode::Backspace | KeyCode::Char('h') => app.collapse_current(),
        KeyCode::Tab => app.toggle_expand(),
        KeyCode::Char('H') => app.collapse_all(),
        KeyCode::Char('L') => app.expand_all(),

        // Marking
        KeyCode::Char(' ') => app.toggle_mark(),
        KeyCode::Esc => app.clear_marks(),

        // Clipboard operations
        KeyCode::Char('y') => app.yank(),
        KeyCode::Char('d') => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                app.confirm_delete();
            } else {
                app.cut();
            }
        }
        KeyCode::Char('D') | KeyCode::Delete => app.confirm_delete(),
        KeyCode::Char('p') => app.paste(),

        // File operations
        KeyCode::Char('r') => app.start_rename(),
        KeyCode::Char('a') => app.start_new_file(),
        KeyCode::Char('A') => app.start_new_dir(),

        // Search
        KeyCode::Char('/') => app.start_search(),
        KeyCode::Char('n') => app.search_next(),

        // Refresh
        KeyCode::Char('R') | KeyCode::F(5) => app.refresh(),

        // Toggle hidden files
        KeyCode::Char('.') => app.toggle_hidden(),

        // Copy path to clipboard
        KeyCode::Char('c') => app.copy_path(),
        KeyCode::Char('C') => app.copy_filename(),

        // Open in external editor
        KeyCode::Char('o') => app.open_in_editor(),

        // Help
        KeyCode::Char('?') => {
            app.message = Some("c:copy path  C:copy name  y:yank  d:cut  p:paste  D:del  r:rename  a:file  A:dir  /:search".to_string());
        }

        _ => {}
    }
}

pub fn handle_mouse_event(app: &mut App, mouse: MouseEvent) {
    if app.input_mode != InputMode::Normal {
        return;
    }

    match mouse.kind {
        MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            // Tree area starts at row 1 (after border)
            if mouse.row > 0 {
                app.handle_click(mouse.row - 1);
            }
        }
        MouseEventKind::ScrollUp => {
            app.scroll_up(3);
        }
        MouseEventKind::ScrollDown => {
            app.scroll_down(3);
        }
        _ => {}
    }
}

fn handle_input_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => app.confirm_input(),
        KeyCode::Esc => app.cancel_input(),
        KeyCode::Backspace => {
            app.input_buffer.pop();
        }
        KeyCode::Char(c) => {
            app.input_buffer.push(c);
        }
        _ => {}
    }
}

fn handle_confirm_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            if let InputMode::Confirm(ConfirmAction::Delete) = app.input_mode {
                app.execute_delete();
            }
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.message = Some("Cancelled".to_string());
        }
        _ => {}
    }
}
