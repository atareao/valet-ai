import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AuthProvider } from "../contexts/AuthProvider";
import { useAuth } from "../contexts/AuthContext";

/**
 * Fase GREEN del change `oidc-auth`.
 *
 * Destino del logout (spec scenario "Logout local y SSO desde el header"):
 * - se sigue `end_session_url` solo si es una URL absoluta `https:`;
 * - `null` o una URL insegura (`http:`) caen a la raíz `/` de la app
 *   (anti open redirect);
 * - en todos los casos la sesión local deja de estar disponible.
 *
 * Regresión `base: ''`: con `vite.config.ts` en `base: ''`, la `BASE_URL` de
 * Vite resuelve a cadena VACÍA, así que el fallback debe forzar `/` para no
 * recargar la URL actual. Los asserts contra `"/"` fijan esa regresión.
 */
function jsonResponse(body: unknown, status = 200): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: status === 200 ? "OK" : "",
    json: async () => body,
  } as unknown as Response;
}

function LogoutProbe() {
  const { user, loading, loggingOut, logout } = useAuth();
  return (
    <div>
      <span data-testid="loading">{String(loading)}</span>
      <span data-testid="loggingOut">{String(loggingOut)}</span>
      <span data-testid="email">{user?.email ?? ""}</span>
      <button onClick={() => void logout()}>salir</button>
    </div>
  );
}

const originalLocation = window.location;

function stubLocation() {
  const location = {
    href: "",
    assign: vi.fn((url: string) => {
      location.href = url;
    }),
    replace: vi.fn((url: string) => {
      location.href = url;
    }),
  };
  Object.defineProperty(window, "location", {
    configurable: true,
    writable: true,
    value: location,
  });
  return location;
}

function stubFetchWithLogout(endSessionUrl: string | null) {
  const fetchMock = vi.fn(async (url: RequestInfo | URL): Promise<Response> => {
    const target = String(url);
    if (target.includes("/api/auth/logout")) {
      return jsonResponse({ end_session_url: endSessionUrl });
    }
    return jsonResponse({ sub: "u1", email: "a@b.c", name: "Lorenzo" });
  });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  Object.defineProperty(window, "location", {
    configurable: true,
    writable: true,
    value: originalLocation,
  });
});

describe("AuthProvider — destino del logout", () => {
  it("con end_session_url https navega al proveedor y deja user a null", async () => {
    const location = stubLocation();
    stubFetchWithLogout("https://idp.example/logout");
    const user = userEvent.setup();

    render(
      <AuthProvider>
        <LogoutProbe />
      </AuthProvider>,
    );

    await waitFor(() =>
      expect(screen.getByTestId("email")).toHaveTextContent("a@b.c"),
    );

    await user.click(screen.getByRole("button", { name: /salir/i }));

    await waitFor(() =>
      expect(location.href).toBe("https://idp.example/logout"),
    );
    expect(screen.getByTestId("email")).toHaveTextContent("");
  });

  it("con end_session_url null cae a la raíz / de la app (solo cierre local)", async () => {
    const location = stubLocation();
    stubFetchWithLogout(null);
    const user = userEvent.setup();

    render(
      <AuthProvider>
        <LogoutProbe />
      </AuthProvider>,
    );

    await waitFor(() =>
      expect(screen.getByTestId("email")).toHaveTextContent("a@b.c"),
    );

    await user.click(screen.getByRole("button", { name: /salir/i }));

    // Fija la regresión de `base: ''`: con una base relativa (`""` o `"./"`,
    // según cómo la normalice Vite) la normalización de `APP_BASE_URL` lleva a
    // la raíz absoluta `/`, nunca a la cadena vacía ni a una ruta relativa.
    await waitFor(() => expect(location.href).toBe("/"));
    expect(location.href).not.toBe("");
    expect(location.href).not.toBe("https://idp.example/logout");
    expect(screen.getByTestId("email")).toHaveTextContent("");
  });

  it("con una URL no https (http) no navega a ella y usa el fallback", async () => {
    const location = stubLocation();
    stubFetchWithLogout("http://evil.example/logout");
    const user = userEvent.setup();

    render(
      <AuthProvider>
        <LogoutProbe />
      </AuthProvider>,
    );

    await waitFor(() =>
      expect(screen.getByTestId("email")).toHaveTextContent("a@b.c"),
    );

    await user.click(screen.getByRole("button", { name: /salir/i }));

    // Fija la regresión de `base: ''`: con una base relativa (`""` o `"./"`,
    // según cómo la normalice Vite) la normalización de `APP_BASE_URL` lleva a
    // la raíz absoluta `/`, nunca a la cadena vacía ni a una ruta relativa.
    await waitFor(() => expect(location.href).toBe("/"));
    expect(location.href).not.toBe("");
    expect(location.href).not.toContain("evil.example");
  });
});
