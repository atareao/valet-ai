import { lazy, Suspense } from "react";
import { AppLoader } from "./components/AppLoader";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { LoginPage } from "./components/LoginPage";
import { AuthProvider } from "./contexts/AuthProvider";
import { useAuth } from "./contexts/AuthContext";

const AuthenticatedApp = lazy(() => import("./AuthenticatedApp"));

/**
 * Guard de sesión: mientras se resuelve `/api/auth/me` muestra carga; sin
 * sesión, la pantalla de login; con sesión, la aplicación autenticada.
 *
 * La app autenticada (antd + router + vistas) se carga con `React.lazy` para
 * que nada de ello entre en el bundle inicial.
 */
function AuthGate() {
  const { user, loading } = useAuth();

  if (loading) return <AppLoader />;
  if (!user) return <LoginPage />;

  return (
    // `ErrorBoundary` envuelve al `Suspense`: si falla la descarga del chunk de
    // la app autenticada (`React.lazy` rechaza), se muestra un aviso recuperable
    // en lugar de dejar la pantalla en blanco.
    <ErrorBoundary>
      <Suspense fallback={<AppLoader />}>
        <AuthenticatedApp />
      </Suspense>
    </ErrorBoundary>
  );
}

function App() {
  return (
    <AuthProvider>
      <AuthGate />
    </AuthProvider>
  );
}

export default App;
