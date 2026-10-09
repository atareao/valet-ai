import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
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

    // Cifras del origen con actividad: llamadas, tokens, coste, latencia y errores.
    expect(screen.getByText("42")).toBeInTheDocument();
    expect(screen.getByText("1234")).toBeInTheDocument();
    expect(screen.getByText("$0.056789")).toBeInTheDocument();
    expect(screen.getByText("250 ms")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
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
});
