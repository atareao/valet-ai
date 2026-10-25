import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import dayjs from "dayjs";
import type { CalendarEvent } from "../types";

// ---------------------------------------------------------------------------
// Ant Design matchMedia mock (jsdom does not implement window.matchMedia) —
// CalendarView usa useMediaQuery.
// ---------------------------------------------------------------------------
beforeEach(() => {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
});

const { mockUseEvents } = vi.hoisted(() => ({
  mockUseEvents: vi.fn(),
}));

vi.mock("../hooks/useEvents", () => ({
  useEvents: mockUseEvents,
  useCreateEvent: vi.fn(() => ({ create: vi.fn(), loading: false })),
  useUpdateEvent: vi.fn(() => ({ update: vi.fn(), loading: false })),
  // `CalendarView` también consume `useDeleteEvent` (change
  // `calendar-event-delete`): el mock parcial debe exponerlo o el render falla.
  useDeleteEvent: vi.fn(() => ({ delete: vi.fn(), loading: false })),
}));

import { CalendarView } from "../components/CalendarView";
import { CATEGORY_COLORS } from "../components/calendar.constants";

function eventToday(
  id: string,
  title: string,
  category: CalendarEvent["category"],
): CalendarEvent {
  const start = dayjs().hour(10).minute(0).second(0);
  return {
    id,
    title,
    start_time: start.toISOString(),
    end_time: start.add(1, "hour").toISOString(),
    category,
    scope: "shared",
    all_day: false,
    profile_id: "p1",
    created_at: "2026-09-26T00:00:00Z",
    updated_at: "2026-09-26T00:00:00Z",
  };
}

const events: CalendarEvent[] = [
  eventToday("1", "Work task", "work"),
  eventToday("2", "Personal task", "personal"),
  eventToday("3", "Health task", "health"),
];

function dotFor(title: string): HTMLElement {
  const titleEl = screen.getByText(title);
  const badge = titleEl.closest(".ant-badge");
  const dot = badge?.querySelector<HTMLElement>(".ant-badge-status-dot");
  if (!dot) throw new Error(`No badge dot found for "${title}"`);
  return dot;
}

describe("CalendarView — caracterización", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseEvents.mockReturnValue({
      events,
      loading: false,
      error: null,
      refetch: vi.fn(),
    });
  });

  it("renderiza la estructura base del calendario", () => {
    render(<CalendarView />);

    expect(screen.getByText("Calendar")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /New Event/i }),
    ).toBeInTheDocument();
  });

  it("muestra los eventos con el color de su categoría", () => {
    render(<CalendarView />);

    expect(screen.getByText("Work task")).toBeInTheDocument();
    expect(screen.getByText("Personal task")).toBeInTheDocument();
    expect(screen.getByText("Health task")).toBeInTheDocument();

    expect(dotFor("Work task")).toHaveStyle({
      backgroundColor: CATEGORY_COLORS.work,
    });
    expect(dotFor("Personal task")).toHaveStyle({
      backgroundColor: CATEGORY_COLORS.personal,
    });
    expect(dotFor("Health task")).toHaveStyle({
      backgroundColor: CATEGORY_COLORS.health,
    });
  });
});
