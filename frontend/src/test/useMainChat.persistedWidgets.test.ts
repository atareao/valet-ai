import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook } from "@testing-library/react";
import { useMainChat } from "../hooks/useMainChat";
import { api } from "../api/client";
import type { Message } from "../types";

// ---------------------------------------------------------------------------
// Mock del cliente API y de useSSE (mismo patrón que useMainChat.widgets.test.ts).
// Aquí el foco es la reconstrucción de widgets desde el historial de chatInit.
// ---------------------------------------------------------------------------
vi.mock("../api/client", () => ({
  api: {
    chatInit: vi.fn(),
    listMessages: vi.fn(),
    getSettings: vi.fn(),
    approveAction: vi.fn().mockResolvedValue(undefined),
  },
  BASE_URL: "http://localhost:3000",
}));

vi.mock("../hooks/useSSE", () => ({
  useSSE: vi.fn(() => ({
    connect: vi.fn(),
    disconnect: vi.fn(),
    connected: false,
  })),
}));

const chatInitMock = api.chatInit as unknown as ReturnType<typeof vi.fn>;

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

describe("useMainChat — widgets persistidos", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("reconstruye widgetsByMessage desde message.widgets del historial", async () => {
    const history: Message[] = [
      {
        id: "m1",
        role: "assistant",
        content: "aquí tienes un sitio",
        created_at: "2026-01-01T00:00:00Z",
        widgets: [{ id: "w1", name: "LocationWidget", data: {} }],
      },
      {
        id: "m2",
        role: "assistant",
        content: "sin widgets",
        created_at: "2026-01-01T00:01:00Z",
      },
    ];
    chatInitMock.mockResolvedValue({ messages: history, settings: {} });

    const result = await mountChat();

    expect(result.current.widgetsByMessage["m1"]).toEqual([
      { id: "w1", name: "LocationWidget", data: {} },
    ]);
    expect(result.current.widgetsByMessage["m2"]).toBeUndefined();
  });

  it("no crea entradas para mensajes con widgets vacíos o nulos", async () => {
    const history: Message[] = [
      {
        id: "m1",
        role: "assistant",
        content: "vacío",
        created_at: "2026-01-01T00:00:00Z",
        widgets: [],
      },
      {
        id: "m2",
        role: "assistant",
        content: "nulo",
        created_at: "2026-01-01T00:01:00Z",
        widgets: null,
      },
    ];
    chatInitMock.mockResolvedValue({ messages: history, settings: {} });

    const result = await mountChat();

    expect(result.current.widgetsByMessage["m1"]).toBeUndefined();
    expect(result.current.widgetsByMessage["m2"]).toBeUndefined();
  });
});
