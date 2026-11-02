import type { WidgetInstance } from "./types";

/**
 * Tramo del mensaje del asistente. `text` es un trozo literal del `content`
 * persistido; `widget` es un `WidgetInstance` que ocupa su tramo.
 */
export type MessageSegment =
  | { kind: "text"; text: string }
  | { kind: "widget"; widget: WidgetInstance };

/**
 * Longitud en bytes UTF-8 de un punto de código.
 * (El `span` del backend se expresa en bytes; los strings de JS en UTF-16.)
 */
function utf8Length(codePoint: number): number {
  if (codePoint < 0x80) return 1;
  if (codePoint < 0x800) return 2;
  if (codePoint < 0x10000) return 3;
  return 4;
}

/**
 * Construye el mapa `byteOffset → charIndex` (índice de carácter UTF-16) del
 * `content`. Los offsets de byte que caen **en medio** de un carácter multibyte
 * quedan a `-1`: usarlos como límite de un `span` lo invalida.
 *
 * Se itera por **puntos de código** (`codePointAt`), no por unidades UTF-16,
 * para que un par sustituto avance 4 bytes y 2 índices de carácter a la vez.
 */
function buildByteToChar(content: string): Int32Array {
  const byteLength = utf8LengthOf(content);
  const map = new Int32Array(byteLength + 1).fill(-1);
  let byte = 0;
  let charIndex = 0;
  for (let i = 0; i < content.length; ) {
    const codePoint = content.codePointAt(i);
    if (codePoint === undefined) break;
    map[byte] = charIndex;
    byte += utf8Length(codePoint);
    const units = codePoint > 0xffff ? 2 : 1;
    charIndex += units;
    i += units;
  }
  // El límite final siempre es válido y equivale a la longitud del string.
  map[byteLength] = content.length;
  return map;
}

/** Longitud en bytes UTF-8 de un string. */
export function utf8LengthOf(content: string): number {
  let bytes = 0;
  for (let i = 0; i < content.length; ) {
    const codePoint = content.codePointAt(i);
    if (codePoint === undefined) break;
    bytes += utf8Length(codePoint);
    i += codePoint > 0xffff ? 2 : 1;
  }
  return bytes;
}

/**
 * Resultado de validar el `span` de un widget contra el contenido. `start` y
 * `end` están en **índices de carácter** (listos para `String.prototype.slice`).
 */
interface ResolvedSpan {
  start: number;
  end: number;
}

/**
 * Traduce el `span` (bytes) de un widget a índices de carácter del `content`,
 * o `null` si es inválido. Un `span` es inválido cuando: no es un par de
 * enteros, sale del rango `[0, byteLength]`, es vacío o invertido (`start >= end`),
 * o cualquiera de sus límites cae en medio de un carácter multibyte.
 */
function resolveSpan(
  span: unknown,
  byteToChar: Int32Array,
  byteLength: number,
): ResolvedSpan | null {
  if (!Array.isArray(span) || span.length !== 2) return null;
  const [start, end] = span;
  if (!Number.isInteger(start) || !Number.isInteger(end)) return null;
  if (start < 0 || end > byteLength || start >= end) return null;
  const charStart = byteToChar[start];
  const charEnd = byteToChar[end];
  if (charStart < 0 || charEnd < 0) return null;
  return { start: charStart, end: charEnd };
}

interface AcceptedWidget {
  widget: WidgetInstance;
  span: ResolvedSpan;
}

/**
 * Compone el mensaje del asistente como una **secuencia de tramos** ordenada:
 * los trozos de texto del `content` y, en el sitio de su tramo, cada widget.
 *
 * El `span` de cada widget es un rango **semiabierto** `[start, end)` en
 * **bytes UTF-8** sobre `content` (`widget-projection`, D2 opción (c)).
 *
 * Reglas (la degradación es siempre de **colocación**, nunca de contenido):
 *
 * - **Widget sin `span`**: se coloca **al final**, tras todo el texto —el render
 *   de hoy—.
 * - **`span` inválido** (fuera de rango, invertido, no entero o partiendo un
 *   carácter multibyte): ese widget cae **al final**; el resto conserva su sitio.
 *   Nunca se lanza.
 * - **`span` solapado** con el de otro widget: gana el que **empieza antes**
 *   (el primero en orden de inicio); el solapado se **degrada al final**. Así
 *   ningún tramo de un mismo mensaje se pinta dos veces.
 * - **Contenido vacío**: no hay tramos de texto; los widgets (todos al final)
 *   se devuelven igualmente.
 *
 * La función es **pura** y no muta `widgets`.
 */
export function composeMessage(
  content: string,
  widgets: WidgetInstance[],
): MessageSegment[] {
  const byteLength = utf8LengthOf(content);
  const byteToChar = buildByteToChar(content);

  const accepted: AcceptedWidget[] = [];
  const demoted: WidgetInstance[] = [];

  // 1. Resolver cada `span` a índices de carácter, descartando los inválidos.
  const candidates: AcceptedWidget[] = [];
  for (const widget of widgets) {
    const span =
      widget.span !== undefined
        ? resolveSpan(widget.span, byteToChar, byteLength)
        : null;
    if (span === null) {
      demoted.push(widget);
    } else {
      candidates.push({ widget, span });
    }
  }

  // 2. Resolver solapes: por orden de inicio, el primero gana; el que se solape
  //    con el tramo ya aceptado se degrada al final.
  candidates.sort((a, b) => a.span.start - b.span.start);
  let lastEnd = -1;
  for (const candidate of candidates) {
    if (candidate.span.start < lastEnd) {
      demoted.push(candidate.widget);
    } else {
      accepted.push(candidate);
      lastEnd = candidate.span.end;
    }
  }

  // 3. Intercalar los tramos de texto con los widgets aceptados.
  const segments: MessageSegment[] = [];
  let cursor = 0;
  for (const { widget, span } of accepted) {
    if (span.start > cursor) {
      segments.push({ kind: "text", text: content.slice(cursor, span.start) });
    }
    segments.push({ kind: "widget", widget });
    cursor = span.end;
  }
  if (cursor < content.length) {
    segments.push({ kind: "text", text: content.slice(cursor) });
  }

  // 4. Los widgets degradados (sin `span` o con `span` inválido/solapado) van
  //    debajo del texto, como el render de hoy.
  for (const widget of demoted) {
    segments.push({ kind: "widget", widget });
  }

  return segments;
}
