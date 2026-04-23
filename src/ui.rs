use std::io::{self, Write};
use std::sync::{mpsc, Arc, Mutex};

use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    queue,
    terminal,
};

pub struct ChatUi {
    rows: u16,
    partial: Arc<Mutex<String>>,
    out: Arc<Mutex<io::Stdout>>,
}

impl ChatUi {
    pub fn new() -> Result<Self> {
        let (_, rows) = terminal::size()?;
        terminal::enable_raw_mode()?;

        let out = Arc::new(Mutex::new(io::stdout()));
        {
            let mut stdout = out.lock().unwrap();
            // Reserve the last row for the prompt by restricting the scroll region
            // ANSI uses 1-indexed rows; crossterm cursor::MoveTo uses 0-indexed.
            write!(stdout, "\x1b[1;{}r", rows - 1)?;
            queue!(stdout, cursor::MoveTo(0, rows - 1))?;
            write!(stdout, "\x1b[2K\x1b[1m[you]>\x1b[0m ")?;
            stdout.flush()?;
        }

        Ok(Self {
            rows,
            partial: Arc::new(Mutex::new(String::new())),
            out,
        })
    }

    /// Print lines into the scroll area and repaint the prompt below them.
    /// Safe to call from any thread.
    pub fn print_message(&self, lines: &[String]) {
        // Snapshot partial input before locking stdout
        let partial = self.partial.lock().unwrap().clone();
        let mut stdout = self.out.lock().unwrap();

        // Position at the bottom row of the scroll region (0-indexed: rows - 2).
        // Writing \r\n there scrolls the region up one row and keeps the cursor at rows - 2.
        queue!(stdout, cursor::MoveTo(0, self.rows - 2)).ok();
        for line in lines {
            write!(stdout, "\r\n{}", line).ok();
        }

        // Repaint prompt on the fixed last row
        queue!(stdout, cursor::MoveTo(0, self.rows - 1)).ok();
        write!(stdout, "\x1b[2K\x1b[1m[you]>\x1b[0m {}", partial).ok();

        // Park cursor after whatever the user had typed
        let col = 7 + partial.len() as u16; // "[you]> " = 7 chars
        queue!(stdout, cursor::MoveTo(col, self.rows - 1)).ok();

        stdout.flush().ok();
    }

    /// Spawn the raw-mode input thread. Returns a receiver yielding `Some(line)` per
    /// submitted message and `None` when the user quits (Ctrl-D / Ctrl-C / /quit).
    pub fn spawn_input_thread(&self) -> mpsc::Receiver<Option<String>> {
        let (tx, rx) = mpsc::channel();
        let partial = self.partial.clone();
        let out = self.out.clone();
        let rows = self.rows;

        std::thread::spawn(move || {
            loop {
                let Ok(Event::Key(key)) = event::read() else {
                    continue;
                };

                match key.code {
                    KeyCode::Enter => {
                        // Take the current input, clear partial — do this before locking stdout
                        let line = {
                            let mut p = partial.lock().unwrap();
                            let s = p.trim().to_string();
                            p.clear();
                            s
                        };
                        {
                            let mut stdout = out.lock().unwrap();
                            queue!(stdout, cursor::MoveTo(0, rows - 1)).ok();
                            write!(stdout, "\x1b[2K\x1b[1m[you]>\x1b[0m ").ok();
                            stdout.flush().ok();
                        }
                        if line == "/quit" || line == "quit" {
                            tx.send(None).ok();
                            break;
                        }
                        if !line.is_empty() && tx.send(Some(line)).is_err() {
                            break;
                        }
                    }
                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        tx.send(None).ok();
                        break;
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        tx.send(None).ok();
                        break;
                    }
                    KeyCode::Char(c) => {
                        // Push to partial before writing to terminal
                        { partial.lock().unwrap().push(c); }
                        let mut stdout = out.lock().unwrap();
                        write!(stdout, "{}", c).ok();
                        stdout.flush().ok();
                    }
                    KeyCode::Backspace => {
                        let popped = { partial.lock().unwrap().pop().is_some() };
                        if popped {
                            let mut stdout = out.lock().unwrap();
                            write!(stdout, "\x08 \x08").ok();
                            stdout.flush().ok();
                        }
                    }
                    _ => {}
                }
            }
        });

        rx
    }
}

impl Drop for ChatUi {
    fn drop(&mut self) {
        if let Ok(mut stdout) = self.out.lock() {
            // Reset scroll region to the full terminal
            write!(stdout, "\x1b[r").ok();
            queue!(stdout, cursor::MoveTo(0, self.rows - 1)).ok();
            write!(stdout, "\r\n").ok();
            stdout.flush().ok();
        }
        terminal::disable_raw_mode().ok();
    }
}
