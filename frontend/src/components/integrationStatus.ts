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
