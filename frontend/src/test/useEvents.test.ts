import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { useEvents } from "../hooks/useEvents";
import type { CalendarEvent } from "../types";
import { api } from "../api/client";

const mockEvents: CalendarEvent[] = [
  {
    id: "1",
    title: "Test Event",
    start_time: "2026-09-26T10:00:00Z",
    end_time: "2026-09-26T11:00:00Z",
    category: "work",
    scope: "shared",
    all_day: false,
    profile_id: "p1",
    created_at: "2026-09-26T00:00:00Z",
    updated_at: "2026-09-26T00:00:00Z",
  },
];

const mockNewEvent: CalendarEvent = {
  id: "2",
  title: "New Event",
  start_time: "2026-09-26T14:00:00Z",
  end_time: "2026-09-26T15:00:00Z",
  category: "personal",
  scope: "shared",
  all_day: false,
  profile_id: "p1",
  created_at: "2026-09-26T00:00:00Z",
  updated_at: "2026-09-26T00:00:00Z",
};

vi.mock("../api/client", () => ({
  api: {
    listEvents: vi.fn(),
  },
  BASE_URL: "http://localhost:3000",
}));

describe("useEvents", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.listEvents).mockResolvedValue(mockEvents);
  });

  afterEach(() => {
    // Clean up any custom event listeners between tests
    window.dispatchEvent(new CustomEvent("cleanup-test"));
  });

  it("fetches events on mount", async () => {
    const { result } = renderHook(() => useEvents("2026-09-01", "2026-09-30"));

    expect(result.current.loading).toBe(true);

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(api.listEvents).toHaveBeenCalledTimes(1);
    expect(api.listEvents).toHaveBeenCalledWith("2026-09-01", "2026-09-30");
    expect(result.current.events).toEqual(mockEvents);
  });

  it("refetches when events-changed custom event is dispatched", async () => {
    // First load
    const { result } = renderHook(() => useEvents("2026-09-01", "2026-09-30"));

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(api.listEvents).toHaveBeenCalledTimes(1);

    // Now mock a different response for the refetch
    vi.mocked(api.listEvents).mockResolvedValue([...mockEvents, mockNewEvent]);

    // Dispatch the custom event
    window.dispatchEvent(new CustomEvent("events-changed"));

    // Wait for refetch to complete
    await waitFor(() => {
      expect(api.listEvents).toHaveBeenCalledTimes(2);
    });

    await waitFor(() => {
      expect(result.current.events).toHaveLength(2);
    });
    expect(result.current.events[1].title).toBe("New Event");
  });

  it("does not refetch for unrelated custom events", async () => {
    const { result } = renderHook(() => useEvents("2026-09-01", "2026-09-30"));

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(api.listEvents).toHaveBeenCalledTimes(1);

    // Dispatch unrelated event
    window.dispatchEvent(new CustomEvent("some-other-event"));

    // Wait a bit to ensure no extra call
    await new Promise((resolve) => setTimeout(resolve, 100));

    expect(api.listEvents).toHaveBeenCalledTimes(1);
  });

  it("removes event listener on unmount", async () => {
    const { result, unmount } = renderHook(() =>
      useEvents("2026-09-01", "2026-09-30"),
    );

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(api.listEvents).toHaveBeenCalledTimes(1);

    // Unmount the hook
    unmount();

    // Dispatch after unmount — should NOT trigger refetch
    window.dispatchEvent(new CustomEvent("events-changed"));

    // Wait a bit to ensure no extra call
    await new Promise((resolve) => setTimeout(resolve, 100));

    expect(api.listEvents).toHaveBeenCalledTimes(1);
  });
});
