import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import App from "../App";
import { UNAUTHORIZED_EVENT } from "../api/client";

/**
 * Fase GREEN del change `oidc-auth`.
 *
 * Cadena completa del requirement "Un 401 global redirige a login": un evento
 * `valet:unauthorized` (el que emite `api/client` y `useSSE` ante un 401) debe
 * llegar a `AuthProvider`, dejar `user` a `null` y hacer que el guard de
 * `App.tsx` muestre la pantalla de login en lugar del contenido autenticado.
 */
vi.mock("../components/AppLayout", () => ({
  AppLayout: () => <div data-testid="app-layout">app</div>,
}));

beforeEach(() => {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("cadena 401 → valet:unauthorized → AuthProvider → LoginPage", () => {
  // Timeout holgado en el propio `it`: este es el único test que monta la app
  // real (`App`) con antd, cuyo arranque en jsdom excede el 1 s por defecto.
  // No se tocan los timeouts globales para no enmascarar cuelgues en el resto.
  it("pasa a no autenticado y muestra la pantalla de login", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: RequestInfo | URL): Promise<Response> => {
        const target = String(url);
        if (target.includes("/api/auth/me")) {
          return {
            ok: true,
            status: 200,
            json: async () => ({ sub: "u1", email: "a@b.c", name: "Lorenzo" }),
          } as unknown as Response;
        }
        return {
          ok: true,
          status: 200,
          json: async () => ({}),
        } as unknown as Response;
      }),
    );

    render(<App />);

    // Sesión resuelta: se muestra la aplicación. `findByTestId` con timeout
    // propio para absorber la carga diferida del `Suspense` (app autenticada).
    expect(
      await screen.findByTestId("app-layout", undefined, { timeout: 10000 }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /iniciar sesión/i }),
    ).toBeNull();

    // Un 401 en cualquier petición dispara el evento global.
    await act(async () => {
      window.dispatchEvent(new Event(UNAUTHORIZED_EVENT));
    });

    await waitFor(
      () =>
        expect(
          screen.getByRole("button", { name: /iniciar sesión/i }),
        ).toBeInTheDocument(),
      { timeout: 10000 },
    );
    expect(screen.queryByTestId("app-layout")).toBeNull();
  }, 15000);
});
