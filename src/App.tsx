import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

function App() {
  const [output, setOutput] = useState("");
  const [command, setCommand] = useState("");
  const [history, setHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  const outputRef = useRef<HTMLPreElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const unlistenOutput = listen<string>("pty:output", (event) => {
      setOutput((prev) => prev + event.payload);
    });

    const unlistenExit = listen<number>("pty:exit", (event) => {
      setOutput((prev) => prev + `\n[Process exited with code ${event.payload}]`);
    });

    return () => {
      unlistenOutput.then((fn) => fn());
      unlistenExit.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [output]);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (!command.trim()) return;

      try {
        await invoke("send_command", { command });
        setHistory((prev) => [...prev, command]);
        setHistoryIndex(-1);
        setCommand("");
      } catch (err) {
        console.error("Failed to send command:", err);
      }
    },
    [command]
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "ArrowUp") {
        e.preventDefault();
        if (history.length === 0) return;
        const newIndex =
          historyIndex === -1 ? history.length - 1 : Math.max(0, historyIndex - 1);
        setHistoryIndex(newIndex);
        setCommand(history[newIndex]);
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        if (historyIndex === -1) return;
        const newIndex = historyIndex + 1;
        if (newIndex >= history.length) {
          setHistoryIndex(-1);
          setCommand("");
        } else {
          setHistoryIndex(newIndex);
          setCommand(history[newIndex]);
        }
      }
    },
    [history, historyIndex]
  );

  return (
    <div className="app">
      <div className="output-container">
        <pre ref={outputRef} className="terminal-output">
          {output}
        </pre>
      </div>
      <form className="input-container" onSubmit={handleSubmit}>
        <span className="prompt-symbol">❯</span>
        <input
          ref={inputRef}
          type="text"
          value={command}
          onChange={(e) => setCommand(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Type a command..."
          className="command-input"
          autoFocus
          spellCheck={false}
          autoComplete="off"
          autoCorrect="off"
        />
        <button type="submit" className="submit-btn">
          Run
        </button>
      </form>
    </div>
  );
}

export default App;
