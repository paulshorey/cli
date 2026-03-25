import {
  useEffect,
  useRef,
  useCallback,
  forwardRef,
  useImperativeHandle,
} from "react";
import {
  EditorView,
  keymap,
  placeholder,
  drawSelection,
} from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";

interface Props {
  onSubmit: (text: string) => void;
  history: string[];
}

export interface CommandEditorHandle {
  submit: () => void;
  focus: () => void;
}

const editorTheme = EditorView.theme({
  "&": {
    backgroundColor: "transparent",
    fontSize: "13px",
  },
  ".cm-content": {
    caretColor: "#89b4fa",
    fontFamily: "'SF Mono', 'Menlo', 'Monaco', 'Cascadia Code', monospace",
    padding: "6px 0",
    minHeight: "20px",
    color: "#cdd6f4",
  },
  "&.cm-focused": {
    outline: "none",
  },
  ".cm-cursor, .cm-dropCursor": {
    borderLeftColor: "#89b4fa",
    borderLeftWidth: "2px",
  },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection":
    {
      backgroundColor: "rgba(137, 180, 250, 0.3) !important",
    },
  ".cm-activeLine": {
    backgroundColor: "transparent",
  },
  ".cm-gutters": {
    display: "none",
  },
  ".cm-placeholder": {
    color: "#6c7086",
  },
  ".cm-line": {
    padding: "0",
  },
  ".cm-scroller": {
    overflow: "auto",
  },
});

const CommandEditor = forwardRef<CommandEditorHandle, Props>(
  ({ onSubmit, history: cmdHistory }, ref) => {
    const containerRef = useRef<HTMLDivElement>(null);
    const viewRef = useRef<EditorView | null>(null);
    const onSubmitRef = useRef(onSubmit);
    const historyRef = useRef(cmdHistory);
    const historyIndexRef = useRef(-1);

    onSubmitRef.current = onSubmit;
    historyRef.current = cmdHistory;

    const submitCommand = useCallback((view: EditorView) => {
      const text = view.state.doc.toString().trim();
      onSubmitRef.current(text);
      if (view.state.doc.length > 0) {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "" },
        });
      }
      historyIndexRef.current = -1;
      return true;
    }, []);

    const navigateHistory = useCallback(
      (view: EditorView, direction: "back" | "forward") => {
        const h = historyRef.current;
        if (h.length === 0) return false;

        let newIndex: number;
        if (direction === "back") {
          newIndex =
            historyIndexRef.current === -1
              ? h.length - 1
              : Math.max(0, historyIndexRef.current - 1);
        } else {
          if (historyIndexRef.current === -1) return false;
          newIndex = historyIndexRef.current + 1;
          if (newIndex >= h.length) {
            historyIndexRef.current = -1;
            view.dispatch({
              changes: { from: 0, to: view.state.doc.length, insert: "" },
            });
            return true;
          }
        }

        historyIndexRef.current = newIndex;
        const cmd = h[newIndex];
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: cmd },
          selection: { anchor: cmd.length },
        });
        return true;
      },
      []
    );

    useImperativeHandle(ref, () => ({
      submit: () => {
        if (viewRef.current) submitCommand(viewRef.current);
      },
      focus: () => {
        viewRef.current?.focus();
      },
    }));

    useEffect(() => {
      if (!containerRef.current) return;

      const submitKeymap = keymap.of([
        {
          key: "Mod-Enter",
          run: (view) => submitCommand(view),
        },
        {
          key: "ArrowUp",
          run: (view) => {
            const line = view.state.doc.lineAt(
              view.state.selection.main.head
            );
            if (line.number === 1) {
              return navigateHistory(view, "back");
            }
            return false;
          },
        },
        {
          key: "ArrowDown",
          run: (view) => {
            const line = view.state.doc.lineAt(
              view.state.selection.main.head
            );
            if (line.number === view.state.doc.lines) {
              return navigateHistory(view, "forward");
            }
            return false;
          },
        },
      ]);

      const view = new EditorView({
        state: EditorState.create({
          doc: "",
          extensions: [
            submitKeymap,
            keymap.of([...defaultKeymap, ...historyKeymap]),
            history(),
            editorTheme,
            placeholder("Type a command..."),
            drawSelection(),
            EditorView.lineWrapping,
          ],
        }),
        parent: containerRef.current,
      });

      viewRef.current = view;
      view.focus();

      return () => {
        view.destroy();
      };
    }, []); // eslint-disable-line react-hooks/exhaustive-deps

    return <div ref={containerRef} className="command-editor-container" />;
  }
);

CommandEditor.displayName = "CommandEditor";

export default CommandEditor;
