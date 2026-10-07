import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { LoginPage } from "../components/LoginPage";

/**
 * Contrato (changes `oidc-auth` + `login-dark-ux`, spec `frontend`):
 *
 * - Al activar el control de login, el navegador navega a `/api/auth/login`.
 * - La pantalla ocupa toda la ventana con fondo oscuro (`#000000`), muestra el
 *   logo real de Valet (`valet-icon.svg`) a 120 px y no usa el emoji `💬`, ni
 *   título, ni el texto "Inicia sesión para continuar".
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

/**
 * Lee una dimensión (`width`/`height`) de un elemento aceptando tanto el
 * atributo HTML como el estilo en línea, para no acoplarse a una sola forma
 * de renderizar el logo.
 */
function readDimension(
  el: HTMLElement,
  prop: "width" | "height",
): number | null {
  const raw = el.getAttribute(prop) ?? el.style[prop];
  if (raw === null || raw === undefined || raw === "") return null;
  const parsed = Number.parseInt(raw, 10);
  return Number.isNaN(parsed) ? null : parsed;
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

  // ════════════════════════════════════════════════════════════════
  // change `login-dark-ux`: fondo oscuro y logo real (implementado)
  // ════════════════════════════════════════════════════════════════

  it("muestra el logo de Valet como imagen y NO el emoji, título ni texto", () => {
    const { container } = render(<LoginPage />);

    const logo = container.querySelector("img[src*='valet-icon']");
    expect(logo).not.toBeNull();
    expect(logo!.getAttribute("alt")).toBeTruthy();

    expect(screen.queryByText(/💬/)).toBeNull();
    expect(screen.queryByText(/inicia sesión para continuar/i)).toBeNull();
    expect(screen.queryAllByRole("heading")).toHaveLength(0);
  });

  it("renderiza el logo de Valet a 120 px de ancho y alto", () => {
    const { container } = render(<LoginPage />);

    const logo = container.querySelector(
      "img[src*='valet-icon']",
    ) as HTMLImageElement | null;
    expect(logo).not.toBeNull();

    expect(readDimension(logo!, "width")).toBe(120);
    expect(readDimension(logo!, "height")).toBe(120);
  });

  it("usa un fondo oscuro a pantalla completa en el contenedor raíz", () => {
    const { container } = render(<LoginPage />);

    const root = container.firstElementChild as HTMLElement;
    expect(root).not.toBeNull();

    expect(root.style.minHeight).toBe("100vh");
    expect(root.style.background).toMatch(/#000000|rgb\(0,\s*0,\s*0\)/);
  });
});
