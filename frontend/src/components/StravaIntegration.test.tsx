import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App as AntdApp } from "antd";
import type { ReactNode } from "react";

// Se mockea parcialmente el cliente API (patrón de `useSkills.test.ts`): solo se
// sustituyen los endpoints de Strava, el resto queda intacto. Se accede a los
// spies con `vi.mocked` tras importar, evitando referencias en el factory
// hoisted.
vi.mock("../api/client", async () => {
  const actual =
    await vi.importActual<typeof import("../api/client")>("../api/client");
  return {
    ...actual,
    api: {
      ...actual.api,
      getStravaStatus: vi.fn(),
      disconnectStrava: vi.fn(),
      checkStrava: vi.fn(),
    },
  };
});

import { api } from "../api/client";
import { StravaIntegration } from "./StravaIntegration";
import type { StravaStatus } from "../types";

const mockGetStravaStatus = vi.mocked(api.getStravaStatus);
const mockDisconnectStrava = vi.mocked(api.disconnectStrava);
const mockCheckStrava = vi.mocked(api.checkStrava);

const disconnected: StravaStatus = {
  connected: false,
  athlete_id: null,
  athlete_name: null,
  scope: null,
};

const connected: StravaStatus = {
  connected: true,
  athlete_id: "12345",
  athlete_name: "Ana Corredora",
  scope: "read,activity:read_all",
};

// Settings con credenciales informadas y, a propósito, tokens sembrados: la
// interfaz no debe pintarlos jamás.
const settingsWithTokens: Record<string, string> = {
  strava_client_id: "99999",
  strava_client_secret: "shh-client-secret",
  strava_access_token: "SECRET_ACCESS_TOKEN",
  strava_refresh_token: "SECRET_REFRESH_TOKEN",
};

const AppWrapper = ({ children }: { children: ReactNode }) => (
  <AntdApp>{children}</AntdApp>
);

const renderSection = (
  settings: Record<string, string> | null = settingsWithTokens,
  updateSettings: (data: Record<string, string>) => Promise<void> = vi.fn(),
): void => {
  render(
    <StravaIntegration settings={settings} updateSettings={updateSettings} />,
    { wrapper: AppWrapper },
  );
};

describe("StravaIntegration", () => {
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

  it("muestra «No conectada» cuando no hay conexión", async () => {
    mockGetStravaStatus.mockResolvedValue(disconnected);

    renderSection();

    expect(await screen.findByText("No conectada")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Desconectar" }),
    ).not.toBeInTheDocument();
  });

  it("muestra «Conectada como <atleta>» cuando hay conexión", async () => {
    mockGetStravaStatus.mockResolvedValue(connected);

    renderSection();

    expect(
      await screen.findByText("Conectada como Ana Corredora"),
    ).toBeInTheDocument();
  });

  it("muestra un aviso cuando falla la consulta del estado", async () => {
    mockGetStravaStatus.mockRejectedValue(new Error("backend caído"));

    renderSection();

    expect(
      await screen.findByText("No se pudo consultar el estado de Strava"),
    ).toBeInTheDocument();
    expect(screen.getByText("backend caído")).toBeInTheDocument();
  });

  it("ofrece los campos client_id y client_secret y el enlace Connect with Strava", async () => {
    mockGetStravaStatus.mockResolvedValue(disconnected);

    renderSection();

    expect(screen.getByLabelText("Strava Client ID")).toBeInTheDocument();
    expect(screen.getByLabelText("Strava Client Secret")).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "Connect with Strava" }),
    ).toBeInTheDocument();
  });

  it("«Connect with Strava» es un enlace navegable al endpoint de autorización", async () => {
    mockGetStravaStatus.mockResolvedValue(disconnected);

    renderSection();

    await screen.findByText("No conectada");

    // Un `<a href>` nativo: se activa con teclado y no necesita JS.
    const link = screen.getByRole("link", { name: "Connect with Strava" });
    expect(link).toHaveAttribute("href", "/api/strava/authorize");
  });

  it("guarda las credenciales client_id y client_secret", async () => {
    const user = userEvent.setup();
    const updateSettings = vi.fn().mockResolvedValue(undefined);
    mockGetStravaStatus.mockResolvedValue(disconnected);

    renderSection(null, updateSettings);

    await user.type(screen.getByLabelText("Strava Client ID"), "12345");
    await user.type(screen.getByLabelText("Strava Client Secret"), "top-secret");

    await user.click(screen.getByRole("button", { name: "Guardar credenciales" }));

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledWith({
        strava_client_id: "12345",
        strava_client_secret: "top-secret",
      });
    });
  });

  it("«Desconectar» llama al endpoint y refresca el estado", async () => {
    const user = userEvent.setup();
    mockGetStravaStatus
      .mockResolvedValueOnce(connected)
      .mockResolvedValueOnce(disconnected);
    mockDisconnectStrava.mockResolvedValue({ connected: false });

    renderSection();

    await screen.findByText("Conectada como Ana Corredora");

    await user.click(screen.getByRole("button", { name: "Desconectar" }));

    await waitFor(() => {
      expect(mockDisconnectStrava).toHaveBeenCalledTimes(1);
    });
    // Relectura del estado: la primera es la de montaje, la segunda el refresco.
    await waitFor(() => {
      expect(mockGetStravaStatus).toHaveBeenCalledTimes(2);
    });
    expect(await screen.findByText("No conectada")).toBeInTheDocument();
  });

  it("no confunde un fallo del refresco con un fallo al desconectar", async () => {
    const user = userEvent.setup();
    mockGetStravaStatus
      .mockResolvedValueOnce(connected) // montaje
      .mockRejectedValueOnce(new Error("refresco caído")); // relectura tras desconectar
    mockDisconnectStrava.mockResolvedValue({ connected: false });

    renderSection();

    await screen.findByText("Conectada como Ana Corredora");

    await user.click(screen.getByRole("button", { name: "Desconectar" }));

    await waitFor(() => {
      expect(mockDisconnectStrava).toHaveBeenCalledTimes(1);
    });
    // El fallo del refresco no se etiqueta como fallo de la desconexión…
    await waitFor(() => {
      expect(
        screen.queryByText("Error al desconectar la cuenta de Strava"),
      ).not.toBeInTheDocument();
    });
    // …ni deja la UI mostrando «Conectada».
    expect(screen.queryByText(/Conectada/)).not.toBeInTheDocument();
  });

  it("«Probar conexión» consulta /strava/check y confirma con el atleta", async () => {
    const user = userEvent.setup();
    mockGetStravaStatus.mockResolvedValue(connected);
    mockCheckStrava.mockResolvedValue({
      ok: true,
      athlete_id: "12345",
      athlete_name: "Ana Corredora",
      error: null,
    });

    renderSection();

    await screen.findByText("Conectada como Ana Corredora");

    await user.click(screen.getByRole("button", { name: "Probar conexión" }));

    await waitFor(() => {
      expect(mockCheckStrava).toHaveBeenCalledTimes(1);
    });
    expect(
      await screen.findByText("Conexión con Strava correcta"),
    ).toBeInTheDocument();
    expect(screen.getByText("Atleta: Ana Corredora")).toBeInTheDocument();
  });

  it("con ok:false muestra el error del servidor y no cambia el estado", async () => {
    const user = userEvent.setup();
    mockGetStravaStatus.mockResolvedValue(connected);
    mockCheckStrava.mockResolvedValue({
      ok: false,
      athlete_id: null,
      athlete_name: null,
      error: "El token de Strava caducó, vuelve a conectar",
    });

    renderSection();

    await screen.findByText("Conectada como Ana Corredora");

    await user.click(screen.getByRole("button", { name: "Probar conexión" }));

    expect(
      await screen.findByText("El token de Strava caducó, vuelve a conectar"),
    ).toBeInTheDocument();
    // El estado de la conexión no cambia a desconectado por un fallo del sondeo.
    expect(screen.getByText("Conectada como Ana Corredora")).toBeInTheDocument();
    expect(screen.queryByText("No conectada")).not.toBeInTheDocument();
  });

  it("muestra el scope concedido cuando la cuenta está conectada", async () => {
    mockGetStravaStatus.mockResolvedValue(connected);

    renderSection();

    expect(
      await screen.findByText(/Permisos concedidos: read,activity:read_all/),
    ).toBeInTheDocument();
  });

  it("avisa de volver a conectar cuando falta activity:read_all", async () => {
    mockGetStravaStatus.mockResolvedValue({ ...connected, scope: "read" });

    renderSection();

    expect(await screen.findByText(/vuelve a conectar/i)).toBeInTheDocument();
  });

  it("no avisa de reconectar cuando el scope incluye activity:read_all", async () => {
    mockGetStravaStatus.mockResolvedValue(connected);

    renderSection();

    await screen.findByText(/Permisos concedidos: read,activity:read_all/);

    expect(screen.queryByText(/vuelve a conectar/i)).not.toBeInTheDocument();
  });

  it("muestra el aviso cuando la desconexión no confirma la revocación", async () => {
    const user = userEvent.setup();
    mockGetStravaStatus
      .mockResolvedValueOnce(connected)
      .mockResolvedValueOnce(disconnected);
    mockDisconnectStrava.mockResolvedValue({
      connected: false,
      warning:
        "no se pudo confirmar la revocación del acceso en Strava; retíralo también desde https://www.strava.com/settings/apps",
    });

    renderSection();

    await screen.findByText("Conectada como Ana Corredora");

    await user.click(screen.getByRole("button", { name: "Desconectar" }));

    expect(
      await screen.findByText(/https:\/\/www\.strava\.com\/settings\/apps/),
    ).toBeInTheDocument();
  });

  it("no ofrece «Probar conexión» cuando no hay conexión", async () => {
    mockGetStravaStatus.mockResolvedValue(disconnected);

    renderSection();

    await screen.findByText("No conectada");

    expect(
      screen.queryByRole("button", { name: "Probar conexión" }),
    ).not.toBeInTheDocument();
  });

  it("un sondeo que rechaza deja de cargar y muestra feedback al usuario", async () => {
    const user = userEvent.setup();
    mockGetStravaStatus.mockResolvedValue(connected);
    mockCheckStrava.mockRejectedValue(new Error("fallo de red"));

    renderSection();

    await screen.findByText("Conectada como Ana Corredora");

    const button = screen.getByRole("button", { name: "Probar conexión" });
    await user.click(button);

    // El fallo de la petición se traduce en un aviso legible, no en un error crudo.
    expect(
      await screen.findByText("La comprobación con Strava ha fallado"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("No se pudo comprobar la conexión con Strava"),
    ).toBeInTheDocument();
    // `checking` vuelve a false: el botón no se queda colgado en estado de carga.
    expect(button).not.toHaveClass("ant-btn-loading");
  });

  it("nunca renderiza los tokens, ni conectada ni desconectada", async () => {
    mockGetStravaStatus.mockResolvedValue(connected);

    renderSection();

    await screen.findByText("Conectada como Ana Corredora");

    const body = document.body.textContent ?? "";
    expect(body).not.toContain("SECRET_ACCESS_TOKEN");
    expect(body).not.toContain("SECRET_REFRESH_TOKEN");
    expect(screen.queryByText("SECRET_ACCESS_TOKEN")).not.toBeInTheDocument();
    expect(screen.queryByText("SECRET_REFRESH_TOKEN")).not.toBeInTheDocument();
    // `queryByText`/`textContent` no ven el `value` de un `<input>`: hay que
    // comprobarlo explícitamente (incluido el `<Input.Password>`).
    expect(screen.queryByDisplayValue(/SECRET_/)).toBeNull();
    const inputValues = Array.from(
      document.querySelectorAll<HTMLInputElement>("input"),
    ).map((input) => input.value);
    expect(inputValues.some((value) => value.includes("SECRET_"))).toBe(false);
  });
});
