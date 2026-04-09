import React from 'react';
import { Button } from './Button';

interface Props {
  children:  React.ReactNode;
  fallback?: React.ReactNode;
}

interface State {
  hasError: boolean;
  error:    Error | null;
}

export class ErrorBoundary extends React.Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    // In production, forward to an error-reporting service here.
    console.error('[ErrorBoundary]', error, info);
  }

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) return this.props.fallback;
      return (
        <div className="flex flex-col items-center justify-center min-h-[40vh] gap-4 p-8 text-center">
          <div className="text-red-500 text-4xl">⚠</div>
          <h2 className="text-lg font-semibold text-slate-800">
            Something went wrong
          </h2>
          <p className="text-sm text-slate-500 max-w-sm">
            {this.state.error?.message ?? 'An unexpected error occurred.'}
          </p>
          <Button
            onClick={() => {
              this.setState({ hasError: false, error: null });
              window.location.reload();
            }}
          >
            Reload page
          </Button>
        </div>
      );
    }
    return this.props.children;
  }
}
