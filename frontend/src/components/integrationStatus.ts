/**
 * Etiquetas del indicador de configuración de las sub-pestañas de
 * «Integraciones».
 *
 * El criterio es único para las cinco integraciones de clave: la **clave** (no
 * la URL base) tiene valor no vacío en `settings`. Se centraliza aquí para que
 * la etiqueta de la sub-pestaña y la línea de estado del panel no puedan
 * divergir: el nombre del estado se decide por el criterio (`isKeyConfigured`),
 * nunca comparando el texto ya formateado.
 */

import type { SettingsMap } from "../types";

/** Texto del indicador cuando la integración tiene su clave configurada. */
const CONFIGURED_LABEL = "Configurada";

/** Texto del indicador cuando la integración no tiene su clave configurada. */
const UNCONFIGURED_LABEL = "Sin configurar";

/**
 * Texto del indicador cuando una lectura remota falló y el estado es
 * desconocido.
 *
 * Sin él, un fallo de `GET /api/strava/status`, `GET /api/zennotes/status` o
 * `GET /api/opentodo/status` dejaría el indicador en «Consultando…» para
 * siempre, que miente dos veces: ni se está consultando, ni se sabe el estado.
 * Se comparte con los paneles de las sub-pestañas remotas para que estos no
 * puedan contradecir a su etiqueta (una diría «Sin datos» y el otro «Sin
 * configurar»).
 */
export const UNAVAILABLE_LABEL = "Sin datos";

/**
 * `true` si `key` tiene un valor no vacío (ignorando espacios) en `settings`.
 *
 * Se lee de `settings` en cada render —nunca se copia a un `useState`— para que
 * el indicador refleje el valor recién guardado sin recargar la página.
 */
export function isKeyConfigured(
  settings: SettingsMap | undefined,
  key: string,
): boolean {
  return (settings?.[key] ?? "").trim() !== "";
}

/** Texto del indicador de configuración para la clave `key`. */
export function keyStatusLabel(
  settings: SettingsMap | undefined,
  key: string,
): string {
  return isKeyConfigured(settings, key)
    ? CONFIGURED_LABEL
    : UNCONFIGURED_LABEL;
}
