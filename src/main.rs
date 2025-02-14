use crossterm::terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::{
    fs,
    io::{self, stdin, stdout, Write},
    path::Path,
};

struct EditorState {
    mode: Mode,
    cursor: (usize, usize),
    content: Vec<String>,
    file_path: String,
    status_message: Option<String>,
    screen_size: (usize, usize),
    should_exit: bool,
    command_buffer: String,
}

#[derive(PartialEq)]
enum Mode {
    Normal,
    Insert,
    Command,
}

impl EditorState {
    fn new(file_path: String) -> Self {
        let mut content = Vec::new();
        if Path::new(&file_path).exists() {
            content = fs::read_to_string(&file_path)
                .unwrap_or_default()
                .lines()
                .map(|line| line.to_string())
                .collect();
        }
        if content.is_empty() {
            content.push(String::new());
        }
        let (rows, cols) = crossterm::terminal::size().unwrap_or((24, 80));
        EditorState {
            mode: Mode::Normal,
            cursor: (0, 0),
            content,
            file_path,
            status_message: None,
            screen_size: (rows as usize, cols as usize),
            should_exit: false,
            command_buffer: String::new(),
        }
    }

    fn adjust_column(&mut self) {
        if self.cursor.0 >= self.content.len() {
            self.cursor.0 = self.content.len().saturating_sub(1);
        }
        let line_len = self.content[self.cursor.0].chars().count();
        if self.cursor.1 > line_len {
            self.cursor.1 = line_len;
        }
    }

    fn move_to_line_start(&mut self) {
        self.cursor.1 = 0;
    }

    fn move_to_line_end(&mut self) {
        self.cursor.1 = self.content[self.cursor.0].chars().count();
    }

    fn save_file(&mut self) {
        match fs::write(&self.file_path, self.content.join("\n")) {
            Ok(_) => self.status_message = Some("File saved".to_string()),
            Err(e) => self.status_message = Some(format!("Save error: {}", e)),
        }
    }
}

fn draw_content(state: &EditorState, frame: &mut String) -> io::Result<()> {
    let (_, cols) = crossterm::terminal::size()?;
    let visible_lines = state.screen_size.0 - 1;

    for (row, line) in state.content.iter().enumerate().take(visible_lines) {
        frame.push_str(&format!("\x1b[{};1H\x1b[34m{:4} \x1b[0m", row + 1, row + 1));
        
        let line = line.chars().take(cols as usize - 5).collect::<String>();
        frame.push_str(&format!("\x1b[{};6H{}", row + 1, line));
    }
    Ok(())
}

fn handle_normal_mode(event: &KeyEvent, state: &mut EditorState) {
    match event.code {
        KeyCode::Char('h') | KeyCode::Left => state.cursor.1 = state.cursor.1.saturating_sub(1),
        KeyCode::Char('j') | KeyCode::Down => {
            if state.cursor.0 < state.content.len().saturating_sub(1) {
                state.cursor.0 += 1;
                state.adjust_column();
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.cursor.0 = state.cursor.0.saturating_sub(1);
            state.adjust_column();
        }
        KeyCode::Char('l') | KeyCode::Right => {
            let line_len = state.content[state.cursor.0].chars().count();
            if state.cursor.1 < line_len {
                state.cursor.1 += 1;
            }
        }
        KeyCode::Char('i') => state.mode = Mode::Insert,
        KeyCode::Char(':') => state.mode = Mode::Command,
        KeyCode::Char('0') => state.move_to_line_start(),
        KeyCode::Char('$') => state.move_to_line_end(),
        KeyCode::Char('w') if event.modifiers.contains(KeyModifiers::CONTROL) => state.save_file(),
        KeyCode::Char('q') if event.modifiers.contains(KeyModifiers::CONTROL) => {
            state.should_exit = true
        }
        KeyCode::Char('o') => {
            state.content.insert(state.cursor.0 + 1, String::new());
            state.cursor.0 += 1;
            state.cursor.1 = 0;
            state.mode = Mode::Insert;
        }
        KeyCode::Char('d') if event.modifiers.contains(KeyModifiers::CONTROL) => {
            if !state.content.is_empty() {
                state.content.remove(state.cursor.0);
                if state.cursor.0 >= state.content.len() && !state.content.is_empty() {
                    state.cursor.0 = state.content.len() - 1;
                }
            }
        }
        _ => {}
    }
}

fn handle_insert_mode(event: &KeyEvent, state: &mut EditorState) {
    match event.code {
        KeyCode::Esc => state.mode = Mode::Normal,
        KeyCode::Backspace => {
            if state.cursor.1 > 0 {
                let line = &mut state.content[state.cursor.0];
                let mut chars: Vec<char> = line.chars().collect();
                chars.remove(state.cursor.1 - 1);
                *line = chars.into_iter().collect();
                state.cursor.1 -= 1;
            } else if state.cursor.0 > 0 {
                let current_line = state.content.remove(state.cursor.0);
                state.cursor.0 -= 1;
                let prev_line = &mut state.content[state.cursor.0];
                state.cursor.1 = prev_line.chars().count();
                prev_line.push_str(&current_line);
            }
        }
        KeyCode::Delete => {
            let line = &mut state.content[state.cursor.0];
            let chars_len = line.chars().count();
            if state.cursor.1 < chars_len {
                let mut chars: Vec<char> = line.chars().collect();
                chars.remove(state.cursor.1);
                *line = chars.into_iter().collect();
            }
        }
        KeyCode::Enter => {
            let current_line = state.content[state.cursor.0].clone();
            let (left, right) = current_line.split_at(state.cursor.1);
            state.content[state.cursor.0] = left.to_string();
            state.content.insert(state.cursor.0 + 1, right.to_string());
            state.cursor.0 += 1;
            state.cursor.1 = 0;
        }
        KeyCode::Char(c) => {
            if c.is_control() || event.modifiers != KeyModifiers::NONE {
                return;
            }
            let line = &mut state.content[state.cursor.0];
            let mut chars: Vec<char> = line.chars().collect();
            chars.insert(state.cursor.1, c);
            *line = chars.into_iter().collect();
            state.cursor.1 += 1;
        }
        _ => {}
    }
}

fn handle_command_mode(state: &mut EditorState) {
    match state.command_buffer.as_str() {
        "w" => state.save_file(),
        "q" => state.should_exit = true,
        "wq" => {
            state.save_file();
            state.should_exit = true;
        }
        _ => state.status_message = Some(format!("Unknown command: {}", state.command_buffer)),
    }
    state.command_buffer.clear();
}

fn main() -> io::Result<()> {
    let mut file_path = String::new();
    print!("Enter file path: ");
    stdout().flush()?;
    stdin().read_line(&mut file_path)?;
    let file_path = file_path.trim().to_string();

    enable_raw_mode()?;
    let mut stdout = stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(Hide)?;

    let mut state = EditorState::new(file_path);

    while !state.should_exit {
        let (rows, cols) = crossterm::terminal::size()?;
        state.screen_size = (rows as usize, cols as usize);

        let mut frame = String::new();
        
        frame.push_str("\x1b[2J");
        frame.push_str("\x1b[1;1H");
        
        draw_content(&state, &mut frame)?;
        
        frame.push_str(&format!(
            "\x1b[{};1H\x1b[44m\x1b[37m{:<width$}\x1b[0m",
            rows,
            format!(" {} | {} | {}:{} ", 
                match state.mode {
                    Mode::Normal => "NORMAL",
                    Mode::Insert => "INSERT",
                    Mode::Command => "COMMAND",
                },
                state.file_path, 
                state.cursor.0 + 1, 
                state.cursor.1 + 1),
            width = cols as usize - 1
        ));

        frame.push_str(&format!(
            "\x1b[{};{}H",
            (state.cursor.0 + 1).min(rows as usize),
            (state.cursor.1 + 6).min(cols as usize)
        ));

        print!("{}", frame);
        stdout.flush()?;

        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(KeyEvent { code, modifiers, kind, .. }) 
                    if kind == event::KeyEventKind::Press => 
                {
                    let key_event = KeyEvent::new(code, modifiers);
                    match state.mode {
                        Mode::Normal => handle_normal_mode(&key_event, &mut state),
                        Mode::Insert => handle_insert_mode(&key_event, &mut state),
                        Mode::Command => match key_event.code {
                            KeyCode::Enter => handle_command_mode(&mut state),
                            KeyCode::Char(c) => state.command_buffer.push(c),
                            KeyCode::Backspace => {
                                state.command_buffer.pop();
                            }
                            KeyCode::Esc => {
                                state.mode = Mode::Normal;
                                state.command_buffer.clear();
                            }
                            _ => {}
                        },
                    }
                }
                _ => {}
            }
        }
    }

    stdout.execute(Show)?;
    stdout.execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}