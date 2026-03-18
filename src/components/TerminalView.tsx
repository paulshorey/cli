import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "@xterm/xterm/css/xterm.css";

const THEME = {
  background: "#181825",
  foreground: "#cdd6f4",
  cursor: "#89b4fa",
  cursorAccent: "#181825",
  selectionBackground: "rgba(137, 180, 250, 0.3)",
  selectionForeground: "#cdd6f4",
  black: "#45475a",
  red: "#f38ba8",
  green: "#a6e3a1",
  yellow: "#f9e2af",
  blue: "#89b4fa",
  magenta: "#f5c2e7",
  cyan: "#94e2d5",
  white: "#bac2de",
  brightBlack: "#585b70",
  brightRed: "#f38ba8",
  brightGreen: "#a6e3a1",
  brightYellow: "#f9e2af",
  brightBlue: "#89b4fa",
  brightMagenta: "#f5c2e7",
  brightCyan: "#94e2d5",
  brightWhite: "#a6adc8",
};

export default function TerminalView() {
  const containerRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    const terminal = new Terminal({
      disableStdin: true,
      cursorBlink: false,
      cursorStyle: "underline",
      theme: THEME,
      fontFamily: "'SF Mono', 'Menlo', 'Monaco', 'Cascadia Code', monospace",
      fontSize: 13,
      lineHeight: 1.4,
      scrollback: 10000,
      convertEol: true,
      allowProposedApi: true,
    });

    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(containerRef.current);

    terminalRef.current = terminal;
    fitAddonRef.current = fitAddon;

    requestAnimationFrame(() => {
      fitAddon.fit();
      syncSize(terminal);
    });

    terminal.onResize(({ cols, rows }) => {
      invoke("resize_pty", { cols, rows }).catch(console.error);
    });

    const resizeObserver = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        fitAddon.fit();
      });
    });
    resizeObserver.observe(containerRef.current);

    const unlistenOutput = listen<string>("pty:output", (event) => {
      terminal.write(event.payload);
    });

    const unlistenExit = listen<number>("pty:exit", (event) => {
      terminal.writeln(
        `\r\n\x1b[90m[Process exited with code ${event.payload}]\x1b[0m`
      );
    });

    return () => {
      unlistenOutput.then((fn) => fn());
      unlistenExit.then((fn) => fn());
      resizeObserver.disconnect();
      terminal.dispose();
    };
  }, []);

  return <div ref={containerRef} className="terminal-view" />;
}

function syncSize(terminal: Terminal) {
  invoke("resize_pty", {
    cols: terminal.cols,
    rows: terminal.rows,
  }).catch(console.error);
}
