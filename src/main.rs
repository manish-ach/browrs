use std::path::PathBuf;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style, Stylize},
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
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
                }
            }

            KeyCode::Down => {
                if self.selected + 1 < self.files.len() {
                    self.selected += 1;
                    self.update_scroll();
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

    fn update_scroll(&mut self) {}

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
        let title = Line::from("< Tmages - image converter TUI >".green().bold());
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

        let selected_path = self
            .current_dir
            .join(self.files[self.selected].trim_end_matches('/'));
        if selected_path.is_file() {
            if let Some(ext) = selected_path.extension().and_then(|e| e.to_str()) {
                let ext = ext.to_lowercase();
                if ["png", "jpg", "jpeg", "gif", "bmp", "webp"].contains(&ext.as_str()) {}
            }
        }

        Block::bordered()
            .title(" Preview ".blue().bold().into_right_aligned_line())
            .border_set(border::PLAIN)
            .render(preview_rect, buf);

        let max_visible = list_rect.height.saturating_sub(2) as usize;

        let mut app_copy = App {
            current_dir: self.current_dir.clone(),
            files: self.files.clone(),
            selected: self.selected,
            scroll: self.scroll,
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

        Block::bordered()
            .title(" Preview ".blue().bold().into_right_aligned_line())
            .border_set(border::PLAIN)
            .render(preview_rect, buf);
    }
}
