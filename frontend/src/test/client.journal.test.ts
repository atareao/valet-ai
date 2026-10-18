import { describe, it, expect, vi, afterEach } from "vitest";
import { api } from "../api/client";
import type { JournalEntry, Mood } from "../types";

/**
 * Contratos del cliente HTTP de la bitácora (spec `journal-entries`, frontend,
 * requirement "El frontend SHALL definir el tipo `JournalEntry` y los métodos
 * de la API de la bitácora").
 *
 * Se mockea `fetch` para leer la URL, el método y el cuerpo reales que emite
 * `request<T>`.
 */
describe("api/client — bitácora", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  const entry = (overrides: Partial<JournalEntry> = {}): JournalEntry => ({
    id: "c2f4aca4-97e0-4dd0-9eda-94f083608b4a",
    profile_id: "profile-id",
    date: "2026-10-18",
    mood: "good",
    content: "un día largo",
    achievements: ["terminar el informe"],
    struggles: [],
    tags: ["trabajo"],
    created_at: "2026-10-18 11:02:05",
    updated_at: "2026-10-18 11:02:05",
    ...overrides,
  });

  const stubFetch = (payload: unknown) => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => payload,
    });
    vi.stubGlobal("fetch", fetchMock);
    return fetchMock;
  };

  it("listJournalEntries arma la query con from, to y limit", async () => {
    const fetchMock = stubFetch([entry()]);

    const result = await api.listJournalEntries({
      from: "2026-10-01",
      to: "2026-10-18",
      limit: 30,
    });

    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/journal?from=2026-10-01&to=2026-10-18&limit=30");
    expect(result[0].content).toBe("un día largo");
    expect(result[0].mood).toBe("good");
  });

  it("listJournalEntries sin parámetros no añade query string", async () => {
    const fetchMock = stubFetch([entry()]);

    await api.listJournalEntries();

    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/journal");
  });

  it("getJournalEntry solicita GET /journal/{date}", async () => {
    const fetchMock = stubFetch(entry());

    const result = await api.getJournalEntry("2026-10-18");

    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/journal/2026-10-18");
    expect(result.date).toBe("2026-10-18");
  });

  it("upsertJournalEntry envía PUT /journal/{date} con el cuerpo", async () => {
    const fetchMock = stubFetch(entry());
    const mood: Mood = "great";

    await api.upsertJournalEntry("2026-10-18", {
      content: "un día largo",
      mood,
      achievements: ["terminar el informe"],
      struggles: [],
      tags: ["trabajo"],
    });

    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/journal/2026-10-18");
    expect(init).toMatchObject({ method: "PUT" });
    expect(init.body).toBe(
      JSON.stringify({
        content: "un día largo",
        mood: "great",
        achievements: ["terminar el informe"],
        struggles: [],
        tags: ["trabajo"],
      }),
    );
  });

  it("deleteJournalEntry envía DELETE /journal/{date}", async () => {
    // `204` sin cuerpo: el cliente no debe intentar parsear JSON.
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 204,
      json: async () => {
        throw new Error("204 no lleva cuerpo");
      },
    });
    vi.stubGlobal("fetch", fetchMock);

    await api.deleteJournalEntry("2026-10-18");

    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/journal/2026-10-18");
    expect(init).toMatchObject({ method: "DELETE" });
  });
});
