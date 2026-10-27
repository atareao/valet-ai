import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

// Se mockea el cliente API (no `fetch`) siguiendo el patrón de
// `useStrava.test.ts`. Solo se sustituyen los endpoints de OpenTodo.
vi.mock("../api/client", async () => {
  const actual =
    await vi.importActual<typeof import("../api/client")>("../api/client");
  return {
    ...actual,
    api: {
      ...actual.api,
      getOpentodoStatus: vi.fn(),
      setOpentodoToken: vi.fn(),
      checkOpentodo: vi.fn(),
    },
  };
});

import { api } from "../api/client";
import { useOpentodo } from "../hooks/useOpentodo";
import type { OpentodoCheckResult, OpentodoStatus } from "../types";

const mockGetOpentodoStatus = vi.mocked(api.getOpentodoStatus);
const mockSetOpentodoToken = vi.mocked(api.setOpentodoToken);
const mockCheckOpentodo = vi.mocked(api.checkOpentodo);

const notConfigured: OpentodoStatus = {
  configured: false,
  base_url: "https://todo.territoriolinux.es",
};

const configured: OpentodoStatus = {
  configured: true,
  base_url: "https://todo.territoriolinux.es",
};

describe("useOpentodo", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetOpentodoStatus.mockResolvedValue(notConfigured);
    mockSetOpentodoToken.mockResolvedValue(undefined);
  });

  it("carga el estado al montar sin tocar el token", async () => {
    const { result } = renderHook(() => useOpentodo());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.status).toEqual(notConfigured);
    expect(result.current.error).toBeNull();
    expect(mockGetOpentodoStatus).toHaveBeenCalledTimes(1);
  });

  it("expone el error de lectura sin re-lanzar", async () => {
    mockGetOpentodoStatus.mockRejectedValue(new Error("OpenTodo caído"));

    const { result } = renderHook(() => useOpentodo());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.error).toBe("OpenTodo caído");
    expect(result.current.status).toBeNull();
  });

  it("saveToken guarda por su endpoint dedicado y relee el estado", async () => {
    mockGetOpentodoStatus
      .mockResolvedValueOnce(notConfigured)
      .mockResolvedValueOnce(configured);

    const { result } = renderHook(() => useOpentodo());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.saveToken("tok-nuevo");
    });

    expect(mockSetOpentodoToken).toHaveBeenCalledWith("tok-nuevo");
    // Una lectura al montar + otra tras guardar.
    expect(mockGetOpentodoStatus).toHaveBeenCalledTimes(2);
    expect(result.current.status).toEqual(configured);
    expect(result.current.saving).toBe(false);
  });

  it("saveToken rechaza si el guardado falla y vuelve a dejar saving en false", async () => {
    mockSetOpentodoToken.mockRejectedValue(new Error("boom"));

    const { result } = renderHook(() => useOpentodo());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await expect(result.current.saveToken("tok")).rejects.toThrow("boom");
    });

    expect(result.current.saving).toBe(false);
  });

  it("check devuelve el resultado y no relee el estado", async () => {
    const checkResult: OpentodoCheckResult = {
      ok: true,
      detail: "conexión correcta",
    };
    mockCheckOpentodo.mockResolvedValue(checkResult);

    const { result } = renderHook(() => useOpentodo());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let returned: OpentodoCheckResult | undefined;
    await act(async () => {
      returned = await result.current.check();
    });

    expect(returned).toEqual(checkResult);
    // No toca `status` (sigue siendo el de la carga de montaje)…
    expect(result.current.status).toEqual(notConfigured);
    expect(result.current.checking).toBe(false);
    // …ni vuelve a consultar `GET /api/opentodo/status`.
    expect(mockGetOpentodoStatus).toHaveBeenCalledTimes(1);
  });

  it("checking es true mientras dura la comprobación y false al terminar", async () => {
    let resolveCheck!: (value: OpentodoCheckResult) => void;
    mockCheckOpentodo.mockReturnValue(
      new Promise<OpentodoCheckResult>((resolve) => {
        resolveCheck = resolve;
      }),
    );

    const { result } = renderHook(() => useOpentodo());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let pending!: Promise<OpentodoCheckResult>;
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
