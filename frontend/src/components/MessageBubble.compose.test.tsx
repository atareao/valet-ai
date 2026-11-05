import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { MessageBubble } from "./MessageBubble";
import type { Message } from "../types";
import type { WidgetInstance } from "./widgets/types";
import { utf8LengthOf } from "./widgets/compose";

// antd consulta `matchMedia` al montar algunos de sus componentes (checkbox,
// botón). El entorno jsdom no lo trae.
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

const encoder = new TextEncoder();
function byteOffset(content: string, charIndex: number): number {
  return encoder.encode(content.slice(0, charIndex)).length;
}

/**
 * Respuesta con una lista en prosa. El widget sustituye el bloque de la lista
 * (entre `- A)` y el final de `Barcelona`) y su etiqueta es **distinta** de la
 * prosa, para poder detectar una duplicación.
 */
const CONTENT = [
  "Aquí van las opciones:",
  "",
  "- A) Madrid",
  "- B) Barcelona",
  "",
  "Elige una.",
].join("\n");

const listStart = CONTENT.indexOf("- A)");
const listEnd = CONTENT.indexOf("Barcelona") + "Barcelona".length;
const LIST_SPAN: [number, number] = [
  byteOffset(CONTENT, listStart),
  byteOffset(CONTENT, listEnd),
];

const checklist: WidgetInstance = {
  id: "w1",
  name: "Checklist",
  data: { title: "Opciones", items: [{ id: "a", label: "Opción elegida" }] },
  span: LIST_SPAN,
};

function assistantMessage(overrides: Partial<Message> = {}): Message {
  return {
    id: "m1",
    role: "assistant",
    content: CONTENT,
    created_at: "2026-11-02T10:30:00Z",
    ...overrides,
  };
}

describe("MessageBubble — composición por tramos", () => {
  it("pinta el widget en el sitio del tramo y NO duplica la lista en prosa", () => {
    const { container } = render(
      <MessageBubble message={assistantMessage()} widgets={[checklist]} />,
    );

    const text = container.textContent ?? "";

    // El texto anterior al span va antes del widget.
    expect(text).toContain("Aquí van las opciones:");
    // El widget se pinta en el sitio del tramo.
    expect(screen.getByText("Opción elegida")).toBeInTheDocument();
    // El texto posterior al span va después.
    expect(text).toContain("Elige una.");

    // Orden: texto previo → widget → texto posterior.
    const beforeIdx = text.indexOf("Aquí van las opciones:");
    const widgetIdx = text.indexOf("Opción elegida");
    const afterIdx = text.indexOf("Elige una.");
    expect(widgetIdx).toBeGreaterThan(beforeIdx);
    expect(afterIdx).toBeGreaterThan(widgetIdx);

    // La lista en prosa NO se duplica.
    expect(text).not.toContain("A) Madrid");
    expect(text).not.toContain("B) Barcelona");
  });

  it("un widget sin span se pinta debajo del texto íntegro", () => {
    const noSpan: WidgetInstance = {
      id: "w2",
      name: "Checklist",
      data: { title: "Opciones", items: [{ id: "a", label: "Opción elegida" }] },
    };
    const { container } = render(
      <MessageBubble message={assistantMessage()} widgets={[noSpan]} />,
    );

    const text = container.textContent ?? "";
    // El texto se pinta íntegro (la lista en prosa sigue visible)...
    expect(text).toContain("A) Madrid");
    expect(text).toContain("B) Barcelona");
    // ...y el widget va debajo.
    expect(screen.getByText("Opción elegida")).toBeInTheDocument();
    expect(text.indexOf("Opción elegida")).toBeGreaterThan(
      text.indexOf("Elige una."),
    );
  });

  it("un span inválido cae al render de hoy sin lanzar", () => {
    const invalid: WidgetInstance = {
      id: "w3",
      name: "Checklist",
      data: { title: "Opciones", items: [{ id: "a", label: "Opción elegida" }] },
      span: [0, utf8LengthOf(CONTENT) + 1000],
    };

    let container!: HTMLElement;
    expect(() => {
      ({ container } = render(
        <MessageBubble message={assistantMessage()} widgets={[invalid]} />,
      ));
    }).not.toThrow();

    const text = container.textContent ?? "";

    // Texto íntegro (incluida la lista en prosa) y widget debajo.
    expect(text).toContain("A) Madrid");
    expect(text).toContain("B) Barcelona");
    expect(screen.getByText("Opción elegida")).toBeInTheDocument();
    expect(text.indexOf("Opción elegida")).toBeGreaterThan(
      text.indexOf("Elige una."),
    );
  });

  it("varios widgets con spans válidos se pintan cada uno en su sitio", () => {
    const content = "Uno DOS Tres";
    const dosStart = content.indexOf("DOS");
    const dosEnd = dosStart + "DOS".length;
    const widget: WidgetInstance = {
      id: "w4",
      name: "Checklist",
      data: { items: [{ id: "a", label: "Tarea" }] },
      span: [byteOffset(content, dosStart), byteOffset(content, dosEnd)],
    };

    const { container } = render(
      <MessageBubble
        message={assistantMessage({ content })}
        widgets={[widget]}
      />,
    );

    const text = container.textContent ?? "";
    expect(text.indexOf("Uno")).toBeLessThan(text.indexOf("Tarea"));
    expect(text.indexOf("Tarea")).toBeLessThan(text.indexOf("Tres"));
    expect(text).not.toContain("DOS");
  });
});
