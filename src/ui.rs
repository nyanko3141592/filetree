use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, ConfirmAction, InputMode};
use crate::git_status::GitStatus;

pub fn draw(frame: &mut Frame, app: &mut App) -> usize {
    // If in preview mode, draw preview instead
    if app.input_mode == InputMode::Preview {
        return draw_preview(frame, app);
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_file_tree(frame, app, chunks[0]);
    draw_status_bar(frame, app, chunks[1]);

    // Draw input popup if in input mode
    match &app.input_mode {
        InputMode::Search | InputMode::Rename | InputMode::NewFile | InputMode::NewDir => {
            draw_input_popup(frame, app);
        }
        InputMode::Confirm(action) => {
            draw_confirm_popup(frame, app, action);
        }
        InputMode::Normal | InputMode::Preview => {}
    }

    app.tree_area_height
}

fn draw_file_tree(frame: &mut Frame, app: &mut App, area: Rect) {
    let visible_height = area.height.saturating_sub(2) as usize;
    app.adjust_scroll(visible_height);

    let items: Vec<ListItem> = (app.scroll_offset..app.tree.len())
        .take(visible_height)
        .filter_map(|i| {
            let node = app.tree.get_node(i)?;
            let indent = "  ".repeat(node.depth);

            let icon = if node.is_dir {
                if node.expanded { "" } else { "" }
            } else {
                get_file_icon(&node.name)
            };

            let is_selected = i == app.selected;
            let is_marked = app.marked.contains(&node.path);
            let is_cut = app.clipboard.content.as_ref().map_or(false, |c| {
                matches!(c, crate::file_ops::ClipboardContent::Cut(paths) if paths.contains(&node.path))
            });
            let git_status = app.git_repo.get_status(&node.path);

            let mark_indicator = if is_marked { "*" } else { " " };

            let mut style = Style::default();
            if is_selected {
                style = style.bg(Color::DarkGray).add_modifier(Modifier::BOLD);
            }
            if is_cut {
                style = style.fg(Color::DarkGray);
            } else {
                // Apply git status color
                style = style.fg(match git_status {
                    GitStatus::Modified => Color::Yellow,
                    GitStatus::Added => Color::Green,
                    GitStatus::Untracked => Color::Green,
                    GitStatus::Deleted => Color::Red,
                    GitStatus::Renamed => Color::Cyan,
                    GitStatus::Conflict => Color::Magenta,
                    GitStatus::Ignored => Color::DarkGray,
                    GitStatus::None => {
                        if node.is_dir {
                            Color::Blue
                        } else {
                            Color::Reset
                        }
                    }
                });
            }

            let line = Line::from(vec![
                Span::styled(mark_indicator, Style::default().fg(Color::Yellow)),
                Span::styled(format!("{}{} {}", indent, icon, node.name), style),
            ]);

            Some(ListItem::new(line))
        })
        .collect();

    let title = format!(" {} ", app.tree.root.path.display());
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title));

    frame.render_widget(list, area);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left: message or help
    let message = app.message.as_deref().unwrap_or("? for help");
    let msg = Paragraph::new(message)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(msg, chunks[0]);

    // Right: stats
    let marked_count = app.marked.len();
    let clipboard_info = if app.clipboard.is_empty() {
        String::new()
    } else {
        match &app.clipboard.content {
            Some(crate::file_ops::ClipboardContent::Copy(p)) => format!(" | Copied: {}", p.len()),
            Some(crate::file_ops::ClipboardContent::Cut(p)) => format!(" | Cut: {}", p.len()),
            None => String::new(),
        }
    };

    let stats = format!(
        "{}/{}{}{}",
        app.selected + 1,
        app.tree.len(),
        if marked_count > 0 { format!(" | Marked: {}", marked_count) } else { String::new() },
        clipboard_info
    );
    let stats_widget = Paragraph::new(stats)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(stats_widget, chunks[1]);
}

fn draw_input_popup(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 3, frame.area());

    let title = match app.input_mode {
        InputMode::Search => "Search",
        InputMode::Rename => "Rename",
        InputMode::NewFile => "New File",
        InputMode::NewDir => "New Directory",
        _ => "",
    };

    let input = Paragraph::new(app.input_buffer.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title(title));

    frame.render_widget(Clear, area);
    frame.render_widget(input, area);
}

fn draw_confirm_popup(frame: &mut Frame, _app: &App, action: &ConfirmAction) {
    let area = centered_rect(40, 5, frame.area());

    let message = match action {
        ConfirmAction::Delete => "Delete selected item(s)?",
    };

    let content = vec![
        Line::from(message),
        Line::from(""),
        Line::from(vec![
            Span::styled("y", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" to confirm, "),
            Span::styled("n", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(" to cancel"),
        ]),
    ];

    let popup = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title("Confirm"));

    frame.render_widget(Clear, area);
    frame.render_widget(popup, area);
}

fn draw_preview(frame: &mut Frame, app: &App) -> usize {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let visible_height = chunks[0].height.saturating_sub(2) as usize;

    let title = app.preview_path
        .as_ref()
        .map(|p| format!(" {} ", p.display()))
        .unwrap_or_else(|| " Preview ".to_string());

    let lines: Vec<Line> = app.preview_content
        .iter()
        .skip(app.preview_scroll)
        .take(visible_height)
        .enumerate()
        .map(|(i, line)| {
            let line_num = app.preview_scroll + i + 1;
            Line::from(vec![
                Span::styled(
                    format!("{:4} ", line_num),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(line.as_str()),
            ])
        })
        .collect();

    let preview = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title));

    frame.render_widget(preview, chunks[0]);

    // Status bar
    let total_lines = app.preview_content.len();
    let current_line = app.preview_scroll + 1;
    let percent = if total_lines > 0 {
        (current_line * 100) / total_lines
    } else {
        100
    };

    let status = format!(
        " Line {}/{} ({}%) | j/k:scroll  f/b:page  g/G:top/bottom  q/Esc:close ",
        current_line, total_lines, percent
    );
    let status_widget = Paragraph::new(status)
        .style(Style::default().bg(Color::DarkGray));

    frame.render_widget(status_widget, chunks[1]);

    visible_height
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((area.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn get_file_icon(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("");
    match ext.to_lowercase().as_str() {
        "rs" => "",
        "py" => "",
        "js" | "jsx" => "",
        "ts" | "tsx" => "",
        "html" => "",
        "css" | "scss" | "sass" => "",
        "json" => "",
        "toml" | "yaml" | "yml" => "",
        "md" => "",
        "txt" => "",
        "git" | "gitignore" => "",
        "lock" => "",
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "ico" => "",
        "mp3" | "wav" | "flac" => "",
        "mp4" | "mkv" | "avi" => "",
        "zip" | "tar" | "gz" | "rar" => "",
        "pdf" => "",
        "doc" | "docx" => "",
        "sh" | "bash" | "zsh" => "",
        _ => "",
    }
}
