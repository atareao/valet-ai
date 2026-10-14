import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { useNotifications } from "../hooks/useNotifications";
import type { Notification, PaginatedResponse } from "../types";

// `api/client.ts` todavía no expone los métodos de avisos en RED, así que el
// mock es un objeto propio (hoisted) en vez de parchear el import real.
const mockApi = vi.hoisted(() => ({
  listUnreadNotifications: vi.fn(),
  markNotificationsRead: vi.fn(),
  markAllNotificationsRead: vi.fn(),
}));

vi.mock("../api/client", () => ({
  api: mockApi,
  BASE_URL: "/api",
}));

const notification = (id: string): Notification => ({
  id,
  origin: "strava",
  author: "Valet",
  message: `Aviso ${id}`,
  level: "info",
  metadata: null,
  created_at: "2026-10-13 19:12:33",
});

const paginated = (
  items: Notification[],
  next_cursor: string | null,
  total = items.length,
): PaginatedResponse<Notification> => ({ data: items, next_cursor, total });

describe("useNotifications", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads the first page on mount", async () => {
    mockApi.listUnreadNotifications.mockResolvedValue(
      paginated([notification("n1")], null, 1),
    );

    const { result } = renderHook(() => useNotifications());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(mockApi.listUnreadNotifications).toHaveBeenCalledWith({ limit: 10 });
    expect(result.current.notifications).toHaveLength(1);
    expect(result.current.total).toBe(1);
    expect(result.current.page).toBe(1);
    expect(result.current.hasPrev).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it("goNext fetches the next page with the cursor", async () => {
    mockApi.listUnreadNotifications
      .mockResolvedValueOnce(paginated([notification("n1")], "c1", 25))
      .mockResolvedValueOnce(paginated([notification("n2")], null, 25));

    const { result } = renderHook(() => useNotifications());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    await act(async () => {
      result.current.goNext();
    });

    await waitFor(() => {
      expect(result.current.page).toBe(2);
    });

    expect(mockApi.listUnreadNotifications).toHaveBeenLastCalledWith({
      limit: 10,
      cursor: "c1",
    });
    expect(result.current.hasPrev).toBe(true);
  });

  it("goPrev returns to the first page", async () => {
    mockApi.listUnreadNotifications
      .mockResolvedValueOnce(paginated([notification("n1")], "c1", 25))
      .mockResolvedValueOnce(paginated([notification("n2")], null, 25));

    const { result } = renderHook(() => useNotifications());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    await act(async () => {
      result.current.goNext();
    });

    await waitFor(() => {
      expect(result.current.page).toBe(2);
    });

    await act(async () => {
      result.current.goPrev();
    });

    await waitFor(() => {
      expect(result.current.page).toBe(1);
    });

    expect(mockApi.listUnreadNotifications).toHaveBeenLastCalledWith({ limit: 10 });
  });

  it("hasNext is false when the server issues no cursor", async () => {
    mockApi.listUnreadNotifications.mockResolvedValue(
      paginated([notification("n1")], null, 1),
    );

    const { result } = renderHook(() => useNotifications());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.hasNext).toBe(false);

    await act(async () => {
      result.current.goNext();
    });

    expect(mockApi.listUnreadNotifications).toHaveBeenCalledTimes(1);
  });

  it("markRead marks one notification and refetches", async () => {
    mockApi.listUnreadNotifications.mockResolvedValue(
      paginated([notification("n1")], null, 1),
    );
    mockApi.markNotificationsRead.mockResolvedValue({ marked: 1 });

    const { result } = renderHook(() => useNotifications());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    await act(async () => {
      await result.current.markRead("n1");
    });

    expect(mockApi.markNotificationsRead).toHaveBeenCalledWith(["n1"]);
    await waitFor(() => {
      expect(mockApi.listUnreadNotifications).toHaveBeenCalledTimes(2);
    });
  });

  it("markAll marks everything and empties the list", async () => {
    mockApi.listUnreadNotifications
      .mockResolvedValueOnce(paginated([notification("n1")], null, 1))
      .mockResolvedValueOnce(paginated([], null, 0));
    mockApi.markAllNotificationsRead.mockResolvedValue({ marked: 1 });

    const { result } = renderHook(() => useNotifications());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    await act(async () => {
      await result.current.markAll();
    });

    expect(mockApi.markAllNotificationsRead).toHaveBeenCalledTimes(1);
    await waitFor(() => {
      expect(result.current.notifications).toHaveLength(0);
    });
  });

  it("markAll resets to the first page", async () => {
    mockApi.listUnreadNotifications
      .mockResolvedValueOnce(paginated([notification("n1")], "c1", 25))
      .mockResolvedValueOnce(paginated([notification("n2")], null, 25))
      .mockResolvedValueOnce(paginated([], null, 0));
    mockApi.markAllNotificationsRead.mockResolvedValue({ marked: 25 });

    const { result } = renderHook(() => useNotifications());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    await act(async () => {
      result.current.goNext();
    });

    await waitFor(() => {
      expect(result.current.page).toBe(2);
    });

    await act(async () => {
      await result.current.markAll();
    });

    await waitFor(() => {
      expect(result.current.page).toBe(1);
    });

    // La recarga va sin cursor: vuelve a la primera página.
    expect(mockApi.listUnreadNotifications).toHaveBeenLastCalledWith({
      limit: 10,
    });
    expect(result.current.hasPrev).toBe(false);
  });

  it("markRead goes back a page when the last page becomes empty", async () => {
    mockApi.listUnreadNotifications
      .mockResolvedValueOnce(paginated([notification("n1")], "c1", 2))
      .mockResolvedValueOnce(paginated([notification("n2")], null, 2))
      .mockResolvedValueOnce(paginated([], null, 1))
      .mockResolvedValueOnce(paginated([notification("n1")], null, 1));
    mockApi.markNotificationsRead.mockResolvedValue({ marked: 1 });

    const { result } = renderHook(() => useNotifications());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    await act(async () => {
      result.current.goNext();
    });

    await waitFor(() => {
      expect(result.current.page).toBe(2);
    });

    await act(async () => {
      await result.current.markRead("n2");
    });

    await waitFor(() => {
      expect(result.current.page).toBe(1);
    });

    // Tras vaciarse la página 2, se pide la página 1 sin cursor.
    expect(mockApi.listUnreadNotifications).toHaveBeenLastCalledWith({
      limit: 10,
    });
  });

  it("markAll exposes the error when the request rejects", async () => {
    mockApi.listUnreadNotifications.mockResolvedValue(
      paginated([notification("n1")], null, 1),
    );
    mockApi.markAllNotificationsRead.mockRejectedValue(new Error("nope"));

    const { result } = renderHook(() => useNotifications());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    await act(async () => {
      await result.current.markAll();
    });

    expect(result.current.error).toBe("nope");
  });

  it("exposes the error message when the request rejects", async () => {
    mockApi.listUnreadNotifications.mockRejectedValue(new Error("boom"));

    const { result } = renderHook(() => useNotifications());

    await waitFor(() => {
      expect(result.current.error).toBe("boom");
    });

    expect(result.current.loading).toBe(false);
  });
});
