# Architecture: Phase 3 -- Shell Integration and Command Tracking

## The Core Problem

A normal terminal emulator receives a raw stream of bytes from the shell process. It has no idea where one command's output ends and the next begins. It doesn't know whether the shell is waiting for input or running a program. It just renders bytes.

Our app needs to *understand* what the shell is doing. It needs to know:
- When the shell is showing its prompt (idle, waiting for a command)
- When a command starts running
- When a command finishes, and what its exit code was
- What the current working directory is

This understanding is what makes our app different from a dumb terminal emulator. Phase 3 solves this problem.

---

## How It Works: The Three-Layer Pipeline

The system has three layers. Each layer transforms data and passes it to the next.

```
Layer 1: Shell Integration Script (runs inside zsh)
  -- Injects invisible marker sequences into the terminal output stream

Layer 2: Output Pipeline (runs in the Rust backend)
  -- Scans the byte stream, extracts markers, feeds the state machine

Layer 3: State Machine (runs in the Rust backend)
  -- Tracks what the shell is doing right now, emits typed events to the frontend
```

### Layer 1: Shell Integration Script

**File:** `src-tauri/src/shell/integration.rs` (the script is embedded as a Rust string constant)

#### What is it?

A small zsh script that hooks into two shell events:
- `precmd` -- a function zsh calls **before** it displays each prompt
- `preexec` -- a function zsh calls **after** the user enters a command but **before** it executes

These hooks emit invisible "marker" sequences into the terminal output stream. The markers are [OSC escape sequences](https://en.wikipedia.org/wiki/ANSI_escape_code#OSC_(Operating_System_Command)_sequences) -- a standard mechanism for terminals and shells to exchange metadata. They are invisible to the user because terminal emulators consume them silently.

#### The markers

The script emits four types of markers:

| Marker | When it fires | What it means |
|---|---|---|
| `ESC ] 133;A BEL` | In `precmd`, after reporting the previous command's result | The shell is about to draw its prompt |
| `ESC ] 133;C BEL` | In `preexec`, when the user submits a command | A command is about to start executing |
| `ESC ] 133;D;{exit_code} BEL` | In `precmd`, before the next prompt | The previous command finished with this exit code |
| `ESC ] 7;file://{host}{path} BEL` | In `precmd`, between D and A | The current working directory |

OSC 133 is the same protocol used by VS Code, iTerm2, and Kitty for their shell integration features. OSC 7 is a standard for CWD reporting.

#### The actual script

```zsh
__cli_app_precmd() {
    local __exit=$?
    if [[ -n "$__cli_app_cmd_running" ]]; then
        printf '\033]133;D;%d\007' "$__exit"   # command just finished
        unset __cli_app_cmd_running
    fi
    printf '\033]7;file://%s%s\007' "${HOST}" "${PWD}"  # report CWD
    printf '\033]133;A\007'                              # prompt starting
}

__cli_app_preexec() {
    printf '\033]133;C\007'                              # command starting
    __cli_app_cmd_running=1
}
```

Each `printf` writes an escape sequence directly into the terminal's output stream. The `\033]` is `ESC ]` (the OSC introducer), and `\007` is `BEL` (the OSC terminator).

#### How it gets loaded: the ZDOTDIR trick

Zsh loads its config files from `$ZDOTDIR` (defaults to `$HOME`). At startup, our app:

1. Creates a temporary directory (e.g., `/tmp/cli-app-shell-12345/`)
2. Writes proxy dotfiles (`.zshenv`, `.zprofile`, `.zshrc`, `.zlogin`) into it
3. Each proxy file sources the user's original file from `$HOME`, so oh-my-zsh and all user config loads normally
4. The proxy `.zshrc` appends our integration script **after** the user's config, so our hooks are added on top of any existing `precmd`/`preexec` hooks
5. Sets `ZDOTDIR` to the temp dir before spawning the shell
6. After loading, restores `ZDOTDIR` to the original value so sub-shells behave normally

This is the same technique VS Code uses for its terminal shell integration.

---

### Layer 2: Output Pipeline (OSC Scanner)

**File:** `src-tauri/src/pty/output_pipeline.rs`

#### What is it?

A byte-level scanner that sits between the PTY reader and the frontend. Every byte of terminal output passes through it. It has one job: find our OSC markers in the stream, extract their data, and remove them so the frontend never sees them (they would render as garbage in xterm.js otherwise).

#### How the scanner works

The scanner is a small state machine with four states:

```
Normal  --[ESC]-->  AfterEsc  --[']']-->  InOsc  --[BEL]-->  Normal
                       |                    |
                   [other]              [ESC]
                       |                    |
                       v                    v
                    Normal            InOscEscEnd --['\']-->  Normal
```

- **Normal**: Every byte goes into the output buffer. If we see `ESC` (0x1b), enter `AfterEsc`.
- **AfterEsc**: If the next byte is `]`, we're entering an OSC sequence -- switch to `InOsc`. Otherwise, it was a regular escape sequence (like a color code); emit both bytes as output and go back to `Normal`.
- **InOsc**: Accumulate bytes into a separate OSC buffer. If we see `BEL` (0x07), the OSC is complete. If we see `ESC`, it might be the start of an ST terminator (`ESC \`).
- **InOscEscEnd**: If the next byte is `\`, the OSC is complete (ST terminator). Otherwise, those bytes were part of the OSC content.

When an OSC sequence completes, `finish_osc()` checks if it's one of our markers (starts with `133;` or `7;`). If yes:
1. Flush any output bytes accumulated *before* this marker as a `PipelineItem::Output`
2. Parse the marker data and emit a `PipelineItem::Event`

If it's not one of our markers (e.g., OSC 0 for window title), reconstruct it and add it to the output buffer so it passes through to xterm.js unchanged.

#### Why interleaving matters

The pipeline returns a `Vec<PipelineItem>` where Output and Event items are interleaved in the exact order they appeared in the byte stream. This matters because the frontend needs to insert a visual separator *between* the last line of command output and the next shell prompt. If we emitted all output first and then all events, the separator would appear in the wrong position.

Example: the raw stream `...output_bytes...[OSC 133;D;0]...[OSC 7;cwd]...[OSC 133;A]...prompt_bytes...` produces:

```
PipelineItem::Output("...output_bytes...")
PipelineItem::Event(CommandDone { exit_code: 0 })   <-- separator goes here
PipelineItem::Event(CwdChanged { cwd: "/path" })
PipelineItem::Event(PromptStart)
PipelineItem::Output("...prompt_bytes...")
```

---

### Layer 3: State Machine

**File:** `src-tauri/src/pty/state_machine.rs`

#### What is a state machine?

A state machine is a model where a system is always in exactly one "state" from a finite set, and transitions between states are triggered by "events." It's a way to make complex, asynchronous behavior predictable and debuggable.

In our case, the shell is always doing one of these things:

```
                        +--------------+
                        |  ShellReady  |  <-- shell is idle, showing prompt
                        +------+-------+
                               |
                    user submits command
                               |
                        +------v-------+
                        | CommandSent  |  <-- command written to PTY, waiting for shell
                        +------+-------+
                               |
                    OSC 133;C received (shell confirms execution)
                               |
                        +------v--------+
                        |CommandRunning |  <-- command is executing, output flowing
                        +------+--------+
                               |
                    OSC 133;D received (command finished)
                               |
                        +------v-------+
                        |  ShellReady  |  <-- back to idle
                        +------+-------+
```

#### Implementation

The state machine struct holds three pieces of data:

- `state: PtyState` -- the current state (ShellReady, CommandSent, CommandRunning, or Exited)
- `cwd: String` -- the last known working directory
- `pending_command: String` -- the command text the user submitted (so we can associate it with completion events)

It exposes two methods:

**`on_command_sent(command)`** -- called when the user clicks Run or presses Enter. Transitions to `CommandSent` and remembers the command text.

**`on_osc_event(event)`** -- called when the output pipeline extracts an OSC marker. The transitions:

| Current State | Event | New State | Side Effects |
|---|---|---|---|
| any | PromptStart (A) | ShellReady | emit `pty:state_changed` |
| CommandSent | CommandStart (C) | CommandRunning | emit `pty:state_changed` |
| CommandRunning | CommandDone (D) | ShellReady | emit `pty:command_done` + `pty:state_changed` |
| any | CwdChanged | (unchanged) | emit `pty:cwd_changed` |

Both methods return a `Vec<Emission>` -- a list of actions the caller should perform. The state machine itself never touches Tauri or the network. It just returns instructions like "emit a StateChanged event" or "emit a CommandDone event." The caller (the reader thread in `lib.rs`) executes those instructions.

This separation keeps the state machine pure and testable.

---

## The Reader Thread

**File:** `src-tauri/src/lib.rs` (`start_output_thread`)

The reader thread ties everything together. It runs in a dedicated OS thread (not an async task) because PTY reads are blocking.

```
loop:
  1. Read raw bytes from PTY master fd
  2. Pass through OutputPipeline  -->  Vec<PipelineItem>
  3. For each item:
     - Output(bytes):  emit "pty:output" event to frontend (for xterm.js)
     - Event(osc):     feed to PtyStateMachine  -->  Vec<Emission>
                        for each emission, emit the corresponding Tauri event
  4. Repeat
```

The frontend receives a stream of events in the correct order:
- `pty:output` -- raw terminal bytes (ANSI colors, text, etc.) for xterm.js to render
- `pty:state_changed` -- the shell transitioned to a new state
- `pty:command_done` -- a command finished (includes exit code)
- `pty:cwd_changed` -- the working directory changed

---

## Frontend Event Handling

The frontend subscribes to these events in two places:

### TerminalView (`src/components/TerminalView.tsx`)

Writes output to xterm.js and injects visual separators:
- On `pty:output`: writes the bytes to xterm.js for rendering
- On `pty:command_done`: writes a horizontal separator line to xterm.js with the exit code (green for 0, red for non-zero)

### useTranscript hook (`src/hooks/useTranscript.ts`)

Maintains a structured transcript of all commands:
- On `pty:state_changed` with type `CommandRunning`: creates a new transcript entry
- On `pty:command_done`: finalizes the latest entry with the exit code and end time
- On `pty:cwd_changed`: updates the current working directory

The transcript data and shell state are consumed by the StatusBar (shows CWD, state, last exit code) and will be used by future features like command search and structured history view.

---

## Data Flow Diagram

```
User types "ls -la" and presses Enter
        |
        v
[React App]  -->  invoke("send_command", { command: "ls -la" })
        |
        v
[Tauri Command Handler]  -->  state_machine.on_command_sent("ls -la")
        |                          |
        |                     emits pty:state_changed { type: "CommandSent" }
        |
        v
[PtySession.send_command]  -->  writes "ls -la\n" to PTY stdin
        |
        v
[zsh process receives input]
        |
        +-- preexec fires: writes  ESC]133;C BEL  to PTY output
        +-- executes `ls -la`, writes file listing to PTY output
        +-- command finishes
        +-- precmd fires: writes   ESC]133;D;0 BEL
        |                          ESC]7;file://host/path BEL
        |                          ESC]133;A BEL
        +-- draws prompt
        |
        v
[Reader Thread reads raw bytes from PTY]
        |
        v
[OutputPipeline.process(bytes)]
        |
        +-- PipelineItem::Event(CommandStart)       --> state_machine  --> emits pty:state_changed
        +-- PipelineItem::Output("file listing")    --> emits pty:output
        +-- PipelineItem::Event(CommandDone { 0 })  --> state_machine  --> emits pty:command_done
        +-- PipelineItem::Event(CwdChanged)         --> state_machine  --> emits pty:cwd_changed
        +-- PipelineItem::Event(PromptStart)        --> state_machine  --> emits pty:state_changed
        +-- PipelineItem::Output("prompt text")     --> emits pty:output
        |
        v
[Frontend receives events in order]
        |
        +-- pty:state_changed { CommandRunning }  --> StatusBar shows "Running: ls -la"
        +-- pty:output "file listing"             --> xterm.js renders the file listing
        +-- pty:command_done { exit_code: 0 }     --> xterm.js draws green "── ok ──" separator
        +-- pty:cwd_changed { cwd: "/path" }      --> StatusBar updates CWD
        +-- pty:state_changed { ShellReady }      --> StatusBar shows "Ready"
        +-- pty:output "prompt text"              --> xterm.js renders the shell prompt
```

---

## File Reference

| File | Role |
|---|---|
| `src-tauri/src/shell/integration.rs` | Zsh integration script + ZDOTDIR temp dir setup |
| `src-tauri/src/pty/output_pipeline.rs` | Byte-level OSC scanner, strips markers, returns interleaved items |
| `src-tauri/src/pty/state_machine.rs` | Tracks shell state, produces typed emissions from OSC events |
| `src-tauri/src/pty/session.rs` | PTY lifecycle, spawns shell with ZDOTDIR injection |
| `src-tauri/src/lib.rs` | Reader thread: pipeline + state machine + event emission |
| `src-tauri/src/commands.rs` | Tauri command handlers (send_command triggers state transition) |
| `src/hooks/useTranscript.ts` | Frontend state: transcript entries, CWD, shell state |
| `src/components/TerminalView.tsx` | xterm.js rendering + command separator injection |
| `src/components/StatusBar.tsx` | Displays CWD, shell state, last exit code |
| `src/types/pty.ts` | TypeScript types mirroring Rust PtyState and event payloads |
