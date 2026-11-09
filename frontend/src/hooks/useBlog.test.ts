import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

// Se mockea el cliente API (no `fetch`) siguiendo el patrón de
// `useOpentodo.test.ts`. Solo se sustituyen los endpoints del Blog.
vi.mock("../api/client", async () => {
  const actual =
    await vi.importActual<typeof import("../api/client")>("../api/client");
  return {
    ...actual,
    api: {
      ...actual.api,
      getBlogStatus: vi.fn(),
      setBlogPassword: vi.fn(),
      checkBlog: vi.fn(),
    },
  };
});

import { api } from "../api/client";
import { useBlog } from "./useBlog";
import type { BlogCheckResult, BlogStatus } from "../types";

const mockGetBlogStatus = vi.mocked(api.getBlogStatus);
const mockSetBlogPassword = vi.mocked(api.setBlogPassword);
const mockCheckBlog = vi.mocked(api.checkBlog);

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

describe("useBlog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetBlogStatus.mockResolvedValue(notConfigured);
    mockSetBlogPassword.mockResolvedValue(undefined);
  });

  it("carga el estado al montar sin tocar la contraseña", async () => {
    const { result } = renderHook(() => useBlog());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.status).toEqual(notConfigured);
    expect(result.current.error).toBeNull();
    expect(mockGetBlogStatus).toHaveBeenCalledTimes(1);
    // El montaje solo hace `GET /status`: nunca toca la contraseña.
    expect(mockSetBlogPassword).not.toHaveBeenCalled();
  });

  it("expone el error de lectura sin re-lanzar", async () => {
    mockGetBlogStatus.mockRejectedValue(new Error("Blog caído"));

    const { result } = renderHook(() => useBlog());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.error).toBe("Blog caído");
    expect(result.current.status).toBeNull();
  });

  it("savePassword guarda por su endpoint dedicado y relee el estado", async () => {
    mockGetBlogStatus
      .mockResolvedValueOnce(notConfigured)
      .mockResolvedValueOnce(configured);

    const { result } = renderHook(() => useBlog());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.savePassword("app-password-nueva");
    });

    expect(mockSetBlogPassword).toHaveBeenCalledWith("app-password-nueva");
    // Una lectura al montar + otra tras guardar.
    expect(mockGetBlogStatus).toHaveBeenCalledTimes(2);
    expect(result.current.status).toEqual(configured);
    expect(result.current.saving).toBe(false);
  });

  it("savePassword rechaza si el guardado falla y vuelve a dejar saving en false", async () => {
    mockSetBlogPassword.mockRejectedValue(new Error("boom"));

    const { result } = renderHook(() => useBlog());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await expect(result.current.savePassword("pw")).rejects.toThrow("boom");
    });

    expect(result.current.saving).toBe(false);
  });

  it("check devuelve el resultado y no relee el estado", async () => {
    const checkResult: BlogCheckResult = {
      ok: true,
      detail: "conexión correcta",
    };
    mockCheckBlog.mockResolvedValue(checkResult);

    const { result } = renderHook(() => useBlog());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let returned: BlogCheckResult | undefined;
    await act(async () => {
      returned = await result.current.check();
    });

    expect(returned).toEqual(checkResult);
    // No toca `status` (sigue siendo el de la carga de montaje)…
    expect(result.current.status).toEqual(notConfigured);
    expect(result.current.checking).toBe(false);
    // …ni vuelve a consultar `GET /api/blog/status`.
    expect(mockGetBlogStatus).toHaveBeenCalledTimes(1);
  });

  it("checking es true mientras dura la comprobación y false al terminar", async () => {
    let resolveCheck!: (value: BlogCheckResult) => void;
    mockCheckBlog.mockReturnValue(
      new Promise<BlogCheckResult>((resolve) => {
        resolveCheck = resolve;
      }),
    );

    const { result } = renderHook(() => useBlog());
    await waitFor(() => expect(result.current.loading).toBe(false));

    let pending!: Promise<BlogCheckResult>;
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
