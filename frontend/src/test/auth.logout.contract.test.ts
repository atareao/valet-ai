import { describe, it, expect, vi, afterEach } from "vitest";
import { api } from "../api/client";

/**
 * Fase GREEN del change `oidc-auth`.
 *
 * Test de contrato del logout. El backend (`src/routes/auth.rs`) documenta y
 * serializa:
 *
 *   #[derive(Debug, Serialize)]
 *   pub struct LogoutResponse {
 *       pub end_session_url: Option<String>,
 *   }
 *
 * `api.logout()` DEBE leer `end_session_url`. Este test fija ese nombre de
 * campo: si el frontend vuelve a esperar (`end_session_endpoint`), la
 * aserción estricta falla.
 */
describe("api/client — contrato de logout", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("lee end_session_url tal y como lo serializa el backend", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        json: async () => ({ end_session_url: "https://idp.example/logout" }),
      }),
    );

    const result = await api.logout();

    expect(result).toEqual({ end_session_url: "https://idp.example/logout" });
    expect(result).not.toHaveProperty("end_session_endpoint");
  });

  it("acepta end_session_url: null (solo cierre de sesión local)", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        json: async () => ({ end_session_url: null }),
      }),
    );

    const result = await api.logout();

    expect(result.end_session_url).toBeNull();
  });
});
