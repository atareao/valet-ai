import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, act, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App as AntdApp, Form } from "antd";
import type { ReactNode } from "react";

// Se mockea solo el módulo de API de OxiMeal: el hook `useOximeal` corre de
// verdad. Los tres endpoints viven en su propio módulo (`api/oximeal.ts`),
// siguiendo el patrón de `api/ingestKeys.ts`.
const mockApi = vi.hoisted(() => ({
  getOximealStatus: vi.fn(),
  checkOximeal: vi.fn(),
  setOximealSecret: vi.fn(),
}));

vi.mock("../api/oximeal", () => mockApi);

import { OximealSection } from "./OximealSection";
import type { OximealCheckResult, OximealStatus } from "../types";

const mockGetOximealStatus = vi.mocked(mockApi.getOximealStatus);
const mockCheckOximeal = vi.mocked(mockApi.checkOximeal);
const mockSetOximealSecret = vi.mocked(mockApi.setOximealSecret);

const RESOURCE = "https://oximeal.territoriolinux.es/api";

const notConfigured: OximealStatus = {
  configured: false,
  base_url: "https://oximeal.territoriolinux.es",
  resource: RESOURCE,
  issuer_url: "https://auth.territoriolinux.es",
  has_client_id: false,
};

const configured: OximealStatus = {
  configured: true,
  base_url: "https://oximeal.territoriolinux.es",
  resource: RESOURCE,
  issuer_url: "https://auth.territoriolinux.es",
  has_client_id: true,
};

// Los campos no sensibles cuelgan del formulario de ajustes del padre: la
// sección se monta dentro de un `Form` real, como en el diálogo.
const Wrapper = ({ children }: { children: ReactNode }) => (
  <AntdApp>
    <Form>{children}</Form>
  </AntdApp>
);

const renderSection = (): void => {
  render(<OximealSection />, { wrapper: Wrapper });
};

// Réplica del contrato del diálogo de ajustes: la sección cuelga del `Form`
// compartido y su botón «Guardar» (submit) entrega todos los valores no
// sensibles —incluido `oximeal_resource`— al `onFinish`, que es el cuerpo que
// viaja por `PUT /api/settings`.
const renderSectionWithSubmit = (
  onFinish: (values: Record<string, string>) => void,
): void => {
  render(
    <AntdApp>
      <Form onFinish={onFinish}>
        <OximealSection />
      </Form>
    </AntdApp>,
  );
};

describe("OximealSection", () => {
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
    mockGetOximealStatus.mockResolvedValue(notConfigured);
    mockSetOximealSecret.mockResolvedValue(undefined);
    mockCheckOximeal.mockResolvedValue({
      ok: true,
      detail: "conexión correcta",
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("muestra los campos de base/issuer/client_id y el estado de configuración", async () => {
    mockGetOximealStatus.mockResolvedValue(configured);

    renderSection();

    expect(await screen.findByText("Configurado")).toBeInTheDocument();

    expect(screen.getByLabelText("OxiMeal · URL base")).toHaveAttribute(
      "placeholder",
      "https://oximeal.territoriolinux.es",
    );
    expect(screen.getByLabelText("OxiMeal · URL del emisor")).toHaveAttribute(
      "placeholder",
      "https://auth.territoriolinux.es",
    );
    expect(screen.getByLabelText("OxiMeal · Client ID")).toHaveAttribute(
      "placeholder",
      "valet-agent",
    );
  });

  it("muestra el campo «OxiMeal · Resource» con su placeholder junto a los no sensibles", async () => {
    mockGetOximealStatus.mockResolvedValue(configured);

    renderSection();

    await screen.findByText("Configurado");

    expect(screen.getByLabelText("OxiMeal · Resource")).toHaveAttribute(
      "placeholder",
      "https://oximeal.territoriolinux.es/api",
    );
  });

  it("la línea de estado refleja el resource resuelto del status sin exponer el secreto", async () => {
    const user = userEvent.setup();
    mockGetOximealStatus.mockResolvedValue({
      ...configured,
      resource: "https://oximeal.territoriolinux.es/api",
    });

    renderSection();

    expect(
      await screen.findByText(
        "Resource: https://oximeal.territoriolinux.es/api",
      ),
    ).toBeInTheDocument();
    // El secreto no se pinta nunca como texto: ni el guardado (el status jamás
    // lo trae) ni el que se teclea en su campo `type=password`.
    await user.type(
      screen.getByLabelText("OxiMeal · client secret"),
      "SECRETO-DE-PRUEBA",
    );
    expect(screen.queryByText(/SECRETO-DE-PRUEBA/)).toBeNull();
  });

  it("el campo «OxiMeal · Resource» viaja en el guardado de los ajustes no sensibles", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();

    renderSectionWithSubmit(onSave);
    await screen.findByText("Sin configurar");

    const resource = screen.getByLabelText("OxiMeal · Resource");
    await user.type(resource, "https://oximeal.territoriolinux.es/api");

    await user.click(screen.getByRole("button", { name: "Guardar" }));

    // El cuerpo del submit —lo que el diálogo manda por `PUT /api/settings`—
    // incluye el nuevo ajuste no sensible.
    await waitFor(() => {
      expect(onSave).toHaveBeenCalledWith(
        expect.objectContaining({
          oximeal_resource: "https://oximeal.territoriolinux.es/api",
        }),
      );
    });
  });

  it("el campo del secreto arranca siempre vacío aunque ya haya uno guardado", async () => {
    // El estado sale de `GET /api/oximeal/status` (sin el secreto): configurado.
    mockGetOximealStatus.mockResolvedValue(configured);

    renderSection();

    await screen.findByText("Configurado");

    const secret = screen.getByLabelText("OxiMeal · client secret");
    expect(secret).toHaveAttribute("type", "password");
    // Nunca se relee el Valor guardado: el campo nace vacío.
    expect((secret as HTMLInputElement).value).toBe("");
  });

  it("«Probar conexión» llama a check y muestra el detail del veredicto", async () => {
    const user = userEvent.setup();
    const result: OximealCheckResult = {
      ok: false,
      detail: "el token fue rechazado",
    };
    mockCheckOximeal.mockResolvedValue(result);

    renderSection();
    await screen.findByText("Sin configurar");

    await user.click(screen.getByRole("button", { name: "Probar conexión" }));

    expect(mockCheckOximeal).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("el token fue rechazado")).toBeInTheDocument();
  });

  it("«Probar conexión» cae al mensaje de fallback si el sondeo rechaza", async () => {
    const user = userEvent.setup();
    // Un rechazo del propio `GET /api/oximeal/check` (p. ej. la red cae): la
    // ruta responde siempre `200`, así que un rechazo es un fallo de transporte
    // y la sección publica su propio `detail` de fallback en la línea de estado.
    mockCheckOximeal.mockRejectedValue(new Error("red caída"));

    renderSection();
    await screen.findByText("Sin configurar");

    await user.click(screen.getByRole("button", { name: "Probar conexión" }));

    expect(mockCheckOximeal).toHaveBeenCalledTimes(1);
    // El `detail` de fallback vive en la línea de estado de la sección (el
    // mismo texto sale además en el toast, fuera de la región).
    const region = screen.getByRole("region", { name: "OxiMeal" });
    expect(
      await within(region).findByText(
        "No se pudo comprobar la conexión con OxiMeal",
      ),
    ).toBeInTheDocument();
  });

  it("muestra el indicador de carga mientras el estado aún no ha llegado", async () => {
    // Lectura diferida: el estado sigue en vuelo y la sección debe mostrar la
    // región de carga (`loading && !status`), no un estado falso.
    let resolveStatus!: (value: OximealStatus) => void;
    mockGetOximealStatus.mockReturnValue(
      new Promise<OximealStatus>((resolve) => {
        resolveStatus = resolve;
      }),
    );

    renderSection();

    // Mientras la promesa está pendiente: región viva de carga con su `Spin`.
    const loadingRegion = screen.getByRole("status", {
      name: "Cargando estado de OxiMeal",
    });
    expect(loadingRegion).toBeInTheDocument();
    expect(loadingRegion.querySelector(".ant-spin")).not.toBeNull();

    // Se resuelve la lectura dentro de `act`: el `setState` forma parte del
    // ciclo de React, no de un microtask suelto.
    await act(async () => {
      resolveStatus(notConfigured);
    });

    // Resuelta la lectura, el indicador cede su sitio a la línea de estado.
    expect(await screen.findByText("Sin configurar")).toBeInTheDocument();
    expect(
      screen.queryByRole("status", { name: "Cargando estado de OxiMeal" }),
    ).not.toBeInTheDocument();
  });

  it("guardar el secreto usa su endpoint dedicado y limpia el campo", async () => {
    const user = userEvent.setup();

    renderSection();
    await screen.findByText("Sin configurar");

    const secret = screen.getByLabelText("OxiMeal · client secret");
    await user.type(secret, "secreto-nuevo");
    await user.click(screen.getByRole("button", { name: "Guardar secreto" }));

    await waitFor(() => {
      expect(mockSetOximealSecret).toHaveBeenCalledWith("secreto-nuevo");
    });
    // El secreto guardado no se relee: el campo se limpia tras guardar.
    await waitFor(() => {
      expect((secret as HTMLInputElement).value).toBe("");
    });
  });

  it("un secreto vacío no se envía", async () => {
    const user = userEvent.setup();

    renderSection();
    await screen.findByText("Sin configurar");

    await user.click(screen.getByRole("button", { name: "Guardar secreto" }));

    expect(mockSetOximealSecret).not.toHaveBeenCalled();
  });
});
