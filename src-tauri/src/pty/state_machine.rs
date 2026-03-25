use serde::Serialize;

use super::output_pipeline::OscEvent;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum PtyState {
    ShellReady,
    CommandSent { command: String },
    CommandRunning { command: String },
    Exited { exit_code: i32 },
}

impl Default for PtyState {
    fn default() -> Self {
        PtyState::ShellReady
    }
}

/// Payload emitted to the frontend when a command completes.
#[derive(Clone, Debug, Serialize)]
pub struct CommandDonePayload {
    pub command: String,
    pub exit_code: i32,
}

/// Payload emitted when the working directory changes.
#[derive(Clone, Debug, Serialize)]
pub struct CwdPayload {
    pub cwd: String,
}

/// Actions the state machine asks the caller to perform (emit events, etc.)
pub enum Emission {
    StateChanged(PtyState),
    CommandDone(CommandDonePayload),
    CwdChanged(CwdPayload),
}

pub struct PtyStateMachine {
    pub state: PtyState,
    pub cwd: String,
    pending_command: String,
}

impl PtyStateMachine {
    pub fn new() -> Self {
        Self {
            state: PtyState::default(),
            cwd: String::new(),
            pending_command: String::new(),
        }
    }

    /// Called by the send_command handler when the user submits a command.
    pub fn on_command_sent(&mut self, command: &str) -> Vec<Emission> {
        self.pending_command = command.to_string();
        self.state = PtyState::CommandSent {
            command: command.to_string(),
        };
        vec![Emission::StateChanged(self.state.clone())]
    }

    /// Called by the reader thread when the PTY reaches EOF or a read error occurs.
    pub fn on_exit(&mut self, exit_code: i32) -> Vec<Emission> {
        self.state = PtyState::Exited { exit_code };
        vec![Emission::StateChanged(self.state.clone())]
    }

    /// Called by the reader thread when the output pipeline detects an OSC event.
    pub fn on_osc_event(&mut self, event: OscEvent) -> Vec<Emission> {
        let mut emissions = Vec::new();

        match event {
            OscEvent::PromptStart => {
                if !matches!(self.state, PtyState::ShellReady) {
                    self.state = PtyState::ShellReady;
                    emissions.push(Emission::StateChanged(self.state.clone()));
                }
            }
            OscEvent::PromptEnd => {
                // B marker -- no state transition needed, we skip B in our integration
            }
            OscEvent::CommandStart => {
                let command = self.pending_command.clone();
                self.state = PtyState::CommandRunning {
                    command: command.clone(),
                };
                emissions.push(Emission::StateChanged(self.state.clone()));
            }
            OscEvent::CommandDone { exit_code } => {
                let command = self.pending_command.clone();
                emissions.push(Emission::CommandDone(CommandDonePayload {
                    command,
                    exit_code,
                }));
                self.state = PtyState::ShellReady;
                emissions.push(Emission::StateChanged(self.state.clone()));
            }
            OscEvent::CwdChanged { cwd } => {
                if cwd != self.cwd {
                    self.cwd = cwd.clone();
                    emissions.push(Emission::CwdChanged(CwdPayload { cwd }));
                }
            }
        }

        emissions
    }
}
