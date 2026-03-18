import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import TerminalView from "./components/TerminalView";
import CommandEditor from "./components/CommandEditor";
import StatusBar from "./components/StatusBar";
import "./App.css";

type ShellState = "ready" | "running" | "exited";

function App() {
  const [history, setHistory] = useState<string[]>([]);
  const [shellState, setShellState] = useState<ShellState>("ready");

  const handleSubmit = useCallback(async (command: string) => {
    try {
      setShellState("running");
      await invoke("send_command", { command });
      setHistory((prev) => [...prev, command]);
      setTimeout(() => setShellState("ready"), 500);
    } catch (err) {
      console.error("Failed to send command:", err);
      setShellState("ready");
    }
  }, []);

  return (
    <div className="app">
      <div className="terminal-pane">
        <TerminalView />
      </div>
      <div className="editor-pane">
        <span className="prompt-symbol">❯</span>
        <CommandEditor onSubmit={handleSubmit} history={history} />
        <button
          className="submit-btn"
          onClick={() => {
            const cm = document.querySelector(".cm-content") as HTMLElement;
            if (cm) {
              const text = cm.textContent?.trim();
              if (text) handleSubmit(text);
            }
          }}
          title="Run (Cmd+Enter)"
        >
          Run
        </button>
      </div>
      <StatusBar state={shellState} />
    </div>
  );
}

export default App;
