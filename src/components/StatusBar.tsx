interface Props {
  state: "ready" | "running" | "exited";
  processName?: string;
}

export default function StatusBar({ state, processName }: Props) {
  const stateLabel = (() => {
    switch (state) {
      case "ready":
        return "Ready";
      case "running":
        return processName ? `Running: ${processName}` : "Running...";
      case "exited":
        return "Exited";
    }
  })();

  const stateClass = `status-state status-${state}`;

  return (
    <div className="status-bar">
      <span className="status-shell">zsh</span>
      <span className="status-separator">|</span>
      <span className={stateClass}>{stateLabel}</span>
    </div>
  );
}
