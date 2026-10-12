import { getVapidKey, subscribePush, unsubscribePush } from "../api/push";

/** Estado de las notificaciones WebPush en este navegador. */
export type PushStatus =
  | "unsupported"
  | "denied"
  | "granted-unsubscribed"
  | "subscribed";

/**
 * Convierte una clave pública VAPID en base64url al `Uint8Array` que consume
 * `PushManager.subscribe({ applicationServerKey })`. Acepta el alfabeto
 * url-safe (`-`/`_`) y maneja la ausencia o presencia de padding.
 */
export function urlBase64ToUint8Array(
  base64String: string,
): Uint8Array<ArrayBuffer> {
  if (base64String.length === 0) {
    return new Uint8Array(new ArrayBuffer(0));
  }

  const padding = "=".repeat((4 - (base64String.length % 4)) % 4);
  const base64 = (base64String + padding)
    .replace(/-/g, "+")
    .replace(/_/g, "/");
  const raw = atob(base64);

  // Construimos sobre un `ArrayBuffer` explícito para que el tipo sea
  // `Uint8Array<ArrayBuffer>`, que es el que espera `applicationServerKey`
  // (`BufferSource`) en `lib.dom`. Un `new Uint8Array(n)` sin buffer da
  // `Uint8Array<ArrayBufferLike>`, no asignable.
  const output = new Uint8Array(new ArrayBuffer(raw.length));
  for (let i = 0; i < raw.length; i += 1) {
    output[i] = raw.charCodeAt(i);
  }
  return output;
}

/** ¿Este navegador puede recibir notificaciones WebPush? */
export function isPushSupported(): boolean {
  return (
    "serviceWorker" in navigator &&
    "PushManager" in window &&
    "Notification" in window
  );
}

/**
 * Estado real: sin soporte, permiso denegado, permiso concedido sin
 * suscripción o suscrito. Consulta la suscripción viva del `PushManager`, no
 * un valor cacheado.
 */
export async function getPushStatus(): Promise<PushStatus> {
  if (!isPushSupported()) return "unsupported";
  if (Notification.permission === "denied") return "denied";

  const registration = await navigator.serviceWorker.ready;
  const subscription = await registration.pushManager.getSubscription();
  return subscription ? "subscribed" : "granted-unsubscribed";
}

/**
 * Activa las notificaciones. Pide el permiso **aquí y solo aquí**, tras el
 * gesto del usuario: si no queda concedido lanza un error legible y no publica
 * nada. Con el permiso concedido obtiene la clave pública VAPID, suscribe el
 * navegador y publica la suscripción en el backend.
 */
export async function enablePushNotifications(): Promise<void> {
  if (!isPushSupported()) {
    throw new Error("Este navegador no soporta notificaciones push.");
  }

  const permission = await Notification.requestPermission();
  if (permission !== "granted") {
    throw new Error(
      "No se concedió el permiso de notificaciones; actívalas en el navegador.",
    );
  }

  const { public_key } = await getVapidKey();
  const registration = await navigator.serviceWorker.ready;
  const subscription = await registration.pushManager.subscribe({
    userVisibleOnly: true,
    applicationServerKey: urlBase64ToUint8Array(public_key),
  });

  const { endpoint, keys } = subscription.toJSON();
  if (!endpoint || !keys?.p256dh || !keys.auth) {
    throw new Error("El navegador no devolvió las claves de la suscripción.");
  }

  await subscribePush({ endpoint, keys: { p256dh: keys.p256dh, auth: keys.auth } });
}

/**
 * Desactiva las notificaciones: cancela la suscripción del navegador y la
 * borra en el backend. Sin suscripción no hace nada.
 */
export async function disablePushNotifications(): Promise<void> {
  if (!isPushSupported()) return;

  const registration = await navigator.serviceWorker.ready;
  const subscription = await registration.pushManager.getSubscription();
  if (!subscription) return;

  const endpoint = subscription.endpoint;
  await subscription.unsubscribe();
  await unsubscribePush(endpoint);
}
