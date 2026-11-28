import { lazy, Suspense } from "react";
import { AppLoader } from "./AppLoader";
import { ErrorBoundary } from "./ErrorBoundary";

// El guard de sesión (AuthProvider + comprobación de `/api/auth/me` + login)
// se descarga con el mismo límite lazy que la app autenticada. Así el cliente
// HTTP compartido por ambas —y, con él, cualquier integración— queda fuera del
// grafo inicial: el arranque solo necesita React, el indicador de carga y el
// registro del service worker.
const App = lazy(() => import("../App"));

export function LazyApp() {
  return (
    <ErrorBoundary>
      <Suspense fallback={<AppLoader />}>
        <App />
      </Suspense>
    </ErrorBoundary>
  );
}
