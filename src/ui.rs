use std::io::{self, Write};
use std::sync::{mpsc, Arc, Mutex};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal,
};

pub struct ChatUi {
    partial: Arc<Mutex<String>>,
    out: Arc<Mutex<io::Stdout>>,
}

impl ChatUi {
    pub fn new() -> Result<Self> {
        terminal::enable_raw_mode()?;
        let out = Arc::new(Mutex::new(io::stdout()));
        {
            let mut stdout = out.lock().unwrap();
            write!(stdout, "\x1b[1m[you]>\x1b[0m ")?;
            stdout.flush()?;
        }
        Ok(Self {
            partial: Arc::new(Mutex::new(String::new())),
            out,
        })
    }

    /// Print message lines above the prompt, then repaint the prompt on a new line.
    /// If the user had partial input, it is preserved in the reprinted prompt.
    pub fn print_message(&self, lines: &[String]) {
        let partial = self.partial.lock().unwrap().clone();
        let mut stdout = self.out.lock().unwrap();

        // Clear the current prompt line, print each message, then repaint prompt.
        write!(stdout, "\r\x1b[2K").ok();
        for line in lines {
            write!(stdout, "{}\r\n", line).ok();
        }
        write!(stdout, "\x1b[1m[you]>\x1b[0m {}", partial).ok();

        stdout.flush().ok();
    }

    /// Spawn the raw-mode input thread. Returns a receiver that yields
    /// `Some(line)` per submitted message and `None` on quit.
    pub fn spawn_input_thread(&self) -> mpsc::Receiver<Option<String>> {
        let (tx, rx) = mpsc::channel();
        let partial = self.partial.clone();
        let out = self.out.clone();

        std::thread::spawn(move || {
            loop {
                let Ok(Event::Key(key)) = event::read() else {
                    continue;
                };

                match key.code {
                    KeyCode::Enter => {
                        // Snapshot and clear partial before locking stdout
                        let line = {
                            let mut p = partial.lock().unwrap();
                            let s = p.trim().to_string();
                            p.clear();
                            s
                        };
                        {
                            let mut stdout = out.lock().unwrap();
                            write!(stdout, "\r\x1b[2K\x1b[1m[you]>\x1b[0m ").ok();
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
            write!(stdout, "\r\n").ok();
            stdout.flush().ok();
        }
        terminal::disable_raw_mode().ok();
    }
}
