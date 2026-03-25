import { useState, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import type {
  PtyState,
  CommandDonePayload,
  CwdPayload,
  TranscriptEntry,
} from "../types/pty";

export interface TranscriptState {
  entries: TranscriptEntry[];
  cwd: string;
  shellState: PtyState;
}

export function useTranscript(): TranscriptState {
  const [entries, setEntries] = useState<TranscriptEntry[]>([]);
  const [cwd, setCwd] = useState("");
  const [shellState, setShellState] = useState<PtyState>({ type: "ShellReady" });
  const nextId = useRef(0);

  useEffect(() => {
    const unlistenState = listen<PtyState>("pty:state_changed", (event) => {
      setShellState(event.payload);

      if (event.payload.type === "CommandRunning") {
        const command = event.payload.command;
        setEntries((prev) => {
          const last = prev[prev.length - 1];
          if (last && last.exitCode === null && last.command === command) {
            return prev;
          }
          const id = nextId.current++;
          return [
            ...prev,
            {
              id,
              command,
              exitCode: null,
              startTime: Date.now(),
              endTime: null,
            },
          ];
        });
      }
    });

    const unlistenDone = listen<CommandDonePayload>(
      "pty:command_done",
      (event) => {
        setEntries((prev) => {
          const updated = [...prev];
          const last = updated[updated.length - 1];
          if (last && last.exitCode === null) {
            last.exitCode = event.payload.exit_code;
            last.endTime = Date.now();
          }
          return updated;
        });
      }
    );

    const unlistenCwd = listen<CwdPayload>("pty:cwd_changed", (event) => {
      setCwd(event.payload.cwd);
    });

    return () => {
      unlistenState.then((fn) => fn());
      unlistenDone.then((fn) => fn());
      unlistenCwd.then((fn) => fn());
    };
  }, []);

  return { entries, cwd, shellState };
}
