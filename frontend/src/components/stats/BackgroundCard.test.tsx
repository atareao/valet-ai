import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, within } from "@testing-library/react";
import { BackgroundCard } from "./BackgroundCard";
import { parseSqliteTimestamp } from "../../utils/sqliteTimestamp";
import type { BackgroundStats } from "../../types";

/**
 * Construye una pareja (rol, modelo). Por defecto sin actividad; los tests
 * sobrescriben lo que importe.
 */
const pair = (
  kind: string,
  model: string | null,
  overrides: Partial<BackgroundStats> = {},
): BackgroundStats => ({
  kind,
  model,
  calls: 0,
  input_tokens: 0,
  output_tokens: 0,
  total_tokens: 0,
  total_cost: 0,
  total_errors: 0,
  suspicious_calls: 0,
  last_call_at: null,
  avg_duration_ms: null,
  ...overrides,
});

/** Entrada de un origen sin filas (contadores a cero y sin modelo). */
const zero = (kind: string): BackgroundStats => pair(kind, null);

const mockBackground: BackgroundStats[] = [
  pair("router", "gpt-4o", {
    calls: 42,
    input_tokens: 800,
    output_tokens: 434,
    total_tokens: 1234,
    total_cost: 0.056789,
    total_errors: 2,
    avg_duration_ms: 250,
  }),
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

  it("etiqueta el origen timeline con su nombre legible", () => {
    const withTimeline: BackgroundStats[] = [
      pair("timeline", "mistralai/mistral-small-24b", {
        calls: 3,
        input_tokens: 100,
        output_tokens: 50,
        total_tokens: 150,
        total_cost: 0.001,
        total_errors: 0,
        avg_duration_ms: 120,
      }),
    ];
    render(<BackgroundCard data={withTimeline} loading={false} />);

    const row = within(screen.getByRole("row", { name: /timeline/ }));
    // Etiqueta legible y `kind` crudo conviven en la fila.
    expect(row.getByText("Línea temporal")).toBeInTheDocument();
    expect(row.getByText("timeline")).toBeInTheDocument();
  });

  it("etiqueta el origen journal con su nombre legible", () => {
    const withJournal: BackgroundStats[] = [
      pair("journal", "role-model", {
        calls: 2,
        input_tokens: 100,
        output_tokens: 50,
        total_tokens: 150,
        total_cost: 0.001,
        total_errors: 0,
        avg_duration_ms: 120,
      }),
    ];
    render(<BackgroundCard data={withJournal} loading={false} />);

    const row = within(screen.getByRole("row", { name: /journal/ }));
    // Etiqueta legible y `kind` crudo conviven en la fila.
    expect(row.getByText("Bitácora")).toBeInTheDocument();
    expect(row.getByText("journal")).toBeInTheDocument();
  });

  it("renderiza una fila por pareja (rol, modelo) cuando un rol tiene dos modelos", () => {
    const data: BackgroundStats[] = [
      pair("consolidator", "qwen/qwen3-235b-a22b-2507", { calls: 33 }),
      pair("consolidator", "deepseek/deepseek-v4.1-flash", { calls: 54 }),
    ];
    render(<BackgroundCard data={data} loading={false} />);

    const rows = screen.getAllByRole("row", { name: /consolidator/ });
    expect(rows).toHaveLength(2);
    expect(
      within(rows[0]).getByText("qwen/qwen3-235b-a22b-2507"),
    ).toBeInTheDocument();
    expect(
      within(rows[1]).getByText("deepseek/deepseek-v4.1-flash"),
    ).toBeInTheDocument();
  });

  it("muestra un rol sin filas con ceros y sin modelo, de forma legible", () => {
    const data: BackgroundStats[] = [
      pair("router", "gpt-4o", { calls: 5 }),
      zero("collapse"),
    ];
    render(<BackgroundCard data={data} loading={false} />);

    const row = within(screen.getByRole("row", { name: /collapse/ }));
    expect(row.getByText(/sin modelo/i)).toBeInTheDocument();
    expect(row.getByTestId("bg-model")).toHaveTextContent(/sin modelo/i);
    // Un cero en llamadas, no una celda vacía. Se asevera sobre la celda
    // concreta porque la fila tiene varios «0» (tokens, coste, errores…).
    expect(row.getByTestId("bg-calls")).toHaveTextContent("0");
  });

  it("marca las llamadas sospechosas cuando suspicious_calls > 0", () => {
    const data: BackgroundStats[] = [
      pair("consolidator", "deepseek/deepseek-v4.1-flash", {
        calls: 54,
        suspicious_calls: 54,
      }),
    ];
    render(<BackgroundCard data={data} loading={false} />);

    const row = within(screen.getByRole("row", { name: /consolidator/ }));
    expect(row.getByTestId("bg-suspicious")).toBeInTheDocument();
  });

  it("no marca sospechosas cuando suspicious_calls es 0", () => {
    const data: BackgroundStats[] = [pair("router", "gpt-4o", { calls: 5 })];
    render(<BackgroundCard data={data} loading={false} />);

    const row = within(screen.getByRole("row", { name: /router/ }));
    expect(row.queryByTestId("bg-suspicious")).not.toBeInTheDocument();
  });

  it("resalta los errores cuando total_errors > 0", () => {
    const data: BackgroundStats[] = [
      pair("router", "gpt-4o", { calls: 5, total_errors: 2 }),
    ];
    render(<BackgroundCard data={data} loading={false} />);

    const cell = within(
      screen.getByRole("row", { name: /router/ }),
    ).getByTestId("bg-errors");
    expect(cell).toHaveAttribute("data-highlighted", "true");
  });

  it("no resalta los errores cuando total_errors es 0", () => {
    const data: BackgroundStats[] = [pair("router", "gpt-4o", { calls: 5 })];
    render(<BackgroundCard data={data} loading={false} />);

    const cell = within(
      screen.getByRole("row", { name: /router/ }),
    ).getByTestId("bg-errors");
    expect(cell).toHaveAttribute("data-highlighted", "false");
  });

  it("marca los errores de forma no cromática (icono y aria-label), no solo por color", () => {
    const data: BackgroundStats[] = [
      pair("router", "gpt-4o", { calls: 5, total_errors: 2 }),
    ];
    render(<BackgroundCard data={data} loading={false} />);

    const cell = within(
      screen.getByRole("row", { name: /router/ }),
    ).getByTestId("bg-errors");
    // Marca perceptible sin depender del color rojo.
    expect(cell).toHaveAttribute("aria-label");
    expect(cell.textContent).toContain("⛔");
    // El color sigue presente (no se retira el resaltado).
    expect(cell).toHaveStyle({ color: "#ff4d4f" });
  });

  it("no añade la marca no cromática de errores cuando total_errors es 0", () => {
    const data: BackgroundStats[] = [pair("router", "gpt-4o", { calls: 5 })];
    render(<BackgroundCard data={data} loading={false} />);

    const cell = within(
      screen.getByRole("row", { name: /router/ }),
    ).getByTestId("bg-errors");
    expect(cell).not.toHaveAttribute("aria-label");
    expect(cell.textContent).not.toContain("⛔");
  });

  it("muestra la fecha de la última llamada en el formato crudo de SQLite", () => {
    // El backend NO emite ISO-8601: entrega `datetime('now')` tal cual,
    // "YYYY-MM-DD HH:MM:SS", sin `T`, sin `Z` y sin zona.
    const data: BackgroundStats[] = [
      pair("router", "gpt-4o", {
        calls: 5,
        last_call_at: "2026-10-12 19:08:45",
      }),
    ];
    render(<BackgroundCard data={data} loading={false} />);

    const cell = within(
      screen.getByRole("row", { name: /router/ }),
    ).getByTestId("bg-last-call");
    // WebKit rechaza el crudo de SQLite y degradaría la celda a "—": eso la
    // haría indistinguible de «sin última llamada».
    expect(cell).not.toHaveTextContent(/^—$/);
    expect(cell).not.toHaveTextContent("Invalid Date");
  });

  it("interpreta el crudo de SQLite como UTC (instante exacto, sin depender de la TZ de la máquina)", () => {
    // Determinista respecto a la zona horaria: se compara contra el **epoch**
    // esperado (`Date.UTC`), no contra un texto formateado con
    // `toLocaleString()`. Así el test vale en cualquier `TZ`.
    const instant = parseSqliteTimestamp("2026-10-12 19:08:45");
    expect(instant).not.toBeNull();
    expect(instant?.getTime()).toBe(Date.UTC(2026, 9, 12, 19, 8, 45));
  });

  it("sigue aceptando ISO-8601 con zona si algún día llega así", () => {
    const instant = parseSqliteTimestamp("2026-10-12T19:08:45Z");
    expect(instant?.getTime()).toBe(Date.UTC(2026, 9, 12, 19, 8, 45));
  });

  it("devuelve null para un valor no parseable", () => {
    expect(parseSqliteTimestamp("no es una fecha")).toBeNull();
  });

  it("muestra la ausencia de última llamada como — y nunca como Invalid Date", () => {
    const data: BackgroundStats[] = [pair("router", "gpt-4o", { calls: 5 })];
    render(<BackgroundCard data={data} loading={false} />);

    const cell = within(
      screen.getByRole("row", { name: /router/ }),
    ).getByTestId("bg-last-call");
    expect(cell).toHaveTextContent("—");
    expect(cell).not.toHaveTextContent("Invalid Date");
  });

  it("muestra un valor no parseable como — y nunca como Invalid Date", () => {
    const data: BackgroundStats[] = [
      pair("router", "gpt-4o", { calls: 5, last_call_at: "no es una fecha" }),
    ];
    render(<BackgroundCard data={data} loading={false} />);

    const cell = within(
      screen.getByRole("row", { name: /router/ }),
    ).getByTestId("bg-last-call");
    expect(cell).toHaveTextContent("—");
    expect(cell).not.toHaveTextContent("Invalid Date");
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
    // Con llamadas reales: sin actividad la tarjeta hace early return con
    // «No data yet» y nunca llega a pintar la tabla (donde vive el «N/A»).
    const nullLatency: BackgroundStats[] = [
      pair("router", "gpt-4o", { calls: 1, avg_duration_ms: null }),
    ];
    render(<BackgroundCard data={nullLatency} loading={false} />);

    expect(screen.getByText("N/A")).toBeInTheDocument();
  });
});
