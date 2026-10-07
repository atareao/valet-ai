import React from "react";

interface ErrorBoundaryProps {
  children: React.ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
}

/**
 * Frontera de error del bundle inicial (sin antd).
 *
 * `React.lazy` rechaza su promesa cuando falla la descarga de un chunk diferido
 * (p. ej. un 404 tras un despliegue con hashes nuevos). Sin una frontera, ese
 * rechazo desmonta el árbol y deja la pantalla en blanco. Aquí se captura y se
 * ofrece una acción de recarga para recuperar la aplicación.
 *
 * Es el único componente de clase del proyecto: React solo expone la captura de
 * errores de renderizado (`getDerivedStateFromError`) a las clases.
 */
export class ErrorBoundary extends React.Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  state: ErrorBoundaryState = { hasError: false };

  static getDerivedStateFromError(): ErrorBoundaryState {
    return { hasError: true };
  }

  componentDidCatch(error: unknown, info: React.ErrorInfo): void {
    // Sin antd ni servicios de telemetría en el bundle inicial: se registra en
    // consola para no perder el error.
    console.error("ErrorBoundary capturó un error:", error, info);
  }

  render(): React.ReactNode {
    if (!this.state.hasError) return this.props.children;

    return (
      <div className="error-boundary" role="alert">
        <p className="error-boundary__message">
          Algo ha ido mal al cargar la aplicación.
        </p>
        <button
          type="button"
          className="login-button"
          onClick={() => window.location.reload()}
        >
          Recargar
        </button>
      </div>
    );
  }
}
