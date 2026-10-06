import React from "react";
import { Button, Space, Typography } from "antd";

const { Title, Text } = Typography;

/**
 * Pantalla mostrada cuando no hay sesión válida. El control inicia el flujo
 * OIDC navegando al endpoint `login` del backend.
 */
export const LoginPage: React.FC = () => {
  const handleLogin = () => {
    window.location.assign("/api/auth/login");
  };

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        minHeight: "100vh",
      }}
    >
      <Space direction="vertical" align="center" size="large">
        <Title level={3} style={{ margin: 0 }}>
          💬 Valet
        </Title>
        <Text type="secondary">Inicia sesión para continuar</Text>
        <Button type="primary" size="large" onClick={handleLogin}>
          Iniciar sesión
        </Button>
      </Space>
    </div>
  );
};
