import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App as AntdApp, Form } from "antd";
import type { ReactNode } from "react";

import { MastodonSection } from "./MastodonSection";

/**
 * Tests de la sub-pestaña «Mastodon» centrados en lo que hace el usuario.
 *
 * Se mockea `fetch` —no el módulo API— para comprobar las URLs y los cuerpos
 * reales que salen del cliente: `GET /api/mastodon/status`,
 * `GET /api/mastodon/check` y `PUT /api/mastodon/token`. La URL base cuelga del
 * formulario de ajustes del padre, así que su guardado se comprueba en el
 * `onFinish` que el diálogo conecta con `PUT /api/settings`.
 */

interface MastodonStatusFixture {
  configured: boolean;
  base_url: string;
}

interface CheckFixture {
  ok: boolean;
  detail: string;
}

let statusFixture: MastodonStatusFixture;
let checkFixture: CheckFixture;
let fetchMock: ReturnType<typeof vi.fn>;

const jsonResponse = (payload: unknown, ok = true, status = 200) => ({
  ok,
  status,
  statusText: ok ? "OK" : "Error",
  json: async () => payload,
});

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
  statusFixture = {
    configured: true,
    base_url: "https://mastodon.social",
  };
  checkFixture = { ok: true, detail: "conexión correcta" };
  fetchMock = vi.fn().mockImplementation(async (input: RequestInfo | URL) => {
    const url = String(input);
    if (url.endsWith("/api/mastodon/status")) {
      return jsonResponse(statusFixture);
    }
    if (url.endsWith("/api/mastodon/check")) {
      return jsonResponse(checkFixture);
    }
    if (url.endsWith("/api/mastodon/token")) {
      return { ok: true, status: 204, statusText: "No Content", json: async () => undefined };
    }
    throw new Error(`petición inesperada: ${url}`);
  });
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

const Wrapper = ({ children }: { children: ReactNode }) => (
  <AntdApp>
    <Form>{children}</Form>
  </AntdApp>
);

const renderSection = () => render(<MastodonSection />, { wrapper: Wrapper });

const renderSectionWithSubmit = (
  onFinish: (values: Record<string, string>) => void,
): void => {
  render(
    <AntdApp>
      <Form onFinish={onFinish}>
        <MastodonSection />
      </Form>
    </AntdApp>,
  );
};

const callsTo = (path: string) =>
  fetchMock.mock.calls.filter(([url]) => String(url) === path);

describe("MastodonSection", () => {
  it("el campo del token nace vacío aunque ya haya uno guardado", async () => {
    statusFixture = { configured: true, base_url: "https://mastodon.social" };

    renderSection();

    expect(await screen.findByText("Configurado")).toBeInTheDocument();
    const token = screen.getByLabelText("Mastodon · Token");
    expect(token).toHaveAttribute("type", "password");
    expect((token as HTMLInputElement).value).toBe("");
  });

  it("la línea de estado muestra Configurado o Sin configurar según el endpoint", async () => {
    statusFixture = { configured: false, base_url: "" };
    const first = renderSection();
    expect(await screen.findByText("Sin configurar")).toBeInTheDocument();
    expect(callsTo("/api/mastodon/status")).toHaveLength(1);

    // Una segunda apertura con el token ya guardado cambia el texto, siempre
    // con etiqueta legible (no solo el color de una etiqueta).
    first.unmount();
    statusFixture = { configured: true, base_url: "https://mastodon.social" };
    renderSection();
    expect(await screen.findByText("Configurado")).toBeInTheDocument();
    const region = screen.getByRole("region", { name: "Mastodon" });
    expect(within(region).getByText("Configurado").textContent).toBe("Configurado");
  });

  it("«Probar conexión» llama a la sonda y muestra su detail sin revelar el token", async () => {
    const user = userEvent.setup();
    checkFixture = {
      ok: false,
      detail: "el token fue rechazado por mastodon.social",
    };

    renderSection();
    await screen.findByText("Configurado");

    await user.type(screen.getByLabelText("Mastodon · Token"), "mastodon-token-secreto");
    await user.click(screen.getByRole("button", { name: "Probar conexión" }));

    expect(callsTo("/api/mastodon/check")).toHaveLength(1);
    expect(
      await screen.findByText("el token fue rechazado por mastodon.social"),
    ).toBeInTheDocument();
    const region = screen.getByRole("region", { name: "Mastodon" });
    expect(within(region).queryByText(/mastodon-token-secreto/)).toBeNull();
  });

  it("la URL base se guarda con el formulario de ajustes y el token no viaja en él", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();

    renderSectionWithSubmit(onSave);
    await screen.findByText("Configurado");

    await user.clear(screen.getByLabelText("Mastodon · URL base"));
    await user.type(
      screen.getByLabelText("Mastodon · URL base"),
      "https://mastodon.social",
    );
    await user.type(screen.getByLabelText("Mastodon · Token"), "mastodon-token-secreto");
    await user.click(screen.getByRole("button", { name: "Guardar" }));

    await waitFor(() => {
      expect(onSave).toHaveBeenCalledWith(
        expect.objectContaining({ mastodon_base_url: "https://mastodon.social" }),
      );
    });
    expect(onSave).toHaveBeenCalledWith(
      expect.not.objectContaining({ mastodon_token: expect.anything() }),
    );
    expect(callsTo("/api/mastodon/token")).toHaveLength(0);
  });

  it("guardar el token usa PUT /api/mastodon/token y limpia el campo", async () => {
    const user = userEvent.setup();

    renderSection();
    await screen.findByText("Configurado");

    const token = screen.getByLabelText("Mastodon · Token");
    await user.type(token, "mastodon-token-nuevo");
    await user.click(screen.getByRole("button", { name: "Guardar token" }));

    await waitFor(() => {
      expect(callsTo("/api/mastodon/token")).toHaveLength(1);
    });
    const [url, init] = callsTo("/api/mastodon/token")[0];
    expect(url).toBe("/api/mastodon/token");
    expect(init?.method).toBe("PUT");
    expect(JSON.parse(String(init?.body))).toEqual({ token: "mastodon-token-nuevo" });
    await waitFor(() => {
      expect((token as HTMLInputElement).value).toBe("");
    });
  });

  it("un token vacío no dispara ninguna petición de guardado", async () => {
    const user = userEvent.setup();

    renderSection();
    await screen.findByText("Configurado");

    await user.click(screen.getByRole("button", { name: "Guardar token" }));

    expect(callsTo("/api/mastodon/token")).toHaveLength(0);
  });
});
