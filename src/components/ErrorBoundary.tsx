import { Component } from "react";
import type { ReactNode, ErrorInfo } from "react";

interface Props {
  children: ReactNode;
  /** Custom fallback rendered instead of the default error UI. */
  fallback?: ReactNode;
}

interface State {
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(
      "[ErrorBoundary] Uncaught render error:",
      error,
      info.componentStack,
    );
  }

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    if (this.props.fallback) return this.props.fallback;

    return (
      <div className="h-full flex items-center justify-center p-8">
        <div className="max-w-lg w-full rounded-lg border border-destructive/50 bg-destructive/5 p-6">
          <p className="text-sm font-semibold text-destructive mb-2">
            Something went wrong
          </p>
          <pre className="text-xs text-muted-foreground whitespace-pre-wrap break-all font-mono">
            {error.message}
          </pre>
        </div>
      </div>
    );
  }
}
