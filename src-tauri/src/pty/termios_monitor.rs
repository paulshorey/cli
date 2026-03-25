use std::os::fd::BorrowedFd;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::pty::state_machine::PtyStateMachine;

/// Polls terminal attributes (ICANON/ECHO) every 100ms on the PTY master fd.
/// Feeds changes into the state machine, which may trigger transitions to
/// InputExpected (canonical + stalled) or RawMode (!ICANON).
pub async fn run_termios_monitor(
    raw_fd: i32,
    state_machine: Arc<Mutex<PtyStateMachine>>,
    app_handle: tauri::AppHandle,
) {
    loop {
        let result = {
            let borrowed = unsafe { BorrowedFd::borrow_raw(raw_fd) };
            nix::sys::termios::tcgetattr(borrowed)
        };

        match result {
            Ok(termios) => {
                let lflag = termios.local_flags;
                let canonical = lflag.contains(nix::sys::termios::LocalFlags::ICANON);
                let echo = lflag.contains(nix::sys::termios::LocalFlags::ECHO);

                let emissions = {
                    let mut sm = state_machine.lock().unwrap();
                    sm.on_termios_check(canonical, echo)
                };
                crate::emit_all(&app_handle, &emissions);
            }
            Err(_) => {
                break;
            }
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
