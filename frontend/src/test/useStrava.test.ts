import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

// Se mockea el cliente API (no `fetch`) siguiendo el patrón de
// `useSkills.test.ts`. Solo se sustituyen los endpoints de Strava.
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
import { useStrava } from "../hooks/useStrava";
import type { StravaCheckResult, StravaStatus } from "../types";

const mockGetStravaStatus = vi.mocked(api.getStravaStatus);
const mockDisconnectStrava = vi.mocked(api.disconnectStrava);
const mockCheckStrava = vi.mocked(api.checkStrava);

const connected: StravaStatus = {
  connected: true,
  athlete_id: "12345",
  athlete_name: "Ana Corredora",
  scope: "read,activity:read_all",
};

const disconnected: StravaStatus = {
  connected: false,
  athlete_id: null,
  athlete_name: null,
  scope: null,
};

describe("useStrava", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetStravaStatus.mockResolvedValue(connected);
  });

  it("check devuelve el resultado y no toca el estado de conexión", async () => {
    const checkResult: StravaCheckResult = {
      ok: true,
      athlete_id: "12345",
      athlete_name: "Ana Corredora",
      error: null,
    };
    mockCheckStrava.mockResolvedValue(checkResult);

    const { result } = renderHook(() => useStrava());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let returned: StravaCheckResult | undefined;
    await act(async () => {
      returned = await result.current.check();
    });

    expect(returned).toEqual(checkResult);
    expect(result.current.status).toEqual(connected);
    expect(result.current.checking).toBe(false);
  });

  it("checking es true mientras dura la comprobación y false al terminar", async () => {
    let resolveCheck!: (value: StravaCheckResult) => void;
    mockCheckStrava.mockReturnValue(
      new Promise<StravaCheckResult>((resolve) => {
        resolveCheck = resolve;
      }),
    );

    const { result } = renderHook(() => useStrava());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let pending!: Promise<StravaCheckResult>;
    act(() => {
      pending = result.current.check();
    });

    await waitFor(() => expect(result.current.checking).toBe(true));

    await act(async () => {
      resolveCheck({
        ok: true,
        athlete_id: null,
        athlete_name: null,
        error: null,
      });
      await pending;
    });

    await waitFor(() => expect(result.current.checking).toBe(false));
  });

  it("check que rechaza vuelve a dejar checking en false y propaga el error", async () => {
    mockCheckStrava.mockRejectedValue(new Error("fallo de red"));

    const { result } = renderHook(() => useStrava());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await expect(result.current.check()).rejects.toThrow("fallo de red");
    });

    // La guarda del `finally` debe desactivar el sondeo aunque la petición falle.
    expect(result.current.checking).toBe(false);
  });

  it("disconnect resuelve con el aviso del servidor", async () => {
    mockDisconnectStrava.mockResolvedValue({
      connected: false,
      warning: "retíralo desde https://www.strava.com/settings/apps",
    });
    mockGetStravaStatus
      .mockResolvedValueOnce(connected)
      .mockResolvedValueOnce(disconnected);

    const { result } = renderHook(() => useStrava());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let outcome: { warning: string | null } | undefined;
    await act(async () => {
      outcome = await result.current.disconnect();
    });

    expect(outcome).toEqual({
      warning: "retíralo desde https://www.strava.com/settings/apps",
    });
  });

  it("disconnect sin aviso resuelve con warning null", async () => {
    mockDisconnectStrava.mockResolvedValue({ connected: false });
    mockGetStravaStatus
      .mockResolvedValueOnce(connected)
      .mockResolvedValueOnce(disconnected);

    const { result } = renderHook(() => useStrava());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let outcome: { warning: string | null } | undefined;
    await act(async () => {
      outcome = await result.current.disconnect();
    });

    expect(outcome).toEqual({ warning: null });
  });

  it("disconnect rechaza si la propia petición falla", async () => {
    mockDisconnectStrava.mockRejectedValue(new Error("boom"));

    const { result } = renderHook(() => useStrava());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await expect(result.current.disconnect()).rejects.toThrow("boom");
    });
  });
});
