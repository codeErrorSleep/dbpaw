import { Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

const baseThemeSpec: Parameters<typeof EditorView.theme>[0] = {
  "&": {
    backgroundColor: "var(--background) !important",
    color: "var(--foreground)",
  },
  ".cm-editor": {
    backgroundColor: "var(--background) !important",
    color: "var(--foreground)",
  },
  ".cm-scroller": {
    backgroundColor: "var(--background) !important",
  },
  ".cm-content": {
    caretColor: "var(--foreground)",
  },
  ".cm-cursor, .cm-dropCursor": {
    borderLeftColor: "var(--foreground)",
  },
  ".cm-gutters": {
    backgroundColor: "var(--background) !important",
    color: "var(--muted-foreground)",
    borderRight: "1px solid var(--border)",
  },
  ".cm-activeLine": {
    backgroundColor: "color-mix(in oklab, var(--accent) 70%, transparent)",
  },
  ".cm-activeLineGutter": {
    backgroundColor: "color-mix(in oklab, var(--accent) 70%, transparent)",
  },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": {
    backgroundColor: "var(--editor-selection-bg) !important",
  },
  ".cm-selectionLayer .cm-selectionBackground, &.cm-focused .cm-selectionLayer .cm-selectionBackground":
    {
      backgroundColor: "var(--editor-selection-bg) !important",
    },
  ".cm-line::selection, .cm-line > span::selection, .cm-content ::selection": {
    backgroundColor: "var(--editor-selection-bg) !important",
    color: "var(--editor-selection-fg) !important",
  },
  "&.cm-focused .cm-line::selection, &.cm-focused .cm-line > span::selection, &.cm-focused .cm-content ::selection":
    {
      backgroundColor: "var(--editor-selection-bg) !important",
      color: "var(--editor-selection-fg) !important",
    },
  ".cm-tooltip": {
    backgroundColor: "var(--popover)",
    color: "var(--popover-foreground)",
    border: "1px solid var(--border)",
  },
  ".cm-tooltip-autocomplete": {
    backgroundColor: "var(--popover)",
    color: "var(--popover-foreground)",
    border: "1px solid var(--border)",
  },
  ".cm-tooltip-autocomplete ul li[aria-selected]": {
    backgroundColor: "color-mix(in oklab, var(--accent) 85%, transparent)",
    color: "var(--popover-foreground)",
  },
  ".cm-panels": {
    backgroundColor: "var(--popover)",
    color: "var(--popover-foreground)",
    borderTop: "1px solid var(--border)",
  },
  ".cm-panel input": {
    backgroundColor: "var(--input-background)",
    color: "var(--foreground)",
    border: "1px solid var(--border)",
  },
  ".cm-panel button": {
    backgroundColor: "var(--secondary)",
    color: "var(--secondary-foreground)",
    border: "1px solid var(--border)",
  },
};

export const sqlEditorThemeLight: Extension = EditorView.theme(baseThemeSpec, {
  dark: false,
});
export const sqlEditorThemeDark: Extension = EditorView.theme(baseThemeSpec, {
  dark: true,
});
