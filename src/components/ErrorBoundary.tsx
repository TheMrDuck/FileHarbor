import { Component, ErrorInfo, ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
}

export default class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null, errorInfo: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error, errorInfo: null };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error("Uncaught error caught by ErrorBoundary:", error, errorInfo);
    this.setState({ error, errorInfo });
  }

  render() {
    if (this.state.hasError) {
      return (
        <div
          style={{
            padding: "40px",
            color: "var(--text)",
            background: "var(--bg)",
            minHeight: "100vh",
            boxSizing: "border-box",
          }}
        >
          <h2 style={{ color: "var(--danger)", marginTop: 0 }}>Something went wrong</h2>
          <p style={{ color: "var(--text-dim)", marginBottom: "16px" }}>
            An unexpected error occurred while rendering the application:
          </p>
          <pre
            className="mono"
            style={{
              background: "var(--bg-card)",
              padding: "16px",
              borderRadius: "var(--radius)",
              border: "1px solid var(--border)",
              overflow: "auto",
              color: "var(--danger)",
              fontSize: "13px",
              lineHeight: "1.4",
            }}
          >
            {this.state.error?.toString()}
            {"\n\n"}
            {this.state.errorInfo?.componentStack}
          </pre>
          <button
            onClick={() => window.location.reload()}
            className="btn btn-primary"
            style={{ marginTop: "20px" }}
          >
            Reload Application
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
