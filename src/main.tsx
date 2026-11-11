import React, { Component, ErrorInfo, ReactNode } from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
}

class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null, errorInfo: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error, errorInfo: null };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error("Uncaught runtime error in component tree:", error, errorInfo);
    this.setState({ errorInfo });
  }

  render() {
    if (this.state.hasError) {
      return (
        <div
          style={{
            minHeight: "100vh",
            backgroundColor: "#090d16",
            color: "#f87171",
            fontFamily: "monospace",
            padding: "2rem",
            boxSizing: "border-box",
            display: "flex",
            flexDirection: "column",
            gap: "1rem",
          }}
        >
          <h1
            style={{
              fontSize: "1.25rem",
              color: "#ef4444",
              margin: 0,
              textTransform: "uppercase",
              letterSpacing: "0.05em",
            }}
          >
            [MURMUR RUNTIME ERROR]
          </h1>
          <p style={{ color: "#e2e8f0", margin: 0, fontSize: "0.875rem" }}>
            The frontend encountered an unexpected exception:
          </p>
          <div
            style={{
              backgroundColor: "#1e1e2e",
              border: "1px solid #ef4444",
              borderRadius: "6px",
              padding: "1rem",
              color: "#fca5a5",
              fontSize: "0.875rem",
              whiteSpace: "pre-wrap",
              wordBreak: "break-word",
            }}
          >
            {this.state.error?.toString()}
          </div>
          {this.state.errorInfo?.componentStack && (
            <div
              style={{
                backgroundColor: "#181825",
                border: "1px solid #3b4261",
                borderRadius: "6px",
                padding: "1rem",
                color: "#94a3b8",
                fontSize: "0.75rem",
                whiteSpace: "pre-wrap",
                overflowX: "auto",
              }}
            >
              {this.state.errorInfo.componentStack}
            </div>
          )}
          <button
            onClick={() => window.location.reload()}
            style={{
              alignSelf: "flex-start",
              padding: "0.5rem 1rem",
              backgroundColor: "#3b82f6",
              color: "#fff",
              border: "none",
              borderRadius: "4px",
              cursor: "pointer",
              fontWeight: 600,
              fontSize: "0.875rem",
            }}
          >
            Reload UI
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
