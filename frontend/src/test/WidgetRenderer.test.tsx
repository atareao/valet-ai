import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import type { FC } from "react";
import { WidgetRenderer } from "../components/widgets/WidgetRenderer";
import { WidgetErrorBoundary } from "../components/widgets/WidgetErrorBoundary";
import { WIDGET_REGISTRY } from "../components/widgets/registry";
import type { WidgetComponent } from "../components/widgets/types";

// Ant Design necesita matchMedia en jsdom.
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

describe("WIDGET_REGISTRY", () => {
  it("registra QuickForm y Checklist", () => {
    expect(WIDGET_REGISTRY.QuickForm).toBeDefined();
    expect(WIDGET_REGISTRY.Checklist).toBeDefined();
  });
});

describe("WidgetRenderer", () => {
  it("renderiza un widget conocido (Checklist)", () => {
    render(
      <WidgetRenderer
        id="w1"
        name="Checklist"
        data={{ title: "Tareas pendientes", items: [{ id: "a", label: "A" }] }}
        onAction={vi.fn()}
      />,
    );

    expect(screen.getByText("Tareas pendientes")).toBeInTheDocument();
  });

  it("con un nombre desconocido muestra un aviso y no revienta", () => {
    render(
      <WidgetRenderer
        id="w2"
        name="SystemMonitor"
        data={{}}
        onAction={vi.fn()}
      />,
    );

    expect(screen.getByText(/SystemMonitor/)).toBeInTheDocument();
  });
});

describe("WidgetErrorBoundary", () => {
  beforeEach(() => {
    // React registra el error capturado por el boundary en consola; silenciar.
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  const Boom: FC = () => {
    throw new Error("boom");
  };

  it("captura un widget que lanza y muestra un aviso de error", () => {
    render(
      <WidgetErrorBoundary name="Boom">
        <Boom />
      </WidgetErrorBoundary>,
    );

    expect(screen.getByText(/Boom/)).toBeInTheDocument();
  });

  it("WidgetRenderer aísla un widget que lanza sin tumbar el resto", () => {
    WIDGET_REGISTRY.Boom = Boom as WidgetComponent;
    try {
      const { container } = render(
        <div>
          <span>resto del chat</span>
          <WidgetRenderer id="wb" name="Boom" data={{}} onAction={vi.fn()} />
        </div>,
      );

      expect(screen.getByText("resto del chat")).toBeInTheDocument();
      expect(screen.getByText(/Boom/)).toBeInTheDocument();
      expect(container.querySelector(".ant-alert-error")).not.toBeNull();
    } finally {
      delete WIDGET_REGISTRY.Boom;
    }
  });
});