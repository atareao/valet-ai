import { describe, it, expect, vi, afterEach } from "vitest";
import { api, chatEventsUrl } from "../api/client";

/**
 * Fase RED de `chat-cross-device-sync` (spec `frontend`).
 *
 * El cursor de mensajes pasó a ser **opaco** y su forma de cable contiene `|`
 * (inválido en un request-target) y `+` (que el servidor decodifica como
 * espacio). Sin codificar, la paginación se rompe en silencio o devuelve `400`.
 */

const CURSOR = "2026-10-24 10:00:01|u1+x";
// `URLSearchParams`: el espacio va como `+` y los caracteres reservados del
// cursor (`|`, `+`, `:`) quedan escapados.
const CURSOR_ENCODED = "2026-10-24+10%3A00%3A01%7Cu1%2Bx";

describe("api/client — cursor opaco de mensajes", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  const stubFetch = () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({ data: [], next_cursor: null }),
    });
    vi.stubGlobal("fetch", fetchMock);
    return fetchMock;
  };

  it("listMessages codifica el cursor", async () => {
    const fetchMock = stubFetch();

    await api.listMessages(50, CURSOR);

    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe(`/api/messages?limit=50&cursor=${CURSOR_ENCODED}`);
  });

  it("listMessages sin cursor solo lleva el límite", async () => {
    const fetchMock = stubFetch();

    await api.listMessages(50);

    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/messages?limit=50");
  });
});

describe("api/client — URL del stream de eventos", () => {
  it("sin cursor no añade query string", () => {
    expect(chatEventsUrl()).toBe("/api/chat/events");
    expect(chatEventsUrl(null)).toBe("/api/chat/events");
  });

  it("codifica el cursor recibido como `since`", () => {
    expect(chatEventsUrl(CURSOR)).toBe(
      `/api/chat/events?since=${CURSOR_ENCODED}`,
    );
  });
});
