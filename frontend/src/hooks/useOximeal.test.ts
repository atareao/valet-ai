import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

// Se mockea el módulo de API de OxiMeal (no `fetch`) siguiendo el patrón de
// `api/ingestKeys.ts`: los tres endpoints viven en su propio módulo, que se
// carga de forma diferida con la sección de ajustes.
vi.mock("../api/oximeal", () => ({
  getOximealStatus: vi.fn(),
  checkOximeal: vi.fn(),
  setOximealSecret: vi.fn(),
}));

import { checkOximeal, getOximealStatus, setOximealSecret } from "../api/oximeal";
import { useOximeal } from "./useOximeal";
import type { OximealCheckResult, OximealStatus } from "../types";

const mockGetOximealStatus = vi.mocked(getOximealStatus);
const mockCheckOximeal = vi.mocked(checkOximeal);
const mockSetOximealSecret = vi.mocked(setOximealSecret);

const notConfigured: OximealStatus = {
  configured: false,
  base_url: "https://oximeal.territoriolinux.es",
  issuer_url: "https://auth.territoriolinux.es",
  has_client_id: false,
};

const configured: OximealStatus = {
  configured: true,
  base_url: "https://oximeal.territoriolinux.es",
  issuer_url: "https://auth.territoriolinux.es",
  has_client_id: true,
};

describe("useOximeal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetOximealStatus.mockResolvedValue(notConfigured);
    mockSetOximealSecret.mockResolvedValue(undefined);
  });

  it("arranca en loading y carga el estado al montar sin tocar el secreto", async () => {
    const { result } = renderHook(() => useOximeal());

    expect(result.current.loading).toBe(true);

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.status).toEqual(notConfigured);
    expect(result.current.error).toBeNull();
    expect(mockGetOximealStatus).toHaveBeenCalledTimes(1);
    // El montaje solo hace `GET /status`: nunca toca el secreto (ni lo lee ni lo
    // guarda). Es lo que promete el título del test.
    expect(mockSetOximealSecret).not.toHaveBeenCalled();
  });

  it("expone el estado degradado (error de lectura) sin re-lanzar", async () => {
    mockGetOximealStatus.mockRejectedValue(new Error("OxiMeal caído"));

    const { result } = renderHook(() => useOximeal());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.error).toBe("OxiMeal caído");
    expect(result.current.status).toBeNull();
  });

  it("saveSecret guarda por su endpoint dedicado y relee el estado", async () => {
    mockGetOximealStatus
      .mockResolvedValueOnce(notConfigured)
      .mockResolvedValueOnce(configured);

    const { result } = renderHook(() => useOximeal());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.saveSecret("secreto-nuevo");
    });

    expect(mockSetOximealSecret).toHaveBeenCalledWith("secreto-nuevo");
    // Una lectura al montar + otra tras guardar.
    expect(mockGetOximealStatus).toHaveBeenCalledTimes(2);
    expect(result.current.status).toEqual(configured);
    expect(result.current.saving).toBe(false);
  });

  it("saveSecret rechaza si el guardado falla y vuelve a dejar saving en false", async () => {
    mockSetOximealSecret.mockRejectedValue(new Error("boom"));

    const { result } = renderHook(() => useOximeal());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await expect(result.current.saveSecret("secreto")).rejects.toThrow("boom");
    });

    expect(result.current.saving).toBe(false);
  });

  it("check devuelve el resultado y no relee el estado", async () => {
    const checkResult: OximealCheckResult = {
      ok: true,
      detail: "conexión correcta",
    };
    mockCheckOximeal.mockResolvedValue(checkResult);

    const { result } = renderHook(() => useOximeal());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let returned: OximealCheckResult | undefined;
    await act(async () => {
      returned = await result.current.check();
    });

    expect(returned).toEqual(checkResult);
    // No toca `status` (sigue siendo el de la carga de montaje)…
    expect(result.current.status).toEqual(notConfigured);
    expect(result.current.checking).toBe(false);
    // …ni vuelve a consultar `GET /api/oximeal/status`.
    expect(mockGetOximealStatus).toHaveBeenCalledTimes(1);
  });

  it("checking es true mientras dura la comprobación y false al terminar", async () => {
    let resolveCheck!: (value: OximealCheckResult) => void;
    mockCheckOximeal.mockReturnValue(
      new Promise<OximealCheckResult>((resolve) => {
        resolveCheck = resolve;
      }),
    );

    const { result } = renderHook(() => useOximeal());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let pending!: Promise<OximealCheckResult>;
    act(() => {
      pending = result.current.check();
    });

    await waitFor(() => expect(result.current.checking).toBe(true));

    await act(async () => {
      resolveCheck({ ok: true, detail: "conexión correcta" });
      await pending;
    });

    await waitFor(() => expect(result.current.checking).toBe(false));
  });
});
