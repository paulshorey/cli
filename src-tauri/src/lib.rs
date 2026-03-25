mod commands;
mod pty;
mod shell;

use commands::AppState;
use pty::output_pipeline::{OutputPipeline, PipelineItem};
use pty::session::PtySession;
use pty::state_machine::{Emission, PtyState, PtyStateMachine};
use std::io::Read;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let (session, reader) = PtySession::spawn()
                .map_err(|e| format!("Failed to spawn PTY: {}", e))?;

            let raw_fd = session.raw_fd();
            let shell_pid = session.shell_pid();

            let state_machine = Arc::new(Mutex::new(PtyStateMachine::new(shell_pid)));

            app.manage(AppState {
                pty_session: Mutex::new(session),
                state_machine: Arc::clone(&state_machine),
            });

            let handle = app.handle().clone();
            start_output_thread(handle.clone(), reader, Arc::clone(&state_machine));

            if let Some(fd) = raw_fd {
                tauri::async_runtime::spawn(pty::termios_monitor::run_termios_monitor(
                    fd,
                    Arc::clone(&state_machine),
                    handle.clone(),
                ));
                tauri::async_runtime::spawn(pty::process_monitor::run_process_monitor(
                    fd,
                    shell_pid,
                    Arc::clone(&state_machine),
                    handle,
                ));
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::send_command,
            commands::send_input,
            commands::resize_pty,
            commands::signal_foreground,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Background thread: reads raw PTY output, passes through the OutputPipeline
/// to extract OSC markers, feeds the state machine, and emits typed events.
fn start_output_thread(
    handle: tauri::AppHandle,
    mut reader: pty::session::PtyReader,
    state_machine: Arc<Mutex<PtyStateMachine>>,
) {
    std::thread::spawn(move || {
        let mut pipeline = OutputPipeline::new();
        let mut buf = [0u8; 4096];

        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    let emissions = {
                        let mut sm = state_machine.lock().unwrap();
                        sm.on_exit(0)
                    };
                    emit_all(&handle, &emissions);
                    break;
                }
                Ok(n) => {
                    let items = pipeline.process(&buf[..n]);

                    {
                        let last_line = pipeline.last_line();
                        let mut sm = state_machine.lock().unwrap();
                        let timing_emissions = sm.on_output_activity(&last_line);
                        emit_all(&handle, &timing_emissions);
                    }

                    for item in items {
                        match item {
                            PipelineItem::Output(bytes) => {
                                let text = String::from_utf8_lossy(&bytes).to_string();
                                let _ = handle.emit("pty:output", &text);
                            }
                            PipelineItem::Event(osc_event) => {
                                let emissions = {
                                    let mut sm = state_machine.lock().unwrap();
                                    sm.on_osc_event(osc_event)
                                };
                                emit_all(&handle, &emissions);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("PTY read error: {}", e);
                    let emissions = {
                        let mut sm = state_machine.lock().unwrap();
                        sm.on_exit(1)
                    };
                    emit_all(&handle, &emissions);
                    break;
                }
            }
        }
    });
}

pub(crate) fn emit_all(handle: &tauri::AppHandle, emissions: &[Emission]) {
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
