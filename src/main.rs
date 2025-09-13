use std::{path::PathBuf, process::Command};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style, Stylize},
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget, Wrap},
};

fn main() -> std::io::Result<()> {
    let mut terminal = ratatui::init();
    let app_result = App::new()?.run(&mut terminal);
    ratatui::restore();
    app_result
}

#[derive(Debug)]
pub struct App {
    current_dir: PathBuf,
    files: Vec<String>,
    selected: usize,
    scroll: usize,
    preview_content: Option<String>,
    exit: bool,
}

impl App {
    pub fn new() -> std::io::Result<Self> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let files = Self::read_dir(&home)?;
        Ok(Self {
            current_dir: home,
            files,
            selected: 0,
            scroll: 0,
            preview_content: None,
            exit: false,
        })
    }

    pub fn read_dir(path: &PathBuf) -> std::io::Result<Vec<String>> {
        let mut entries = vec![];
        entries.push("..".into());
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !file_name.starts_with('.') {
                if entry.file_type()?.is_dir() {
                    entries.push(format!("{}/", file_name));
                } else {
                    entries.push(file_name);
                }
            }
        }
        entries.sort();
        Ok(entries)
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_event()?;
        }
        Ok(())
    }

    pub fn handle_event(&mut self) -> std::io::Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),

            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                    self.update_scroll();
                    self.update_preview();
                }
            }

            KeyCode::Down => {
                if self.selected + 1 < self.files.len() {
                    self.selected += 1;
                    self.update_scroll();
                    self.update_preview();
                }
            }

            KeyCode::Enter => {
                if let Some(name) = self.files.get(self.selected).cloned() {
                    if name == ".." {
                        if let Some(parent) = self.current_dir.parent() {
                            self.current_dir = parent.to_path_buf();
                        }
                    } else {
                        let candidate = self.current_dir.join(&name.trim_end_matches('/'));
                        if candidate.is_dir() {
                            self.current_dir = candidate;
                        } else {
                            self.open_file_in_vim(&candidate);
                        }
                    }
                    if let Ok(new_files) = Self::read_dir(&self.current_dir) {
                        self.files = new_files;
                        self.selected = 0;
                        self.scroll = 0;
                    }
                }
            }

            _ => {}
        }
    }

    fn open_file_in_vim(&self, file_path: &PathBuf) -> std::io::Result<()> {
        ratatui::restore();

        let status = Command::new("vim").arg(file_path).status()?;

        let mut terminal = ratatui::init();
        if !status.success() {
            eprintln!("Vim exited with status: {}", status);
        }

        Ok(())
    }

    fn update_preview(&mut self) {
        if let Some(selected_name) = self.files.get(self.selected) {
            if selected_name == ".." {
                self.preview_content = Some("← Parent Directory".to_string());
                return;
            }

            let selected_path = self.current_dir.join(selected_name.trim_end_matches('/'));

            if selected_path.is_dir() {
                self.preview_content = self.read_dir_preview(&selected_path);
            } else if selected_path.is_file() {
                if let Some(ext) = selected_path.extension().and_then(|e| e.to_str()) {
                    let ext = ext.to_lowercase();
                    if ["png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico"]
                        .contains(&ext.as_str())
                    {
                        self.preview_content = Some(format!(
                            "📷 Image file: {}\n\nDimensions: [Image preview not available in terminal]\nType: {}",
                            selected_name,
                            ext.to_uppercase()
                        ));
                        return;
                    }
                }
                // For text files and files without extension
                self.preview_content = self.read_file_preview(&selected_path);
            } else {
                self.preview_content = Some("Unable to access file".to_string());
            }
        } else {
            self.preview_content = None;
        }
    }

    fn read_file_preview(&self, file_path: &PathBuf) -> Option<String> {
        if let Ok(metadata) = std::fs::metadata(file_path) {
            if metadata.len() > 1_048_576 {
                // 1MB
                return Some(format!(
                    "📄 File too large for preview\nSize: {} bytes\nUse Enter to open in vim",
                    metadata.len()
                ));
            }
        }

        match std::fs::read(file_path) {
            Ok(bytes) => {
                // Check if file appears to be binary
                if bytes
                    .iter()
                    .take(1024)
                    .any(|&b| b == 0 || (b < 32 && b != 9 && b != 10 && b != 13))
                {
                    return Some(format!(
                        "📄 Binary file\nSize: {} bytes\nUse Enter to open in vim",
                        bytes.len()
                    ));
                }

                let byteslen = bytes.len();
                // Convert to string and limit lines for preview
                match String::from_utf8(bytes) {
                    Ok(content) => {
                        let lines: Vec<&str> = content.lines().take(50).collect();
                        let preview = lines.join("\n");

                        let file_info = if let Ok(metadata) = std::fs::metadata(file_path) {
                            format!(
                                "📄 {} | {} bytes | {} lines\n{}\n",
                                file_path.file_name().unwrap_or_default().to_string_lossy(),
                                metadata.len(),
                                content.lines().count(),
                                "─".repeat(40)
                            )
                        } else {
                            format!(
                                "📄 {}\n{}\n",
                                file_path.file_name().unwrap_or_default().to_string_lossy(),
                                "─".repeat(40)
                            )
                        };

                        let mut result = file_info + &preview;

                        if content.lines().count() > 50 {
                            result.push_str(&format!(
                                "\n{}\n... ({} more lines)\nPress Enter to open full file in vim",
                                "─".repeat(40),
                                content.lines().count() - 50
                            ));
                        }

                        Some(result)
                    }
                    Err(_) => Some(format!(
                        "📄 File contains invalid UTF-8\nSize: {} bytes\nUse Enter to open in vim",
                        byteslen
                    )),
                }
            }
            Err(e) => Some(format!("❌ Error reading file: {}", e)),
        }
    }

    fn read_dir_preview(&self, file_path: &PathBuf) -> Option<String> {
        match std::fs::read_dir(file_path) {
            Ok(entries) => {
                let mut dirs = Vec::new();
                let mut files = Vec::new();
                let mut total_size = 0u64;

                for entry in entries {
                    if let Ok(entry) = entry {
                        let name = entry.file_name().to_string_lossy().to_string();

                        // Skip hidden files for preview
                        if name.starts_with('.') {
                            continue;
                        }

                        if let Ok(file_type) = entry.file_type() {
                            if file_type.is_dir() {
                                dirs.push(format!("📁 {}/", name));
                            } else {
                                let size_info = if let Ok(metadata) = entry.metadata() {
                                    total_size += metadata.len();
                                    if metadata.len() > 1024 {
                                        format!(" ({:.1} KB)", metadata.len() as f64 / 1024.0)
                                    } else {
                                        format!(" ({} B)", metadata.len())
                                    }
                                } else {
                                    String::new()
                                };
                                files.push(format!("📄 {}{}", name, size_info));
                            }
                        }
                    }
                }

                // Sort and combine
                dirs.sort();
                files.sort();

                let mut result = format!(
                    "📂 Directory: {}\n",
                    file_path.file_name().unwrap_or_default().to_string_lossy()
                );
                result.push_str(&format!(
                    "📊 {} directories, {} files",
                    dirs.len(),
                    files.len()
                ));

                if total_size > 0 {
                    if total_size > 1024 * 1024 {
                        result.push_str(&format!(
                            " (Total: {:.1} MB)",
                            total_size as f64 / (1024.0 * 1024.0)
                        ));
                    } else if total_size > 1024 {
                        result.push_str(&format!(" (Total: {:.1} KB)", total_size as f64 / 1024.0));
                    } else {
                        result.push_str(&format!(" (Total: {} B)", total_size));
                    }
                }

                result.push_str(&format!("\n{}\n", "─".repeat(40)));

                // Add items (limit to prevent overwhelming)
                let mut items = dirs;
                items.extend(files);

                for (i, item) in items.iter().take(30).enumerate() {
                    result.push_str(&format!("{}\n", item));
                }

                if items.len() > 30 {
                    result.push_str(&format!("... and {} more items\n", items.len() - 30));
                }

                result.push_str("\nPress Enter to navigate into directory");

                Some(result)
            }
            Err(e) => Some(format!("❌ Error reading directory: {}", e)),
        }
    }

    fn update_scroll(&mut self) {
        // what to do here?
    }

    fn update_scroll_with_height(&mut self, max_visible: usize) {
        if max_visible == 0 {
            return;
        }

        let scroll_threshold = 3.min(max_visible);

        let visible_pos = self.selected.saturating_sub(self.scroll);

        if visible_pos >= max_visible.saturating_sub(scroll_threshold) {
            let max_scroll = self.files.len().saturating_sub(max_visible);
            if self.scroll < max_scroll {
                self.scroll = (self.selected + scroll_threshold).saturating_sub(max_visible - 1);
                self.scroll = self.scroll.min(max_scroll);
            }
        } else if visible_pos < scroll_threshold {
            if self.selected >= scroll_threshold {
                self.scroll = self.selected.saturating_sub(scroll_threshold);
            } else {
                self.scroll = 0;
            }
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    pub fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from("< Browrs >".green().bold());
        let instructions = Line::from(vec![
            " Up/Down ".into(),
            "<↑/↓>".blue().bold(),
            " Enter ".into(),
            "<↵>".blue().bold(),
            " Quit ".into(),
            "<Q>".red().bold(),
        ]);

        let outer = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_set(border::EMPTY);

        let inner = outer.inner(area);
        outer.render(area, buf);

        let chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Percentage(50),
                ratatui::layout::Constraint::Percentage(50),
            ])
            .split(inner);

        let list_rect = chunks[0];
        let preview_rect = chunks[1];

        let max_visible = list_rect.height.saturating_sub(2) as usize;

        let mut app_copy = App {
            current_dir: self.current_dir.clone(),
            files: self.files.clone(),
            selected: self.selected,
            scroll: self.scroll,
            preview_content: self.preview_content.clone(),
            exit: self.exit,
        };
        app_copy.update_scroll_with_height(max_visible);
        let scroll = app_copy.scroll;

        let total = self.files.len();
        let start = scroll;
        let end = (start + max_visible).min(total);

        let file_lines: Vec<Line> = self.files[start..end]
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let absolute_index = start + i;
                if absolute_index == self.selected {
                    Line::from(name.clone()).style(
                        Style::default()
                            .bg(ratatui::style::Color::Blue)
                            .fg(ratatui::style::Color::White)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Line::from(name.clone())
                }
            })
            .collect();

        let file_paragraph = Paragraph::new(Text::from(file_lines)).block(
            Block::bordered()
                .title(format!(" Directory: {}", self.current_dir.display()).blue())
                .border_set(border::PLAIN),
        );
        file_paragraph.render(list_rect, buf);

        let preview_block = Block::bordered()
            .title(" Preview ".blue().bold().into_right_aligned_line())
            .border_set(border::PLAIN);

        if let Some(content) = &self.preview_content {
            let preview_paragraph = Paragraph::new(content.clone())
                .block(preview_block)
                .wrap(Wrap { trim: true });
            preview_paragraph.render(preview_rect, buf);
        } else {
            preview_block.render(preview_rect, buf);
        }
    }
}
