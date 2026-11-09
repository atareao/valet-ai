import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, act, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App as AntdApp, Form } from "antd";
import type { ReactNode } from "react";

// Se mockea solo el módulo API de GitHub (no `fetch`): el hook `useGithub` corre
// de verdad. Los tres endpoints viven en `api/github.ts`, siguiendo el patrón de
// `useBlog.test.ts`/`BlogSection.test.tsx`.
const mockApi = vi.hoisted(() => ({
  getGithubStatus: vi.fn(),
  checkGithub: vi.fn(),
  putGithubToken: vi.fn(),
}));

vi.mock("../api/github", () => ({
  getGithubStatus: mockApi.getGithubStatus,
  checkGithub: mockApi.checkGithub,
  putGithubToken: mockApi.putGithubToken,
}));

import { GithubSection } from "./GithubSection";
import type { GithubCheckResult, GithubStatus } from "../types";

const mockGetGithubStatus = vi.mocked(mockApi.getGithubStatus);
const mockCheckGithub = vi.mocked(mockApi.checkGithub);
const mockPutGithubToken = vi.mocked(mockApi.putGithubToken);

const notConfigured: GithubStatus = {
  configured: false,
  default_repo: "atareao/valet-ai",
};

const configured: GithubStatus = {
  configured: true,
  default_repo: "atareao/valet-ai",
};

// Los campos no sensibles cuelgan del formulario de ajustes del padre: la
// sección se monta dentro de un `Form` real, como en el diálogo.
const Wrapper = ({ children }: { children: ReactNode }) => (
  <AntdApp>
    <Form>{children}</Form>
  </AntdApp>
);

const renderSection = (): void => {
  render(<GithubSection />, { wrapper: Wrapper });
};

// Réplica del contrato del diálogo de ajustes: la sección cuelga del `Form`
// compartido y su botón «Guardar» (submit) entrega todos los valores no
// sensibles a `onFinish`, que es el cuerpo que viaja por `PUT /api/settings`.
const renderSectionWithSubmit = (
  onFinish: (values: Record<string, string>) => void,
): void => {
  render(
    <AntdApp>
      <Form onFinish={onFinish}>
        <GithubSection />
      </Form>
    </AntdApp>,
  );
};

describe("GithubSection", () => {
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
    mockGetGithubStatus.mockResolvedValue(notConfigured);
    mockPutGithubToken.mockResolvedValue(undefined);
    mockCheckGithub.mockResolvedValue({
      ok: true,
      detail: "conexión correcta",
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("muestra los campos repo/labels y el estado de configuración", async () => {
    mockGetGithubStatus.mockResolvedValue(configured);

    renderSection();

    expect(await screen.findByText("Configurado")).toBeInTheDocument();

    expect(screen.getByLabelText("GitHub · Repositorio por defecto")).toHaveAttribute(
      "placeholder",
      "atareao/valet-ai",
    );
    expect(screen.getByLabelText("GitHub · Labels por defecto")).toHaveAttribute(
      "placeholder",
      "feature, valet",
    );
  });

  it("la línea de estado refleja el estado del endpoint sin exponer el token", async () => {
    const user = userEvent.setup();
    mockGetGithubStatus.mockResolvedValue(configured);

    renderSection();

    expect(await screen.findByText("Configurado")).toBeInTheDocument();

    const region = screen.getByRole("region", { name: "GitHub" });
    // El token nunca se pinta como texto: ni el guardado (el status jamás lo
    // trae) ni el que se teclea en su campo `type=password`.
    await user.type(screen.getByLabelText("GitHub · Token"), "ghp_token-de-prueba");
    expect(within(region).queryByText(/ghp_token-de-prueba/)).toBeNull();
  });

  it("el repo y los labels viajan en el guardado de los ajustes no sensibles", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();

    renderSectionWithSubmit(onSave);
    await screen.findByText("Sin configurar");

    await user.type(
      screen.getByLabelText("GitHub · Repositorio por defecto"),
      "atareao/valet-ai",
    );
    await user.type(
      screen.getByLabelText("GitHub · Labels por defecto"),
      "feature, valet",
    );

    await user.click(screen.getByRole("button", { name: "Guardar" }));

    // El cuerpo del submit —lo que el diálogo manda por `PUT /api/settings`—
    // incluye los dos ajustes no sensibles y NUNCA el token.
    await waitFor(() => {
      expect(onSave).toHaveBeenCalledWith(
        expect.objectContaining({
          github_default_repo: "atareao/valet-ai",
          github_default_labels: "feature, valet",
        }),
      );
    });
    expect(onSave).toHaveBeenCalledWith(
      expect.not.objectContaining({ github_token: expect.anything() }),
    );
  });

  it("el campo del token arranca siempre vacío aunque ya haya uno guardado", async () => {
    mockGetGithubStatus.mockResolvedValue(configured);

    renderSection();

    await screen.findByText("Configurado");

    const token = screen.getByLabelText("GitHub · Token");
    expect(token).toHaveAttribute("type", "password");
    // Nunca se relee el valor guardado: el campo nace vacío.
    expect((token as HTMLInputElement).value).toBe("");
  });

  it("guardar el token usa su endpoint dedicado y limpia el campo", async () => {
    const user = userEvent.setup();

    renderSection();
    await screen.findByText("Sin configurar");

    const token = screen.getByLabelText("GitHub · Token");
    await user.type(token, "ghp_token-nuevo");
    await user.click(screen.getByRole("button", { name: "Guardar token" }));

    await waitFor(() => {
      expect(mockPutGithubToken).toHaveBeenCalledWith("ghp_token-nuevo");
    });
    // El token guardado no se relee: el campo se limpia tras guardar.
    await waitFor(() => {
      expect((token as HTMLInputElement).value).toBe("");
    });
  });

  it("un token vacío no se envía", async () => {
    const user = userEvent.setup();

    renderSection();
    await screen.findByText("Sin configurar");

    await user.click(screen.getByRole("button", { name: "Guardar token" }));

    expect(mockPutGithubToken).not.toHaveBeenCalled();
  });

  it("«Probar conexión» llama a check y muestra el detail del veredicto", async () => {
    const user = userEvent.setup();
    const result: GithubCheckResult = {
      ok: false,
      detail: "el token fue rechazado",
    };
    mockCheckGithub.mockResolvedValue(result);

    renderSection();
    await screen.findByText("Sin configurar");

    await user.click(screen.getByRole("button", { name: "Probar conexión" }));

    expect(mockCheckGithub).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("el token fue rechazado")).toBeInTheDocument();
  });

  it("«Probar conexión» cae al mensaje de fallback si el sondeo rechaza", async () => {
    const user = userEvent.setup();
    // Un rechazo del propio `GET /api/github/check` (p. ej. la red cae): la
    // ruta responde siempre `200`, así que un rechazo es un fallo de transporte
    // y la sección publica su propio `detail` de fallback en la línea de estado.
    mockCheckGithub.mockRejectedValue(new Error("red caída"));

    renderSection();
    await screen.findByText("Sin configurar");

    await user.click(screen.getByRole("button", { name: "Probar conexión" }));

    expect(mockCheckGithub).toHaveBeenCalledTimes(1);
    const region = screen.getByRole("region", { name: "GitHub" });
    expect(
      await within(region).findByText(
        "No se pudo comprobar la conexión con GitHub",
      ),
    ).toBeInTheDocument();
  });

  it("muestra el indicador de carga mientras el estado aún no ha llegado", async () => {
    // Lectura diferida: el estado sigue en vuelo y la sección debe mostrar la
    // región de carga (`loading && !status`), no un estado falso.
    let resolveStatus!: (value: GithubStatus) => void;
    mockGetGithubStatus.mockReturnValue(
      new Promise<GithubStatus>((resolve) => {
        resolveStatus = resolve;
      }),
    );

    renderSection();

    const loadingRegion = screen.getByRole("status", {
      name: "Cargando estado de GitHub",
    });
    expect(loadingRegion).toBeInTheDocument();
    expect(loadingRegion.querySelector(".ant-spin")).not.toBeNull();

    await act(async () => {
      resolveStatus(notConfigured);
    });

    expect(await screen.findByText("Sin configurar")).toBeInTheDocument();
    expect(
      screen.queryByRole("status", { name: "Cargando estado de GitHub" }),
    ).not.toBeInTheDocument();
  });

  it("muestra «Sin datos» cuando la lectura del estado falla, no «Sin configurar»", async () => {
    // La lectura de `GET /api/github/status` rechaza: el panel y la etiqueta
    // deben coincidir en «Sin datos» (no lo sé), nunca en «Sin configurar».
    mockGetGithubStatus.mockRejectedValue(new Error("GitHub caído"));

    renderSection();

    const region = screen.getByRole("region", { name: "GitHub" });
    expect(await within(region).findByText("Sin datos")).toBeInTheDocument();
    expect(within(region).queryByText("Sin configurar")).toBeNull();
  });

  it("no ofrece operaciones de listado ni búsqueda de issues", async () => {
    renderSection();
    await screen.findByText("Sin configurar");

    // Solo gestiona el token, el repositorio por defecto y los labels por
    // defecto: ningún botón de listar/buscar issues ni campo de búsqueda.
    for (const name of ["Listar issues", "Buscar issues", "Ver issues"]) {
      expect(screen.queryByRole("button", { name })).toBeNull();
    }
    expect(screen.queryByRole("textbox", { name: /buscar/i })).toBeNull();
  });
});
