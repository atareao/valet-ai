import { describe, it, expect, vi, afterEach } from "vitest";
import { api } from "../api/client";

/**
 * Contratos del cliente HTTP para la bandeja de avisos
 * (spec `notifications-inbox`, frontend, requirement
 * "El frontend SHALL definir el tipo `Notification` y los métodos de la API
 * de avisos").
 *
 * Se mockea `fetch` para leer la URL y el cuerpo reales que emite `request<T>`.
 */
describe("api/client — avisos", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  const stubFetch = () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({ data: [], next_cursor: null, total: 0 }),
    });
    vi.stubGlobal("fetch", fetchMock);
    return fetchMock;
  };

  it("listUnreadNotifications arma la query con limit", async () => {
    const fetchMock = stubFetch();

    await api.listUnreadNotifications({ limit: 10 });

    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/notifications/unread?limit=10");
  });

  it("listUnreadNotifications arma la query con limit y cursor", async () => {
    const fetchMock = stubFetch();

    await api.listUnreadNotifications({ limit: 10, cursor: "c1" });

    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/notifications/unread?limit=10&cursor=c1");
  });

  it("listUnreadNotifications sin parámetros no añade query string", async () => {
    const fetchMock = stubFetch();

    await api.listUnreadNotifications();

    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/notifications/unread");
  });

  it("markNotificationsRead envía POST con { ids }", async () => {
    const fetchMock = stubFetch();

    await api.markNotificationsRead(["a", "b"]);

    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/notifications/mark-read");
    expect(init).toMatchObject({ method: "POST" });
    expect(init.body).toBe(JSON.stringify({ ids: ["a", "b"] }));
  });

  it("markAllNotificationsRead envía POST con cuerpo exactamente {}", async () => {
    const fetchMock = stubFetch();

    await api.markAllNotificationsRead();

    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/notifications/mark-read");
    expect(init).toMatchObject({ method: "POST" });
    // `{}` (sin `ids`) es lo que marca todas; `{"ids":[]}` daría 400.
    expect(init.body).toBe("{}");
  });
});
