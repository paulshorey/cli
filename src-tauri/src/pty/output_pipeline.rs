/// Placeholder for the VTE-based output processing pipeline.
///
/// In Phase 3, this will:
/// - Parse the raw PTY byte stream using the `vte` crate
/// - Intercept OSC 133 shell integration markers
/// - Strip OSC markers from the forwarded stream
/// - Feed parsed events to the PtyStateMachine
/// - Buffer clean bytes for batched emission to the frontend
///
/// For Phase 1, raw bytes are forwarded directly from the PTY reader
/// to the frontend without any VTE processing.
pub struct OutputPipeline;

impl OutputPipeline {
    pub fn new() -> Self {
        Self
    }
}
