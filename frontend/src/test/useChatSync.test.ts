import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import {
  useChatSync,
  retryDelayMs,
  CHAT_SYNC_MAX_RETRY_MS,
} from "../hooks/useChatSync";
import { UNAUTHORIZED_EVENT } from "../api/client";
import type { Message } from "../types";

/**
 * Fase RED de `chat-cross-device-sync` (spec `frontend`).
 *
 * La suscripción se simula con un `ReadableStream` falso y `fetch` mockeado: no
 * hay servidor ni `EventSource`.
 */

// ---------------------------------------------------------------------------
// Stream SSE controlable desde el test
// ---------------------------------------------------------------------------

interface FakeStream {
  response: Response;
  /** Entrega un trozo de stream (una o varias líneas `data:`). */
  push: (chunk: string) => void;
  /** Termina el cuerpo: el reader devuelve `done`. */
  end: () => void;
}

const encoder = new TextEncoder();

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

  const response = {
    ok: true,
    status: 200,
    statusText: "OK",
    body: { getReader: () => reader },
    json: async () => ({}),
  } as unknown as Response;

  return { response, push, end };
}

const event = (payload: Record<string, unknown>) =>
  `data: ${JSON.stringify(payload)}\n`;

const message = (
  id: string,
  created_at: string,
  content = id,
): Message => ({ id, role: "user", content, created_at });

const messageEvent = (msg: Message, cursor: string) =>
  event({ type: "message_created", message: msg, cursor });

type FetchMock = ReturnType<typeof vi.fn>;

let fetchMock: FetchMock;
let stream: FakeStream;

function stubFetch(): FetchMock {
  const mock = vi.fn();
  vi.stubGlobal("fetch", mock);
  return mock;
}

/** Deja correr los microtasks y el ciclo de render sin usar temporizadores. */
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

function setVisibility(state: "visible" | "hidden") {
  Object.defineProperty(document, "visibilityState", {
    configurable: true,
    get: () => state,
  });
}

describe("useChatSync — suscripción al bus de eventos", () => {
  beforeEach(() => {
    stream = fakeStream();
    fetchMock = stubFetch();
    fetchMock.mockResolvedValue(stream.response);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("abre la suscripción sin `since` y fusiona por upsert sin duplicar", async () => {
    const { result } = renderHook(() => useChatSync());
    await settle();

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0][0]).toBe("/api/chat/events");

    // Vía 1: la burbuja optimista del turno propio (mismo id que propondrá el
    // cliente al servidor).
    act(() => {
      result.current.upsert(message("cli-1", "2026-10-24T10:00:00.000Z"), {
        pending: true,
      });
    });
    expect(result.current.messages).toHaveLength(1);

    // Vía 2: la fila canónica de ese mismo mensaje por el bus de eventos.
    await act(async () => {
      stream.push(
        messageEvent(message("cli-1", "2026-10-24 10:00:05"), "c1"),
      );
    });
    await settle();

    expect(result.current.messages).toHaveLength(1);
    expect(result.current.messages[0].id).toBe("cli-1");
    expect(result.current.messages[0].created_at).toBe(
      "2026-10-24T10:00:05.000Z",
    );
  });

  it("ordena por (created_at, id) usando la marca temporal del servidor", async () => {
    const { result } = renderHook(() => useChatSync());
    await settle();

    await act(async () => {
      stream.push(
        messageEvent(message("b", "2026-10-24 10:00:02"), "c-b") +
          messageEvent(message("c", "2026-10-24 10:00:01"), "c-c") +
          messageEvent(message("a", "2026-10-24 10:00:02"), "c-a"),
      );
    });
    await settle();

    expect(result.current.messages.map((m) => m.id)).toEqual(["c", "a", "b"]);
  });

  it("cierra la suscripción al desmontarse", async () => {
    const { unmount } = renderHook(() => useChatSync());
    await settle();

    const signal = fetchMock.mock.calls[0][1]?.signal;
    expect(signal).toBeInstanceOf(AbortSignal);
    expect(signal?.aborted).toBe(false);

    unmount();

    expect(signal?.aborted).toBe(true);
  });

  it("un evento ilegible no rompe la suscripción", async () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const { result } = renderHook(() => useChatSync());
    await settle();

    await act(async () => {
      stream.push("data: {no-es-json\n");
      stream.push(messageEvent(message("m1", "2026-10-24 10:00:01"), "c1"));
    });
    await settle();

    expect(result.current.messages.map((m) => m.id)).toEqual(["m1"]);
    expect(warnSpy).toHaveBeenCalled();

    warnSpy.mockRestore();
  });
});

describe("useChatSync — estado del turno (indicador de turno ajeno)", () => {
  beforeEach(() => {
    stream = fakeStream();
    fetchMock = stubFetch();
    fetchMock.mockResolvedValue(stream.response);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("aparece con turn_started y desaparece con turn_finished", async () => {
    const { result } = renderHook(() => useChatSync());
    await settle();

    expect(result.current.busy).toBe(false);

    await act(async () => {
      stream.push(event({ type: "turn_started" }));
    });
    await settle();
    expect(result.current.busy).toBe(true);

    await act(async () => {
      stream.push(event({ type: "turn_finished" }));
    });
    await settle();
    expect(result.current.busy).toBe(false);
  });

  it("el snapshot inicial informa de un turno ya en curso", async () => {
    const ocupado = fakeStream();
    fetchMock.mockResolvedValue(ocupado.response);

    const { result } = renderHook(() => useChatSync());
    await settle();

    await act(async () => {
      ocupado.push(event({ type: "snapshot", busy: true }));
    });
    await settle();

    expect(result.current.busy).toBe(true);
  });
});

describe("useChatSync — recuperación al volver a la pestaña", () => {
  beforeEach(() => {
    stream = fakeStream();
    fetchMock = stubFetch();
    fetchMock.mockResolvedValue(stream.response);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    setVisibility("visible");
  });

  it("relee lo perdido y reabre la suscripción con el último cursor", async () => {
    const segundo = fakeStream();
    fetchMock.mockResolvedValueOnce(stream.response);
    fetchMock.mockResolvedValueOnce(segundo.response);

    const { result } = renderHook(() => useChatSync());
    await settle();

    await act(async () => {
      stream.push(
        messageEvent(message("m1", "2026-10-24 10:00:01"), "2026-10-24 10:00:01|m1"),
      );
    });
    await settle();
    expect(result.current.messages.map((m) => m.id)).toEqual(["m1"]);

    // La suscripción dejó de recibir mientras la pestaña estaba oculta.
    setVisibility("hidden");
    await act(async () => {
      stream.end();
    });
    await settle();
    expect(fetchMock).toHaveBeenCalledTimes(1);

    // La pestaña vuelve: se reabre con el último cursor conocido y el replay
    // devuelve lo persistido en el intervalo.
    setVisibility("visible");
    window.document.dispatchEvent(new Event("visibilitychange"));
    await settle();

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(fetchMock.mock.calls[1][0]).toBe(
      "/api/chat/events?since=2026-10-24+10%3A00%3A01%7Cm1",
    );

    await act(async () => {
      segundo.push(
        messageEvent(message("m2", "2026-10-24 10:00:09"), "2026-10-24 10:00:09|m2"),
      );
    });
    await settle();

    // Devolver un evento demuestra que la suscripción reabierta está viva.
    expect(result.current.messages.map((m) => m.id)).toEqual(["m1", "m2"]);
  });
});

describe("useChatSync — reintento con retroceso exponencial", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    stream = fakeStream();
    fetchMock = stubFetch();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("espera un tiempo creciente entre intentos", async () => {
    fetchMock.mockRejectedValue(new Error("network down"));

    renderHook(() => useChatSync());
    await vi.advanceTimersByTimeAsync(0);

    expect(fetchMock).toHaveBeenCalledTimes(1);

    // Primer reintento: 1 s.
    await vi.advanceTimersByTimeAsync(999);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(fetchMock).toHaveBeenCalledTimes(2);

    // Segundo reintento: 2 s.
    await vi.advanceTimersByTimeAsync(1999);
    expect(fetchMock).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(1);
    expect(fetchMock).toHaveBeenCalledTimes(3);

    // Tercero: 4 s.
    await vi.advanceTimersByTimeAsync(3999);
    expect(fetchMock).toHaveBeenCalledTimes(3);
    await vi.advanceTimersByTimeAsync(1);
    expect(fetchMock).toHaveBeenCalledTimes(4);
  });

  it("el reintento envía el último cursor conocido", async () => {
    fetchMock.mockResolvedValueOnce(stream.response);
    fetchMock.mockRejectedValue(new Error("network down"));

    renderHook(() => useChatSync());
    await vi.advanceTimersByTimeAsync(0);
    expect(fetchMock).toHaveBeenCalledTimes(1);

    stream.push(
      messageEvent(message("m1", "2026-10-24 10:00:01"), "2026-10-24 10:00:01|m1"),
    );
    await vi.advanceTimersByTimeAsync(0);

    stream.end();
    await vi.advanceTimersByTimeAsync(1000);
    expect(fetchMock).toHaveBeenCalledTimes(2);

    const url = fetchMock.mock.calls[1][0];
    expect(url).toContain("since=");
    expect(url).toContain(encodeURIComponent("|m1"));
  });

  it("el retroceso nunca supera los 30 segundos", () => {
    expect(retryDelayMs(0)).toBe(1000);
    expect(retryDelayMs(1)).toBe(2000);
    expect(retryDelayMs(2)).toBe(4000);
    expect(retryDelayMs(3)).toBe(8000);
    expect(retryDelayMs(4)).toBe(16000);
    expect(retryDelayMs(5)).toBe(CHAT_SYNC_MAX_RETRY_MS);
    expect(retryDelayMs(12)).toBe(CHAT_SYNC_MAX_RETRY_MS);
    expect(CHAT_SYNC_MAX_RETRY_MS).toBe(30_000);
  });
});

describe("useChatSync — resincronización", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    stream = fakeStream();
    fetchMock = stubFetch();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("reabre la suscripción al recibir `resync`, con el último cursor", async () => {
    const segundo = fakeStream();
    fetchMock.mockResolvedValueOnce(stream.response);
    fetchMock.mockResolvedValueOnce(segundo.response);

    renderHook(() => useChatSync());
    await vi.advanceTimersByTimeAsync(0);

    stream.push(
      event({ type: "message_created", message: message("m1", "2026-10-24 10:00:01"), cursor: "2026-10-24 10:00:01|m1" }),
    );
    await vi.advanceTimersByTimeAsync(0);

    stream.push(event({ type: "resync" }));
    // No se espera el retroceso: la reapertura es inmediata.
    await vi.advanceTimersByTimeAsync(0);

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(fetchMock.mock.calls[1][0]).toContain(
      encodeURIComponent("|m1"),
    );
  });

  it("un 400 limpia el cursor y vuelve a conectar sin `since`", async () => {
    const rechazo = {
      ok: false,
      status: 400,
      statusText: "Bad Request",
      body: null,
      json: async () => ({ error: "invalid cursor: expected \"<created_at>|<id>\"" }),
    } as unknown as Response;

    fetchMock.mockResolvedValueOnce(stream.response);
    fetchMock.mockResolvedValueOnce(rechazo);

    renderHook(() => useChatSync());
    await vi.advanceTimersByTimeAsync(0);

    stream.push(
      event({ type: "message_created", message: message("m1", "2026-10-24 10:00:01"), cursor: "2026-10-24 10:00:01|m1" }),
    );
    await vi.advanceTimersByTimeAsync(0);

    stream.end();
    await vi.advanceTimersByTimeAsync(1000);
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(fetchMock.mock.calls[1][0]).toContain("since=");

    await vi.advanceTimersByTimeAsync(2000);
    expect(fetchMock).toHaveBeenCalledTimes(3);
    expect(fetchMock.mock.calls[2][0]).toBe("/api/chat/events");
  });
});

describe("useChatSync — la marca de optimista no se filtra a la fila canónica", () => {
  beforeEach(() => {
    stream = fakeStream();
    fetchMock = stubFetch();
    fetchMock.mockResolvedValue(stream.response);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("el `done` del stream no deja al asistente anterior saltando por debajo del mensaje nuevo", async () => {
    // Regresión de `chat-pending-flag-leak` (design D5).
    //
    // La fila canónica del asistente llega por el bus ANTES que el `done` del
    // stream. HOY FALLA: el `done` entra por la rama de descarte de `upsert`
    // (`return prev`) y deja el id del asistente marcado en `pendingIdsRef` para
    // siempre, de modo que `sortMessages` lo empuja al final en cada reordenado
    // y el asistente anterior cae POR DEBAJO del mensaje nuevo
    // (`["cli-1","cli-2","a1"]`). Con la marca viajando en el mensaje el residuo
    // desaparece y el orden correcto es `["cli-1","a1","cli-2"]`.
    const NAVEGADOR = "2000-01-01T00:00:00.000Z";

    const { result } = renderHook(() => useChatSync());
    await settle();

    // 1. Burbuja optimista del usuario (hora del navegador, marcada pendiente).
    act(() => {
      result.current.upsert(message("cli-1", NAVEGADOR, "Hola"), {
        pending: true,
      });
    });
    expect(result.current.messages.map((m) => m.id)).toEqual(["cli-1"]);

    // 2. El bus entrega el usuario canónico y, acto seguido, el asistente
    //    canónico (marca temporal del servidor, posterior).
    await act(async () => {
      stream.push(
        messageEvent(message("cli-1", "2026-10-24 10:00:05", "Hola"), "c1") +
          messageEvent(
            {
              id: "a1",
              role: "assistant",
              content: "respuesta",
              created_at: "2026-10-24 10:00:07",
            },
            "c2",
          ),
      );
    });
    await settle();
    expect(result.current.messages.map((m) => m.id)).toEqual(["cli-1", "a1"]);

    // 3. El `done` del stream llega DESPUÉS que el bus, con la hora del
    //    navegador y marcado como pendiente (la fila canónica ya está).
    act(() => {
      result.current.upsert(
        {
          id: "a1",
          role: "assistant",
          content: "respuesta",
          created_at: NAVEGADOR,
        },
        { pending: true },
      );
    });
    await settle();

    // 4. Envío del mensaje nuevo y su fila canónica por el bus.
    act(() => {
      result.current.upsert(message("cli-2", NAVEGADOR, "Otra vez"), {
        pending: true,
      });
    });
    await act(async () => {
      stream.push(
        messageEvent(message("cli-2", "2026-10-24 10:00:09", "Otra vez"), "c3"),
      );
    });
    await settle();

    // El asistente anterior debe quedar POR ENCIMA del mensaje nuevo.
    expect(result.current.messages.map((m) => m.id)).toEqual([
      "cli-1",
      "a1",
      "cli-2",
    ]);

    // Y ninguna fila conserva la marca de optimista tras el `done`.
    expect(result.current.messages.some((m) => m.pending === true)).toBe(false);
  });
});

describe("useChatSync — sesión caducada", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    fetchMock = stubFetch();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("un 401 dispara el evento global y no reintenta en bucle", async () => {
    fetchMock.mockResolvedValue({
      ok: false,
      status: 401,
      statusText: "Unauthorized",
      body: null,
      json: async () => ({ error: "unauthorized" }),
    } as unknown as Response);

    const handler = vi.fn();
    window.addEventListener(UNAUTHORIZED_EVENT, handler);

    renderHook(() => useChatSync());
    await vi.advanceTimersByTimeAsync(0);

    expect(handler).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(5 * 60_000);

    expect(handler).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledTimes(1);

    window.removeEventListener(UNAUTHORIZED_EVENT, handler);
  });
});
