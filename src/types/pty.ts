export type PtyState =
  | { type: "ShellReady" }
  | { type: "CommandSent"; command: string }
  | { type: "CommandRunning"; command: string }
  | { type: "InputExpected"; hint: string; echo_enabled: boolean }
  | { type: "RawMode"; process_name: string; is_editor: boolean }
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
