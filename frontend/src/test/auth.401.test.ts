import { describe, it, expect, vi, afterEach } from "vitest";
import { api } from "../api/client";

/**
 * Fase GREEN del change `oidc-auth`.
 *
 * Contrato (spec `frontend`, requirement "Un 401 global redirige a login" y
 * design: "Detección global de 401: en el manejo de `ApiError` (status 401)
 * disparar el estado no autenticado/redirección a login").
 *
 * Se fija como contrato observable que el cliente HTTP emite el evento
 * `valet:unauthorized` en `window` ante un 401 de una ruta de API no-auth;
 * `AuthProvider` se suscribe para forzar el estado no autenticado. El
 * `request()` actual no lo emite, así que el assert falla.
 */
describe("api/client — detección global de 401", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("un 401 en una ruta no-auth dispara valet:unauthorized", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 401,
        statusText: "Unauthorized",
        json: async () => ({ error: "unauthorized" }),
      }),
    );

    const handler = vi.fn();
    window.addEventListener("valet:unauthorized", handler);

    await api.getSettings().catch(() => undefined);

    window.removeEventListener("valet:unauthorized", handler);

    expect(handler).toHaveBeenCalledTimes(1);
  });
});
