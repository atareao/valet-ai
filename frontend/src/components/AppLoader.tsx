import React from "react";

/**
 * Indicador de carga nativo (sin antd) del bundle inicial: se muestra mientras
 * se resuelve la sesión y como fallback del Suspense de la app autenticada.
 */
export const AppLoader: React.FC = () => (
  <div
    data-testid="auth-loading"
    role="status"
    aria-label="Cargando"
    style={{
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      minHeight: "100vh",
      background: "#000000",
    }}
  >
    {/* Texto para lectores de pantalla: `role="status"` con solo `aria-label`
        no lo anuncian todos los lectores, así que se incluye contenido real
        (visualmente oculto) dentro de la región live. */}
    <span className="visually-hidden">Cargando…</span>
    <span className="valet-spinner" aria-hidden="true" />
  </div>
);
