import { BrowserRouter, Routes, Route } from "react-router-dom";
import { ConfigProvider, App as AntdApp, Spin } from "antd";
import { valetTheme } from "./theme";
import { AppLayout } from "./components/AppLayout";
import { LoginPage } from "./components/LoginPage";
import { ProfileProvider } from "./contexts/ProfileProvider";
import { AuthProvider } from "./contexts/AuthProvider";
import { useAuth } from "./contexts/AuthContext";

/**
 * Guard de sesión: mientras se resuelve `/api/auth/me` muestra carga; sin
 * sesión, la pantalla de login; con sesión, la aplicación autenticada.
 */
function AuthGate() {
  const { user, loading } = useAuth();

  if (loading) {
    return (
      <div
        data-testid="auth-loading"
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          minHeight: "100vh",
        }}
      >
        <Spin size="large" />
      </div>
    );
  }

  if (!user) {
    return <LoginPage />;
  }

  return (
    <ProfileProvider>
      <Routes>
        <Route path="*" element={<AppLayout />} />
      </Routes>
    </ProfileProvider>
  );
}

function App() {
  return (
    <BrowserRouter>
      <ConfigProvider theme={valetTheme}>
        <AntdApp>
          <AuthProvider>
            <AuthGate />
          </AuthProvider>
        </AntdApp>
      </ConfigProvider>
    </BrowserRouter>
  );
}

export default App;
