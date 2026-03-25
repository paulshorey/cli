import { useState, useCallback, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import TerminalView from "./components/TerminalView";
import CommandEditor, {
  type CommandEditorHandle,
} from "./components/CommandEditor";
import StatusBar from "./components/StatusBar";
import { useTranscript } from "./hooks/useTranscript";
import "./App.css";

function App() {
  const [history, setHistory] = useState<string[]>([]);
  const editorRef = useRef<CommandEditorHandle>(null);
  const { entries, cwd, shellState } = useTranscript();

  const lastEntry = entries.length > 0 ? entries[entries.length - 1] : null;
  const isRawMode = shellState.type === "RawMode";

  const handleSubmit = useCallback(async (command: string) => {
    try {
      if (command) {
        await invoke("send_command", { command });
        setHistory((prev) => [...prev, command]);
      } else {
        await invoke("send_input", { input: "\r" });
      }
    } catch (err) {
      console.error("Failed to send command:", err);
    }
  }, []);

  const handleInputSubmit = useCallback(async (text: string) => {
    try {
      await invoke("send_input", { input: text + "\n" });
    } catch (err) {
      console.error("Failed to send input:", err);
    }
  }, []);

  // Global Ctrl+C handler
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "c" && (e.ctrlKey || e.metaKey) && e.shiftKey === false) {
        if (
          shellState.type === "CommandRunning" ||
          shellState.type === "InputExpected" ||
          shellState.type === "RawMode"
        ) {
          e.preventDefault();
          invoke("signal_foreground", { signal: "SIGINT" }).catch(
            console.error
          );
        }
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [shellState]);

  return (
    <div className="app">
      <div className="terminal-pane">
        <TerminalView rawMode={isRawMode} />
      </div>
      <div className={`editor-pane ${isRawMode ? "editor-pane-raw" : ""}`}>
        {!isRawMode && <span className="prompt-symbol">&rsaquo;</span>}
        <CommandEditor
          ref={editorRef}
          onSubmit={handleSubmit}
          onInputSubmit={handleInputSubmit}
          history={history}
          shellState={shellState}
        />
        {!isRawMode && shellState.type !== "InputExpected" && (
          <button
            className="submit-btn"
            onClick={() => editorRef.current?.submit()}
            title="Run (Cmd+Enter)"
          >
            Run
          </button>
        )}
        {shellState.type === "InputExpected" && (
          <button
            className="submit-btn submit-btn-input"
            onClick={() => editorRef.current?.submit()}
            title="Submit (Enter)"
          >
            Send
          </button>
        )}
      </div>
      <StatusBar shellState={shellState} cwd={cwd} lastEntry={lastEntry} />
    </div>
  );
}

export default App;
