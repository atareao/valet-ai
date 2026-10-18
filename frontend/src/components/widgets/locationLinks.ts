/**
 * Resolución del destino de la acción «Abrir» del `LocationWidget`.
 *
 * Módulo puro: no toca el DOM ni lee `navigator`. La UA y el número de puntos
 * táctiles entran por parámetro, así que la decisión se puede testear sin
 * navegador. Se mantiene fuera del componente para no romper la regla de Fast
 * Refresh (`only-export-components`), igual que `locationTiles.ts`.
 */

/** Plataforma del navegador que decide el esquema del destino. */
export type MapPlatform = "android" | "ios" | "other";

/** Punto a abrir: las coordenadas mandan y la etiqueta solo decora el pin. */
export interface OpenLocationInput {
  latitude: number;
  longitude: number;
  /** Etiqueta decorativa del punto: el título o, en su defecto, la dirección. Nunca una consulta de búsqueda. */
  label?: string;
}

const ANDROID_RE = /Android/i;
const IOS_RE = /iPhone|iPad|iPod/i;
const MACINTOSH_RE = /Macintosh/i;

/**
 * Codificador estricto RFC 3986: `encodeURIComponent` no escapa `( ) ! ' *`, y
 * un paréntesis crudo en la etiqueta cerraría el paréntesis estructural del
 * `geo:` al primer `)`, rompiendo la estructura del destino.
 */
const encodeLabel = (label: string): string =>
  encodeURIComponent(label).replace(/[!'()*]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`);

/**
 * Deduce la plataforma a partir de la UA. iPadOS 13+ se declara `Macintosh`,
 * así que se desambigua por `maxTouchPoints > 1`; sin táctil se degrada a
 * escritorio porque no hay forma de saberlo.
 */
export function detectPlatform(userAgent: string, maxTouchPoints = 0): MapPlatform {
  if (ANDROID_RE.test(userAgent)) return "android";
  if (IOS_RE.test(userAgent)) return "ios";
  if (MACINTOSH_RE.test(userAgent) && maxTouchPoints > 1) return "ios";
  return "other";
}

/**
 * Construye el destino según la plataforma:
 *
 * - `android`: `geo:{lat},{lon}?q={lat},{lon}({etiqueta})`, sin fijar paquete,
 *   para que elija el sistema.
 * - `ios`: `https://maps.apple.com/?q={lat},{lon}`, sin etiqueta (Apple no
 *   documenta nombrar un pin anclado a coordenadas).
 * - `other`: la URL oficial de Google Maps con las coordenadas como consulta.
 */
export function buildOpenLocationUrl(input: OpenLocationInput, platform: MapPlatform): string {
  const coords = `${input.latitude},${input.longitude}`;

  if (platform === "android") {
    const suffix = input.label ? `(${encodeLabel(input.label)})` : "";
    return `geo:${coords}?q=${coords}${suffix}`;
  }

  if (platform === "ios") {
    return `https://maps.apple.com/?q=${coords}`;
  }

  return `https://www.google.com/maps/search/?api=1&query=${coords}`;
}

/** Compuesto que consumirá el widget: detecta la plataforma y resuelve el destino. */
export function openLocationUrl(
  input: OpenLocationInput,
  userAgent: string,
  maxTouchPoints?: number,
): string {
  return buildOpenLocationUrl(input, detectPlatform(userAgent, maxTouchPoints));
}
