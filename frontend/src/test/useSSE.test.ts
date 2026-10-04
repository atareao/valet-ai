import { describe, it, expect, vi, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useSSE } from "../hooks/useSSE";

/**
 * Construye una respuesta fetch mínima cuyo body expone un `getReader()` que
 * entrega las líneas del stream (cada una terminada en `\n`).
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

describe("useSSE — evento widget", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("invoca onWidget con id, name y data del evento SSE", async () => {
    const onWidget = vi.fn();
    const event = {
      type: "widget",
      id: "w1",
      name: "QuickForm",
      data: { ciudad: "Madrid" },
    };

    const fetchMock = vi
      .fn()
      .mockResolvedValue(streamResponse([`data: ${JSON.stringify(event)}\n`]));
    vi.stubGlobal("fetch", fetchMock);

    const { result } = renderHook(() => useSSE());

    await act(async () => {
      await result.current.connect("hola", { onWidget });
    });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(onWidget).toHaveBeenCalledWith("w1", "QuickForm", {
      ciudad: "Madrid",
    });
  });

  it("no interrumpe el stream: sigue invocando onChunk y onDone tras el widget", async () => {
    const onWidget = vi.fn();
    const onChunk = vi.fn();
    const onDone = vi.fn();

    const lines = [
      `data: ${JSON.stringify({ type: "widget", id: "w2", name: "Checklist", data: {} })}\n`,
      `data: ${JSON.stringify({ type: "chunk", content: "hola" })}\n`,
      `data: ${JSON.stringify({ type: "done", message_id: "m1" })}\n`,
    ];

    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(streamResponse(lines)),
    );

    const { result } = renderHook(() => useSSE());

    await act(async () => {
      await result.current.connect("hola", { onWidget, onChunk, onDone });
    });

    expect(onWidget).toHaveBeenCalledWith("w2", "Checklist", {});
    expect(onChunk).toHaveBeenCalledWith("hola");
    expect(onDone).toHaveBeenCalled();
  });

  it("descarta un evento widget con id vacío y avisa por consola", async () => {
    const onWidget = vi.fn();
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    const event = {
      type: "widget",
      id: "",
      name: "QuickForm",
      data: {},
    };
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValue(
          streamResponse([`data: ${JSON.stringify(event)}\n`]),
        ),
    );

    const { result } = renderHook(() => useSSE());

    await act(async () => {
      await result.current.connect("hola", { onWidget });
    });

    expect(onWidget).not.toHaveBeenCalled();
    expect(warnSpy).toHaveBeenCalled();

    warnSpy.mockRestore();
  });
});