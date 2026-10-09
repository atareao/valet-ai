import { Component, type ErrorInfo, type ReactNode } from "react";
import { Alert } from "antd";

export interface WidgetErrorBoundaryProps {
  /** Nombre del widget, usado en el aviso de error. */
  name: string;
  children: ReactNode;
}

interface WidgetErrorBoundaryState {
  hasError: boolean;
}

/**
 * Aísla el renderizado de un widget: si su componente lanza una excepción,
 * muestra un `Alert` de error en lugar de tumbar todo el chat.
 */
export class WidgetErrorBoundary extends Component<
  WidgetErrorBoundaryProps,
  WidgetErrorBoundaryState
> {
  state: WidgetErrorBoundaryState = { hasError: false };

  static getDerivedStateFromError(): WidgetErrorBoundaryState {
    return { hasError: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error(
      "[WidgetErrorBoundary] Widget render failed:",
      this.props.name,
      error,
      info,
    );
  }

  render(): ReactNode {
    if (this.state.hasError) {
      return (
        <Alert
          type="error"
          showIcon
          message={`No se pudo renderizar el widget «${this.props.name}»`}
          style={{ marginTop: 8 }}
        />
      );
    }
    return this.props.children;
  }
}