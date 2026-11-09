import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, act, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App as AntdApp, Form } from "antd";
import type { ReactNode } from "react";

// Se mockea solo el cliente API (no `fetch`): el hook `useBlog` corre de
// verdad. Los tres endpoints del Blog viven en `api/client.ts`, siguiendo el
// patrón de `useOpentodo.test.ts`.
const mockApi = vi.hoisted(() => ({
  getBlogStatus: vi.fn(),
  checkBlog: vi.fn(),
  setBlogPassword: vi.fn(),
}));

vi.mock("../api/client", async () => {
  const actual =
    await vi.importActual<typeof import("../api/client")>("../api/client");
  return {
    ...actual,
    api: {
      ...actual.api,
      getBlogStatus: mockApi.getBlogStatus,
      checkBlog: mockApi.checkBlog,
      setBlogPassword: mockApi.setBlogPassword,
    },
  };
});

import { BlogSection } from "./BlogSection";
import type { BlogCheckResult, BlogStatus } from "../types";

const mockGetBlogStatus = vi.mocked(mockApi.getBlogStatus);
const mockCheckBlog = vi.mocked(mockApi.checkBlog);
const mockSetBlogPassword = vi.mocked(mockApi.setBlogPassword);

const notConfigured: BlogStatus = {
  configured: false,
  base_url: "https://atareao.es",
  user: "",
};

const configured: BlogStatus = {
  configured: true,
  base_url: "https://atareao.es",
  user: "valet",
};

// Los campos no sensibles cuelgan del formulario de ajustes del padre: la
// sección se monta dentro de un `Form` real, como en el diálogo.
const Wrapper = ({ children }: { children: ReactNode }) => (
  <AntdApp>
    <Form>{children}</Form>
  </AntdApp>
);

const renderSection = (): void => {
  render(<BlogSection />, { wrapper: Wrapper });
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
        <BlogSection />
      </Form>
    </AntdApp>,
  );
};

describe("BlogSection", () => {
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
    mockGetBlogStatus.mockResolvedValue(notConfigured);
    mockSetBlogPassword.mockResolvedValue(undefined);
    mockCheckBlog.mockResolvedValue({
      ok: true,
      detail: "conexión correcta",
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("muestra los campos base/usuario y el estado de configuración", async () => {
    mockGetBlogStatus.mockResolvedValue(configured);

    renderSection();

    expect(await screen.findByText("Configurado")).toBeInTheDocument();

    expect(screen.getByLabelText("Blog · URL base")).toHaveAttribute(
      "placeholder",
      "https://atareao.es",
    );
    expect(screen.getByLabelText("Blog · Usuario del servicio")).toBeInTheDocument();
  });

  it("la línea de estado refleja el estado del endpoint sin exponer la contraseña", async () => {
    mockGetBlogStatus.mockResolvedValue(configured);

    renderSection();

    expect(await screen.findByText("Configurado")).toBeInTheDocument();

    const region = screen.getByRole("region", { name: "Blog" });
    // La contraseña nunca se pinta como texto.
    expect(within(region).queryByText(/app-password/)).toBeNull();
  });

  it("la URL base y el usuario viajan en el guardado de los ajustes no sensibles", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();

    renderSectionWithSubmit(onSave);
    await screen.findByText("Sin configurar");

    await user.type(screen.getByLabelText("Blog · URL base"), "https://atareao.es");
    await user.type(
      screen.getByLabelText("Blog · Usuario del servicio"),
      "valet",
    );

    await user.click(screen.getByRole("button", { name: "Guardar" }));

    // El cuerpo del submit —lo que el diálogo manda por `PUT /api/settings`—
    // incluye los dos ajustes no sensibles y NUNCA la contraseña.
    await waitFor(() => {
      expect(onSave).toHaveBeenCalledWith(
        expect.objectContaining({
          blog_base_url: "https://atareao.es",
          blog_service_user: "valet",
        }),
      );
    });
    expect(onSave).toHaveBeenCalledWith(
      expect.not.objectContaining({ blog_app_password: expect.anything() }),
    );
  });

  it("el campo de la contraseña arranca siempre vacío aunque ya haya una guardada", async () => {
    mockGetBlogStatus.mockResolvedValue(configured);

    renderSection();

    await screen.findByText("Configurado");

    const password = screen.getByLabelText("Blog · Application Password");
    expect(password).toHaveAttribute("type", "password");
    // Nunca se relee el valor guardado: el campo nace vacío.
    expect((password as HTMLInputElement).value).toBe("");
  });

  it("guardar la contraseña usa su endpoint dedicado y limpia el campo", async () => {
    const user = userEvent.setup();

    renderSection();
    await screen.findByText("Sin configurar");

    const password = screen.getByLabelText("Blog · Application Password");
    await user.type(password, "app-password-nueva");
    await user.click(screen.getByRole("button", { name: "Guardar contraseña" }));

    await waitFor(() => {
      expect(mockSetBlogPassword).toHaveBeenCalledWith("app-password-nueva");
    });
    // La contraseña guardada no se relee: el campo se limpia tras guardar.
    await waitFor(() => {
      expect((password as HTMLInputElement).value).toBe("");
    });
  });

  it("una contraseña vacía no se envía", async () => {
    const user = userEvent.setup();

    renderSection();
    await screen.findByText("Sin configurar");

    await user.click(screen.getByRole("button", { name: "Guardar contraseña" }));

    expect(mockSetBlogPassword).not.toHaveBeenCalled();
  });

  it("«Probar conexión» llama a check y muestra el detail del veredicto", async () => {
    const user = userEvent.setup();
    const result: BlogCheckResult = {
      ok: false,
      detail: "credenciales rechazadas",
    };
    mockCheckBlog.mockResolvedValue(result);

    renderSection();
    await screen.findByText("Sin configurar");

    await user.click(screen.getByRole("button", { name: "Probar conexión" }));

    expect(mockCheckBlog).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("credenciales rechazadas")).toBeInTheDocument();
  });

  it("«Probar conexión» cae al mensaje de fallback si el sondeo rechaza", async () => {
    const user = userEvent.setup();
    // Un rechazo del propio `GET /api/blog/check` (p. ej. la red cae): la ruta
    // responde siempre `200`, así que un rechazo es un fallo de transporte y la
    // sección publica su propio `detail` de fallback en la línea de estado.
    mockCheckBlog.mockRejectedValue(new Error("red caída"));

    renderSection();
    await screen.findByText("Sin configurar");

    await user.click(screen.getByRole("button", { name: "Probar conexión" }));

    expect(mockCheckBlog).toHaveBeenCalledTimes(1);
    const region = screen.getByRole("region", { name: "Blog" });
    expect(
      await within(region).findByText(
        "No se pudo comprobar la conexión con el Blog",
      ),
    ).toBeInTheDocument();
  });

  it("muestra el indicador de carga mientras el estado aún no ha llegado", async () => {
    // Lectura diferida: el estado sigue en vuelo y la sección debe mostrar la
    // región de carga (`loading && !status`), no un estado falso.
    let resolveStatus!: (value: BlogStatus) => void;
    mockGetBlogStatus.mockReturnValue(
      new Promise<BlogStatus>((resolve) => {
        resolveStatus = resolve;
      }),
    );

    renderSection();

    const loadingRegion = screen.getByRole("status", {
      name: "Cargando estado del Blog",
    });
    expect(loadingRegion).toBeInTheDocument();
    expect(loadingRegion.querySelector(".ant-spin")).not.toBeNull();

    await act(async () => {
      resolveStatus(notConfigured);
    });

    expect(await screen.findByText("Sin configurar")).toBeInTheDocument();
    expect(
      screen.queryByRole("status", { name: "Cargando estado del Blog" }),
    ).not.toBeInTheDocument();
  });
});
