mod commands;
mod pty;

use commands::AppState;
use pty::session::PtySession;
use pty::state_machine::PtyStateMachine;
use std::io::Read;
use std::sync::Mutex;
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let (session, reader) = PtySession::spawn()
                .map_err(|e| format!("Failed to spawn PTY: {}", e))?;

            app.manage(AppState {
                pty_session: Mutex::new(session),
                state_machine: Mutex::new(PtyStateMachine::new()),
            });

            let handle = app.handle().clone();
            start_pty_reader(handle, reader);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::send_command,
            commands::send_input,
            commands::resize_pty,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Spawns a background thread that reads from the PTY output stream
/// and emits each chunk to the frontend as a "pty:output" event.
fn start_pty_reader(handle: tauri::AppHandle, mut reader: pty::session::PtyReader) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    let _ = handle.emit("pty:exit", 0);
                    break;
                }
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = handle.emit("pty:output", &text);
                }
                Err(e) => {
                    eprintln!("PTY read error: {}", e);
                    let _ = handle.emit("pty:exit", 1);
                    break;
                }
            }
        }
    });
}
