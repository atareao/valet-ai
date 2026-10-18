import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { App as AntdApp } from "antd";
import type { ReactElement, ReactNode } from "react";
import { StatsDashboard } from "./StatsDashboard";
import { api } from "../api/client";
import type { ModelStats } from "../types";

/**
 * Carrera de ventanas (reserva 2 del change `stats-role-model-view`).
 *
 * Por la UI real la carrera es difícil de disparar: mientras hay una petición
 * en vuelo `DailyChart` oculta el selector (skeleton), así que el usuario no
 * puede encadenar dos ventanas solapadas. Aquí se sustituye `DailyChart` por un
 * stub cuyo selector está **siempre** visible, de modo que sí podemos emitir
 * dos peticiones solapadas y comprobar que una respuesta obsoleta no pisa la de
 * la ventana vigente. El resto del dashboard es el real.
 */

vi.mock("../api/client", () => ({
  api: {
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
}));

// `react-chartjs-2` monta canvas que jsdom no resuelve.
vi.mock("react-chartjs-2", () => ({
  Bar: () => <div data-testid="chart-stub" />,
  Line: () => <div data-testid="chart-stub" />,
  Doughnut: () => <div data-testid="chart-stub" />,
}));

// Stub del selector de rango: visible incluso con una petición en vuelo. Se
// conserva el texto «Daily Activity» para no alterar las aserciones de vista.
vi.mock("../components/stats/DailyChart", () => ({
  DailyChart: ({
    onRangeChange,
  }: {
    onRangeChange: (days: number) => void;
  }) => (
    <div>
      <span>Daily Activity</span>
      <button onClick={() => onRangeChange(7)}>7d</button>
      <button onClick={() => onRangeChange(30)}>30d</button>
    </div>
  ),
}));

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

const mockMemory = {
  total_memories: 120,
  total_tokens: 50000,
  messages_total: 1000,
  messages_indexed: 800,
};

const model = (name: string): ModelStats => ({
  model: name,
  calls: 1,
  total_tokens: 100,
  total_cost: 0.5,
  avg_duration_ms: 100,
  total_cached_tokens: 0,
  total_reasoning_tokens: 0,
});

type Pending = { days: number | undefined; resolve: (value: ModelStats[]) => void };

const AppWrapper = ({ children }: { children: ReactNode }) => (
  <AntdApp>{children}</AntdApp>
);
const renderDashboard = (ui: ReactElement) =>
  render(ui, { wrapper: AppWrapper });

describe("StatsDashboard – carrera de ventanas", () => {
  let pending: Pending[];

  beforeEach(() => {
    vi.resetAllMocks();
    pending = [];
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

    vi.mocked(api.getStatsSummary).mockResolvedValue(mockSummary);
    vi.mocked(api.getStatsByDay).mockResolvedValue([]);
    vi.mocked(api.getStatsTools).mockResolvedValue([]);
    vi.mocked(api.getDbSizes).mockResolvedValue([]);
    vi.mocked(api.getMemoryStats).mockResolvedValue(mockMemory);
    vi.mocked(api.getStatsBackground).mockResolvedValue([]);
    vi.mocked(api.getLastApiCall).mockResolvedValue(null);
    // Cada llamada a by-model devuelve su propia promesa, que resolveremos a
    // mano en el orden que interese para forzar la carrera.
    vi.mocked(api.getStatsByModel).mockImplementation(
      (days?: number) =>
        new Promise<ModelStats[]>((resolve) => {
          pending.push({ days, resolve });
        }),
    );
  });

  it("solo aplica la respuesta de la ventana vigente aunque una obsoleta llegue después", async () => {
    renderDashboard(<StatsDashboard />);

    // Primer lote (7d por defecto): se resuelve para que la vista renderice.
    await waitFor(() => expect(pending).toHaveLength(1));
    expect(pending[0].days).toBe(7);
    pending[0].resolve([model("window-7-initial")]);

    // Abrimos la pestaña de modelos y disparamos dos ventanas solapadas.
    fireEvent.click(await screen.findByText("🤖 Modelos"));

    fireEvent.click(await screen.findByRole("button", { name: "30d" }));
    fireEvent.click(screen.getByRole("button", { name: "7d" }));

    await waitFor(() => expect(pending).toHaveLength(3));
    // pending[1] = 30d (más antigua), pending[2] = 7d (la vigente, más reciente).
    expect(pending[1].days).toBe(30);
    expect(pending[2].days).toBe(7);

    // La nueva (7d) se resuelve ANTES que la vieja (30d)…
    pending[2].resolve([model("window-7")]);
    expect(await screen.findByText("window-7")).toBeInTheDocument();

    // …y la vieja llega tarde: NO debe pisar la ventana vigente.
    pending[1].resolve([model("window-30")]);
    await waitFor(() =>
      expect(screen.queryByText("window-30")).not.toBeInTheDocument(),
    );
    expect(screen.getByText("window-7")).toBeInTheDocument();
  });
});
