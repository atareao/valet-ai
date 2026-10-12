import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { ModelChart } from "./ModelChart";
import type { ModelStats } from "../../types";

// El `<Bar>` real monta un canvas que jsdom no puede resolver. Se intercepta
// para capturar las props que `ModelChart` le pasa: eso permite verificar el
// **número de barras** y el **orden** de las etiquetas/datos sin depender de
// canvas.
const { barProps } = vi.hoisted(() => ({
  barProps: [] as Array<{
    data: { labels: string[]; datasets: Array<{ data: number[] }> };
  }>,
}));

vi.mock("react-chartjs-2", () => ({
  Bar: (props: {
    data: { labels: string[]; datasets: Array<{ data: number[] }> };
  }) => {
    barProps.push(props);
    return <div data-testid="bar-stub" />;
  },
}));

const model = (overrides: Partial<ModelStats>): ModelStats => ({
  model: "gpt-4o",
  calls: 10,
  total_tokens: 1000,
  total_cost: 0.1,
  avg_duration_ms: 120,
  total_cached_tokens: 0,
  total_reasoning_tokens: 0,
  ...overrides,
});

describe("ModelChart", () => {
  beforeEach(() => {
    barProps.length = 0;
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

  it("rotula el panel como agregado de todos los orígenes (ya no solo chat)", () => {
    render(<ModelChart data={[model({})]} loading={false} />);

    const title = screen.getByText(/cost by model/i);
    expect(title.textContent?.toLowerCase()).toContain("all origins");
    expect(title.textContent?.toLowerCase()).not.toContain("chat");
  });

  it("incluye en el panel un modelo que solo trabaja en un rol de fondo", () => {
    render(
      <ModelChart
        data={[
          model({
            model: "qwen/qwen3-235b-a22b-2507",
            calls: 33,
            total_tokens: 242522,
            total_cost: 0.12,
          }),
        ]}
        loading={false}
      />,
    );

    expect(
      screen.getByText("qwen/qwen3-235b-a22b-2507"),
    ).toBeInTheDocument();
  });

  it("dibuja una barra por modelo y respeta el orden recibido (coste descendente)", () => {
    // Escenario «Gráfico con 2 modelos» del delta. `ModelChart` NO ordena por
    // coste: pinta el orden que recibe. La ordenación por coste descendente es
    // responsabilidad del backend (`/api/stats/llm/by-model`), y aquí se
    // verifica que el gráfico dibuja 2 barras y conserva ese orden en las
    // etiquetas y en los valores.
    const data = [
      model({ model: "gpt-4o", total_cost: 1.0 }),
      model({ model: "claude-3", total_cost: 0.25 }),
    ];
    render(<ModelChart data={data} loading={false} />);

    expect(barProps).toHaveLength(1);
    const chart = barProps[0];
    // 2 modelos → 2 barras.
    expect(chart.data.labels).toHaveLength(2);
    expect(chart.data.labels).toEqual(["gpt-4o", "claude-3"]);
    expect(chart.data.datasets[0].data).toEqual([1.0, 0.25]);
  });
});
