import type { PtyState, TranscriptEntry } from "../types/pty";

interface Props {
  shellState: PtyState;
  cwd: string;
  lastEntry: TranscriptEntry | null;
}

export default function StatusBar({ shellState, cwd, lastEntry }: Props) {
  const stateInfo = formatState(shellState);
  const displayCwd = cwd ? shortenPath(cwd) : "";
  const exitBadge = formatExitBadge(lastEntry);

  return (
    <div className="status-bar">
      {displayCwd && <span className="status-cwd">{displayCwd}</span>}
      {displayCwd && <span className="status-separator">|</span>}
      <span className="status-shell">zsh</span>
      <span className="status-separator">|</span>
      <span className={`status-state ${stateInfo.cls}`}>{stateInfo.label}</span>
      <span className="status-spacer" />
      {exitBadge && (
        <span className={`status-exit ${exitBadge.cls}`}>{exitBadge.label}</span>
      )}
    </div>
  );
}

function formatState(state: PtyState): { label: string; cls: string } {
  switch (state.type) {
    case "ShellReady":
      return { label: "Ready", cls: "status-ready" };
    case "CommandSent":
      return { label: `Sending: ${state.command}`, cls: "status-running" };
    case "CommandRunning":
      return { label: `Running: ${state.command}`, cls: "status-running" };
    case "Exited":
      return { label: "Exited", cls: "status-exited" };
  }
}

function formatExitBadge(
  entry: TranscriptEntry | null
): { label: string; cls: string } | null {
  if (!entry || entry.exitCode === null) return null;
  const code = entry.exitCode;
  return {
    label: code === 0 ? "exit 0" : `exit ${code}`,
    cls: code === 0 ? "status-exit-ok" : "status-exit-err",
  };
}

function shortenPath(path: string): string {
  const home = "~";
  const homePath = path.replace(/^\/Users\/[^/]+/, home);
  const parts = homePath.split("/");
  if (parts.length <= 3) return homePath;
  return parts[0] + "/.../" + parts.slice(-2).join("/");
}
