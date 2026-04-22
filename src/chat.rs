use std::io::{self, BufRead, Write};
use std::sync::mpsc;

/// Spawn the input thread. Returns a receiver that yields `Some(line)` for each
/// user message and `None` when the user quits (Ctrl-D or "/quit").
pub fn spawn_input_thread() -> mpsc::Receiver<Option<String>> {
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let stdin = io::stdin();
        let mut lines = stdin.lock().lines();

        loop {
            print!("> ");
            io::stdout().flush().ok();

            match lines.next() {
                Some(Ok(line)) => {
                    let line = line.trim().to_string();
                    if line == "/quit" || line == "quit" {
                        tx.send(None).ok();
                        break;
                    }
                    if !line.is_empty() {
                        if tx.send(Some(line)).is_err() {
                            break;
                        }
                    }
                }
                _ => {
                    // EOF (Ctrl-D) or read error
                    tx.send(None).ok();
                    break;
                }
            }
        }
    });

    rx
}
