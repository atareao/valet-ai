import type { CreatedIngestKey, IngestKey } from "../types";
import { request } from "./client";

/**
 * `GET /api/ingest-keys` — lista todas las claves de ingesta (activas y
 * revocadas). **Nunca** incluye el token: solo el `prefix`. Requiere sesión.
 */
export function listIngestKeys(): Promise<IngestKey[]> {
  return request<IngestKey[]>("/ingest-keys");
}

/**
 * `POST /api/ingest-keys` — crea una clave con un `name` y devuelve su token en
 * claro. Es la **única** vez que el token sale del servidor: si se pierde, se
 * revoca la clave y se crea otra.
 */
export function createIngestKey(name: string): Promise<CreatedIngestKey> {
  return request<CreatedIngestKey>("/ingest-keys", {
    method: "POST",
    body: JSON.stringify({ name }),
  });
}

/**
 * `DELETE /api/ingest-keys/{id}` — revoca la clave. Idempotente: `204` tanto si
 * la clave existía como si ya estaba revocada o no existe.
 */
export function revokeIngestKey(id: string): Promise<void> {
  return request<void>(`/ingest-keys/${id}`, { method: "DELETE" });
}
