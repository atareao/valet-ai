import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  render,
  renderHook,
  screen,
  fireEvent,
  waitFor,
} from "@testing-library/react";
import { App as AntdApp } from "antd";
import type { ReactElement, ReactNode } from "react";
import { useEvents } from "../hooks/useEvents";
import { useTasks } from "../hooks/useTasks";
import { StatsDashboard } from "../pages/StatsDashboard";
import type { CalendarEvent, Task } from "../types";
import { api } from "../api/client";

/**
 * Regresión del change frontend-lint-zero: al mover la marca de "petición en
 * curso" fuera del efecto, `loading` dejaba de volver a `true` cuando el
 * efecto cambiaba de dependencias (rango de fechas / filtros). Estas pruebas
 * fijan ese comportamiento observable.
 */

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

const mockTasks: Task[] = [
  {
    id: "1",
    profile_id: "p1",
    content: "Test task",
    status: "todo",
    priority: "medium",
    scope: "shared",
    created_at: "2026-09-26T00:00:00Z",
    updated_at: "2026-09-26T00:00:00Z",
  },
];

const mockSummary = {
  total_calls: 150,
  total_prompt_tokens: 50000,
  total_completion_tokens: 30000,
  total_tokens: 80000,
  total_cached_tokens: 10000,
  total_reasoning_tokens: 5000,
  total_cost: 0.123456,
  total_errors: 3,
  avg_duration_ms: 2500,
};

const mockTools = [{ tool: "search", count: 45 }];
const mockDbSizes = [{ table: "messages", rows: 5000 }];
const mockMemory = {
  total_memories: 120,
  total_tokens: 50000,
  messages_total: 1000,
  messages_indexed: 800,
};

vi.mock("../api/client", () => ({
  api: {
    listEvents: vi.fn(),
    listTasks: vi.fn(),
    getStatsSummary: vi.fn(),
    getStatsByModel: vi.fn(),
    getStatsByDay: vi.fn(),
    getStatsTools: vi.fn(),
    getDbSizes: vi.fn(),
    getMemoryStats: vi.fn(),
    getStatsBackground: vi.fn(),
    getLastApiCall: vi.fn(),
    getRetention: vi.fn(),
    setRetention: vi.fn(),
  },
  BASE_URL: "/api",
}));

// antd `App.useApp()` exige un `<App>` ancestro.
const AppWrapper = ({ children }: { children: ReactNode }) => (
  <AntdApp>{children}</AntdApp>
);

const renderDashboard = (ui: ReactElement) =>
  render(ui, { wrapper: AppWrapper });

// Nunca resuelve: mantiene la petición "en curso" mientras dura la aserción.
const pending = <T,>() => new Promise<T>(() => {});

describe("recarga al cambiar dependencias", () => {
  beforeEach(() => {
    vi.resetAllMocks();
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

  it("useEvents vuelve a loading=true al cambiar start/end", async () => {
    vi.mocked(api.listEvents).mockResolvedValue(mockEvents);

    const { result, rerender } = renderHook(
      ({ start, end }: { start: string; end: string }) =>
        useEvents(start, end),
      { initialProps: { start: "2026-09-01", end: "2026-09-30" } },
    );

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    // El nuevo rango tarda en resolver: el Spin debe volver a aparecer.
    vi.mocked(api.listEvents).mockReturnValue(pending<CalendarEvent[]>());
    rerender({ start: "2026-10-01", end: "2026-10-31" });

    await waitFor(() => {
      expect(result.current.loading).toBe(true);
    });
  });

  it("useTasks vuelve a loading=true al cambiar los filtros", async () => {
    vi.mocked(api.listTasks).mockResolvedValue(mockTasks);

    const { result, rerender } = renderHook(
      ({ filters }: { filters?: Record<string, string> }) =>
        useTasks(filters),
      {
        initialProps: {
          filters: undefined as Record<string, string> | undefined,
        },
      },
    );

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    vi.mocked(api.listTasks).mockReturnValue(pending<Task[]>());
    rerender({ filters: { status: "todo" } });

    await waitFor(() => {
      expect(result.current.loading).toBe(true);
    });
  });

  it("StatsDashboard vuelve a mostrar el indicador de carga al cambiar el rango", async () => {
    vi.mocked(api.getStatsSummary).mockResolvedValue(mockSummary);
    // Sin filas: evita que chart.js intente pintar en jsdom. Basta con que el
    // selector de rango esté presente para ejercitar el cambio de dependencia.
    vi.mocked(api.getStatsByModel).mockResolvedValue([]);
    vi.mocked(api.getStatsByDay).mockResolvedValue([]);
    vi.mocked(api.getStatsTools).mockResolvedValue(mockTools);
    vi.mocked(api.getDbSizes).mockResolvedValue(mockDbSizes);
    vi.mocked(api.getMemoryStats).mockResolvedValue(mockMemory);
    vi.mocked(api.getStatsBackground).mockResolvedValue([]);
    vi.mocked(api.getLastApiCall).mockResolvedValue(null);

    renderDashboard(<StatsDashboard />);

    // Espera a la carga inicial (sin indicador).
    await screen.findByText("LLM Usage Summary");
    expect(document.querySelector(".ant-skeleton")).toBeNull();

    // El nuevo rango queda pendiente: las tarjetas deben mostrar su skeleton.
    vi.mocked(api.getStatsByDay).mockReturnValue(pending());

    fireEvent.click(screen.getByText("🤖 Modelos"));
    fireEvent.click(await screen.findByRole("button", { name: "7d" }));

    await waitFor(() => {
      expect(document.querySelector(".ant-skeleton")).not.toBeNull();
    });
  });
});
