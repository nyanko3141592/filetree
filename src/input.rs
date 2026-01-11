use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::app::{App, ConfirmAction, InputMode};

pub fn handle_key_event(app: &mut App, key: KeyEvent, visible_height: usize) {
    match &app.input_mode {
        InputMode::Normal => handle_normal_mode(app, key),
        InputMode::Search | InputMode::Rename | InputMode::NewFile | InputMode::NewDir => {
            handle_input_mode(app, key);
        }
        InputMode::Confirm(_) => handle_confirm_mode(app, key),
        InputMode::Preview => handle_preview_mode(app, key, visible_height),
    }
}

fn handle_normal_mode(app: &mut App, key: KeyEvent) {
    // Don't clear message while buffering drop
    if app.drop_buffer.is_empty() {
        app.message = None;
    }

    // Handle Ctrl+p for quick preview toggle
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('p') {
        app.toggle_quick_preview();
        return;
    }

    match key.code {
        // Quit
        KeyCode::Char('q') => app.should_quit = true,

        // Navigation (update quick preview after movement)
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_up();
            app.update_quick_preview();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_down();
            app.update_quick_preview();
        }
        KeyCode::Char('g') => {
            app.move_to_top();
            app.update_quick_preview();
        }
        KeyCode::Char('G') => {
            app.move_to_bottom();
            app.update_quick_preview();
        }

        // Expand/Collapse
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
            app.expand_current();
            app.update_quick_preview();
        }
        KeyCode::Backspace | KeyCode::Char('h') | KeyCode::Left => {
            app.collapse_current();
            app.update_quick_preview();
        }
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

        // Search (buffered for drop detection)
        // Also buffer quotes and backslash for quoted/escaped paths
        KeyCode::Char(c @ ('/' | '\'' | '"' | '\\')) => app.buffer_char(c),
        KeyCode::Char('n') => app.search_next(),

        // Reload tree
        KeyCode::Char('R') | KeyCode::F(5) => app.refresh(),

        // Toggle hidden files
        KeyCode::Char('.') => app.toggle_hidden(),

        // Copy path to clipboard
        KeyCode::Char('c') => app.copy_path(),
        KeyCode::Char('C') => app.copy_filename(),

        // Preview file
        KeyCode::Char('o') => app.preview_file(),

        // Help
        KeyCode::Char('?') => {
            app.message = Some("o:preview  ^p:quick  c:path  C:name  y:yank  d:cut  p:paste  D:del  r:rename  a:file  A:dir".to_string());
        }

        // Buffer unknown chars for drop detection
        KeyCode::Char(c) => {
            if !app.drop_buffer.is_empty() {
                app.buffer_char(c);
            }
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
                app.update_quick_preview();
            }
        }
        MouseEventKind::ScrollUp => {
            app.scroll_up(3);
            app.update_quick_preview();
        }
        MouseEventKind::ScrollDown => {
            app.scroll_down(3);
            app.update_quick_preview();
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
            if let InputMode::Confirm(ConfirmAction::Delete(_)) = app.input_mode {
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

fn handle_preview_mode(app: &mut App, key: KeyEvent, visible_height: usize) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('o') => app.close_preview(),
        KeyCode::Up | KeyCode::Char('k') => app.preview_scroll_up(),
        KeyCode::Down | KeyCode::Char('j') => app.preview_scroll_down(visible_height),
        KeyCode::PageUp | KeyCode::Char('b') => app.preview_page_up(visible_height),
        KeyCode::PageDown | KeyCode::Char('f') | KeyCode::Char(' ') => app.preview_page_down(visible_height),
        KeyCode::Char('g') => app.preview_scroll = 0,
        KeyCode::Char('G') => {
            app.preview_scroll = app.preview_content.len().saturating_sub(visible_height);
        }
        _ => {}
    }
}
