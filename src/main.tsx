import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./components/common/ErrorBoundary";

// No native context menu (right click): it's the webview's menu (Reload, Inspect…), out of place
// in a desktop app; the game list has its own (GameContextMenu). Text fields and selected text
// keep it (copy/paste). In development, Alt + right click still opens it (Inspect).
document.addEventListener("contextmenu", (e) => {
  const target = e.target as HTMLElement | null;
  if (target?.closest("input, textarea, [contenteditable='true']")) return;
  if (String(window.getSelection() ?? "").trim()) return;
  if (import.meta.env.DEV && e.altKey) return;
  e.preventDefault();
});

// Images can't be dragged out (covers, boxes, logos…): App.css turns it off with
// -webkit-user-drag, and this catches any webview that ignores it. Dropping files from outside
// is a different event (drop) and still works.
document.addEventListener("dragstart", (e) => {
  if (e.target instanceof HTMLImageElement) e.preventDefault();
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {/* A failure while drawing shows what failed (and Reload) instead of a black window */}
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>,
);
