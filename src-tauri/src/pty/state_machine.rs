use serde::Serialize;

/// Represents the current state of the PTY / shell process.
/// In Phase 1 this is a skeleton. Phases 3-4 will add full transition logic,
/// OSC 133 parsing, termios monitoring, and process tracking.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum PtyState {
    /// Shell is displaying its prompt, waiting for the user to submit a command.
    ShellReady,

    /// A command has been sent and is executing.
    CommandRunning { command: String },

    /// The shell process has exited.
    Exited { exit_code: i32 },
}

impl Default for PtyState {
    fn default() -> Self {
        PtyState::ShellReady
    }
}

/// Skeleton state machine. In later phases this will fuse signals from
/// OSC 133 markers, tcgetattr(), process_group_leader(), and output timing.
pub struct PtyStateMachine {
    pub state: PtyState,
}

impl PtyStateMachine {
    pub fn new() -> Self {
        Self {
            state: PtyState::default(),
        }
    }

    pub fn transition_to_running(&mut self, command: String) -> &PtyState {
        self.state = PtyState::CommandRunning { command };
        &self.state
    }

    pub fn transition_to_ready(&mut self) -> &PtyState {
        self.state = PtyState::ShellReady;
        &self.state
    }

    #[allow(dead_code)]
    pub fn transition_to_exited(&mut self, exit_code: i32) -> &PtyState {
        self.state = PtyState::Exited { exit_code };
        &self.state
    }
}
