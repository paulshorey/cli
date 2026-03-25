use anyhow::Result;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::Mutex;

use crate::shell::integration::ShellIntegration;

pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Mutex<Box<dyn Write + Send>>,
    _integration: Option<ShellIntegration>,
    shell_pid: u32,
    raw_fd: Option<i32>,
}

pub type PtyReader = Box<dyn Read + Send>;

impl PtySession {
    pub fn spawn() -> Result<(Self, PtyReader)> {
        let pty_system = native_pty_system();

        let pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        let is_zsh = shell.ends_with("/zsh") || shell.ends_with("/zsh5");

        let integration = if is_zsh {
            match ShellIntegration::setup_zsh() {
                Ok(si) => Some(si),
                Err(e) => {
                    eprintln!("Warning: failed to set up shell integration: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let mut cmd = CommandBuilder::new(&shell);
        cmd.arg("-l");

        if let Ok(cwd) = std::env::current_dir() {
            cmd.cwd(cwd);
        }

        cmd.env("TERM_PROGRAM", "cli-app");
        cmd.env("TERM", "xterm-256color");

        if let Some(ref si) = integration {
            cmd.env("ZDOTDIR", si.zdotdir().to_string_lossy().as_ref());
        }

        let child = pair.slave.spawn_command(cmd)?;
        let shell_pid = child.process_id().unwrap_or(0);
        drop(child);

        let raw_fd = pair.master.as_raw_fd();
        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        Ok((
            PtySession {
                master: pair.master,
                writer: Mutex::new(writer),
                _integration: integration,
                shell_pid,
                raw_fd,
            },
            reader,
        ))
    }

    pub fn shell_pid(&self) -> u32 {
        self.shell_pid
    }

    pub fn raw_fd(&self) -> Option<i32> {
        self.raw_fd
    }

    pub fn write_all(&self, data: &[u8]) -> Result<()> {
        let mut writer = self.writer.lock().expect("writer lock poisoned");
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }

    pub fn send_command(&self, command: &str) -> Result<()> {
        self.write_all(format!("{}\n", command).as_bytes())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_termios(&self) -> Option<nix::sys::termios::Termios> {
        if let Some(fd) = self.raw_fd {
            use std::os::fd::BorrowedFd;
            let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };
            nix::sys::termios::tcgetattr(borrowed).ok()
        } else {
            None
        }
    }

    pub fn foreground_pid(&self) -> Option<u32> {
        self.master.process_group_leader().map(|p| p as u32)
    }

    pub fn signal_foreground(&self, signal: nix::sys::signal::Signal) -> Result<()> {
        if let Some(pid) = self.foreground_pid() {
            nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(pid as i32),
                signal,
            )?;
        }
        Ok(())
    }
}
