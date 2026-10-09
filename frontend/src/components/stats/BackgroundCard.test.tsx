import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, within } from "@testing-library/react";
import { BackgroundCard } from "./BackgroundCard";
import type { BackgroundStats } from "../../types";

/** Entrada de un origen sin actividad (todos los agregados a cero). */
const zero = (kind: string): BackgroundStats => ({
  kind,
  calls: 0,
  input_tokens: 0,
  output_tokens: 0,
  total_tokens: 0,
  total_cost: 0,
  total_errors: 0,
  avg_duration_ms: null,
});

const mockBackground: BackgroundStats[] = [
  {
    kind: "router",
    calls: 42,
    input_tokens: 800,
    output_tokens: 434,
    total_tokens: 1234,
    total_cost: 0.056789,
    total_errors: 2,
    avg_duration_ms: 250,
  },
  zero("archivist"),
  zero("consolidator"),
  zero("collapse"),
];

describe("BackgroundCard", () => {
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

  it("renders one row per origin with its figures", () => {
    render(<BackgroundCard data={mockBackground} loading={false} />);

    // Una fila por origen no-chat.
    expect(screen.getByText("router")).toBeInTheDocument();
    expect(screen.getByText("archivist")).toBeInTheDocument();
    expect(screen.getByText("consolidator")).toBeInTheDocument();
    expect(screen.getByText("collapse")).toBeInTheDocument();

    // Cifras del origen con actividad, aisladas en su propia fila.
    const routerRow = within(screen.getByRole("row", { name: /router/ }));
    expect(routerRow.getByText("42")).toBeInTheDocument();
    expect(routerRow.getByText("1,234")).toBeInTheDocument();
    expect(routerRow.getByText("$0.056789")).toBeInTheDocument();
    expect(routerRow.getByText("250 ms")).toBeInTheDocument();
    expect(routerRow.getByText("2")).toBeInTheDocument();

    // Las cifras del origen con actividad no se filtran a los orígenes a cero.
    const archivistRow = within(screen.getByRole("row", { name: /archivist/ }));
    expect(archivistRow.queryByText("42")).not.toBeInTheDocument();
    expect(archivistRow.queryByText("1,234")).not.toBeInTheDocument();
    expect(archivistRow.queryByText("$0.056789")).not.toBeInTheDocument();
    expect(archivistRow.queryByText("250 ms")).not.toBeInTheDocument();
  });

  it("renders an empty state when all origins are zero", () => {
    const empty: BackgroundStats[] = [
      zero("router"),
      zero("archivist"),
      zero("consolidator"),
      zero("collapse"),
    ];
    render(<BackgroundCard data={empty} loading={false} />);

    expect(screen.getByText("No data yet")).toBeInTheDocument();
    expect(screen.queryByText("router")).not.toBeInTheDocument();
  });

  it("renders an empty state when there is no data", () => {
    render(<BackgroundCard data={[]} loading={false} />);

    expect(screen.getByText("No data yet")).toBeInTheDocument();
  });

  it("renders a skeleton while loading", () => {
    const { container } = render(<BackgroundCard data={mockBackground} loading={true} />);

    expect(container.querySelector(".ant-skeleton")).toBeInTheDocument();
    expect(screen.queryByText("No data yet")).not.toBeInTheDocument();
  });

  it("renders N/A when the average duration is null", () => {
    const nullLatency: BackgroundStats[] = [
      { ...mockBackground[0], avg_duration_ms: null },
    ];
    render(<BackgroundCard data={nullLatency} loading={false} />);

    expect(screen.getByText("N/A")).toBeInTheDocument();
  });
});
