import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useMainChat } from "../hooks/useMainChat";
import { api } from "../api/client";
import type { Message } from "../types";

/**
 * Fase RED de `chat-cross-device-sync` (spec `frontend`).
 *
 * `useSSE` y el cliente HTTP se mockean; la suscripción al bus de eventos
 * (`useChatSync`) es la implementación real, alimentada por un
 * `ReadableStream` falso.
 */

interface SseCallbacks {
  onChunk?: (chunk: string) => void;
  onDone?: (...args: unknown[]) => void;
  onError?: (message: string, status?: number) => void;
  onWidget?: (id: string, name: string, data: unknown) => void;
}

const { sseCallbacks, capturedClientMessageId } = vi.hoisted(() => ({
  sseCallbacks: {} as SseCallbacks,
  capturedClientMessageId: { current: undefined as string | undefined },
}));

vi.mock("../api/client", () => ({
  api: {
    chatInit: vi.fn().mockResolvedValue({ messages: [], settings: {} }),
    listMessages: vi.fn(),
    getSettings: vi.fn(),
    approveAction: vi.fn().mockResolvedValue(undefined),
  },
  BASE_URL: "http://localhost:3000",
  UNAUTHORIZED_EVENT: "valet:unauthorized",
  chatEventsUrl: (since?: string | null) =>
    `http://localhost:3000/chat/events${
      since ? `?since=${encodeURIComponent(since)}` : ""
    }`,
}));

vi.mock("../hooks/useSSE", () => ({
  useSSE: vi.fn(() => ({
    connect: vi.fn(
      (
        _content: string,
        options: SseCallbacks,
        _browserContext?: unknown,
        clientMessageId?: string,
      ) => {
        sseCallbacks.onChunk = options.onChunk;
        sseCallbacks.onDone = options.onDone;
        sseCallbacks.onError = options.onError;
        sseCallbacks.onWidget = options.onWidget;
        capturedClientMessageId.current = clientMessageId;
      },
    ),
    disconnect: vi.fn(),
    connected: false,
  })),
}));

const encoder = new TextEncoder();

interface FakeStream {
  response: Response;
  push: (chunk: string) => void;
  end: () => void;
}

function fakeStream(): FakeStream {
  const pending: string[] = [];
  let waiter: ((result: { done: boolean; value?: Uint8Array }) => void) | null =
    null;
  let ended = false;

  const next = (): Promise<{ done: boolean; value?: Uint8Array }> => {
    const chunk = pending.shift();
    if (chunk !== undefined) {
      return Promise.resolve({ done: false, value: encoder.encode(chunk) });
    }
    if (ended) return Promise.resolve({ done: true, value: undefined });
    return new Promise((resolve) => {
      waiter = resolve;
    });
  };

  const push = (chunk: string) => {
    if (waiter) {
      const resolve = waiter;
      waiter = null;
      resolve({ done: false, value: encoder.encode(chunk) });
      return;
    }
    pending.push(chunk);
  };

  const end = () => {
    ended = true;
    if (waiter) {
      const resolve = waiter;
      waiter = null;
      resolve({ done: true, value: undefined });
    }
  };

  const reader = {
    read: vi.fn(next),
    cancel: vi.fn(async () => {
      end();
    }),
  };

  return {
    response: {
      ok: true,
      status: 200,
      statusText: "OK",
      body: { getReader: () => reader },
      json: async () => ({}),
    } as unknown as Response,
    push,
    end,
  };
}

let stream: FakeStream;

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

async function mountChat() {
  const { result } = renderHook(() => useMainChat());
  await vi.waitFor(
    () => {
      expect(result.current.loading).toBe(false);
    },
    { timeout: 3000 },
  );
  await settle();
  return result;
}

/** Entrega un evento del bus al cliente, como haría el servidor. */
async function pushBusEvent(payload: Record<string, unknown>) {
  await act(async () => {
    stream.push(`data: ${JSON.stringify(payload)}\n`);
  });
  await settle();
}

const canonical = (
  id: string,
  content: string,
  created_at: string,
  role: Message["role"] = "user",
): Message => ({ id, role, content, created_at });

describe("useMainChat — sincronización entre dispositivos", () => {
  beforeEach(() => {
    sseCallbacks.onChunk = undefined;
    sseCallbacks.onDone = undefined;
    sseCallbacks.onError = undefined;
    sseCallbacks.onWidget = undefined;
    capturedClientMessageId.current = undefined;
    stream = fakeStream();
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(stream.response));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("propone un `client_message_id` y usa el mismo id en la burbuja optimista", async () => {
    const result = await mountChat();

    await act(async () => {
      await result.current.sendMessage("Hola");
    });

    const clientMessageId = capturedClientMessageId.current;
    expect(clientMessageId).toBeTruthy();
    expect(clientMessageId?.length).toBeLessThanOrEqual(64);

    const userMessages = result.current.messages.filter(
      (m) => m.role === "user",
    );
    expect(userMessages).toHaveLength(1);
    expect(userMessages[0].id).toBe(clientMessageId);
  });

  it("la burbuja optimista se sustituye por su fila canónica y no se duplica", async () => {
    const result = await mountChat();

    await act(async () => {
      await result.current.sendMessage("Hola");
    });

    const clientMessageId = capturedClientMessageId.current as string;

    await pushBusEvent({
      type: "message_created",
      message: canonical(clientMessageId, "Hola", "2026-10-24 12:00:03"),
      cursor: `2026-10-24 12:00:03|${clientMessageId}`,
    });

    const userMessages = result.current.messages.filter(
      (m) => m.role === "user",
    );
    expect(userMessages).toHaveLength(1);
    expect(userMessages[0].id).toBe(clientMessageId);
    // La marca temporal es la del servidor, no la del reloj del navegador.
    expect(userMessages[0].created_at).toBe("2026-10-24T12:00:03.000Z");
  });

  it("el mensaje del asistente adopta la marca temporal del servidor", async () => {
    const result = await mountChat();

    await act(async () => {
      await result.current.sendMessage("Hola");
    });

    await act(async () => {
      sseCallbacks.onDone?.("a1");
    });

    expect(result.current.messages.filter((m) => m.role === "assistant")).toHaveLength(1);

    await pushBusEvent({
      type: "message_created",
      message: canonical("a1", "¡Hola!", "2026-10-24 12:00:07", "assistant"),
      cursor: "2026-10-24 12:00:07|a1",
    });

    const assistant = result.current.messages.filter(
      (m) => m.role === "assistant",
    );
    expect(assistant).toHaveLength(1);
    expect(assistant[0].created_at).toBe("2026-10-24T12:00:07.000Z");
  });

  it("no muestra el indicador de turno ajeno mientras se cierra el turno propio", async () => {
    const result = await mountChat();

    await act(async () => {
      await result.current.sendMessage("Hola");
    });

    // El bus ya conoce el turno (es el nuestro).
    await pushBusEvent({ type: "turn_started" });
    expect(result.current.streaming).toBe(true);
    expect(result.current.remoteBusy).toBe(false);

    // `done` cierra el turno propio por el canal del stream; `turn_finished`
    // todavía no ha llegado por el bus (es otro canal), así que `sync.busy`
    // sigue en `true`.
    await act(async () => {
      sseCallbacks.onDone?.("a1");
    });

    expect(result.current.streaming).toBe(false);
    // Sin identificar el turno propio, el originador vería durante ese hueco
    // «Valet está respondiendo en otro dispositivo».
    expect(result.current.remoteBusy).toBe(false);

    await pushBusEvent({ type: "turn_finished" });
    expect(result.current.remoteBusy).toBe(false);
  });

  it("un mensaje escrito en otro dispositivo aparece sin recargar", async () => {
    const result = await mountChat();

    await pushBusEvent({
      type: "message_created",
      message: canonical("ajeno-1", "Desde el móvil", "2026-10-24 12:00:01"),
      cursor: "2026-10-24 12:00:01|ajeno-1",
    });
    await pushBusEvent({
      type: "message_created",
      message: canonical("ajeno-2", "Respuesta", "2026-10-24 12:00:04", "assistant"),
      cursor: "2026-10-24 12:00:04|ajeno-2",
    });

    expect(result.current.messages.map((m) => m.id)).toEqual([
      "ajeno-1",
      "ajeno-2",
    ]);
  });

  it("muestra el indicador de turno ajeno mientras el otro dispositivo responde", async () => {
    const result = await mountChat();

    expect(result.current.remoteBusy).toBe(false);

    await pushBusEvent({ type: "turn_started" });
    expect(result.current.remoteBusy).toBe(true);

    await pushBusEvent({ type: "turn_finished" });
    expect(result.current.remoteBusy).toBe(false);
  });

  it("un 409 muestra el aviso del servidor y no deja el mensaje como enviado", async () => {
    const result = await mountChat();

    await act(async () => {
      await result.current.sendMessage("Hola");
    });
    expect(result.current.messages.filter((m) => m.role === "user")).toHaveLength(
      1,
    );

    await act(async () => {
      sseCallbacks.onError?.("El chat está ocupado por otro turno", 409);
    });

    expect(result.current.error).toBe("El chat está ocupado por otro turno");
    expect(result.current.messages.filter((m) => m.role === "user")).toHaveLength(
      0,
    );
    expect(result.current.streaming).toBe(false);
  });

  it("un 409 devuelve el texto rechazado para poder reenviarlo", async () => {
    const result = await mountChat();

    expect(result.current.rejectedDraft).toBeNull();

    await act(async () => {
      await result.current.sendMessage("Un párrafo que no quiero perder");
    });

    // La burbuja optimista sí se pinta mientras el turno está en vuelo.
    expect(result.current.messages.filter((m) => m.role === "user")).toHaveLength(
      1,
    );

    await act(async () => {
      sseCallbacks.onError?.("El chat está ocupado por otro turno", 409);
    });

    // El texto no se da por enviado, pero tampoco se pierde: queda disponible
    // para reenviarlo cuando el turno ajeno termine.
    expect(result.current.rejectedDraft).toBe("Un párrafo que no quiero perder");
    expect(result.current.messages.filter((m) => m.role === "user")).toHaveLength(
      0,
    );
  });

  it("un error que no es 409 mantiene el mensaje del usuario", async () => {
    const result = await mountChat();

    await act(async () => {
      await result.current.sendMessage("Hola");
    });

    await act(async () => {
      sseCallbacks.onError?.("Se cayó la conexión a mitad de turno");
    });

    expect(result.current.error).toBe("Se cayó la conexión a mitad de turno");
    expect(result.current.messages.filter((m) => m.role === "user")).toHaveLength(
      1,
    );
    // Un fallo a mitad de turno no devuelve el texto al campo: el mensaje sí
    // salió y la burbuja se queda.
    expect(result.current.rejectedDraft).toBeNull();
  });

  it("no pisa con la fila del turno propio una fila canónica ya recibida del bus", async () => {
    const result = await mountChat();

    await act(async () => {
      await result.current.sendMessage("Hola");
    });

    const clientMessageId = capturedClientMessageId.current as string;

    // El bus se adelanta al evento `done` del turno propio.
    await pushBusEvent({
      type: "message_created",
      message: canonical(clientMessageId, "Hola", "2026-10-24 12:00:03"),
      cursor: `2026-10-24 12:00:03|${clientMessageId}`,
    });

    // `done` informa el id del mensaje del asistente y el del mensaje de
    // usuario realmente persistidos (el que propuso el cliente).
    await act(async () => {
      sseCallbacks.onDone?.("a1", clientMessageId);
    });

    const userMessages = result.current.messages.filter(
      (m) => m.role === "user",
    );
    expect(userMessages).toHaveLength(1);
    expect(userMessages[0].created_at).toBe("2026-10-24T12:00:03.000Z");
  });
});

describe("useMainChat — historial y suscripción conviven", () => {
  beforeEach(() => {
    sseCallbacks.onDone = undefined;
    stream = fakeStream();
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(stream.response));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("fusiona el historial de chatInit con lo que llega por el bus", async () => {
    vi.mocked(api.chatInit).mockResolvedValueOnce({
      messages: [
        {
          id: "h1",
          role: "user",
          content: "histórico",
          created_at: "2026-10-24 11:00:00",
        },
      ],
      settings: {},
    });

    const result = await mountChat();
    expect(result.current.messages.map((m) => m.id)).toEqual(["h1"]);
    expect(result.current.messages[0].created_at).toBe("2026-10-24T11:00:00.000Z");

    await pushBusEvent({
      type: "message_created",
      message: canonical("h1", "histórico", "2026-10-24 11:00:00"),
      cursor: "2026-10-24 11:00:00|h1",
    });
    await pushBusEvent({
      type: "message_created",
      message: canonical("n1", "nuevo", "2026-10-24 11:05:00"),
      cursor: "2026-10-24 11:05:00|n1",
    });

    expect(result.current.messages.map((m) => m.id)).toEqual(["h1", "n1"]);
  });
});

describe("useMainChat — la suscripción sin cursor relee el historial", () => {
  beforeEach(() => {
    stream = fakeStream();
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(stream.response));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("relee el historial al abrir la suscripción sin cursor (caso de montaje)", async () => {
    // La suscripción y la carga inicial salen a la vez. Un mensaje persistido
    // entre el instante en que el servidor suscribe al bus y el instante en que
    // `chatInit` lee el historial no está en esa lectura; y si llega `done`...
    // no hay `since`, así que el flujo en vivo tampoco lo trae. La relectura
    // posterior a la respuesta del stream lo recupera.
    vi.mocked(api.chatInit)
      .mockResolvedValueOnce({ messages: [], settings: {} })
      .mockResolvedValue({
        messages: [
          canonical("perdido", "escrito en el hueco", "2026-10-24 12:00:01"),
        ],
        settings: {},
      });

    const result = await mountChat();

    await vi.waitFor(
      () => {
        expect(result.current.messages.map((m) => m.id)).toContain("perdido");
      },
      { timeout: 3000 },
    );
  });

  it("una sesión que nunca vio un `message_created` relee el historial al reconectar", async () => {
    // La red se cae antes del primer evento: el cliente nunca llega a tener
    // cursor, así que cada reapertura va sin `since` y el replay del servidor no
    // recupera nada. El reintento debe releer el historial entero.
    vi.mocked(api.chatInit)
      .mockResolvedValueOnce({ messages: [], settings: {} })
      .mockResolvedValue({
        messages: [
          canonical("perdido", "durante la caída", "2026-10-24 12:00:01"),
        ],
        settings: {},
      });

    let calls = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn(() => {
        calls += 1;
        if (calls === 1) return Promise.reject(new Error("network down"));
        return Promise.resolve(stream.response);
      }),
    );

    const result = await mountChat();

    // Primer intento caído (antes de cualquier `message_created`); el reintento
    // abre sin cursor y debe releer el historial.
    await vi.waitFor(
      () => {
        expect(result.current.messages.map((m) => m.id)).toContain("perdido");
      },
      { timeout: 3000 },
    );
    expect(calls).toBeGreaterThanOrEqual(2);
  });
});
