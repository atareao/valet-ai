import type {
  PushSubscriptionPayload,
  PushSubscriptionResponse,
  VapidKeyResponse,
} from "../types";
import { request } from "./client";

/**
 * `GET /api/push/vapid-key` — clave pública VAPID en base64url con la que el
 * navegador se suscribe. El backend responde `503 push_unavailable` si no
 * puede cargar ni generar las claves.
 */
export function getVapidKey(): Promise<VapidKeyResponse> {
  return request<VapidKeyResponse>("/push/vapid-key");
}

/**
 * `POST /api/push/subscriptions` — registra (o actualiza) la suscripción del
 * navegador. El alta es un *upsert* por `endpoint` que conserva el `id`.
 */
export function subscribePush(
  subscription: PushSubscriptionPayload,
): Promise<PushSubscriptionResponse> {
  return request<PushSubscriptionResponse>("/push/subscriptions", {
    method: "POST",
    body: JSON.stringify(subscription),
  });
}

/**
 * `DELETE /api/push/subscriptions` — da de baja la suscripción por `endpoint`
 * (idempotente: `204` exista o no la fila). El endpoint viaja en el cuerpo.
 */
export function unsubscribePush(endpoint: string): Promise<void> {
  return request<void>("/push/subscriptions", {
    method: "DELETE",
    body: JSON.stringify({ endpoint }),
  });
}
