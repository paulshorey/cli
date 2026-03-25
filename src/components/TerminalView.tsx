import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CommandDonePayload } from "../types/pty";
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

interface Props {
  rawMode?: boolean;
}

export default function TerminalView({ rawMode = false }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const dataListenerRef = useRef<{ dispose: () => void } | null>(null);

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

    requestAnimationFrame(() => {
      fitAddon.fit();
      invoke("resize_pty", { cols: terminal.cols, rows: terminal.rows }).catch(
        console.error
      );
    });

    terminal.onResize(({ cols, rows }) => {
      invoke("resize_pty", { cols, rows }).catch(console.error);
    });

    const resizeObserver = new ResizeObserver(() => {
      requestAnimationFrame(() => fitAddon.fit());
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

    const unlistenDone = listen<CommandDonePayload>(
      "pty:command_done",
      (event) => {
        const code = event.payload.exit_code;
        const color = code === 0 ? "32" : "31";
        const label = code === 0 ? "ok" : `exit ${code}`;
        const bar = "\u2500".repeat(Math.max(0, terminal.cols - label.length - 6));
        terminal.write(
          `\x1b[90m\u2500\u2500 \x1b[${color}m${label}\x1b[90m \u2500${bar}\x1b[0m\r\n`
        );
      }
    );

    return () => {
      unlistenOutput.then((fn) => fn());
      unlistenExit.then((fn) => fn());
      unlistenDone.then((fn) => fn());
      resizeObserver.disconnect();
      terminal.dispose();
    };
  }, []);

  // Toggle stdin and keyboard forwarding based on rawMode
  useEffect(() => {
    const terminal = terminalRef.current;
    if (!terminal) return;

    if (rawMode) {
      terminal.options.disableStdin = false;
      terminal.options.cursorBlink = true;
      terminal.focus();

      const listener = terminal.onData((data: string) => {
        invoke("send_input", { input: data }).catch(console.error);
      });
      dataListenerRef.current = listener;
    } else {
      terminal.options.disableStdin = true;
      terminal.options.cursorBlink = false;

      if (dataListenerRef.current) {
        dataListenerRef.current.dispose();
        dataListenerRef.current = null;
      }
    }

    return () => {
      if (dataListenerRef.current) {
        dataListenerRef.current.dispose();
        dataListenerRef.current = null;
      }
    };
  }, [rawMode]);

  return <div ref={containerRef} className="terminal-view" />;
}
