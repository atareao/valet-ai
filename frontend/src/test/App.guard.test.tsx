import type { ReactNode } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import App from "../App";
import { useAuth } from "../contexts/AuthContext";
import type { AuthUser } from "../contexts/AuthContext";

/**
 * Contrato del guard de sesión de `App.tsx`:
 *   - mientras se resuelve `/api/auth/me` (`loading`) → `AppLoader`
 *     (`data-testid="auth-loading"`);
 *   - sin sesión → pantalla de login;
 *   - con sesión → app autenticada, cargada en diferido (`React.lazy` +
 *     `Suspense`), por lo que aparece tras resolverse el chunk
 *     (`data-testid="app-layout"`).
 *
 * `AuthenticatedApp` se mockea para no arrastrar antd real (ni su chunk lazy)
 * al test del guard.
 */
vi.mock("../contexts/AuthContext", () => ({
  useAuth: vi.fn(),
}));

vi.mock("../contexts/AuthProvider", () => ({
  AuthProvider: ({ children }: { children: ReactNode }) => children,
}));

vi.mock("../AuthenticatedApp", () => ({
  default: () => <div data-testid="app-layout">app</div>,
}));

vi.mock("../api/client", () => ({
  api: { getProfile: vi.fn().mockResolvedValue(null) },
}));

const mockUseAuth = vi.mocked(useAuth);

const authedUser: AuthUser = {
  sub: "u1",
  email: "a@b.c",
  name: "Lorenzo",
};

beforeEach(() => {
  vi.clearAllMocks();
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

describe("App — guard de sesión", () => {
  it("mientras /api/auth/me está en curso muestra carga y ni app ni login", () => {
    mockUseAuth.mockReturnValue({
      user: null,
      loading: true,
      loggingOut: false,
      logout: async () => undefined,
    });

    render(<App />);

    expect(screen.getByTestId("auth-loading")).toBeInTheDocument();
    expect(screen.queryByTestId("app-layout")).toBeNull();
    expect(
      screen.queryByRole("button", { name: /iniciar sesión/i }),
    ).toBeNull();
  });

  it("sin sesión muestra la pantalla de login y no la aplicación", () => {
    mockUseAuth.mockReturnValue({
      user: null,
      loading: false,
      loggingOut: false,
      logout: async () => undefined,
    });

    render(<App />);

    expect(
      screen.getByRole("button", { name: /iniciar sesión/i }),
    ).toBeInTheDocument();
    expect(screen.queryByTestId("app-layout")).toBeNull();
  });

  it("con sesión muestra la aplicación y no la pantalla de login", async () => {
    mockUseAuth.mockReturnValue({
      user: authedUser,
      loading: false,
      loggingOut: false,
      logout: async () => undefined,
    });

    render(<App />);

    expect(await screen.findByTestId("app-layout")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /iniciar sesión/i }),
    ).toBeNull();
  });
});
