import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { LoginPage } from "../components/LoginPage";

/**
 * Fase GREEN del change `oidc-auth`.
 *
 * Contrato (spec `frontend`, scenario "El control de login redirige a
 * /api/auth/login"): al activar el control, el navegador navega a
 * `/api/auth/login`.
 *
 * `LoginPage` es un placeholder sin `onClick`, así que la navegación no se
 * produce y el assert falla.
 */
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

describe("LoginPage", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    Object.defineProperty(window, "location", {
      configurable: true,
      writable: true,
      value: originalLocation,
    });
  });

  it("navega a /api/auth/login al activar el control de inicio de sesión", async () => {
    const user = userEvent.setup();
    const location = stubLocation();

    render(<LoginPage />);

    await user.click(
      screen.getByRole("button", { name: /iniciar sesión/i }),
    );

    expect(location.href).toBe("/api/auth/login");
  });
});
