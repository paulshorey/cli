mod commands;
mod pty;
mod shell;

use commands::AppState;
use pty::output_pipeline::{OutputPipeline, PipelineItem};
use pty::session::PtySession;
use pty::state_machine::{Emission, PtyState, PtyStateMachine};
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

            start_output_thread(app.handle().clone(), reader);

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

/// Background thread: reads raw PTY output, passes through the OutputPipeline
/// to extract OSC markers, feeds the state machine, and emits typed events.
fn start_output_thread(handle: tauri::AppHandle, mut reader: pty::session::PtyReader) {
    std::thread::spawn(move || {
        let mut pipeline = OutputPipeline::new();
        let mut buf = [0u8; 4096];

        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    let app_state: tauri::State<AppState> = handle.state();
                    let emissions = {
                        let mut sm = app_state.state_machine.lock().unwrap();
                        sm.on_exit(0)
                    };
                    emit_all(&handle, &emissions);
                    break;
                }
                Ok(n) => {
                    let items = pipeline.process(&buf[..n]);
                    for item in items {
                        match item {
                            PipelineItem::Output(bytes) => {
                                let text = String::from_utf8_lossy(&bytes).to_string();
                                let _ = handle.emit("pty:output", &text);
                            }
                            PipelineItem::Event(osc_event) => {
                                let app_state: tauri::State<AppState> = handle.state();
                                let emissions = {
                                    let mut sm = app_state.state_machine.lock().unwrap();
                                    sm.on_osc_event(osc_event)
                                };
                                emit_all(&handle, &emissions);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("PTY read error: {}", e);
                    let app_state: tauri::State<AppState> = handle.state();
                    let emissions = {
                        let mut sm = app_state.state_machine.lock().unwrap();
                        sm.on_exit(1)
                    };
                    emit_all(&handle, &emissions);
                    break;
                }
            }
        }
    });
}

fn emit_all(handle: &tauri::AppHandle, emissions: &[Emission]) {
    for emission in emissions {
        match emission {
            Emission::StateChanged(state) => {
                let _ = handle.emit("pty:state_changed", state);
                if let PtyState::Exited { exit_code } = state {
                    let _ = handle.emit("pty:exit", exit_code);
                }
            }
            Emission::CommandDone(payload) => {
                let _ = handle.emit("pty:command_done", payload);
            }
            Emission::CwdChanged(payload) => {
                let _ = handle.emit("pty:cwd_changed", payload);
            }
        }
    }
}
