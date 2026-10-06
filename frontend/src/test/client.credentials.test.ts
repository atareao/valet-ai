import { describe, it, expect, vi, afterEach } from "vitest";
import { api } from "../api/client";

/**
 * Fase GREEN del change `oidc-auth`.
 *
 * Contrato (spec `frontend`, requirement "Todas las peticiones incluyen
 * credenciales"): el cliente HTTP debe enviar `credentials: "include"` en
 * todas las peticiones a la API para que la cookie de sesión viaje.
 *
 * Hoy `request()` no añade `credentials`, así que estos asserts fallan.
 */
describe("api/client — credenciales de sesión", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("una petición GET incluye credentials: include", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({}),
    });
    vi.stubGlobal("fetch", fetchMock);

    await api.getSettings();

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [, init] = fetchMock.mock.calls[0];
    expect(init).toMatchObject({ credentials: "include" });
  });

  it("una petición POST incluye credentials: include", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({ id: "m1" }),
    });
    vi.stubGlobal("fetch", fetchMock);

    await api.createMessage({ role: "user", content: "hola" });

    const [, init] = fetchMock.mock.calls[0];
    expect(init).toMatchObject({ method: "POST", credentials: "include" });
  });
});
