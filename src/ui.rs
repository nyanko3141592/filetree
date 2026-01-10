use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, ConfirmAction, DeleteInfo, ImagePreview, InputMode};
use crate::git_status::GitStatus;

pub fn draw(frame: &mut Frame, app: &mut App) -> usize {
    // If in preview mode, draw preview instead
    if app.input_mode == InputMode::Preview {
        return draw_preview(frame, app);
    }

    // Calculate layout based on quick preview state
    let quick_preview_height = if app.quick_preview_enabled { 12 } else { 0 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(quick_preview_height),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_file_tree(frame, app, chunks[0]);

    if app.quick_preview_enabled {
        draw_quick_preview(frame, app, chunks[1]);
    }

    draw_status_bar(frame, app, chunks[2]);

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

    let max_title_width = area.width.saturating_sub(4) as usize; // Account for borders and padding
    let title = format!(" {} ", abbreviate_path(&app.tree.root.path, max_title_width));
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

    let branch_info = app.git_repo.branch.as_ref()
        .map(|b| format!(" {}", b))
        .unwrap_or_default();

    let stats = format!(
        "{}/{}{}{}{}",
        app.selected + 1,
        app.tree.len(),
        if marked_count > 0 { format!(" | Marked: {}", marked_count) } else { String::new() },
        clipboard_info,
        branch_info
    );
    let stats_widget = Paragraph::new(stats)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(stats_widget, chunks[1]);
}

fn draw_quick_preview(frame: &mut Frame, app: &App, area: Rect) {
    // If we have an image preview, render it
    if let Some(img) = &app.quick_preview_image {
        let title = app.quick_preview_path
            .as_ref()
            .map(|p| {
                let name = p.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                format!(" {} ({}x{}) [Ctrl+p: close] ", name, img.width, img.height)
            })
            .unwrap_or_else(|| " Quick Preview ".to_string());

        let img_width = area.width.saturating_sub(2) as u32;
        let img_height = (area.height.saturating_sub(2) * 2) as u32;

        let lines = render_image_to_lines(img, img_width, img_height);

        let preview = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(title));

        frame.render_widget(preview, area);
        return;
    }

    let visible_height = area.height.saturating_sub(2) as usize;

    let title = app.quick_preview_path
        .as_ref()
        .map(|p| {
            let name = p.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            format!(" {} [Ctrl+p: close] ", name)
        })
        .unwrap_or_else(|| " Quick Preview ".to_string());

    let lines: Vec<Line> = app.quick_preview_content
        .iter()
        .skip(app.quick_preview_scroll)
        .take(visible_height)
        .enumerate()
        .map(|(i, line)| {
            let line_num = app.quick_preview_scroll + i + 1;
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

    frame.render_widget(preview, area);
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
    match action {
        ConfirmAction::Delete(info) => draw_delete_confirm_popup(frame, info),
    }
}

fn draw_delete_confirm_popup(frame: &mut Frame, info: &DeleteInfo) {
    // Calculate height based on content
    let max_items_to_show = 8;
    let items_count = info.paths.len().min(max_items_to_show);
    let has_more = info.paths.len() > max_items_to_show;

    // Height: title(1) + warning(2 if dir) + items + "more" line + blank + confirm line + borders(2)
    let warning_lines = if info.has_directories { 2 } else { 0 };
    let more_line = if has_more { 1 } else { 0 };
    let height = (3 + warning_lines + items_count + more_line + 2) as u16;

    let area = centered_rect(60, height, frame.area());

    let mut content = Vec::new();

    // Directory warning (emphasized)
    if info.has_directories {
        content.push(Line::from(vec![
            Span::styled(
                "!! WARNING: FOLDER DELETION !!",
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK),
            ),
        ]));
        content.push(Line::from(vec![
            Span::styled(
                "Folders and all contents will be permanently deleted",
                Style::default().fg(Color::Yellow),
            ),
        ]));
        content.push(Line::from(""));
    }

    // List items to delete
    content.push(Line::from(vec![
        Span::styled(
            format!("Delete {} item(s):", info.paths.len()),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]));

    for path in info.paths.iter().take(max_items_to_show) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());

        let (icon, style) = if path.is_dir() {
            ("", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        } else {
            ("", Style::default().fg(Color::White))
        };

        content.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{} {}", icon, name), style),
        ]));
    }

    if has_more {
        content.push(Line::from(vec![
            Span::styled(
                format!("  ... and {} more", info.paths.len() - max_items_to_show),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    content.push(Line::from(""));
    content.push(Line::from(vec![
        Span::styled("y", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(" to confirm, "),
        Span::styled("n", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw(" to cancel"),
    ]));

    let title = if info.has_directories {
        " !! DELETE FOLDERS !! "
    } else {
        " Confirm Delete "
    };

    let title_style = if info.has_directories {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let popup = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if info.has_directories {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default()
                })
                .title(Span::styled(title, title_style)),
        );

    frame.render_widget(Clear, area);
    frame.render_widget(popup, area);
}

fn draw_preview(frame: &mut Frame, app: &App) -> usize {
    // If we have an image preview, use the image preview renderer
    if app.image_preview.is_some() {
        return draw_image_preview(frame, app);
    }

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

fn draw_image_preview(frame: &mut Frame, app: &App) -> usize {
    let area = frame.area();
    let is_wide = area.width > area.height * 2;

    // Decide layout: wide = side by side, narrow = stacked
    let (tree_area, image_area) = if is_wide {
        // Wide: file tree on left, image on right
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);
        (Some(chunks[0]), chunks[1])
    } else {
        // Narrow: image at bottom (no tree shown in preview mode for simplicity)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(1)])
            .split(area);
        (None, chunks[0])
    };

    // Draw file tree if in wide mode (simplified version)
    if let Some(_tree_rect) = tree_area {
        // In wide mode, we could show the tree, but for simplicity just show image info
    }

    // Draw image preview
    let img = app.image_preview.as_ref().unwrap();
    let title = app.preview_path
        .as_ref()
        .map(|p| {
            let name = p.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            format!(" {} ({}x{}) ", name, img.width, img.height)
        })
        .unwrap_or_else(|| " Image Preview ".to_string());

    // Calculate available space for image (minus borders)
    let img_width = image_area.width.saturating_sub(2) as u32;
    let img_height = (image_area.height.saturating_sub(3) * 2) as u32; // *2 because we use half blocks

    let lines = render_image_to_lines(img, img_width, img_height);

    let preview = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title));

    frame.render_widget(preview, image_area);

    // Status bar at bottom
    let status_area = if is_wide {
        Rect::new(area.x, area.height - 1, area.width, 1)
    } else {
        Rect::new(area.x, area.height - 1, area.width, 1)
    };

    let status = format!(
        " {}x{} | q/Esc:close ",
        img.width, img.height
    );
    let status_widget = Paragraph::new(status)
        .style(Style::default().bg(Color::DarkGray));

    frame.render_widget(status_widget, status_area);

    image_area.height.saturating_sub(2) as usize
}

fn render_image_to_lines(img: &ImagePreview, target_width: u32, target_height: u32) -> Vec<Line<'static>> {
    if target_width == 0 || target_height == 0 || img.width == 0 || img.height == 0 {
        return vec![Line::from("Image too small to display")];
    }

    // Terminal characters are roughly 2:1 (height:width ratio)
    // Each row displays 2 vertical pixels using half blocks
    // So effective pixel aspect is: width=1char, height=2pixels per row
    // To maintain aspect ratio, we need to account for character aspect ratio (~2:1)
    let char_aspect = 2.0; // Terminal chars are about twice as tall as wide

    let img_aspect = img.width as f32 / img.height as f32;
    let term_pixel_width = target_width as f32;
    let term_pixel_height = target_height as f32; // Already doubled for half-blocks

    // Adjust for character aspect ratio
    let adjusted_term_aspect = (term_pixel_width * char_aspect) / term_pixel_height;

    let (display_width, display_height) = if img_aspect > adjusted_term_aspect {
        // Image is wider - fit to width
        let w = target_width;
        let h = ((target_width as f32 / char_aspect) / img_aspect * 2.0) as u32;
        (w, h.max(2))
    } else {
        // Image is taller - fit to height
        let h = target_height;
        let w = (target_height as f32 / 2.0 * img_aspect * char_aspect) as u32;
        (w.max(1), h)
    };

    let term_rows = display_height / 2;
    let mut lines = Vec::new();

    for row in 0..term_rows {
        let mut spans = Vec::new();

        for col in 0..display_width {
            // Map terminal position to source image position
            let src_x = ((col as f32 / display_width as f32) * img.width as f32) as u32;
            let src_y_top = ((row as f32 * 2.0 / display_height as f32) * img.height as f32) as u32;
            let src_y_bottom = (((row as f32 * 2.0 + 1.0) / display_height as f32) * img.height as f32) as u32;

            let src_x = src_x.min(img.width - 1);
            let src_y_top = src_y_top.min(img.height - 1);
            let src_y_bottom = src_y_bottom.min(img.height - 1);

            let idx_top = (src_y_top * img.width + src_x) as usize;
            let idx_bottom = (src_y_bottom * img.width + src_x) as usize;

            let (r1, g1, b1) = img.pixels.get(idx_top).copied().unwrap_or((0, 0, 0));
            let (r2, g2, b2) = img.pixels.get(idx_bottom).copied().unwrap_or((0, 0, 0));

            spans.push(Span::styled(
                "▀",
                Style::default()
                    .fg(Color::Rgb(r1, g1, b1))
                    .bg(Color::Rgb(r2, g2, b2)),
            ));
        }

        lines.push(Line::from(spans));
    }

    lines
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

fn abbreviate_path(path: &std::path::Path, max_width: usize) -> String {
    let full_path = path.display().to_string();

    if full_path.len() <= max_width {
        return full_path;
    }

    let components: Vec<&str> = full_path.split('/').collect();
    if components.is_empty() {
        return full_path;
    }

    // Keep the last component (directory name) intact
    let last = components.last().unwrap_or(&"");

    // Abbreviate all but the last component to first character
    let mut abbreviated: Vec<String> = components[..components.len() - 1]
        .iter()
        .map(|c| {
            if c.is_empty() {
                String::new()
            } else {
                c.chars().next().unwrap_or_default().to_string()
            }
        })
        .collect();
    abbreviated.push(last.to_string());

    let result = abbreviated.join("/");

    // If still too long, just show the last component
    if result.len() > max_width {
        if last.len() > max_width {
            format!("…{}", &last[last.len().saturating_sub(max_width - 1)..])
        } else {
            last.to_string()
        }
    } else {
        result
    }
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
