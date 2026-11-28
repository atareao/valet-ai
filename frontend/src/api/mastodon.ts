import type { MastodonCheckResult, MastodonStatus } from "../types";
import { request } from "./client";

/**
 * `GET /api/mastodon/status` — estado de la configuración sin el
 * `mastodon_token`: si hay credenciales resueltas (`configured`) y la URL base
 * vigente.
 */
export function getMastodonStatus(): Promise<MastodonStatus> {
  return request<MastodonStatus>("/mastodon/status");
}

/**
 * `GET /api/mastodon/check` — sondeo activo de la conexión. Responde **siempre**
 * `200`: el diagnóstico viaja en `ok`/`detail`, por eso no rechaza ante un fallo
 * de Mastodon (solo si la propia petición HTTP falla).
 */
export function checkMastodon(): Promise<MastodonCheckResult> {
  return request<MastodonCheckResult>("/mastodon/check");
}

/**
 * `PUT /api/mastodon/token` — guarda el `mastodon_token` (clave sensible) por su
 * endpoint dedicado. Un token vacío → `400`; uno válido guarda y responde
 * `204`. El valor **nunca** se relee.
 */
export function putMastodonToken(token: string): Promise<void> {
  return request<void>("/mastodon/token", {
    method: "PUT",
    body: JSON.stringify({ token }),
  });
}
