import { describe, it, expect, vi, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useSSE } from "../hooks/useSSE";
import { UNAUTHORIZED_EVENT } from "../api/client";

/**
 * Fase GREEN del change `oidc-auth`.
 *
 * Contrato (spec `frontend`): "la sesión se envía igualmente en las peticiones
 * de streaming". Hoy `useSSE.connect` llama a `fetch` sin `credentials`, así
 * que este assert falla.
 */
function streamResponse(lines: string[]) {
  let index = 0;
  const encoder = new TextEncoder();
  const reader = {
    read: vi.fn(async () => {
      if (index >= lines.length) {
        return { done: true, value: undefined };
      }
      const value = encoder.encode(lines[index]);
      index += 1;
      return { done: false, value };
    }),
  };
  return {
    ok: true,
    status: 200,
    statusText: "OK",
    body: { getReader: () => reader },
    json: async () => ({}),
  };
}

describe("useSSE — credenciales en streaming", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("la petición de streaming incluye credentials: include", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValue(streamResponse([`data: ${JSON.stringify({ type: "done", message_id: "m1" })}\n`]));
    vi.stubGlobal("fetch", fetchMock);

    const { result } = renderHook(() => useSSE());

    await act(async () => {
      await result.current.connect("hola");
    });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [, init] = fetchMock.mock.calls[0];
    expect(init).toMatchObject({ credentials: "include" });
  });
});

describe("useSSE — 401 en streaming", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("un 401 emite valet:unauthorized (además de llamar a onError)", async () => {
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
    const onError = vi.fn();
    window.addEventListener(UNAUTHORIZED_EVENT, handler);

    const { result } = renderHook(() => useSSE());

    await act(async () => {
      await result.current.connect("hola", { onError });
    });

    window.removeEventListener(UNAUTHORIZED_EVENT, handler);

    expect(handler).toHaveBeenCalledTimes(1);
    expect(onError).toHaveBeenCalledTimes(1);
  });
});
