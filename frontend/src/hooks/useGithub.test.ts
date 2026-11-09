import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

// Se mockea el módulo API de GitHub (no `fetch`) siguiendo el patrón de
// `useOximeal.test.ts`. Los tres endpoints viven en `api/github.ts`, fuera del
// grafo inicial (solo los alcanza `useGithub`).
vi.mock("../api/github", () => ({
  getGithubStatus: vi.fn(),
  putGithubToken: vi.fn(),
  checkGithub: vi.fn(),
}));

import {
  checkGithub,
  getGithubStatus,
  putGithubToken,
} from "../api/github";
import { useGithub } from "./useGithub";
import type { GithubCheckResult, GithubStatus } from "../types";

const mockGetGithubStatus = vi.mocked(getGithubStatus);
const mockPutGithubToken = vi.mocked(putGithubToken);
const mockCheckGithub = vi.mocked(checkGithub);

const notConfigured: GithubStatus = {
  configured: false,
  default_repo: "atareao/valet-ai",
};

const configured: GithubStatus = {
  configured: true,
  default_repo: "atareao/valet-ai",
};

describe("useGithub", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetGithubStatus.mockResolvedValue(notConfigured);
    mockPutGithubToken.mockResolvedValue(undefined);
  });

  it("carga el estado al montar sin tocar el token", async () => {
    const { result } = renderHook(() => useGithub());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.status).toEqual(notConfigured);
    expect(result.current.error).toBeNull();
    expect(mockGetGithubStatus).toHaveBeenCalledTimes(1);
    // El montaje solo hace `GET /status`: nunca toca el token.
    expect(mockPutGithubToken).not.toHaveBeenCalled();
  });

  it("expone el error de lectura sin re-lanzar", async () => {
    mockGetGithubStatus.mockRejectedValue(new Error("GitHub caído"));

    const { result } = renderHook(() => useGithub());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.error).toBe("GitHub caído");
    expect(result.current.status).toBeNull();
  });

  it("saveToken guarda por su endpoint dedicado y relee el estado", async () => {
    mockGetGithubStatus
      .mockResolvedValueOnce(notConfigured)
      .mockResolvedValueOnce(configured);

    const { result } = renderHook(() => useGithub());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.saveToken("ghp_token-nuevo");
    });

    expect(mockPutGithubToken).toHaveBeenCalledWith("ghp_token-nuevo");
    // Una lectura al montar + otra tras guardar.
    expect(mockGetGithubStatus).toHaveBeenCalledTimes(2);
    expect(result.current.status).toEqual(configured);
    expect(result.current.saving).toBe(false);
  });

  it("saveToken rechaza si el guardado falla y vuelve a dejar saving en false", async () => {
    mockPutGithubToken.mockRejectedValue(new Error("boom"));

    const { result } = renderHook(() => useGithub());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await expect(result.current.saveToken("tk")).rejects.toThrow("boom");
    });

    expect(result.current.saving).toBe(false);
  });

  it("check devuelve el resultado y no relee el estado", async () => {
    const checkResult: GithubCheckResult = {
      ok: true,
      detail: "conexión correcta",
    };
    mockCheckGithub.mockResolvedValue(checkResult);

    const { result } = renderHook(() => useGithub());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let returned: GithubCheckResult | undefined;
    await act(async () => {
      returned = await result.current.check();
    });

    expect(returned).toEqual(checkResult);
    // No toca `status` (sigue siendo el de la carga de montaje)…
    expect(result.current.status).toEqual(notConfigured);
    expect(result.current.checking).toBe(false);
    // …ni vuelve a consultar `GET /api/github/status`.
    expect(mockGetGithubStatus).toHaveBeenCalledTimes(1);
  });

  it("checking es true mientras dura la comprobación y false al terminar", async () => {
    let resolveCheck!: (value: GithubCheckResult) => void;
    mockCheckGithub.mockReturnValue(
      new Promise<GithubCheckResult>((resolve) => {
        resolveCheck = resolve;
      }),
    );

    const { result } = renderHook(() => useGithub());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let pending!: Promise<GithubCheckResult>;
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
