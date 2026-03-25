use std::sync::Mutex;
use tauri::{Emitter, State};

use crate::pty::session::PtySession;
use crate::pty::state_machine::{Emission, PtyStateMachine};

pub struct AppState {
    pub pty_session: Mutex<PtySession>,
    pub state_machine: Mutex<PtyStateMachine>,
}

#[tauri::command]
pub fn send_command(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    command: String,
) -> Result<(), String> {
    let emissions = {
        let mut sm = state.state_machine.lock().map_err(|e| e.to_string())?;
        sm.on_command_sent(&command)
    };

    for emission in &emissions {
        if let Emission::StateChanged(s) = emission {
            let _ = app.emit("pty:state_changed", s);
        }
    }

    let session = state.pty_session.lock().map_err(|e| e.to_string())?;
    session
        .send_command(&command)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn send_input(state: State<'_, AppState>, input: String) -> Result<(), String> {
    let session = state.pty_session.lock().map_err(|e| e.to_string())?;
    session
        .write_all(input.as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn resize_pty(state: State<'_, AppState>, cols: u16, rows: u16) -> Result<(), String> {
    let session = state.pty_session.lock().map_err(|e| e.to_string())?;
    session.resize(cols, rows).map_err(|e| e.to_string())?;
    Ok(())
}
