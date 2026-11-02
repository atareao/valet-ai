import { describe, it, expect } from "vitest";
import { composeMessage, utf8LengthOf } from "./compose";
import type { WidgetInstance } from "./types";

const encoder = new TextEncoder();

/** Byte offset (UTF-8) del índice de carácter `charIndex` de `content`. */
function byteOffset(content: string, charIndex: number): number {
  return encoder.encode(content.slice(0, charIndex)).length;
}

function textWidget(id: string, span?: [number, number]): WidgetInstance {
  return { id, name: "Checklist", data: { items: [] }, ...(span ? { span } : {}) };
}

describe("composeMessage — tramo válido", () => {
  it("intercala [texto, widget, texto] en el sitio del span", () => {
    const content = "Antes LISTA Después";
    const start = content.indexOf("LISTA");
    const end = start + "LISTA".length;
    const widget = textWidget("w1", [
      byteOffset(content, start),
      byteOffset(content, end),
    ]);

    const segments = composeMessage(content, [widget]);

    expect(segments).toEqual([
      { kind: "text", text: "Antes " },
      { kind: "widget", widget },
      { kind: "text", text: " Después" },
    ]);
  });

  it("no pinta un tramo de texto vacío cuando el span llega al final", () => {
    const content = "Solo texto";
    const widget = textWidget("w1", [0, utf8LengthOf(content)]);

    const segments = composeMessage(content, [widget]);

    expect(segments).toEqual([{ kind: "widget", widget }]);
  });

  it("respeta los límites de byte de un carácter multibyte (emoji)", () => {
    const content = "Hola 🌍 mundo";
    // "Hola " = 5 bytes; el emoji ocupa los bytes [5, 9).
    const emojiStart = byteOffset(content, content.indexOf("🌍"));
    const emojiEnd = emojiStart + 4;

    const segments = composeMessage(content, [
      textWidget("w1", [emojiStart, emojiEnd]),
    ]);

    expect(segments).toEqual([
      { kind: "text", text: "Hola " },
      { kind: "widget", widget: textWidget("w1", [emojiStart, emojiEnd]) },
      { kind: "text", text: " mundo" },
    ]);
  });
});

describe("composeMessage — widget sin span (render de hoy)", () => {
  it("coloca el widget al final, tras el texto íntegro", () => {
    const content = "Texto completo";
    const widget = textWidget("w1");

    const segments = composeMessage(content, [widget]);

    expect(segments).toEqual([
      { kind: "text", text: "Texto completo" },
      { kind: "widget", widget },
    ]);
  });

  it("mezcla un widget colocado con otro sin span (colocado + debajo)", () => {
    const content = "A X B";
    const placed = textWidget("w1", [2, 3]); // "X"
    const below = textWidget("w2");

    const segments = composeMessage(content, [placed, below]);

    expect(segments).toEqual([
      { kind: "text", text: "A " },
      { kind: "widget", widget: placed },
      { kind: "text", text: " B" },
      { kind: "widget", widget: below },
    ]);
  });
});

describe("composeMessage — span inválido (fallback al render de hoy)", () => {
  const content = "abcdefghij"; // 10 caracteres = 10 bytes
  const len = utf8LengthOf(content);

  const invalidCases: Array<[string, [number, number]]> = [
    ["start < 0", [-1, 3]],
    ["end > longitud", [2, len + 1]],
    ["start >= end", [5, 5]],
    ["start > end", [7, 3]],
    ["no entero (start)", [1.5, 4]],
    ["no entero (end)", [1, 4.5]],
  ];

  for (const [name, span] of invalidCases) {
    it(`un span con ${name} degrada el widget al final`, () => {
      const widget = textWidget("w1", span);

      const segments = composeMessage(content, [widget]);

      expect(segments).toEqual([
        { kind: "text", text: content },
        { kind: "widget", widget },
      ]);
    });
  }

  it("un span que cae en medio de un carácter multibyte es inválido", () => {
    const multibyte = "Hola 🌍";
    // El emoji vive en los bytes [5, 9); 6 y 7 caen dentro del carácter.
    const widget = textWidget("w1", [6, 9]);

    const segments = composeMessage(multibyte, [widget]);

    expect(segments).toEqual([
      { kind: "text", text: multibyte },
      { kind: "widget", widget },
    ]);
  });

  it("un span con `end` en medio de un carácter multibyte es inválido", () => {
    const multibyte = "Hola 🌍";
    // `start` (5) es válido —inicio del emoji—, pero `end` (7) cae dentro del
    // carácter, que ocupa los bytes [5, 9). El span es inválido y el widget
    // cae al final, igual que cuando es el `start` el que parte el carácter.
    const widget = textWidget("w1", [5, 7]);

    const segments = composeMessage(multibyte, [widget]);

    expect(segments).toEqual([
      { kind: "text", text: multibyte },
      { kind: "widget", widget },
    ]);
  });

  it("un span malformado (no tupla) es inválido sin lanzar", () => {
    const widget = {
      id: "w1",
      name: "Checklist",
      data: {},
      span: "1,2" as unknown as [number, number],
    };

    expect(() => composeMessage(content, [widget])).not.toThrow();
    expect(composeMessage(content, [widget])).toEqual([
      { kind: "text", text: content },
      { kind: "widget", widget },
    ]);
  });

  it("un span inválido no arrastra al widget colocado de otro", () => {
    const placed = textWidget("w1", [0, 2]); // "ab"
    const broken = textWidget("w2", [-5, 2]);

    const segments = composeMessage(content, [placed, broken]);

    expect(segments).toEqual([
      { kind: "widget", widget: placed },
      { kind: "text", text: content.slice(2) },
      { kind: "widget", widget: broken },
    ]);
  });
});

describe("composeMessage — span solapado", () => {
  it("gana el que empieza antes y el solapado se degrada al final", () => {
    const content = "abcdefghij";
    const first = textWidget("w1", [0, 5]); // "abcde"
    const overlapping = textWidget("w2", [3, 8]); // solapa

    const segments = composeMessage(content, [first, overlapping]);

    expect(segments).toEqual([
      { kind: "widget", widget: first },
      { kind: "text", text: "fghij" },
      { kind: "widget", widget: overlapping },
    ]);
  });

  it("dos tramos contiguos (sin solape) se aceptan ambos", () => {
    const content = "abcdefghij";
    const a = textWidget("w1", [0, 3]);
    const b = textWidget("w2", [3, 6]);

    const segments = composeMessage(content, [a, b]);

    expect(segments).toEqual([
      { kind: "widget", widget: a },
      { kind: "widget", widget: b },
      { kind: "text", text: "ghij" },
    ]);
  });
});

describe("composeMessage — contenido vacío", () => {
  it("sin widgets devuelve una lista vacía", () => {
    expect(composeMessage("", [])).toEqual([]);
  });

  it("con un widget sin span devuelve solo el widget", () => {
    const widget = textWidget("w1");

    expect(composeMessage("", [widget])).toEqual([
      { kind: "widget", widget },
    ]);
  });

  it("cualquier span sobre contenido vacío es inválido y degrada al widget", () => {
    const widget = textWidget("w1", [0, 5]);

    expect(composeMessage("", [widget])).toEqual([
      { kind: "widget", widget },
    ]);
  });
});
