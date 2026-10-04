import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useMainChat } from "../hooks/useMainChat";
import type { WidgetInstance } from "../components/widgets/types";

interface SseCallbacks {
  onChunk?: (c: string) => void;
  onDone?: (...args: unknown[]) => void;
  onError?: (m: string) => void;
  onWidget?: (id: string, name: string, data: unknown) => void;
}

// ---------------------------------------------------------------------------
// Igual que en useMainChat.test.ts: capturamos los callbacks que useMainChat
// entrega a useSSE para poder simular el stream desde el test.
// ---------------------------------------------------------------------------
const { sseCallbacks } = vi.hoisted(() => ({
  sseCallbacks: {} as SseCallbacks,
}));

vi.mock("../api/client", () => ({
  api: {
    chatInit: vi.fn().mockResolvedValue({ messages: [], settings: {} }),
    listMessages: vi.fn(),
    getSettings: vi.fn(),
    approveAction: vi.fn().mockResolvedValue(undefined),
  },
  BASE_URL: "http://localhost:3000",
}));

vi.mock("../hooks/useSSE", () => ({
  useSSE: vi.fn(() => ({
    connect: vi.fn((_content: string, options: SseCallbacks) => {
      sseCallbacks.onChunk = options.onChunk;
      sseCallbacks.onDone = options.onDone;
      sseCallbacks.onError = options.onError;
      sseCallbacks.onWidget = options.onWidget;
    }),
    disconnect: vi.fn(),
    connected: false,
  })),
}));

async function mountChat() {
  const { result } = renderHook(() => useMainChat());
  await vi.waitFor(
    () => {
      expect(result.current.loading).toBe(false);
    },
    { timeout: 3000 },
  );
  return result;
}

describe("useMainChat — widgets", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    sseCallbacks.onChunk = undefined;
    sseCallbacks.onDone = undefined;
    sseCallbacks.onError = undefined;
    sseCallbacks.onWidget = undefined;
  });

  it("acumula el widget en streaming y lo mueve al messageId en onDone", async () => {
    const result = await mountChat();

    await act(async () => {
      result.current.sendMessage("Hola");
    });

    await act(async () => {
      sseCallbacks.onWidget?.("w1", "QuickForm", { ciudad: "Madrid" });
    });

    expect(result.current.widgetsByMessage["streaming"]).toEqual([
      { id: "w1", name: "QuickForm", data: { ciudad: "Madrid" } },
    ]);

    await act(async () => {
      sseCallbacks.onDone?.("msg-1");
    });

    expect(result.current.widgetsByMessage["streaming"]).toBeUndefined();
    expect(result.current.widgetsByMessage["msg-1"]).toEqual([
      { id: "w1", name: "QuickForm", data: { ciudad: "Madrid" } },
    ]);
  });

  it("limpia los widgets de streaming en onError", async () => {
    const result = await mountChat();

    await act(async () => {
      result.current.sendMessage("Hola");
    });

    await act(async () => {
      sseCallbacks.onWidget?.("w9", "Checklist", { title: "x", items: [] });
    });

    expect(result.current.widgetsByMessage["streaming"]).toHaveLength(1);

    await act(async () => {
      sseCallbacks.onError?.("Stream error");
    });

    expect(result.current.widgetsByMessage["streaming"]).toBeUndefined();
  });

  it("ignora un widget que llega sin stream activo", async () => {
    const result = await mountChat();

    await act(async () => {
      result.current.sendMessage("Hola");
    });

    await act(async () => {
      sseCallbacks.onDone?.("msg-1");
    });

    // El stream ya terminó; un widget tardío no debe recrear "streaming".
    await act(async () => {
      sseCallbacks.onWidget?.("late", "QuickForm", { ciudad: "Madrid" });
    });

    expect(result.current.widgetsByMessage["streaming"]).toBeUndefined();
    expect(result.current.widgetsByMessage["msg-1"]).toBeUndefined();
  });

  it("no envía la acción de un widget mientras hay un stream activo", async () => {
    const result = await mountChat();

    await act(async () => {
      result.current.sendMessage("Hola");
    });

    // `streaming` sigue activo (no se ha llamado a onDone/onError).
    expect(result.current.streaming).toBe(true);

    await act(async () => {
      result.current.sendWidgetAction(
        { id: "abc", name: "QuickForm", data: {} },
        "submit",
        { ciudad: "Madrid" },
      );
    });

    const userMessages = result.current.messages.filter(
      (m) => m.role === "user",
    );
    expect(userMessages).toHaveLength(1);
  });

  it("sendWidgetAction envía un turno de usuario con el envoltorio estable", async () => {
    const result = await mountChat();
    const widget: WidgetInstance = { id: "abc", name: "QuickForm", data: {} };

    await act(async () => {
      result.current.sendWidgetAction(widget, "submit", { ciudad: "Madrid" });
    });

    const userMessages = result.current.messages.filter(
      (m) => m.role === "user",
    );
    expect(userMessages).toHaveLength(1);
    expect(userMessages[0].content).toBe(
      '[widget:QuickForm#abc] submit {"ciudad":"Madrid"}',
    );
  });
});