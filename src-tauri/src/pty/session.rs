use anyhow::Result;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::Mutex;

pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Mutex<Box<dyn Write + Send>>,
    #[allow(dead_code)]
    shell_pid: u32,
}

/// The reader half, separated from PtySession so it can be moved to a background thread.
pub type PtyReader = Box<dyn Read + Send>;

impl PtySession {
    /// Spawns a new shell in a PTY. Returns the session and a reader for the output stream.
    /// The reader must be consumed on a separate thread (blocking reads).
    pub fn spawn() -> Result<(Self, PtyReader)> {
        let pty_system = native_pty_system();

        let pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        let mut cmd = CommandBuilder::new(&shell);
        cmd.arg("-l"); // login shell to load user profile

        if let Ok(cwd) = std::env::current_dir() {
            cmd.cwd(cwd);
        }

        // Mark that we're running inside our app (shell scripts can detect this)
        cmd.env("TERM_PROGRAM", "cli-app");
        cmd.env("TERM", "xterm-256color");

        let child = pair.slave.spawn_command(cmd)?;
        let shell_pid = child.process_id().unwrap_or(0);
        drop(child); // We don't track the child handle in Phase 1

        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        Ok((
            PtySession {
                master: pair.master,
                writer: Mutex::new(writer),
                shell_pid,
            },
            reader,
        ))
    }

    /// Writes raw bytes to the PTY stdin.
    pub fn write_all(&self, data: &[u8]) -> Result<()> {
        let mut writer = self.writer.lock().expect("writer lock poisoned");
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }

    /// Sends a command string followed by a newline.
    pub fn send_command(&self, command: &str) -> Result<()> {
        self.write_all(format!("{}\n", command).as_bytes())
    }

    /// Resizes the PTY (triggers SIGWINCH in the child process).
    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    /// Returns the raw file descriptor of the PTY master (for tcgetattr/tcgetpgrp in later phases).
    #[allow(dead_code)]
    pub fn master_fd(&self) -> Option<i32> {
        self.master.as_raw_fd().map(|fd| fd as i32)
    }

    /// Queries terminal attributes (ICANON, ECHO, etc.) -- used in Phase 4.
    #[allow(dead_code)]
    pub fn get_termios(&self) -> Option<nix::sys::termios::Termios> {
        if let Some(fd) = self.master.as_raw_fd() {
            use std::os::fd::BorrowedFd;
            // Safety: the fd is valid as long as the master is alive, and we hold &self.
            let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };
            nix::sys::termios::tcgetattr(borrowed).ok()
        } else {
            None
        }
    }

    /// Returns PID of the current foreground process group leader -- used in Phase 4.
    #[allow(dead_code)]
    pub fn foreground_pid(&self) -> Option<u32> {
        self.master.process_group_leader().map(|p| p as u32)
    }
}
