import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

vi.mock("../api/client", async () => {
  const actual =
    await vi.importActual<typeof import("../api/client")>("../api/client");
  return {
    ...actual,
    api: {
      getPersistentMemory: vi.fn(),
      updatePersistentMemory: vi.fn(),
      clearPersistentMemory: vi.fn(),
    },
  };
});

import { api, ApiError } from "../api/client";
import { usePersistentMemory } from "../hooks/usePersistentMemory";

const mockGet = vi.mocked(api.getPersistentMemory);
const mockUpdate = vi.mocked(api.updatePersistentMemory);
const mockClear = vi.mocked(api.clearPersistentMemory);

const stateFixture = {
  payload: { schema_version: 1, user_profile: { name: "Lorenzo" } },
  updated_at: "2026-10-01T10:00:00Z",
  token_count: 120,
  budget_tokens: 500,
  ceiling_tokens: 1000,
  is_empty: false,
};

describe("usePersistentMemory", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGet.mockResolvedValue(stateFixture);
  });

  it("carga el estado al montar", async () => {
    const { result } = renderHook(() => usePersistentMemory());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(mockGet).toHaveBeenCalledTimes(1);
    expect(result.current.state).toEqual(stateFixture);
  });

  it("guarda enviando el payload parseado y el expected_updated_at cargado", async () => {
    mockUpdate.mockResolvedValue({ ...stateFixture, warning: null });
    const { result } = renderHook(() => usePersistentMemory());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let outcome: string = "";
    await act(async () => {
      outcome = await result.current.save(
        JSON.stringify({ schema_version: 1, user_profile: { name: "Ana" } }),
      );
    });

    expect(outcome).toBe("saved");
    expect(mockUpdate).toHaveBeenCalledWith(
      { schema_version: 1, user_profile: { name: "Ana" } },
      "2026-10-01T10:00:00Z",
    );
  });

  it("rechaza JSON inválido sin llamar al servidor", async () => {
    const { result } = renderHook(() => usePersistentMemory());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let outcome: string = "";
    await act(async () => {
      outcome = await result.current.save("esto no es json");
    });

    expect(outcome).toBe("invalid");
    expect(mockUpdate).not.toHaveBeenCalled();
    expect(result.current.error).toBeTruthy();
  });

  it("rechaza schema_version distinta de 1", async () => {
    const { result } = renderHook(() => usePersistentMemory());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let outcome: string = "";
    await act(async () => {
      outcome = await result.current.save(
        JSON.stringify({ schema_version: 2, user_profile: {} }),
      );
    });

    expect(outcome).toBe("invalid");
    expect(mockUpdate).not.toHaveBeenCalled();
  });

  it("rechaza claves de primer nivel no permitidas", async () => {
    const { result } = renderHook(() => usePersistentMemory());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let outcome: string = "";
    await act(async () => {
      outcome = await result.current.save(
        JSON.stringify({ schema_version: 1, user_profile: {}, intruso: true }),
      );
    });

    expect(outcome).toBe("invalid");
    expect(mockUpdate).not.toHaveBeenCalled();
  });

  it("rechaza system_rules que no sea array", async () => {
    const { result } = renderHook(() => usePersistentMemory());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let outcome: string = "";
    await act(async () => {
      outcome = await result.current.save(
        JSON.stringify({ schema_version: 1, system_rules: "no-array" }),
      );
    });

    expect(outcome).toBe("invalid");
    expect(mockUpdate).not.toHaveBeenCalled();
  });

  it("rechaza user_profile que no sea objeto", async () => {
    const { result } = renderHook(() => usePersistentMemory());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let outcome: string = "";
    await act(async () => {
      outcome = await result.current.save(
        JSON.stringify({ schema_version: 1, user_profile: "no-objeto" }),
      );
    });

    expect(outcome).toBe("invalid");
    expect(result.current.error).toBeTruthy();
    expect(mockUpdate).not.toHaveBeenCalled();
  });

  it("rechaza system_rules con elementos que no son strings", async () => {
    const { result } = renderHook(() => usePersistentMemory());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let outcome: string = "";
    await act(async () => {
      outcome = await result.current.save(
        JSON.stringify({ schema_version: 1, system_rules: [1, 2] }),
      );
    });

    expect(outcome).toBe("invalid");
    expect(result.current.error).toBeTruthy();
    expect(mockUpdate).not.toHaveBeenCalled();
  });

  it("muestra el error del servidor ante 422 y conserva el estado", async () => {
    mockUpdate.mockRejectedValue(new ApiError("payload inválido", 422));
    const { result } = renderHook(() => usePersistentMemory());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let outcome: string = "";
    await act(async () => {
      outcome = await result.current.save(JSON.stringify({ schema_version: 1 }));
    });

    expect(outcome).toBe("error");
    expect(result.current.error).toBe("payload inválido");
    expect(result.current.state).toEqual(stateFixture);
  });

  it("expone el warning devuelto por el servidor", async () => {
    mockUpdate.mockResolvedValue({
      ...stateFixture,
      warning: "Supera el presupuesto vigente",
    });
    const { result } = renderHook(() => usePersistentMemory());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.save(JSON.stringify({ schema_version: 1 }));
    });

    expect(result.current.warning).toBe("Supera el presupuesto vigente");
  });

  it("ante 409 recarga el estado y avisa del cambio", async () => {
    mockGet
      .mockResolvedValueOnce(stateFixture)
      .mockResolvedValueOnce({ ...stateFixture, updated_at: "T2" });
    mockUpdate.mockRejectedValue(new ApiError("conflicto", 409));
    const { result } = renderHook(() => usePersistentMemory());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let outcome: string = "";
    await act(async () => {
      outcome = await result.current.save(JSON.stringify({ schema_version: 1 }));
    });

    expect(outcome).toBe("conflict");
    expect(mockGet).toHaveBeenCalledTimes(2);
    expect(result.current.conflict).toBeTruthy();
    expect(result.current.state?.updated_at).toBe("T2");
  });

  it("vacia el estado y recarga", async () => {
    mockClear.mockResolvedValue(undefined);
    const { result } = renderHook(() => usePersistentMemory());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let ok = false;
    await act(async () => {
      ok = await result.current.clear();
    });

    expect(ok).toBe(true);
    expect(mockClear).toHaveBeenCalledTimes(1);
    expect(mockGet).toHaveBeenCalledTimes(2);
  });
});
