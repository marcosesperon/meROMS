import { Component, type ErrorInfo, type ReactNode } from "react";
import { translate } from "../../i18n";

interface State {
  error: Error | null;
  stack: string;
}

/**
 * Si algo falla al pintar la interfaz, React lo quita todo y la ventana se queda en negro. Con
 * esto, en su lugar sale qué ha fallado (para poder contarlo) y un botón para volver a cargar.
 */
export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  state: State = { error: null, stack: "" };

  static getDerivedStateFromError(error: Error): State {
    return { error, stack: "" };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("UI crashed:", error, info.componentStack);
    this.setState({ stack: info.componentStack ?? "" });
  }

  render() {
    const { error, stack } = this.state;
    if (!error) return this.props.children;
    return (
      <div className="h-screen overflow-auto bg-zinc-950 text-zinc-200 p-8">
        <h1 className="text-lg font-semibold text-red-300">{translate("crash.title")}</h1>
        <p className="mt-2 text-sm text-zinc-400">{translate("crash.help")}</p>
        <pre className="mt-4 p-3 rounded bg-zinc-900 text-xs text-red-200 whitespace-pre-wrap select-text">
          {/* En macOS (WebKit) la traza no lleva el mensaje: se pone delante si falta */}
          {error.stack && error.message && !error.stack.includes(error.message)
            ? `${error.name}: ${error.message}\n\n${error.stack}`
            : String(error.stack || error.message || error)}
        </pre>
        {stack && <pre className="mt-2 p-3 rounded bg-zinc-900 text-[11px] text-zinc-500 whitespace-pre-wrap select-text">{stack.trim()}</pre>}
        <button onClick={() => window.location.reload()} className="mt-4 px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500">
          {translate("crash.reload")}
        </button>
      </div>
    );
  }
}
