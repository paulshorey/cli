export type PtyState =
  | { type: "ShellReady" }
  | { type: "CommandSent"; command: string }
  | { type: "CommandRunning"; command: string }
  | { type: "Exited"; exit_code: number };

export interface CommandDonePayload {
  command: string;
  exit_code: number;
}

export interface CwdPayload {
  cwd: string;
}

export interface TranscriptEntry {
  id: number;
  command: string;
  exitCode: number | null;
  startTime: number;
  endTime: number | null;
}
