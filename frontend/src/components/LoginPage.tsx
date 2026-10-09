import React from "react";
import valetIcon from "../assets/valet-icon.svg";

/**
 * Pantalla mostrada cuando no hay sesión válida. Muestra el logo real de Valet
 * sobre el fondo oscuro del layout e inicia el flujo OIDC navegando al endpoint
 * `login` del backend.
 *
 * Usa un `<button>` nativo (clase `.login-button` en `global.css`) para que la
 * pantalla de login no arrastre `antd` al bundle inicial.
 */
export const LoginPage: React.FC = () => {
  const handleLogin = () => {
    window.location.assign("/api/auth/login");
  };

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 24,
        minHeight: "100vh",
        // Replica `colorBgLayout` del tema (frontend/src/theme.ts).
        background: "#000000",
      }}
    >
      <img src={valetIcon} alt="Valet" width={120} height={120} />
      <button type="button" className="login-button" onClick={handleLogin}>
        Iniciar sesión
      </button>
    </div>
  );
};
