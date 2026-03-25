import { useState, useCallback, useRef } from "react";
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

  return (
    <div className="app">
      <div className="terminal-pane">
        <TerminalView />
      </div>
      <div className="editor-pane">
        <span className="prompt-symbol">&rsaquo;</span>
        <CommandEditor
          ref={editorRef}
          onSubmit={handleSubmit}
          history={history}
        />
        <button
          className="submit-btn"
          onClick={() => editorRef.current?.submit()}
          title="Run (Cmd+Enter)"
        >
          Run
        </button>
      </div>
      <StatusBar shellState={shellState} cwd={cwd} lastEntry={lastEntry} />
    </div>
  );
}

export default App;
