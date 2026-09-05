import { Component, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
  info: string | null;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null, info: null };

  static getDerivedStateFromError(error: Error) {
    return { error, info: null };
  }

  componentDidCatch(error: Error, info: { componentStack: string }) {
    this.setState({ error, info: info.componentStack });
    console.error("Boundary caught:", error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <div style={{ padding: 24, fontFamily: "monospace", whiteSpace: "pre-wrap", color: "#f0685a", background: "#17171c" }}>
          <h2>{this.state.error.message}</h2>
          <div>{this.state.error.stack}</div>
          <hr />
          <div>{this.state.info}</div>
        </div>
      );
    }
    return this.props.children;
  }
}
