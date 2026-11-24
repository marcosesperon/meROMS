import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./components/common/ErrorBoundary";

// No context menu (right click): it's the browser's menu (Reload, Inspect…), out of place in a
// desktop app. Text fields keep it (copy/paste), and so does development (to inspect).
if (!import.meta.env.DEV) {
  document.addEventListener("contextmenu", (e) => {
    const target = e.target as HTMLElement | null;
    if (!target?.closest("input, textarea, [contenteditable='true']")) e.preventDefault();
  });
}

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
