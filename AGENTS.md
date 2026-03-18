# A Better CLI User Experience

## Product Design

### Problem

Why do CLI terminal apps have such bad user experience? When the user types into a terminal or when editing a file, user is not able to click and drag the cursor to move the caret or select text. The only way to fix a typo or edit text is to hit "left arrow" key many times until reaching the desired section. User wants to interact with the terminal in-progress text command same way as editing text in any other app or form field - with the mouse, by selecting text, copying/pasting, and dragging to move text from one place to another. Ideally, terminal command text editor should also support rich AI and filesystem features such as autocomplete when strating to type `@` or `/`.

### Solution

User should not interact directly with the terminal. The PTY terminal should be a background process. Only the app internals can interact with the real terminal. User commands are sent all or nothing, not as individual characters.

Show to the user a rich UI layer that we built from scratch and control. It has a "historical view" for scrolling back and reading previous commands and responses. And it has a "text edit" view letting the user type a new command or input to the terminal.

1. Historical view: Top panel of this app is the terminal. It is read-only
2. Text editing view: Bottom panel is a custom full-featured interactive text editor. It only sends text to the terminal after the user has finished editing and clicks "Submit" or "Cmd + Enter".

Bottom panel is usually collapsed to just 1 line. When the user starts typing, the panel will expand taller, so the user can have more space to compose text. When the user submits the edited text, the bottom panel will collapse again to the single-line view.

Top panel (terminal) is used only to read and scroll historical commands and previous CLI outputs. If the terminal expects user input, even something simple like "Y" or "/path" or "Enter", user has to interact with the bottom text editing UI.

Important special case: Sometimes the terminal will launch an in-terminal app such as `vim` to edit a longer text. For example to edit a git commit message, or to modify the instructions during a git rebase. For this case, we must be able to implement our bottom pane as its own text editing application which will be opened by default from the terminal.

## Technical Plan

IMPORTANT: Develop a **transcript-first macOS app**, not a visible terminal emulator. Terminal is in the background process, interacted with by the app internal scripting. User is shown a rich interactive UI. User does not interact with the terminal directly.

## Implementation

As you are building the app, making changes, or reviewing the code, do not limit yourself to only the written plan. If you discover any improvement or issue, deal with it. If you find a better tool or technique, use it.

Initial implementation plan: ./docs/plan/rust-pty-intelligence.md