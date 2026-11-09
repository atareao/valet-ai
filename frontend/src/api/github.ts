import type { GithubCheckResult, GithubStatus } from "../types";
import { request } from "./client";

/**
 * `GET /api/github/status` — estado de la configuración sin el `github_token`:
 * si hay credenciales resueltas (`configured`) y el repositorio por defecto.
 */
export function getGithubStatus(): Promise<GithubStatus> {
  return request<GithubStatus>("/github/status");
}

/**
 * `GET /api/github/check` — sondeo activo de la conexión. Responde **siempre**
 * `200`: el diagnóstico viaja en `ok`/`detail`, por eso no rechaza ante un fallo
 * de GitHub (solo si la propia petición HTTP falla).
 */
export function checkGithub(): Promise<GithubCheckResult> {
  return request<GithubCheckResult>("/github/check");
}

/**
 * `PUT /api/github/token` — guarda el `github_token` (clave sensible) por su
 * endpoint dedicado. Un token vacío → `400`; uno válido guarda y responde
 * `204`. El valor **nunca** se relee.
 */
export function putGithubToken(token: string): Promise<void> {
  return request<void>("/github/token", {
    method: "PUT",
    body: JSON.stringify({ token }),
  });
}
