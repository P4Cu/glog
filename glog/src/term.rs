use std::io::Write;
use std::{env, io};

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::{error, warn};
use tui::{backend::CrosstermBackend, Terminal};

pub struct Term {
    pub terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl Term {
    pub fn new() -> Result<Self, io::Error> {
        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        let mut term = Term { terminal };
        term.enter()?;
        Ok(term)
    }

    pub fn call<F>(&mut self, func: F) -> Result<(), io::Error>
    where
        F: FnOnce(),
    {
        self.restore()?;
        func();
        self.enter()?;
        self.clear();
        Ok(())
    }

    pub fn call_external(&mut self, mut command: std::process::Command) -> Result<(), io::Error> {
        self.call(|| {
            let result = command.status();
            match result {
                Ok(exit_code) => {
                    if !exit_code.success() {
                        warn!(
                            "Command {:#?} finished with exit_code {}",
                            command, exit_code
                        )
                    }
                }
                Err(err) => warn!("Command {:#?} finished with error: {}", command, err),
            };
        })
    }

    pub fn clear(&mut self) {
        if let Err(err) = self.terminal.clear() {
            error!("Error from terminal clear: {}", err);
        }
    }

    fn enter(&mut self) -> Result<(), io::Error> {
        enable_raw_mode()?;
        execute!(
            io::stdout(),
            EnterAlternateScreen,
            // EnableMouseCapture
        )
    }

    fn restore(&mut self) -> Result<(), io::Error> {
        // restore terminal
        disable_raw_mode()?;
        execute!(
            io::stdout(),
            LeaveAlternateScreen,
            // DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn write_main_screen(&mut self, text: &str) -> Result<(), io::Error> {
        self.restore()?;
        println!("{}", text);
        self.enter()
    }

    pub fn text_via_less(&mut self, text: &str) {
        self.call(|| {
            let pager = env::var("PAGER").unwrap_or("less".into());
            let mut command = std::process::Command::new(pager)
                .stdin(std::process::Stdio::piped())
                .spawn()
                .expect("Spawning less failed");

            if let Some(mut stdin) = command.stdin.take() {
                stdin
                    .write_all(text.as_bytes())
                    .expect("Writing to less failed");
            }

            command.wait();
        })
        .expect("Something went wrong");
    }
}

impl Drop for Term {
    fn drop(&mut self) {
        if let Err(err) = self.restore() {
            error!("Error during restoring terminal: {}", err);
        }
    }
}
