import {
  describe,
  it,
  expect,
  vi,
  beforeEach,
  afterEach,
  type MockInstance,
} from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ErrorBoundary } from "../components/ErrorBoundary";

/**
 * Contrato (change `lazy-app-bundle`, spec `frontend`):
 *
 *   Scenario: Un fallo al cargar un chunk diferido no deja la app en blanco
 *   - Given la aplicación en ejecución con la sesión resuelta
 *   - When falla la carga de un chunk diferido (un 404 tras un despliegue)
 *   - Then se muestra un aviso recuperable con una acción de recarga
 *   - And la aplicación no queda desmontada en una pantalla en blanco
 *
 * `ErrorBoundary` captura el rechazo de la promesa de `React.lazy` y en su
 * lugar muestra un aviso con un botón "Recargar" que reejecuta la carga.
 */

const originalLocation = window.location;

/** Stub de `window.location` con `reload` (jsdom no lo implementa). */
function stubLocation() {
  const location = {
    href: "",
    assign: vi.fn((url: string) => {
      location.href = url;
    }),
    replace: vi.fn((url: string) => {
      location.href = url;
    }),
    reload: vi.fn(),
  };
  Object.defineProperty(window, "location", {
    configurable: true,
    writable: true,
    value: location,
  });
  return location;
}

/** Hijo que revienta al renderizar, para forzar la captura del error. */
function Boom(): JSX.Element {
  throw new Error("boom");
}

describe("ErrorBoundary", () => {
  let consoleError: MockInstance;

  beforeEach(() => {
    // React emite `console.error` al capturar el error de un componente; se
    // silencia para no ensuciar la salida y se restaura en `afterEach`.
    consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => undefined);
  });

  afterEach(() => {
    consoleError.mockRestore();
    Object.defineProperty(window, "location", {
      configurable: true,
      writable: true,
      value: originalLocation,
    });
  });

  it("sin error, renderiza los hijos", () => {
    render(
      <ErrorBoundary>
        <div data-testid="child">contenido</div>
      </ErrorBoundary>,
    );

    expect(screen.getByTestId("child")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("con un hijo que lanza, muestra un aviso recuperable con acción de recarga", () => {
    render(
      <ErrorBoundary>
        <Boom />
      </ErrorBoundary>,
    );

    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /recargar/i }),
    ).toBeInTheDocument();
  });

  it("al pulsar Recargar se recarga la página", async () => {
    const user = userEvent.setup();
    const location = stubLocation();

    render(
      <ErrorBoundary>
        <Boom />
      </ErrorBoundary>,
    );

    await user.click(screen.getByRole("button", { name: /recargar/i }));

    expect(location.reload).toHaveBeenCalledTimes(1);
  });
});
