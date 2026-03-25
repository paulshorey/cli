use std::os::fd::BorrowedFd;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::pty::state_machine::PtyStateMachine;

/// Polls the foreground process group on the PTY every 200ms.
/// Resolves PIDs to process names via libproc (macOS).
/// Reports changes to the state machine for RawMode detection.
pub async fn run_process_monitor(
    raw_fd: i32,
    shell_pid: u32,
    state_machine: Arc<Mutex<PtyStateMachine>>,
    app_handle: tauri::AppHandle,
) {
    let mut last_pid: Option<u32> = None;

    loop {
        let pgrp_result = {
            let borrowed = unsafe { BorrowedFd::borrow_raw(raw_fd) };
            nix::unistd::tcgetpgrp(borrowed)
        };

        match pgrp_result {
            Ok(pgrp) => {
                let pid = pgrp.as_raw() as u32;
                if Some(pid) != last_pid {
                    let name = if pid == shell_pid {
                        "shell".to_string()
                    } else {
                        libproc::proc_pid::name(pid as i32)
                            .unwrap_or_else(|_| "unknown".to_string())
                    };

                    let emissions = {
                        let mut sm = state_machine.lock().unwrap();
                        sm.on_foreground_change(pid, name)
                    };
                    crate::emit_all(&app_handle, &emissions);
                    last_pid = Some(pid);
                }
            }
            Err(_) => {
                break;
            }
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
