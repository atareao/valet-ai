import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Notification, PaginatedResponse } from "../types";

// Se mockea solo el cliente HTTP: el hook `useNotifications` corre de verdad
// (spec `notifications-inbox`, frontend). Antd necesita `matchMedia`.
// `api/client.ts` aún no expone los métodos de avisos en RED, así que el mock
// es un objeto propio (hoisted).
const mockApi = vi.hoisted(() => ({
  listUnreadNotifications: vi.fn(),
  markNotificationsRead: vi.fn(),
  markAllNotificationsRead: vi.fn(),
}));

vi.mock("../api/client", () => ({
  api: mockApi,
  BASE_URL: "/api",
}));

import { NotificationsView } from "../components/NotificationsView";

beforeEach(() => {
  vi.clearAllMocks();
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

const notification = (id: string, overrides: Partial<Notification> = {}): Notification => ({
  id,
  origin: "strava",
  author: "Valet",
  message: `Aviso ${id}`,
  level: "info",
  metadata: null,
  created_at: "2026-10-13 19:12:33",
  ...overrides,
});

const paginated = (
  items: Notification[],
  next_cursor: string | null,
  total = items.length,
): PaginatedResponse<Notification> => ({ data: items, next_cursor, total });

describe("NotificationsView", () => {
  it("lists the pending notifications with origin, author and message", async () => {
    mockApi.listUnreadNotifications.mockResolvedValue(
      paginated(
        [
          notification("n1", {
            origin: "strava",
            author: "Valet",
            message: "Nueva actividad",
          }),
        ],
        null,
        25,
      ),
    );

    render(<NotificationsView />);

    expect(await screen.findByText("Nueva actividad")).toBeInTheDocument();
    expect(screen.getByText(/strava/i)).toBeInTheDocument();
    expect(screen.getByText(/valet/i)).toBeInTheDocument();
    expect(screen.getByText(/25 avisos pendientes/i)).toBeInTheDocument();
  });

  it("renders created_at as UTC", async () => {
    // El esperado se calcula igual que el componente debe hacerlo
    // (`Date.UTC` → `toLocaleString()`), de modo que el test vale en cualquier
    // `TZ` del runner. Si el componente mostrara el crudo o `Invalid Date`,
    // este test fallaría.
    const expected = new Date(Date.UTC(2026, 9, 13, 19, 12, 33)).toLocaleString();

    mockApi.listUnreadNotifications.mockResolvedValue(
      paginated([notification("n1", { created_at: "2026-10-13 19:12:33" })], null, 1),
    );

    render(<NotificationsView />);

    expect(await screen.findByText(expected)).toBeInTheDocument();
    expect(screen.queryByText("Invalid Date")).not.toBeInTheDocument();
    expect(screen.queryByText("2026-10-13 19:12:33")).not.toBeInTheDocument();
  });

  it("renders an em dash when created_at is unparseable", async () => {
    mockApi.listUnreadNotifications.mockResolvedValue(
      paginated([notification("n1", { created_at: "no-es-fecha" })], null, 1),
    );

    render(<NotificationsView />);

    expect(await screen.findByText("—")).toBeInTheDocument();
    expect(screen.queryByText("Invalid Date")).not.toBeInTheDocument();
    expect(screen.queryByText("no-es-fecha")).not.toBeInTheDocument();
  });

  it("marks a single notification as read", async () => {
    const user = userEvent.setup();
    mockApi.listUnreadNotifications
      .mockResolvedValueOnce(paginated([notification("n1")], null, 1))
      .mockResolvedValueOnce(paginated([], null, 0));
    mockApi.markNotificationsRead.mockResolvedValue({ marked: 1 });

    render(<NotificationsView />);

    const button = await screen.findByRole("button", {
      name: /marcar como leído/i,
    });
    await user.click(button);

    await waitFor(() => {
      expect(mockApi.markNotificationsRead).toHaveBeenCalledWith(["n1"]);
    });
    await waitFor(() => {
      expect(screen.getByText(/no hay avisos pendientes/i)).toBeInTheDocument();
    });
  });

  it("marks all as read", async () => {
    const user = userEvent.setup();
    mockApi.listUnreadNotifications
      .mockResolvedValueOnce(paginated([notification("n1")], null, 1))
      .mockResolvedValueOnce(paginated([], null, 0));
    mockApi.markAllNotificationsRead.mockResolvedValue({ marked: 1 });

    render(<NotificationsView />);

    const button = await screen.findByRole("button", {
      name: /marcar todos como leídos/i,
    });
    await user.click(button);

    await waitFor(() => {
      expect(mockApi.markAllNotificationsRead).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(screen.getByText(/no hay avisos pendientes/i)).toBeInTheDocument();
    });
  });

  it("shows the empty state when there are no pending notifications", async () => {
    mockApi.listUnreadNotifications.mockResolvedValue(
      paginated([], null, 0),
    );

    render(<NotificationsView />);

    expect(
      await screen.findByText(/no hay avisos pendientes/i),
    ).toBeInTheDocument();
  });

  it("navigates to the next page and disables Siguiente on the last page", async () => {
    const user = userEvent.setup();
    const firstPage = Array.from({ length: 10 }, (_, i) => notification(`n${i + 1}`));
    const secondPage = Array.from({ length: 10 }, (_, i) => notification(`n${i + 11}`));
    mockApi.listUnreadNotifications
      .mockResolvedValueOnce(paginated(firstPage, "c1", 20))
      .mockResolvedValueOnce(paginated(secondPage, null, 20));

    render(<NotificationsView />);

    const next = await screen.findByRole("button", { name: /siguiente/i });
    expect(next).toBeEnabled();

    await user.click(next);

    await waitFor(() => {
      expect(mockApi.listUnreadNotifications).toHaveBeenLastCalledWith({
        limit: 10,
        cursor: "c1",
      });
    });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /siguiente/i })).toBeDisabled();
    });
  });
});
