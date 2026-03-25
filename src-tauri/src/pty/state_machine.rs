use serde::Serialize;
use std::time::{Duration, Instant};

use super::output_pipeline::OscEvent;

const INPUT_STALL_THRESHOLD: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum PtyState {
    ShellReady,
    CommandSent { command: String },
    CommandRunning { command: String },
    InputExpected { hint: String, echo_enabled: bool },
    RawMode { process_name: String, is_editor: bool },
    Exited { exit_code: i32 },
}

impl Default for PtyState {
    fn default() -> Self {
        PtyState::ShellReady
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CommandDonePayload {
    pub command: String,
    pub exit_code: i32,
}

#[derive(Clone, Debug, Serialize)]
pub struct CwdPayload {
    pub cwd: String,
}

pub enum Emission {
    StateChanged(PtyState),
    CommandDone(CommandDonePayload),
    CwdChanged(CwdPayload),
}

const EDITOR_PROCESSES: &[&str] = &["vim", "nvim", "vi", "nano", "emacs", "pico", "micro"];

pub struct PtyStateMachine {
    pub state: PtyState,
    pub cwd: String,
    pending_command: String,
    #[allow(dead_code)]
    shell_pid: u32,
    last_output_time: Option<Instant>,
    last_output_line: String,
    canonical_mode: bool,
    echo_enabled: bool,
    fg_pid: Option<u32>,
    fg_process_name: String,
}

impl PtyStateMachine {
    pub fn new(shell_pid: u32) -> Self {
        Self {
            state: PtyState::default(),
            cwd: String::new(),
            pending_command: String::new(),
            shell_pid,
            last_output_time: None,
            last_output_line: String::new(),
            canonical_mode: true,
            echo_enabled: true,
            fg_pid: None,
            fg_process_name: String::new(),
        }
    }

    pub fn on_command_sent(&mut self, command: &str) -> Vec<Emission> {
        self.pending_command = command.to_string();
        self.last_output_time = None;
        self.last_output_line.clear();
        self.state = PtyState::CommandSent {
            command: command.to_string(),
        };
        vec![Emission::StateChanged(self.state.clone())]
    }

    pub fn on_exit(&mut self, exit_code: i32) -> Vec<Emission> {
        self.state = PtyState::Exited { exit_code };
        vec![Emission::StateChanged(self.state.clone())]
    }

    pub fn on_osc_event(&mut self, event: OscEvent) -> Vec<Emission> {
        let mut emissions = Vec::new();

        match event {
            OscEvent::PromptStart => {
                if !matches!(self.state, PtyState::ShellReady) {
                    self.state = PtyState::ShellReady;
                    emissions.push(Emission::StateChanged(self.state.clone()));
                }
            }
            OscEvent::PromptEnd => {}
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

    /// Called by the output reader thread when PTY output bytes are received.
    /// Updates timing and last-line tracking; transitions InputExpected → CommandRunning.
    pub fn on_output_activity(&mut self, last_line: &str) -> Vec<Emission> {
        self.last_output_time = Some(Instant::now());
        if !last_line.is_empty() {
            self.last_output_line = last_line.to_string();
        }

        if matches!(self.state, PtyState::InputExpected { .. }) {
            self.state = PtyState::CommandRunning {
                command: self.pending_command.clone(),
            };
            return vec![Emission::StateChanged(self.state.clone())];
        }

        Vec::new()
    }

    /// Called by the termios monitor every 100ms. Evaluates transitions based on
    /// terminal mode flags combined with output timing.
    pub fn on_termios_check(&mut self, canonical: bool, echo: bool) -> Vec<Emission> {
        self.canonical_mode = canonical;
        self.echo_enabled = echo;
        self.evaluate_transitions()
    }

    /// Called by the process monitor when the foreground process changes.
    pub fn on_foreground_change(&mut self, pid: u32, name: String) -> Vec<Emission> {
        self.fg_pid = Some(pid);
        self.fg_process_name = name;
        self.evaluate_transitions()
    }

    /// Central transition evaluator. Fuses termios state, process info, and output timing
    /// to decide if a state transition should occur.
    fn evaluate_transitions(&mut self) -> Vec<Emission> {
        match &self.state {
            PtyState::CommandRunning { .. } | PtyState::CommandSent { .. } => {
                if !self.canonical_mode {
                    let process_name = self.fg_process_name.clone();
                    let is_editor = EDITOR_PROCESSES
                        .iter()
                        .any(|&e| process_name == e);
                    self.state = PtyState::RawMode {
                        process_name,
                        is_editor,
                    };
                    return vec![Emission::StateChanged(self.state.clone())];
                }

                if let Some(last_time) = self.last_output_time {
                    let stalled = last_time.elapsed() > INPUT_STALL_THRESHOLD;
                    let is_running = matches!(self.state, PtyState::CommandRunning { .. });
                    if stalled && is_running {
                        self.state = PtyState::InputExpected {
                            hint: self.last_output_line.clone(),
                            echo_enabled: self.echo_enabled,
                        };
                        return vec![Emission::StateChanged(self.state.clone())];
                    }
                }
            }

            PtyState::InputExpected { .. } => {
                if !self.canonical_mode {
                    let process_name = self.fg_process_name.clone();
                    let is_editor = EDITOR_PROCESSES
                        .iter()
                        .any(|&e| process_name == e);
                    self.state = PtyState::RawMode {
                        process_name,
                        is_editor,
                    };
                    return vec![Emission::StateChanged(self.state.clone())];
                }
            }

            PtyState::RawMode { .. } => {
                if self.canonical_mode {
                    self.state = PtyState::CommandRunning {
                        command: self.pending_command.clone(),
                    };
                    return vec![Emission::StateChanged(self.state.clone())];
                }
            }

            _ => {}
        }

        Vec::new()
    }
}
