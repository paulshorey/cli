use serde::Serialize;

/// Events extracted from OSC sequences in the PTY output stream.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum OscEvent {
    PromptStart,
    PromptEnd,
    CommandStart,
    CommandDone { exit_code: i32 },
    CwdChanged { cwd: String },
}

/// Items emitted by the pipeline, interleaved in the order they appear in the stream.
pub enum PipelineItem {
    Output(Vec<u8>),
    Event(OscEvent),
}

enum ScanState {
    Normal,
    AfterEsc,
    InOsc,
    InOscEscEnd,
}

/// Scans the raw PTY byte stream for OSC 133 (shell integration) and OSC 7 (CWD) sequences.
/// Strips matched sequences from the output and returns them as structured events,
/// interleaved with the clean output bytes in the correct order.
///
/// Also tracks the last line of visible output for use as input prompt hints.
pub struct OutputPipeline {
    scan_state: ScanState,
    osc_buf: Vec<u8>,
    output_buf: Vec<u8>,
    current_line_buf: Vec<u8>,
    last_complete_line: String,
}

impl OutputPipeline {
    pub fn new() -> Self {
        Self {
            scan_state: ScanState::Normal,
            osc_buf: Vec::with_capacity(256),
            output_buf: Vec::with_capacity(4096),
            current_line_buf: Vec::with_capacity(256),
            last_complete_line: String::new(),
        }
    }

    /// Returns the last line of visible output (current incomplete line, or last complete line).
    /// Used as the hint text for InputExpected state.
    pub fn last_line(&self) -> String {
        if !self.current_line_buf.is_empty() {
            String::from_utf8_lossy(&self.current_line_buf).trim().to_string()
        } else {
            self.last_complete_line.clone()
        }
    }

    /// Processes a chunk of raw bytes from the PTY. Returns a sequence of interleaved
    /// output chunks and OSC events, preserving the original ordering.
    pub fn process(&mut self, input: &[u8]) -> Vec<PipelineItem> {
        let mut items = Vec::new();

        for &byte in input {
            match self.scan_state {
                ScanState::Normal => {
                    if byte == 0x1b {
                        self.scan_state = ScanState::AfterEsc;
                    } else {
                        self.output_buf.push(byte);
                        self.track_line(byte);
                    }
                }
                ScanState::AfterEsc => {
                    if byte == b']' {
                        self.scan_state = ScanState::InOsc;
                        self.osc_buf.clear();
                    } else {
                        self.output_buf.push(0x1b);
                        self.output_buf.push(byte);
                        self.scan_state = ScanState::Normal;
                    }
                }
                ScanState::InOsc => {
                    if byte == 0x07 {
                        self.finish_osc(&mut items);
                        self.scan_state = ScanState::Normal;
                    } else if byte == 0x1b {
                        self.scan_state = ScanState::InOscEscEnd;
                    } else {
                        self.osc_buf.push(byte);
                    }
                }
                ScanState::InOscEscEnd => {
                    if byte == b'\\' {
                        self.finish_osc(&mut items);
                        self.scan_state = ScanState::Normal;
                    } else {
                        self.osc_buf.push(0x1b);
                        self.osc_buf.push(byte);
                        self.scan_state = ScanState::InOsc;
                    }
                }
            }
        }

        if !self.output_buf.is_empty() {
            items.push(PipelineItem::Output(std::mem::take(&mut self.output_buf)));
        }

        items
    }

    fn track_line(&mut self, byte: u8) {
        if byte == b'\n' {
            if !self.current_line_buf.is_empty() {
                self.last_complete_line =
                    String::from_utf8_lossy(&self.current_line_buf).trim().to_string();
                self.current_line_buf.clear();
            }
        } else if byte == b'\r' {
            // Carriage return: reset current line (handles \r\n and \r overwrites)
        } else {
            self.current_line_buf.push(byte);
        }
    }

    fn finish_osc(&mut self, items: &mut Vec<PipelineItem>) {
        if let Some(event) = self.try_parse_marker() {
            if !self.output_buf.is_empty() {
                items.push(PipelineItem::Output(std::mem::take(&mut self.output_buf)));
            }
            items.push(PipelineItem::Event(event));
        } else {
            self.output_buf.push(0x1b);
            self.output_buf.push(b']');
            self.output_buf.extend_from_slice(&self.osc_buf);
            self.output_buf.push(0x07);
        }
    }

    fn try_parse_marker(&self) -> Option<OscEvent> {
        let buf = &self.osc_buf;
        if buf.starts_with(b"133;") && buf.len() >= 5 {
            match buf[4] {
                b'A' => Some(OscEvent::PromptStart),
                b'B' => Some(OscEvent::PromptEnd),
                b'C' => Some(OscEvent::CommandStart),
                b'D' => {
                    let exit_code = if buf.len() > 6 {
                        std::str::from_utf8(&buf[6..])
                            .ok()
                            .and_then(|s| s.parse::<i32>().ok())
                            .unwrap_or(0)
                    } else {
                        0
                    };
                    Some(OscEvent::CommandDone { exit_code })
                }
                _ => None,
            }
        } else if buf.starts_with(b"7;") {
            let s = std::str::from_utf8(&buf[2..]).ok()?;
            let path_start = s
                .find("//")
                .and_then(|i| s[i + 2..].find('/').map(|j| i + 2 + j))?;
            Some(OscEvent::CwdChanged {
                cwd: s[path_start..].to_string(),
            })
        } else {
            None
        }
    }
}
