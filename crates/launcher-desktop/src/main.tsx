import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/global.css";

/** Catches render errors so a crash shows a readable panel, never a white window. */
class ErrorBoundary extends React.Component<
  { children: React.ReactNode },
  { error: Error | null }
> {
  state = { error: null as Error | null };
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  render() {
    if (this.state.error) {
      return (
        <div
          style={{
            position: "fixed",
            inset: 0,
            background: "#05060d",
            color: "#eef1fa",
            fontFamily: "system-ui, sans-serif",
            padding: 40,
            overflow: "auto",
          }}
        >
          <h2 style={{ marginBottom: 12 }}>Something went wrong</h2>
          <pre style={{ whiteSpace: "pre-wrap", color: "#fca5a5", fontSize: 13 }}>
            {String(this.state.error?.stack || this.state.error)}
          </pre>
          <button
            style={{
              marginTop: 16,
              padding: "10px 18px",
              borderRadius: 12,
              border: "1px solid #ffffff22",
              background: "#ffffff10",
              color: "#fff",
            }}
            onClick={() => location.reload()}
          >
            Reload
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>
);
