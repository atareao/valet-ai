import type { OximealCheckResult, OximealStatus } from "../types";
import { request } from "./client";

/**
 * `GET /api/oximeal/status` — estado de la configuración sin el
 * `client_secret`: si hay credenciales resueltas (`configured`), los valores
 * efectivos de `base_url`/`issuer_url` y si hay `client_id` (`has_client_id`).
 */
export function getOximealStatus(): Promise<OximealStatus> {
  return request<OximealStatus>("/oximeal/status");
}

/**
 * `GET /api/oximeal/check` — sondeo activo de la conexión. Responde **siempre**
 * `200`: el diagnóstico viaja en `ok`/`detail`, por eso no rechaza ante un fallo
 * de OxiMeal (solo si la propia petición HTTP falla).
 */
export function checkOximeal(): Promise<OximealCheckResult> {
  return request<OximealCheckResult>("/oximeal/check");
}

/**
 * `PUT /api/oximeal/secret` — guarda el `client_secret` (clave sensible) por su
 * endpoint dedicado. Un secreto vacío → `400`; uno válido guarda y responde
 * `204`. El valor **nunca** se relee.
 */
export function setOximealSecret(secret: string): Promise<void> {
  return request<void>("/oximeal/secret", {
    method: "PUT",
    body: JSON.stringify({ client_secret: secret }),
  });
}
